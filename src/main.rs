#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
// mod old_engine;

use std::vec;

use eframe::{CreationContext, egui::*};
use glam::Vec3;
use wgpu::util::DeviceExt;

// Import File
use rfd::AsyncFileDialog;
use std::sync::mpsc::{Receiver, Sender, channel};

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> eframe::Result {
    env_logger::init(); // GPU Logs

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Odek 3D Engine",
        options,
        Box::new(|_cc| Ok(Box::new(OdekEngine::new(_cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    // Redirect `log` message to `console.log`
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    // wgpu web settings
    // DEFAULT OPTION
    let web_options = eframe::WebOptions::default();

    // CUSTOM OPTION
    // let required_limits = wgpu::Limits {
    //     max_storage_buffers_per_shader_stage: 4,
    //     ..wgpu::Limits::downlevel_webgl2_defaults()
    // };

    // let setup_config = eframe::egui_wgpu::WgpuSetupCreateNew {
    //     instance_descriptor: wgpu::InstanceDescriptor {
    //         backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all()), // wgpu::Backends::BROWSER_WEBGPU
    //         flags: wgpu::InstanceFlags::from_build_config().with_env(),
    //         memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
    //         backend_options: wgpu::BackendOptions::from_env_or_default(),
    //         display: None,
    //     },
    //     display_handle: None,
    //     power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
    //     native_adapter_selector: None,

    //     device_descriptor: std::sync::Arc::new(move |_adapter| wgpu::DeviceDescriptor {
    //         label: Some("egui_wgpu_compute_device"),
    //         required_features: wgpu::Features::empty(),
    //         required_limits: required_limits.clone(),
    //         experimental_features: wgpu::ExperimentalFeatures::disabled(),
    //         memory_hints: wgpu::MemoryHints::default(),
    //         trace: wgpu::Trace::Off,
    //     }),
    // };

    // let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    // wgpu_options.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup_config);

    // let web_options = eframe::WebOptions {
    //     wgpu_options,
    //     ..Default::default()
    // };

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(OdekEngine::new(_cc)))),
            )
            .await;

        // Remove the loading text and spinner
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

struct Bindings {
    forward: egui::Key,
    left: egui::Key,
    backward: egui::Key,
    right: egui::Key,
}

impl Bindings {
    fn qwerty() -> Self {
        Self {
            forward: egui::Key::W,
            left: egui::Key::A,
            backward: egui::Key::S,
            right: egui::Key::D,
        }
    }

    fn azerty() -> Self {
        Self {
            forward: egui::Key::Z,
            left: egui::Key::Q,
            backward: egui::Key::S,
            right: egui::Key::D,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub _padding: f32, // 16 bytes
}

impl Vertex {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            _padding: 0.0,
        }
    }
}

#[repr(C)] // Prevent rust from reordering struct fields - Memory Layout need to be clean for GPU
// #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelData {
    // vertices: Vec<glam::Vec3>,
    vertices: Vec<Vertex>,
    faces: Vec<Vec<u16>>,
}

struct Buffering {
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    is_mapping: bool,
    mapping_receiver: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

// OdekGPU
struct GPUData {
    wgpu_device: wgpu::Device,
    wgpu_queue: std::sync::Arc<wgpu::Queue>,
    // Buffers
    mvp_buffer: wgpu::Buffer,
    buffer_size: wgpu::BufferAddress,
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    is_mapping: bool,
    mapping_receiver: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    // Pipeline
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    compute_bind_group: wgpu::BindGroup,
}

impl GPUData {
    fn new(cc: &CreationContext, model: &ModelData) -> Option<Self> {
        let state: Option<&eframe::egui_wgpu::RenderState> = cc.wgpu_render_state.as_ref();

        let Some(wgpu_render_state) = state else {
            return None;
        };

        let wgpu_device = wgpu_render_state.device.clone();
        let wgpu_queue: std::sync::Arc<wgpu::Queue> =
            std::sync::Arc::new(wgpu_render_state.queue.clone());

        // SHADER
        let shader = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
        });

        // PIPELINE
        let bind_group_layout =
            wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let pipeline_layout = wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let compute_pipeline =
            wgpu_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        // BUFFERS
        let buffer_size =
            (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;

        // MVP Buffer
        let identity_matrix = glam::Mat4::IDENTITY;

        let mvp_buffer = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MVP Uniform Buffer"),
            contents: bytemuck::cast_slice(&identity_matrix.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Staging Buffer : Copy output buffer -> Staging
        let staging_buffer = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (output_buffer, buffer_size, compute_bind_group) =
            Self::setup_model(&wgpu_device, &bind_group_layout, &mvp_buffer, &model);

        return Some(Self {
            wgpu_device,
            wgpu_queue,
            // Buffers
            mvp_buffer,
            buffer_size,
            staging_buffer,
            is_mapping: false,
            output_buffer,
            mapping_receiver: None,
            // Pipeline
            compute_pipeline,
            bind_group_layout,
            compute_bind_group,
        });
    }

    fn setup_model(
        wgpu_device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        mvp_buffer: &wgpu::Buffer,
        model: &ModelData,
    ) -> (wgpu::Buffer, u64, wgpu::BindGroup) {
        let vertex_buffer = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&model.vertices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let buffer_size =
            (model.vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress;

        // Output buffer
        let output_buffer = wgpu_device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Vertex Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group
        let compute_bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
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

        return (output_buffer, buffer_size, compute_bind_group);
    }

    fn set_model(&mut self, model_data: &ModelData) {
        let (output_buffer, buffer_size, compute_bind_group) = Self::setup_model(
            &self.wgpu_device,
            &self.bind_group_layout,
            &self.mvp_buffer,
            model_data,
        );

        self.output_buffer = output_buffer;
        self.buffer_size = buffer_size;
        self.compute_bind_group = compute_bind_group;
    }

    fn gpu_compute(&mut self, mvp: glam::Mat4, vertex_count: u32) -> Option<wgpu::BufferView> {
        // Called Every frame

        // Send MVP to GPU
        self.wgpu_queue.write_buffer(
            &self.mvp_buffer,
            0,
            bytemuck::cast_slice(&mvp.to_cols_array()),
        );

        if !self.is_mapping {
            let mut encoder = self
                .wgpu_device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

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

            self.wgpu_queue.submit(Some(encoder.finish()));

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

    fn double_buffering(&mut self, mvp: glam::Mat4, vertex_count: u32) -> Option<wgpu::BufferView> {
        // Send MVP to GPU
        self.wgpu_queue.write_buffer(
            &self.mvp_buffer,
            0,
            bytemuck::cast_slice(&mvp.to_cols_array()),
        );

        if !self.is_mapping {
            let mut encoder = self
                .wgpu_device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

                // let vertex_count = self.model_data.vertices.len() as u32;
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

            self.wgpu_queue.submit(Some(encoder.finish()));

            self.is_mapping = true;

            let buffer_slice = self.staging_buffer.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();

            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                // sender.send(result).unwrap();
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
}

struct OdekEngine {
    // RENDERING
    gpu_computing: bool,
    three_d_viewport: egui::Rect,
    stroke: Stroke,
    display_vertices: bool,
    // CAMERA
    // TODO : Store Radians instead of Degrees (Performance)
    smoothed_fps: f32,
    camera_position: Vec3,
    camera_rotation: Vec3, // Degrees : Yaw, Pitch, Roll
    camera_speed: f32,
    sensitivity: f32,
    camera_forward: Vec3,
    fov: f32, // Field of View (Degrees)
    perspective: bool,
    // LOGIC : Transformations
    bindings: Bindings,
    azerty: bool,
    // TODO : Objects List -> Manage Multiple Objects + Draw Origin
    model_position: Vec3,
    model_rotation: Vec3, // Degrees
    model_scale: Vec3,
    translate: bool,
    rotate: bool,
    scale: bool,
    translate_osciallator: f32,
    scale_osciallator: f32,
    // ENGINE DATA
    model_data: ModelData,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    gpu_data: Option<GPUData>,
}

impl OdekEngine {
    fn new(cc: &CreationContext) -> Self {
        let cube = Self::cube();

        // Communication Channel for Async File Loading
        let (tx, rx) = channel::<Vec<u8>>();

        let gpu_data = GPUData::new(cc, &cube);

        return Self {
            // RENDERING
            gpu_computing: false,
            three_d_viewport: cc.egui_ctx.content_rect(),
            stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(190, 110, 40)),
            display_vertices: true,
            // CAMERA
            smoothed_fps: 60.0,
            camera_position: Vec3::new(0.0, 0.0, -1.0),
            camera_rotation: Vec3::new(0.0, 180.0, 0.0),
            camera_speed: 2.0,
            sensitivity: 5.0,
            camera_forward: Vec3::new(0.0, 0.0, 1.0),
            fov: 90.0,
            perspective: true,
            // LOGIC : Inputs
            bindings: Bindings::qwerty(),
            azerty: false,
            // LOGIC : Transformations
            model_position: glam::Vec3::new(0.0, 0.0, 0.0),
            model_rotation: Vec3::new(0.0, 0.0, 0.0),
            model_scale: Vec3::new(1.0, 1.0, 1.0),
            translate: false,
            rotate: true,
            scale: false,
            translate_osciallator: 0.0,
            scale_osciallator: 0.0,
            // ENGINE DATA
            model_data: cube,
            tx,
            rx,
            gpu_data,
        };
    }

    // LOGIC : Transformations
    fn automatic_transform(&mut self, dt: f32) {
        // Model Translation
        if self.translate {
            self.translate_osciallator += dt;
            let amplitude = 0.01;
            let oscillation = self.translate_osciallator.sin() * amplitude;
            self.model_position.x += oscillation; // Oscillate horizontally
        }

        // Model Rotation
        if self.rotate {
            let angle = std::f32::consts::PI * dt; // 180 degrees per second
            self.model_rotation.y = (self.model_rotation.y + angle.to_degrees()) % 360.0;
        }

        // Model Scaling
        if self.scale {
            self.scale_osciallator += dt;
            let amplitude = 0.01;
            let oscillation = self.scale_osciallator.sin() * amplitude;
            self.model_scale += Vec3::new(oscillation, oscillation, oscillation);
        }
    }

    fn calc_camera_forward(&mut self) {
        // Calculate camera's forward vector based on its yaw and pitch (rotation)
        // YAW
        let yaw_rad = self.camera_rotation.y.to_radians();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        // PITCH
        let pitch_rad = self.camera_rotation.x.to_radians();
        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();

        self.camera_forward =
            Vec3::new(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw).normalize();
    }

    // RENDERING

    // Wireframe Rendering
    fn render_frame(&mut self, painter: &egui::Painter) {
        let screen_points: Vec<Option<egui::Vec2>> = self.frame_image();

        // Render Vertices
        if self.display_vertices {
            for point in &screen_points {
                self.render_vertex(&painter, *point);
            }
        }

        // Render Edges
        for face in &self.model_data.faces {
            for i in 0..face.len() {
                self.render_edge(
                    &painter,
                    screen_points[face[i] as usize],
                    screen_points[face[(i + 1) % face.len()] as usize],
                );
            }
        }
    }

    // Base Model -> Model Matrix (Model & Transformations) * View/Camera * Projection -> 2D Frustum (Projection) -> Screen Space
    fn frame_image(&mut self) -> Vec<Option<egui::Vec2>> {
        // 1) Model Matrix = Model + Transformations
        let model = glam::Mat4::from_scale_rotation_translation(
            self.model_scale,
            glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                self.model_rotation.y.to_radians(),
                self.model_rotation.x.to_radians(),
                self.model_rotation.z.to_radians(),
            ),
            self.model_position,
        );

        // 2) Camera / View Matrix
        let view = glam::Mat4::look_at_rh(
            self.camera_position,
            self.camera_position + self.camera_forward,
            Vec3::Y,
        ); // Vec3::Y = (0, 1, 0)

        // 3) Projection Matrix
        let projection = glam::Mat4::perspective_rh(
            self.fov.to_radians(),
            1.0,
            0.1,    // Near clip
            1000.0, // Far clip
        );

        // 4) Apply Matrices : Model -> View -> Projection
        let mvp: glam::Mat4 = projection * view * model;

        // 5) Projection : GPU Computing + CPU Fallback

        // GPU
        if self.gpu_computing
            && let Some(mut gpu_data) = self.gpu_data.take()
        {
            let data = gpu_data.gpu_compute(mvp, self.model_data.vertices.len() as u32);

            if let Some(data) = data {
                // println!("GPU");
                let output_vertices: &[Vertex] = bytemuck::cast_slice(&data);

                let proj_vertices = output_vertices
                    .iter()
                    .map(|v| {
                        return self.vertex_projection(&v);
                    })
                    .collect();

                drop(data);
                gpu_data.staging_buffer.unmap();

                self.gpu_data = Some(gpu_data);
                return proj_vertices;
            }

            self.gpu_data = Some(gpu_data);
        }

        // println!("CPU");
        return self
            .model_data
            .vertices
            .iter()
            .map(|v| {
                // let world_vertex: Vec3 = mvp.project_point3(*v);

                let w: Vec3 =
                    mvp.project_point3(Vec3::new(v.position[0], v.position[1], v.position[2]));
                let world_vertex: Vertex = Vertex::new(w.x, w.y, w.z);

                return self.vertex_projection(&world_vertex);
            })
            .collect();
    }

    // World -> 2D Frustum
    fn vertex_projection(&self, world_v: &Vertex) -> Option<Vec2> {
        // &Vec3
        // let is_in_fov = world_v.x.abs() <= 1.0 && world_v.y.abs() <= 1.0 && world_v.z.abs() <= 1.0;
        let is_in_fov = world_v.position[0].abs() <= 1.0
            && world_v.position[1].abs() <= 1.0
            && world_v.position[2].abs() <= 1.0;

        let fulcrum_point: Vec2;

        if self.perspective {
            fulcrum_point = Self::perspective_project(&world_v);
        } else {
            fulcrum_point = Self::orthographic_project(&world_v);
        }

        return (is_in_fov).then(|| {
            Self::proj_to_screen(
                &fulcrum_point,
                self.three_d_viewport.width(),
                self.three_d_viewport.height(),
            )
        });
    }

    fn perspective_project(vertex: &Vertex) -> Vec2 {
        // &Vec3
        // return Vec2::new(vertex.x / vertex.z, vertex.y / vertex.z);
        return Vec2::new(
            vertex.position[0] / vertex.position[2],
            vertex.position[1] / vertex.position[2],
        );
    }

    fn orthographic_project(vertex: &Vertex) -> Vec2 {
        return Vec2::new(vertex.position[0], vertex.position[1]);
    }

    // 2D Frustum -> Screen space
    fn proj_to_screen(point: &Vec2, width: f32, height: f32) -> Vec2 {
        // Aspect Ratio Correction -> Resize Window
        let min = width.min(height);
        let x_offset = (width.max(height) - min) * 0.5;

        // -1..1 -> 0..2 -> 0..1 -> 0..width/height
        let x = (point.x + 1.0) / 2.0 * min + x_offset;
        let y = (1.0 - (point.y + 1.0) / 2.0) * min;
        return Vec2::new(x, y);
    }

    fn render_vertex(&self, painter: &egui::Painter, vertex_pos: Option<Vec2>) {
        if let Some(point) = vertex_pos {
            let vertex_rect =
                Rect::from_center_size(self.three_d_viewport.left_top() + point, vec2(10.0, 10.0));
            painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
        }
    }

    fn render_edge(&self, painter: &egui::Painter, p1: Option<Vec2>, p2: Option<Vec2>) {
        if let (Some(p1), Some(p2)) = (p1, p2) {
            painter.line_segment(
                [
                    self.three_d_viewport.left_top() + p1,
                    self.three_d_viewport.left_top() + p2,
                ],
                self.stroke,
            );
        }
    }

    // UTILS

    fn cube() -> ModelData {
        // let vertices = vec![
        //     // Front Face
        //     Vec3::new(0.25, 0.25, 0.25),
        //     Vec3::new(-0.25, 0.25, 0.25),
        //     Vec3::new(-0.25, -0.25, 0.25),
        //     Vec3::new(0.25, -0.25, 0.25),
        //     // Back Face
        //     Vec3::new(0.25, 0.25, -0.25),
        //     Vec3::new(-0.25, 0.25, -0.25),
        //     Vec3::new(-0.25, -0.25, -0.25),
        //     Vec3::new(0.25, -0.25, -0.25),
        // ];

        let vertices = vec![
            // Front Face
            Vertex::new(0.25, 0.25, 0.25),
            Vertex::new(-0.25, 0.25, 0.25),
            Vertex::new(-0.25, -0.25, 0.25),
            Vertex::new(0.25, -0.25, 0.25),
            // Back Face
            Vertex::new(0.25, 0.25, -0.25),
            Vertex::new(-0.25, 0.25, -0.25),
            Vertex::new(-0.25, -0.25, -0.25),
            Vertex::new(0.25, -0.25, -0.25),
        ];

        let faces: Vec<Vec<u16>> = vec![
            vec![0, 1, 2, 3], // Front
            vec![4, 5, 6, 7], // Back
            vec![0, 4],
            vec![1, 5],
            vec![2, 6],
            vec![3, 7],
            // Full Faces
            // vec![0, 4, 7, 3], // Right
            // vec![1, 5, 6, 2], // Left
            // vec![0, 1, 5, 4], // Top
            // vec![3, 2, 6, 7], // Bottom
        ];

        return ModelData { vertices, faces };
    }

    fn set_model(&mut self, model: ModelData) {
        self.model_data = model;

        let Some(gpu_data) = self.gpu_data.as_mut() else {
            return;
        };

        gpu_data.set_model(&self.model_data);
    }

    fn hud(&mut self, painter: &egui::Painter, fps: f32) {
        // Displayed on top of the 3D View
        // FPS Display
        let alpha = 0.05;
        self.smoothed_fps = (self.smoothed_fps * (1.0 - alpha)) + (fps * alpha);

        painter.text(
            self.three_d_viewport.left_top() + egui::vec2(10.0, 10.0), // 10px padding from top-left
            egui::Align2::LEFT_TOP,
            format!("FPS: {:.2}", self.smoothed_fps),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );

        // Controls Display
        painter.text(
            self.three_d_viewport.left_top() + egui::vec2(10.0, 30.0),
            egui::Align2::LEFT_TOP,
            "Movement : WASD\nLook : Right Click",
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    // Load OBJ
    fn pick_obj_async(&mut self) {
        // Define Operations
        let pick_file = AsyncFileDialog::new()
            .add_filter("obj", &["obj"])
            .pick_file();

        let tx = self.tx.clone();
        let task = async move {
            if let Some(file_handle) = pick_file.await {
                let bytes = file_handle.read().await;
                tx.send(bytes).unwrap();
            }
        };

        // Execute Operations based on Environment
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(task);
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(task);
    }

    fn load_obj_bytes(&mut self, bytes: Vec<u8>) {
        // Use a cursor to treat the bytes like a file stream
        let mut reader = std::io::Cursor::new(bytes);

        let (models, materials) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |_p| Ok((vec![], ahash::AHashMap::new())), // Material loader
        )
        .expect("Failed to parse OBJ");

        let mesh = &models[0].mesh;
        self.load_mesh(mesh);
    }

    fn load_mesh(&mut self, mesh: &tobj::Mesh) {
        // 1. Convert flat f32 vec [x,y,z, x,y,z] to Vec<Vec3>
        // let vertices: Vec<Vec3> = mesh
        //     .positions
        //     .chunks_exact(3)
        //     .map(|p| Vec3::new(p[0], p[1], p[2]))
        //     .collect();

        let vertices: Vec<Vertex> = mesh
            .positions
            .chunks_exact(3)
            .map(|p| Vertex::new(p[0], p[1], p[2]))
            .collect();

        // 2. Convert flat indices [0,1,2, 3,4,5] to Vec<Vec<u8>>
        let faces: Vec<Vec<u16>> = mesh
            .indices
            .chunks_exact(3)
            .map(|f| f.iter().map(|&i| i as u16).collect())
            .collect();

        // self.model_data = ModelData { vertices, faces };
        self.set_model(ModelData { vertices, faces });
    }

    // FUTURE ENGINE : GPU Render pipeline

    fn future_engine() {
        // let shader_module = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
        //     label: Some("Shader"),
        //     source: wgpu::ShaderSource::Wgsl(include_str!("vertex_shader.wgsl").into()),
        // });

        // // Create Vertex Buffer
        // let vertex_buffer = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("Vertex Buffer"),
        //     contents: bytemuck::cast_slice(&cube.vertices),
        //     usage: wgpu::BufferUsages::VERTEX,
        // });

        // // Create MVP Buffer
        // let identity_matrix = glam::Mat4::IDENTITY;

        // let mvp_buffer = wgpu_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("MVP Uniform Buffer"),
        //     contents: bytemuck::cast_slice(&identity_matrix.to_cols_array()),
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        // });

        // let render_pipeline = Self::gpu_pipeline(&wgpu_device, &mvp_buffer, &shader_module);
    }

    fn gpu_pipeline(
        wgpu_device: &wgpu::Device,
        mvp_buffer: &wgpu::Buffer,
        shader_module: &wgpu::ShaderModule,
    ) {
        // -> wgpu::RenderPipeline
        // Bind Group -> MVP
        let bind_group_layout =
            wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("mvp_bind_group_layout"),
            });

        let bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mvp_buffer.as_entire_binding(),
            }],
            label: Some("mvp_bind_group"),
        });

        // Vertex buffer layout
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0, // @location(0) in vertex_shader.wgsl
                format: wgpu::VertexFormat::Float32x3,
            }],
        };

        let pipeline_layout = wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            // push_constant_ranges: &[],
            immediate_size: 0,
        });

        // let render_pipeline = wgpu_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        //     label: Some("Render Pipeline"),
        //     layout: Some(&pipeline_layout),
        //     vertex: wgpu::VertexState {
        //         module: &shader_module,
        //         entry_point: Some("vs_main"),
        //         compilation_options: Default::default(),
        //         buffers: &[vertex_buffer_layout],
        //     },
        //     fragment: Some(wgpu::FragmentState {
        //         module: &shader_module,
        //         entry_point: Some("fs_main"),
        //         compilation_options: Default::default(),
        //         targets: &[Some(wgpu::ColorTargetState {
        //             format: wgpu::TextureFormat::Bgra8UnormSrgb, // Match the surface format
        //             blend: Some(wgpu::BlendState::REPLACE),
        //             write_mask: wgpu::ColorWrites::ALL,
        //         })],
        //     }),
        //     // ... primitive, depth_stencil, and multisample settings
        // });

        // return render_pipeline;
    }
}

