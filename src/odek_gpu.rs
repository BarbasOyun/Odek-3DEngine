use crate::ModelData;
use crate::Vertex;

use eframe::CreationContext;
use std::sync::mpsc::Receiver;
use wgpu::util::DeviceExt;

struct RingBuffer {
    is_mapping: bool,
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    receiver: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

pub struct GPUData {
    device: wgpu::Device,
    queue: std::sync::Arc<wgpu::Queue>,
    // Buffers
    mvp_buffer: wgpu::Buffer,
    buffer_size: wgpu::BufferAddress,
    // output_buffer: wgpu::Buffer,
    // staging_buffer: wgpu::Buffer,
    // is_mapping: bool,
    // mapping_receiver: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    // Ring Buffers = Double Buffering
    ring_buffers: [RingBuffer; 2],
    current_ring_index: usize,
    // Pipeline
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    // bind_group: wgpu::BindGroup,
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
            source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
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
        let buffer_size: u64 =
            (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;

        // MVP Buffer
        let identity_matrix = glam::Mat4::IDENTITY;

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MVP Uniform Buffer"),
            contents: bytemuck::cast_slice(&identity_matrix.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Staging Buffer : Copy output buffer -> Staging
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
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

        // let (output_buffer, buffer_size, bind_group) =
        //     Self::setup_model(&device, &bind_group_layout, &mvp_buffer, &model);
        let (buffer_size, output_buffer1, output_buffer2, bind_group1, bind_group2) =
            Self::setup_model_ring(&device, &bind_group_layout, &mvp_buffer, &model);

        let ring1 = RingBuffer {
            is_mapping: false,
            output_buffer: output_buffer1,
            staging_buffer,
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

        return Some(Self {
            device,
            queue,
            // Buffers
            mvp_buffer,
            buffer_size,
            // staging_buffer,
            // is_mapping: false,
            // output_buffer,
            // mapping_receiver: None,
            // Ring Buffers
            ring_buffers: [ring1, ring2],
            current_ring_index: 0,
            // Pipeline
            compute_pipeline,
            bind_group_layout,
            // bind_group,
        });
    }

    pub fn setup_model_ring(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        mvp_buffer: &wgpu::Buffer,
        model: &ModelData,
    ) -> (
        u64,
        wgpu::Buffer,
        wgpu::Buffer,
        wgpu::BindGroup,
        wgpu::BindGroup,
    ) {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&model.vertices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let buffer_size =
            (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;

        // Output buffer
        let output_buffer1 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group
        let bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
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

        // Output buffer
        let output_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer2"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group
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

        return (
            buffer_size,
            output_buffer1,
            output_buffer2,
            bind_group1,
            bind_group2,
        );
    }

    pub fn set_model(&mut self, model_data: &ModelData) {
        // let (output_buffer, buffer_size, compute_bind_group) = Self::setup_model(
        //     &self.device,
        //     &self.bind_group_layout,
        //     &self.mvp_buffer,
        //     model_data,
        // );

        let (buffer_size, output_buffer1, output_buffer2, bind_group1, bind_group2) =
            Self::setup_model_ring(
                &self.device,
                &self.bind_group_layout,
                &self.mvp_buffer,
                model_data,
            );

        // self.output_buffer = output_buffer;
        self.buffer_size = buffer_size;
        // self.bind_group = compute_bind_group;
    }

    pub fn double_buffering(&mut self, mvp: glam::Mat4, vertex_count: u32) -> Option<Vec<Vertex>> {
        // wgpu::BufferView
        // Send MVP to GPU
        self.queue.write_buffer(
            &self.mvp_buffer,
            0,
            bytemuck::cast_slice(&mvp.to_cols_array()),
        );

        // Launch current Ring
        let current_ring = &mut self.ring_buffers[self.current_ring_index];

        if !current_ring.is_mapping {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &current_ring.bind_group, &[]);
                // compute_pass.dispatch_workgroups(1, 1, 1);

                let workgroup_count = (vertex_count + 63) / 64;
                compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
            }

            encoder.copy_buffer_to_buffer(
                &current_ring.output_buffer,
                0,
                &current_ring.staging_buffer,
                0,
                self.buffer_size,
            );
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

        // Get Other Ring's Data/Result
        let other_resource_index = (self.current_ring_index + 1) % 2;
        let other_ring = &mut self.ring_buffers[other_resource_index];

        if other_ring.is_mapping {
            if let Some(ref receiver) = other_ring.receiver {
                if let Ok(Ok(())) = receiver.try_recv() {
                    let data = other_ring.staging_buffer.slice(..).get_mapped_range();
                    // return Some(data);

                    let vertices: &[Vertex] = bytemuck::cast_slice(&data);
                    let output_vertices = vertices.to_vec();

                    drop(data);
                    other_ring.staging_buffer.unmap(); // Return ownership to GPU
                    other_ring.is_mapping = false;
                    other_ring.receiver = None;

                    return Some(output_vertices);
                }
            }
        }

        return None;
    }

    // OLD SINGLE BUFFERING
    /*
    pub fn setup_model(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        mvp_buffer: &wgpu::Buffer,
        model: &ModelData,
    ) -> (wgpu::Buffer, u64, wgpu::BindGroup) {
        // Input Buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&model.vertices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let buffer_size =
            (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;

        // Output buffer
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
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
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        return (output_buffer, buffer_size, bind_group);
    }

    fn gpu_compute(&mut self, mvp: glam::Mat4, vertex_count: u32) -> Option<wgpu::BufferView> {
        // Called Every frame

        // Send MVP to GPU
        self.queue.write_buffer(
            &self.mvp_buffer,
            0,
            bytemuck::cast_slice(&mvp.to_cols_array()),
        );

        if !self.is_mapping {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.bind_group, &[]);

                let workgroup_count = (vertex_count + 63) / 64;
                compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
            }

            // output_buffer -> staging_buffer
            encoder.copy_buffer_to_buffer(
                &self.output_buffer,
                0,
                &self.staging_buffer,
                0,
                self.buffer_size,
            );

            self.queue.submit(Some(encoder.finish()));

            self.is_mapping = true;

            let buffer_slice = self.staging_buffer.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();

            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });

            self.mapping_receiver = Some(receiver);
        }

        if let Some(receiver) = &self.mapping_receiver {
            if let Ok(Ok(())) = receiver.try_recv() {
                let buffer_slice = self.staging_buffer.slice(..);
                let data = buffer_slice.get_mapped_range();

                self.mapping_receiver = None;
                self.is_mapping = false;

                return Some(data);
            }
        }

        return None;
    }
    */
}
