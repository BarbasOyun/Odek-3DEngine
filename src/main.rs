#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod odek_gpu;
// mod old_engine;

use crate::odek_gpu::GPUData;
use eframe::{CreationContext, egui::*};
use glam::Vec3;
use std::fmt;
use std::vec;

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
        "Odek 3D Engine 0.2",
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
    // 1) DEFAULT OPTION
    let web_options = eframe::WebOptions::default();

    // 2) CUSTOM OPTION
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

// DATA STRUCTS

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

// GPU vertex struct
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    _padding: f32, // 12 + 4 = 16 bytes
}

impl Vertex {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            _padding: 0.0,
        }
    }
}

struct ModelData {
    // TODO : array
    vertices: Vec<Vertex>,
    faces: Vec<Vec<u16>>,
}

impl fmt::Display for ModelData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        write!(
            f,
            "ModelData (Vertices: {}, Faces: {})\n",
            self.vertices.len(),
            self.faces.len()
        )?;

        // write vertices
        // write!(f, "  Vertices:\n")?;
        // for (i, vertex) in self.vertices.iter().enumerate() {
        //     write!(f, "    [{}]: {:?}\n", i, vertex)?;
        // }

        write!(f, "  Faces:\n")?;
        for (i, face) in self.faces.iter().enumerate() {
            write!(f, "    [{}]: {:?}\n", i, face)?;
        }

        Ok(())
    }
}

struct Model {
    id: u8,
    // Model
    vertex_count: u32,
    face_count: u32,
    // Transform
    position: Vec3,
    rotation: Vec3, // Degrees
    scale: Vec3,
    // Transformations
    is_translating: bool,
    is_rotating: bool,
    is_scaling: bool,
    translate_osciallator: f32,
    scale_osciallator: f32,
}

impl Model {
    fn new(
        id: u8,
        vertex_count: u32,
        face_count: u32,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
        is_translating: bool,
        is_rotating: bool,
        is_scaling: bool,
    ) -> Model {
        return Self {
            id,
            vertex_count,
            face_count,
            position,
            rotation,
            scale,
            is_translating,
            is_rotating,
            is_scaling,
            translate_osciallator: 0.0,
            scale_osciallator: 0.0,
        };
    }

    fn default(id: u8, vertex_count: u32, face_count: u32) -> Self {
        return Self {
            id,
            vertex_count,
            face_count,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            is_translating: false,
            is_rotating: true,
            is_scaling: false,
            translate_osciallator: 0.0,
            scale_osciallator: 0.0,
        };
    }
}

// mut engine data
struct EngineState {
    current_model: usize, // index of selected model
    // Camera
    // TODO : Store Radians instead of Degrees (Performance)
    camera_position: Vec3,
    camera_rotation: Vec3, // Yaw, Pitch, Roll -> Degrees°
    camera_forward: Vec3,
    // Models
    // Flattened vertices + faces
    vertices: Vec<Vertex>,
    faces: Vec<Vec<u16>>,
    models: Vec<Model>, // models data to get the vertices & faces
}

impl EngineState {
    fn new() -> Self {
        let mut cube1 = OdekEngine::cube();

        let mut cube2 = OdekEngine::cube();
        OdekEngine::triangulate_faces(&mut cube2);

        // Models
        let model1 = Model::default(0, cube1.vertices.len() as u32, cube1.faces.len() as u32);
        let model2 = Model::new(
            1,
            cube2.vertices.len() as u32,
            cube2.faces.len() as u32,
            Vec3::new(1.5, 0.0, -1.5),
            Vec3::ZERO,
            Vec3::ONE,
            false,
            true,
            false,
        );

        let models = vec![model1, model2];

        // Global Model Data
        let mut vertices = vec![];
        vertices.append(&mut cube1.vertices);
        vertices.append(&mut cube2.vertices);

        let mut faces: Vec<Vec<u16>> = vec![];
        faces.append(&mut cube1.faces);
        faces.append(&mut cube2.faces);

        return Self {
            current_model: 0,
            // Camera
            camera_position: Vec3::new(0.0, 0.0, 1.5),
            camera_rotation: Vec3::ZERO,
            camera_forward: Vec3::ZERO,
            // Models
            vertices,
            faces,
            models,
        };
    }
}