impl eframe::App for OdekEngine {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ui.request_repaint();
            ui.request_repaint_after(std::time::Duration::from_millis(16)); // 60 FPS
            let dt = ui.input(|i| i.stable_dt); // DeltaTime in second
            let fps = 1.0 / dt;

            // Check for imported model
            if let Ok(bytes) = self.rx.try_recv() {
                self.load_obj_bytes(bytes);
            }

            // USER INTERFACE

            // Settings : Import OBJ, Reset Scene, Perspective, Render Vertices, Bindings
            ui.horizontal(|ui| {
                // Import OBJ
                if ui.button("Import OBJ").clicked() {
                    self.pick_obj_async();
                }

                // Reset Scene
                if ui.button("Reset Scene").clicked() {
                    self.camera_position = Vec3::new(0.0, 0.0, -1.0);
                    self.camera_rotation = Vec3::new(0.0, 180.0, 0.0);

                    self.model_position = Vec3::new(0.0, 0.0, 0.0);
                    self.model_rotation = Vec3::new(0.0, 0.0, 0.0);
                    self.model_scale = Vec3::new(1.0, 1.0, 1.0);

                    self.set_model(Self::cube());
                }

                // Rendering Settings
                ui.checkbox(&mut self.gpu_computing, "GPU Computing");
                ui.checkbox(&mut self.perspective, "Perspective");
                ui.add(
                    egui::DragValue::new(&mut self.fov)
                        .prefix("FOV: ")
                        .speed(0.1)
                        .range(10.0..=170.0),
                );
                ui.checkbox(&mut self.display_vertices, "Render Vertices");

                // Controls
                if ui.checkbox(&mut self.azerty, "AZERTY").clicked() {
                    if self.azerty {
                        self.bindings = Bindings::azerty();
                    } else {
                        self.bindings = Bindings::qwerty();
                    }
                }
            });

