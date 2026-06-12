use crate::ModelData;
use crate::Vertex;

use eframe::CreationContext;
use std::sync::mpsc::Receiver;
use wgpu::util::DeviceExt;

pub struct RingBuffer {
    is_mapping: bool,
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    receiver: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ComputeUniforms {
    // mat4x4 = 16-element f32 array = 64 bytes
    mvp: [f32; 16],
    // 32 / 8 = 4 bytes
    vertex_count: u32,
    // Add padding to be multiple of 16 :
    // 64 + 4 = 68 -> 68 / 16 = 4.25 -> 5 * 16 = 80 -> need to be 80 bytes of size
    // -> 80 - 68 = 12 = 4 * 3
    _padding: [u32; 3],
}

pub struct GPUData {
    device: wgpu::Device,
    queue: std::sync::Arc<wgpu::Queue>,
    // Buffers
    mvp_buffer: wgpu::Buffer,
    buffer_size: wgpu::BufferAddress,
    ring_buffers: [RingBuffer; 2], // Double Buffering
    current_ring_index: usize,
    // Pipeline
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GPUData {
    pub fn new(cc: &CreationContext, model: &ModelData) -> Option<Self> {
        let state: Option<&eframe::egui_wgpu::RenderState> = cc.wgpu_render_state.as_ref();

        let Some(wgpu_render_state) = state else {
            return None;
        };

        let device = wgpu_render_state.device.clone();
        let queue: std::sync::Arc<wgpu::Queue> =
            std::sync::Arc::new(wgpu_render_state.queue.clone());

        // SHADER
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            // source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
            source: wgpu::ShaderSource::Wgsl(include_str!("vertex_shader.wgsl").into()),
        });

        // PIPELINE
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                // MVP Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Input Storage
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output Storage
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // BUFFERS

        // MVP Buffer
        let identity_matrix = glam::Mat4::IDENTITY;

        let uniform: ComputeUniforms = ComputeUniforms {
            mvp: identity_matrix.to_cols_array(),
            vertex_count: model.vertices.len() as u32,
            _padding: [0; 3],
        };

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MVP Uniform Buffer"),
            // contents: bytemuck::cast_slice(&identity_matrix.to_cols_array()),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (buffer_size, ring_buffers) =
            Self::setup_model_ring(&device, &bind_group_layout, &mvp_buffer, &model);

        return Some(Self {
            device,
            queue,
            // Buffers
            mvp_buffer,
            buffer_size,
            // Ring Buffers
            ring_buffers,
            current_ring_index: 0,
            // Pipeline
            compute_pipeline,
            bind_group_layout,
        });
    }

    pub fn setup_model_ring(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        mvp_buffer: &wgpu::Buffer,
        model: &ModelData,
    ) -> (u64, [RingBuffer; 2]) {
        // Input Buffer -> Binding 1
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&model.vertices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Send vertices to buffer

        // let buffer_size =
        //     (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;
        let buffer_size =
            (model.vertices.len() * std::mem::size_of::<glam::Vec4>()) as wgpu::BufferAddress;

        // Output buffers -> Binding 2
        let output_buffer1 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer1"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let output_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer2"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Staging Buffers : Copy output buffer -> Staging
        let staging_buffer1 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer1"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer2"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind Groups
        let bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group1"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mvp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer1.as_entire_binding(),
                },
            ],
        });

        let bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group2"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mvp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer2.as_entire_binding(),
                },
            ],
        });

        let ring1 = RingBuffer {
            is_mapping: false,
            output_buffer: output_buffer1,
            staging_buffer: staging_buffer1,
            bind_group: bind_group1,
            receiver: None,
        };

        let ring2 = RingBuffer {
            is_mapping: false,
            output_buffer: output_buffer2,
            staging_buffer: staging_buffer2,
            bind_group: bind_group2,
            receiver: None,
        };

        let ring_buffers = [ring1, ring2];

        return (buffer_size, ring_buffers);
    }

    pub fn set_model(&mut self, model_data: &ModelData) {
        let (buffer_size, ring_buffers) = Self::setup_model_ring(
            &self.device,
            &self.bind_group_layout,
            &self.mvp_buffer,
            model_data,
        );

        self.buffer_size = buffer_size;
        self.ring_buffers = ring_buffers;
    }

    // use double buffering
    pub fn compute_vertices(
        &mut self,
        mvp: glam::Mat4,
        vertex_count: u32,
    ) -> Option<Vec<glam::Vec4>> {
        // Option<Vec<Vertex>>
        // self.device.poll(wgpu::PollType::Poll).expect("GPU Error");

        // Send MVP to GPU
        let uniform_payload = ComputeUniforms {
            mvp: mvp.to_cols_array(),
            vertex_count: vertex_count as u32,
            _padding: [0; 3],
        };

        self.queue.write_buffer(
            &self.mvp_buffer,
            0,
            // bytemuck::cast_slice(&mvp.to_cols_array()),
            bytemuck::bytes_of(&uniform_payload),
        );

        let mut output_vertices = None;

        // 1] Clear all Ready Buffers
        for ring in &mut self.ring_buffers {
            if !ring.is_mapping {
                continue;
            }

            let Some(ref receiver) = ring.receiver else {
                continue;
            };

            let Ok(Ok(())) = receiver.try_recv() else {
                continue;
            };

            let data = ring.staging_buffer.slice(..).get_mapped_range();
            // let vertices: &[Vertex] = bytemuck::cast_slice(&data);
            let clip_space_vertices: &[glam::Vec4] = bytemuck::cast_slice(&data);
            // let t = clip_space_vertices[0];
            // println!("{t}");
            let out_vertices = clip_space_vertices.to_vec();

            drop(data);
            ring.staging_buffer.unmap(); // Return ownership to GPU
            ring.is_mapping = false;
            ring.receiver = None;

            output_vertices = Some(out_vertices);
        }

        // 2] Launch current Ring
        let current_ring_index = self.current_ring_index;
        let current_ring = &mut self.ring_buffers[current_ring_index];

        if !current_ring.is_mapping {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                // Compute Pass
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &current_ring.bind_group, &[]);
                // compute_pass.dispatch_workgroups(1, 1, 1);

                // Start workgroups -> threads
                let workgroup_count = (vertex_count + 63) / 64;
                compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
            }

            // Copy output -> staging
            encoder.copy_buffer_to_buffer(
                &current_ring.output_buffer,
                0,
                &current_ring.staging_buffer,
                0,
                self.buffer_size,
            );

            // Submit + feedback
            self.queue.submit(Some(encoder.finish()));
            current_ring.is_mapping = true;

            let (sender, receiver) = std::sync::mpsc::channel();
            current_ring
                .staging_buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| {
                    let _ = sender.send(res);
                });
            current_ring.receiver = Some(receiver);
        }

        // 3] Setup next frame
        let next_index = (current_ring_index + 1) % 2;
        self.current_ring_index = next_index;

        return output_vertices;
    }
}
