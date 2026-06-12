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
pub struct Vertex {
    pub position: [f32; 3],
    pub _padding: f32, // 12 + 4 = 16 bytes
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
    near_plane: f32,
    // LOGIC : Transformations
    bindings: Bindings,
    azerty: bool,
    // TODO : Objects List -> Manage Multiple Objects + Draw Origin
    // -> Use face_vertex_index + offset = vertex count of every previous models
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
        // Communication Channel for Async File Loading
        let (tx, rx) = channel::<Vec<u8>>();

        let cube = Self::cube();

        // cube2 = Triangulated
        // let mut cube2 = Self::cube();
        // Self::triangulate_faces(&mut cube2);

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
            near_plane: 0.1,
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
                if let Some(p) = point
                    && self.is_in_screen(*p)
                {
                    self.render_vertex(&painter, *p);
                }
            }
        }

        // Render Edges
        for face in &self.model_data.faces {
            for i in 0..face.len() {
                let point1 = screen_points[face[i] as usize];
                let point2 = screen_points[face[(i + 1) % face.len()] as usize];

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
    }

    // Base Model -> Model Matrix (Model & Transformations) * View/Camera * Projection -> 2D Frustum (Projection) -> Screen Space
    // return points on screen
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
        let projection;

        if self.perspective {
            let fov_y_radians = 2.0
                * f32::atan(
                    f32::tan(self.fov.to_radians() / 2.0) / self.three_d_viewport.aspect_ratio(),
                ); // self.fov.to_radians()

            projection = glam::Mat4::perspective_rh(
                fov_y_radians,
                self.three_d_viewport.aspect_ratio(),
                self.near_plane, // Near clip
                1000.0,          // Far clip
            );
        } else {
            let half_width = self.three_d_viewport.width() * 0.001;
            let half_height = self.three_d_viewport.height() * 0.001;

            projection = glam::Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                self.near_plane,
                1000.0,
            );
        }

        // 4) Apply Matrices : Model -> View -> Projection
        // TODO : Calc vp once / frame -> per model m * vp
        // let vp = view * model;

        let mvp: glam::Mat4 = projection * view * model;

        // 5) Projection : GPU Computing + CPU Fallback

        // GPU
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

        // CPU
        let screen_points = self
            .model_data
            .vertices
            .iter()
            .map(|vertex| {
                let vertex_vec4 = glam::vec4(
                    vertex.position[0],
                    vertex.position[1],
                    vertex.position[2],
                    1.0,
                );

                let clip_space_v = mvp * vertex_vec4;

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

    // UTILS

    fn is_in_screen(&self, screen_point: Vec2) -> bool {
        let is_in_width = 0.0 <= screen_point.x && screen_point.x <= self.three_d_viewport.width();
        let is_in_height =
            0.0 <= screen_point.y && screen_point.y <= self.three_d_viewport.height();

        return is_in_width && is_in_height;
    }

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

    fn reset(&mut self) {
        self.camera_position = Vec3::new(0.0, 0.0, -1.0);
        self.camera_rotation = Vec3::new(0.0, 180.0, 0.0);

        self.model_position = Vec3::new(0.0, 0.0, 0.0);
        self.model_rotation = Vec3::new(0.0, 0.0, 0.0);
        self.model_scale = Vec3::new(1.0, 1.0, 1.0);

        self.set_model(Self::cube());
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

            // UI

            // Settings : Import OBJ, Reset Scene, Perspective, Render Vertices, Bindings
            ui.horizontal(|ui| {
                // Import OBJ
                if ui.button("Import OBJ").clicked() {
                    self.pick_obj_async();
                }

                // Reset Scene
                if ui.button("Reset Scene").clicked() {
                    self.reset()
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