            // Manual Transformations
            ui.horizontal(|ui| {
                ui.label("Model");

                // Model Position
                ui.label("Position :");
                ui.add(
                    egui::DragValue::new(&mut self.model_position.x)
                        .prefix("X: ")
                        .speed(0.01),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_position.y)
                        .prefix("Y: ")
                        .speed(0.01),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_position.z)
                        .prefix("Z: ")
                        .speed(0.01),
                );

                // Model Rotation
                ui.label("Rotation :");
                // let response =
                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.x)
                        .prefix("X: ")
                        .speed(0.05)
                        .range(-360.0..=360.0)
                        .custom_formatter(|n, _| format!("{n:.2}")),
                );

                // if response.changed() {
                //     println!("Rotation X is now: {}", self.model_rotation.x);
                //     // Change to radians
                // }

                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.y)
                        .prefix("Y: ")
                        .speed(0.05)
                        .range(-360.0..=360.0)
                        .custom_formatter(|n, _| format!("{n:.2}")),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.z)
                        .prefix("Z: ")
                        .speed(0.05)
                        .range(-360.0..=360.0)
                        .custom_formatter(|n, _| format!("{n:.2}")),
                );

                // Model Scale
                ui.label("Scale :");
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.x)
                        .prefix("X: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.y)
                        .prefix("Y: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.z)
                        .prefix("Z: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
            });

            // Automatic Transformations
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.rotate, "Rotate");
                ui.checkbox(&mut self.translate, "Translate");
                ui.checkbox(&mut self.scale, "Scale");
            });

            // LOGIC

            // Camera Controls
            self.calc_camera_forward();
            ui.input(|input| {
                // Camera Position
                if input.key_down(self.bindings.forward) {
                    self.camera_position += self.camera_forward * self.camera_speed * dt; // Forward
                } else if input.key_down(self.bindings.backward) {
                    self.camera_position -= self.camera_forward * self.camera_speed * dt; // Backward
                }

                if input.key_down(self.bindings.left) {
                    self.camera_position -=
                        self.camera_forward.cross(Vec3::Y) * self.camera_speed * dt; // Left
                } else if input.key_down(self.bindings.right) {
                    self.camera_position +=
                        self.camera_forward.cross(Vec3::Y) * self.camera_speed * dt; // Right
                }

                // Camera Angle
                if input.pointer.secondary_down() {
                    let mouse_delta = input.pointer.delta();
                    self.camera_rotation.y += mouse_delta.x * self.sensitivity * dt; // Yaw / Horizontal
                    self.camera_rotation.x -= mouse_delta.y * self.sensitivity * dt; // Pitch / Vertical
                }
            });

            self.automatic_transform(dt);

            // 3D RENDERING VIEW

            // Draw Area
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());
            let rect = response.rect;
            self.three_d_viewport = rect;

            // Border
            painter.rect_stroke(
                rect,
                5.0,
                egui::Stroke::new(2.0, egui::Color32::GREEN),
                egui::StrokeKind::Middle,
            );

            self.render_frame(&painter);
            self.hud(&painter, fps);
        });
    }
}