struct Settings {
    // Rendering
    gpu_computing: bool,
    display_vertices: bool,
    // Camera
    camera_speed: f32,
    sensitivity: f32,
    perspective: bool,
    fov: f32, // Field of View -> Degrees°
    // Focal Length
    near_plane: f32,
    // Bindings
    bindings: Bindings,
    azerty: bool,
}

impl Default for Settings {
    fn default() -> Self {
        return Self {
            // Rendering
            gpu_computing: false,
            display_vertices: true,
            // Camera
            camera_speed: 2.0,
            sensitivity: 5.0,
            fov: 90.0,
            perspective: true,
            near_plane: 0.1,
            // Bindings
            bindings: Bindings::qwerty(),
            azerty: false,
        };
    }
}

struct OdekEngine {
    // ENGINE DATA
    state: EngineState,
    settings: Settings,
    gpu_data: Option<GPUData>,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    // RENDERING
    three_d_viewport: egui::Rect,
    stroke: Stroke,
    smoothed_fps: f32,
}

impl OdekEngine {
    fn new(cc: &CreationContext) -> Self {
        let state = EngineState::new();

        // Communication Channel for Async File Loading
        let (tx, rx) = channel::<Vec<u8>>();

        // TODO : Multi model GPU
        // let globalModel = ModelData {state.vertices, state.faces};
        // let gpu_data = GPUData::new(cc, globalModel);

        return Self {
            // ENGINE DATA
            state,
            settings: Settings::default(),
            gpu_data: None,
            tx,
            rx,
            // RENDERING
            three_d_viewport: cc.egui_ctx.content_rect(),
            stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(190, 110, 40)),
            smoothed_fps: 60.0,
        };
    }

    // RENDERING

    // Wireframe Rendering
    fn render_frame(&self, painter: &egui::Painter) {
        let state = &self.state;
        let settings = &self.settings;
        let screen_points: Vec<Option<egui::Vec2>> = self.frame_image();

        // Render Vertices
        if settings.display_vertices {
            for point in &screen_points {
                if let Some(p) = point
                    && self.is_in_screen(*p)
                {
                    self.render_vertex(&painter, *p);
                }
            }
        }

        // Render Edges
        let mut vertex_count: u32 = 0;
        let mut face_count = 0;

        for model in &state.models {
            // foreach model's faces
            for i in 0..model.face_count {
                let face_index = (face_count + i) as usize;
                let face = &state.faces[face_index];

                // foreach vertex index in face
                for j in 0..face.len() {
                    let point1_index = (vertex_count + face[j] as u32) as usize;
                    let point2_index = (vertex_count + face[(j + 1) % face.len()] as u32) as usize;
                    let point1 = screen_points[point1_index];
                    let point2 = screen_points[point2_index];

                    // if vertices are on screen
                    if let (Some(p1), Some(p2)) = (point1, point2) {
                        // TODO : 3D Clipping
                        // let is_p1_in_screen = self.is_in_screen(p1);
                        // let is_p2_in_screen = self.is_in_screen(p2);

                        // if is_p1_in_screen || is_p2_in_screen {
                        //     self.render_edge(&painter, p1, p2);
                        // }

                        self.render_edge(&painter, p1, p2);
                    }
                }
            }

            vertex_count += model.vertex_count;
            face_count += model.face_count;
        }
    }

    // Base Model Vertices * MVP Matrix -> Model (Vertices & Transformations) * View * Projection -> Projection 2D Frustum / Clip Space -> Screen Space
    // return points on screen
    fn frame_image(&self) -> Vec<Option<egui::Vec2>> {
        let state = &self.state;
        let settings = &self.settings;

        // Matrices
        // 1] View -> Camera
        let view = glam::Mat4::look_at_rh(
            state.camera_position,
            state.camera_position + state.camera_forward,
            Vec3::Y,
        ); // Vec3::Y = (0, 1, 0)

        // 2] Projection
        let projection;

        // Perspective
        if settings.perspective {
            let fov_y_radians = 2.0
                * f32::atan(
                    f32::tan(settings.fov.to_radians() / 2.0)
                        / self.three_d_viewport.aspect_ratio(),
                ); // self.fov.to_radians()

            projection = glam::Mat4::perspective_rh(
                fov_y_radians,
                self.three_d_viewport.aspect_ratio(),
                settings.near_plane, // Near clip
                1000.0,              // Far clip
            );
        } else {
            // Orthographic
            let half_width = self.three_d_viewport.width() * 0.001;
            let half_height = self.three_d_viewport.height() * 0.001;

            projection = glam::Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                settings.near_plane,
                1000.0,
            );
        }

        // Calculate VP once
        let vp = projection * view;

        // 3] Model -> Per model MVP
        let mut models_mvp: Vec<glam::Mat4> = vec![];

        for model in &state.models {
            let model_matrix = glam::Mat4::from_scale_rotation_translation(
                model.scale,
                glam::Quat::from_euler(
                    glam::EulerRot::YXZ,
                    model.rotation.y.to_radians(),
                    model.rotation.x.to_radians(),
                    model.rotation.z.to_radians(),
                ),
                model.position,
            );

            let mvp: glam::Mat4 = vp * model_matrix;
            models_mvp.push(mvp);
        }

        // 4] Projection : GPU Computing + CPU Fallback
        // GPU
        /*
        if self.gpu_computing
            && let Some(mut gpu_data) = self.gpu_data.take()
        {
            let clip_space_vertices =
                gpu_data.compute_vertices(mvp, self.model_data.vertices.len() as u32);

            if let Some(clip_space_vertices) = clip_space_vertices {
                let screen_points = clip_space_vertices
                    .iter()
                    .map(|clip_space_v| {
                        return self.clip_to_screen(clip_space_v);
                    })
                    .collect();

                self.gpu_data = Some(gpu_data);
                return screen_points;
            }

            self.gpu_data = Some(gpu_data);
        }
        */

        // CPU
        let mut model_id = 0;
        let mut vertex_count = 0;
        let mut model_vertices_count = 0;

        let screen_points = state
            .vertices
            .iter()
            .map(|vertex| {
                let model_vertices = state.models[model_id].vertex_count;

                if vertex_count >= model_vertices_count + model_vertices {
                    model_vertices_count += model_vertices;
                    model_id += 1;
                }

                let vertex_vec4 = glam::vec4(
                    vertex.position[0],
                    vertex.position[1],
                    vertex.position[2],
                    1.0,
                );

                let clip_space_v = models_mvp[model_id] * vertex_vec4;

                vertex_count += 1;
                return self.clip_to_screen(&clip_space_v);
            })
            .collect();

        return screen_points;
    }

    fn out_of_fov_edge_rendering(clip_space_v: &glam::Vec4) -> glam::Vec4 {
        // find unique edge / model first
        todo!()
    }

    // clip_space_vertex -> screen point
    fn clip_to_screen(&self, clip_space_v: &glam::Vec4) -> Option<Vec2> {
        let is_in_fov = clip_space_v.x.abs() <= clip_space_v.w
            && clip_space_v.y.abs() <= clip_space_v.w
            && clip_space_v.z >= 0.0
            && clip_space_v.z <= clip_space_v.w;

        // TODO : Out of FOV edge rendering
        if is_in_fov {
            // fulcrum
            let ndc_x = clip_space_v.x / clip_space_v.w;
            let ndc_y = clip_space_v.y / clip_space_v.w;
            // let ndc_z = clip_space_v.z / clip_space_v.w;

            // screen
            let screen_x = (ndc_x + 1.0) * 0.5 * self.three_d_viewport.width();
            let screen_y = (1.0 - ndc_y) * 0.5 * self.three_d_viewport.height(); // Inverted Y for UI

            return Some(Vec2::new(screen_x, screen_y));
        }

        return None;
    }

    fn render_vertex(&self, painter: &egui::Painter, point: Vec2) {
        let vertex_rect =
            Rect::from_center_size(self.three_d_viewport.left_top() + point, vec2(10.0, 10.0));

        painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
    }

    fn render_edge(&self, painter: &egui::Painter, p1: Vec2, p2: Vec2) {
        painter.line_segment(
            [
                self.three_d_viewport.left_top() + p1,
                self.three_d_viewport.left_top() + p2,
            ],
            self.stroke,
        );
    }

    fn is_in_screen(&self, screen_point: Vec2) -> bool {
        let is_in_width = 0.0 <= screen_point.x && screen_point.x <= self.three_d_viewport.width();
        let is_in_height =
            0.0 <= screen_point.y && screen_point.y <= self.three_d_viewport.height();

        return is_in_width && is_in_height;
    }

    // SCENE / MODELS

    fn add_model(&mut self, model: &mut ModelData) {
        let state = &mut self.state;

        // Add the models data at then end of the global models data
        state.vertices.append(&mut model.vertices);
        state.faces.append(&mut model.faces);

        // Add Model
        let id = state.models.len();
        let model = Model::default(
            id as u8,
            model.vertices.len() as u32,
            model.faces.len() as u32,
        );
        state.models.push(model);
    }

    // fn set_model(&mut self, model: ModelData) {
    //     self.model_data = model;

    //     let Some(gpu_data) = self.gpu_data.as_mut() else {
    //         return;
    //     };

    //     gpu_data.set_model(&self.model_data);
    // }

    fn remove_model(&mut self, index: u8) {
        todo!()
    }

    fn reset_scene(&mut self) {
        self.state = EngineState::new();
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

        // self.set_model(ModelData { vertices, faces });
    }

    // Models Transformations
    fn automatic_transform(&mut self, dt: f32) {
        let state = &mut self.state;

        for model in &mut state.models {
            // Model Translation
            if model.is_translating {
                model.translate_osciallator += dt;
                let amplitude = 0.01;
                let oscillation = model.translate_osciallator.sin() * amplitude;
                model.position.x += oscillation; // Oscillate horizontally
            }

            // Model Rotation
            if model.is_rotating {
                let angle = std::f32::consts::PI * dt; // 180 degrees per second
                model.rotation.y = (model.rotation.y + angle.to_degrees()) % 360.0;
            }

            // Model Scaling
            if model.is_scaling {
                model.scale_osciallator += dt;
                let amplitude = 0.01;
                let oscillation = model.scale_osciallator.sin() * amplitude;
                model.scale += Vec3::new(oscillation, oscillation, oscillation);
            }
        }
    }

    // Camera
    fn calc_camera_forward(&mut self) {
        let state: &mut EngineState = &mut self.state;

        // Calculate camera's forward vector based on its yaw and pitch (rotation)
        // YAW
        let yaw_rad = state.camera_rotation.y.to_radians();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        // PITCH
        let pitch_rad = state.camera_rotation.x.to_radians();
        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();

        state.camera_forward =
            Vec3::new(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw).normalize();
    }

    fn camera_controls(&mut self, input: &InputState, dt: f32) {
        self.calc_camera_forward();

        let state = &mut self.state;
        let settings = &self.settings;

        let bindings = &settings.bindings;
        let camera_forward = state.camera_forward;
        let camera_speed = settings.camera_speed;
        let sensitivity = settings.sensitivity;

        // Camera Position
        if input.key_down(bindings.forward) {
            state.camera_position += camera_forward * camera_speed * dt; // Forward
        } else if input.key_down(bindings.backward) {
            state.camera_position -= camera_forward * camera_speed * dt; // Backward
        }

        if input.key_down(bindings.left) {
            state.camera_position -= camera_forward.cross(Vec3::Y) * camera_speed * dt; // Left
        } else if input.key_down(bindings.right) {
            state.camera_position += camera_forward.cross(Vec3::Y) * camera_speed * dt; // Right
        }

        // Camera Angle
        if input.pointer.secondary_down() {
            let mouse_delta = input.pointer.delta();
            state.camera_rotation.y += mouse_delta.x * sensitivity * dt; // Yaw / Horizontal
            state.camera_rotation.x -= mouse_delta.y * sensitivity * dt; // Pitch / Vertical
        }
    }

    // UI COMPONENTS

    fn ui_settings(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Import OBJ
            if ui.button("Import OBJ").clicked() {
                self.pick_obj_async();
            }

            // Reset Scene
            if ui.button("Reset Scene").clicked() {
                self.reset_scene()
            }

            let settings = &mut self.settings;

            // Rendering Settings
            ui.checkbox(&mut settings.gpu_computing, "GPU Computing");
            ui.checkbox(&mut settings.perspective, "Perspective");
            ui.add(
                egui::DragValue::new(&mut settings.fov)
                    .prefix("FOV: ")
                    .speed(0.1)
                    .range(10.0..=170.0),
            );
            ui.checkbox(&mut settings.display_vertices, "Render Vertices");

            // Controls
            if ui.checkbox(&mut settings.azerty, "AZERTY").clicked() {
                if settings.azerty {
                    settings.bindings = Bindings::azerty();
                } else {
                    settings.bindings = Bindings::qwerty();
                }
            }
        });
    }

    fn current_model_transform(&mut self, ui: &mut Ui) {
        let current_model = &mut self.state.models[self.state.current_model];

        ui.horizontal(|ui| {
            ui.label("Model");

            // Model Position
            ui.label("Position :");
            ui.add(
                egui::DragValue::new(&mut current_model.position.x)
                    .prefix("X: ")
                    .speed(0.01),
            );
            ui.add(
                egui::DragValue::new(&mut current_model.position.y)
                    .prefix("Y: ")
                    .speed(0.01),
            );
            ui.add(
                egui::DragValue::new(&mut current_model.position.z)
                    .prefix("Z: ")
                    .speed(0.01),
            );

            // Model Rotation
            ui.label("Rotation :");
            // let response =
            ui.add(
                egui::DragValue::new(&mut current_model.rotation.x)
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
                egui::DragValue::new(&mut current_model.rotation.y)
                    .prefix("Y: ")
                    .speed(0.05)
                    .range(-360.0..=360.0)
                    .custom_formatter(|n, _| format!("{n:.2}")),
            );
            ui.add(
                egui::DragValue::new(&mut current_model.rotation.z)
                    .prefix("Z: ")
                    .speed(0.05)
                    .range(-360.0..=360.0)
                    .custom_formatter(|n, _| format!("{n:.2}")),
            );

            // Model Scale
            ui.label("Scale :");
            ui.add(
                egui::DragValue::new(&mut current_model.scale.x)
                    .prefix("X: ")
                    .speed(0.01)
                    .range(0.0..=10.0),
            );
            ui.add(
                egui::DragValue::new(&mut current_model.scale.y)
                    .prefix("Y: ")
                    .speed(0.01)
                    .range(0.0..=10.0),
            );
            ui.add(
                egui::DragValue::new(&mut current_model.scale.z)
                    .prefix("Z: ")
                    .speed(0.01)
                    .range(0.0..=10.0),
            );
        });

        // Automatic Transformations
        ui.horizontal(|ui| {
            ui.checkbox(&mut current_model.is_translating, "Translate");
            ui.checkbox(&mut current_model.is_rotating, "Rotate");
            ui.checkbox(&mut current_model.is_scaling, "Scale");
        });
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

    // UTILS

    // Cube 3D Model
    fn cube() -> ModelData {
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
            vec![1, 5, 6, 2], // Right
            vec![0, 4, 7, 3], // Left
            vec![0, 1, 5, 4], // Top
            vec![3, 2, 6, 7], // Bottom
        ];

        return ModelData { vertices, faces };
    }

    // triangulate Quads and Pentagon
    fn triangulate_faces(model: &mut ModelData) {
        let mut faces: Vec<Vec<u16>> = vec![];

        // TODO : Edges Deduplication

        for face in &model.faces {
            let mut index = 1;

            while index + 1 < face.len() {
                let triangle_face = vec![face[0], face[index], face[index + 1]];
                faces.push(triangle_face);
                index += 1;
            }
        }

        model.faces = faces;

        // println!("{model}")
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

            // UI COMPONENTS
            self.ui_settings(ui);
            self.current_model_transform(ui);

            // LOGIC
            // Transforms
            ui.input(|input| {
                self.camera_controls(input, dt);
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
