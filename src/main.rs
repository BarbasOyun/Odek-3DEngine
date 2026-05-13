#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
// mod old_engine;

use std::vec;

use eframe::egui::*;
use glam::Vec3;

// Import File
use rfd::{AsyncFileDialog};
use std::sync::mpsc::{Receiver, Sender, channel};

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> eframe::Result {
    let mut three_d_engine = ThreeDEngine::new();

    three_d_engine.cube();

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "3D Engine",
        options,
        Box::new(|_cc| Ok(Box::new(three_d_engine))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    let mut three_d_engine = ThreeDEngine::new();
    three_d_engine.cube();

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

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
                Box::new(|_cc| Ok(Box::new(three_d_engine))),
            )
            .await;

        // Remove the loading text and spinner:
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

struct ThreeDEngine {
    // RENDERING
    // TODO : Store Radians instead of Degrees (Performance)
    smoothed_fps: f32,
    camera_position: Vec3,
    camera_rotation: Vec3, // Degrees : Yaw, Pitch, Roll
    camera_speed: f32,
    sensitivity: f32,
    camera_forward: Vec3,
    fov: f32, // Field of View (Degrees)
    stroke: Stroke,
    perspective: bool,
    display_vertices: bool,
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
    // MODEL DATA
    // TODO : Separate Data / Engine
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    vertices: Vec<glam::Vec3>,
    faces: Vec<Vec<u16>>, // TODO : Triangulate + Flatten
}

impl ThreeDEngine {
    fn new() -> Self {
        let (tx, rx) = channel::<Vec<u8>>();

        Self {
            // CAMERA
            smoothed_fps: 60.0,
            camera_position: Vec3::new(0.0, 0.0, -1.0),
            camera_rotation: Vec3::new(0.0, 180.0, 0.0),
            camera_speed: 2.0,
            sensitivity: 5.0,
            camera_forward: Vec3::new(0.0, 0.0, 1.0),
            fov: 90.0,
            // RENDERING
            stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(190, 110, 40)),
            perspective: true,
            display_vertices: true,
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
            // MODEL DATA
            tx,
            rx,
            vertices: Vec::new(),
            faces: Vec::new(),
        }
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

    // Wireframe Rendering -> New Engine : Generate Frame Image
    fn render_frame(&self, rect: &egui::Rect, painter: &egui::Painter) {
        let screen_points: Vec<Option<egui::Vec2>> = self.frame_image(&rect);

        // Render Vertices
        if self.display_vertices {
            for point in &screen_points {
                self.render_vertex(&rect, &painter, *point);
            }
        }

        // Render Edges
        for face in &self.faces {
            for i in 0..face.len() {
                self.render_edge(
                    &rect,
                    &painter,
                    screen_points[face[i] as usize],
                    screen_points[face[(i + 1) % face.len()] as usize],
                );
            }
        }
    }

    // Base Model -> Model Matrix (Model & Transformations) + View/Camera + Projection -> 2D Frustum (Projection) -> Screen Space
    fn frame_image(
        &self,
        rect: &egui::Rect,
    ) -> Vec<Option<egui::Vec2>> {
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

        return self
            .vertices
            .iter()
            .map(|v| {
                let world_v: Vec3 = mvp.project_point3(*v); // World Vertex

                // 5) Projection
                let is_in_fov =
                    world_v.x.abs() <= 1.0 && world_v.y.abs() <= 1.0 && world_v.z.abs() <= 1.0;

                let fulcrum_point: Vec2;

                if self.perspective {
                    fulcrum_point = self.perspective_project(&world_v);
                } else {
                    fulcrum_point = self.orthographic_project(&world_v);
                }

                return (is_in_fov).then(|| {
                    Self::proj_to_screen(
                        &fulcrum_point,
                        rect.width(),
                        rect.height(),
                    )
                });
            })
            .collect();
    }

    // World -> 2D Frustum (Perspective)
    fn perspective_project(&self, vertex: &Vec3) -> Vec2 {
        return Vec2::new(vertex.x / vertex.z, vertex.y / vertex.z);
    }

    // World -> 2D Frustum (Orthographic)
    fn orthographic_project(&self, vertex: &Vec3) -> Vec2 {
        return Vec2::new(vertex.x, vertex.y);
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

    fn render_vertex(&self, rect: &egui::Rect, painter: &egui::Painter, point: Option<Vec2>) {
        if let Some(point) = point {
            let vertex_rect = Rect::from_center_size(rect.left_top() + point, vec2(10.0, 10.0));
            painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
        }
    }

    fn render_edge(
        &self,
        rect: &egui::Rect,
        painter: &egui::Painter,
        p1: Option<Vec2>,
        p2: Option<Vec2>,
    ) {
        if let (Some(p1), Some(p2)) = (p1, p2) {
            painter.line_segment([rect.left_top() + p1, rect.left_top() + p2], self.stroke);
        }
    }

    // UTILS

    fn cube(&mut self) {
        let vertices = vec![
            // Front Face
            Vec3::new(0.25, 0.25, 0.25),
            Vec3::new(-0.25, 0.25, 0.25),
            Vec3::new(-0.25, -0.25, 0.25),
            Vec3::new(0.25, -0.25, 0.25),
            // Back Face
            Vec3::new(0.25, 0.25, -0.25),
            Vec3::new(-0.25, 0.25, -0.25),
            Vec3::new(-0.25, -0.25, -0.25),
            Vec3::new(0.25, -0.25, -0.25),
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

        // Engine Setup
        self.vertices = vertices;
        self.faces = faces;
    }

    fn hud(&mut self, rect: &egui::Rect, painter: &egui::Painter, fps: f32) {
        // Displayed on top of the 3D View
        // FPS Display
        let alpha = 0.05;
        self.smoothed_fps = (self.smoothed_fps * (1.0 - alpha)) + (fps * alpha);

        painter.text(
            rect.left_top() + egui::vec2(10.0, 10.0), // 10px padding from top-left
            egui::Align2::LEFT_TOP,
            format!("FPS: {:.2}", self.smoothed_fps),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );

        // Controls Display
        painter.text(
            rect.left_top() + egui::vec2(10.0, 30.0),
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
        let vertices: Vec<Vec3> = mesh
            .positions
            .chunks_exact(3)
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .collect();

        // 2. Convert flat indices [0,1,2, 3,4,5] to Vec<Vec<u8>>
        let faces: Vec<Vec<u16>> = mesh
            .indices
            .chunks_exact(3)
            .map(|f| f.iter().map(|&i| i as u16).collect())
            .collect();

        self.vertices = vertices;
        self.faces = faces;
    }
}

impl eframe::App for ThreeDEngine {
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
                    *self = Self::new();
                    self.cube();
                }

                // Rendering Settings
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
                        .range(-360.0..=360.0),
                );

                // if response.changed() {
                //     println!("Rotation X is now: {}", self.model_rotation.x);
                //     // Change to radians
                // }

                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.y)
                        .prefix("Y: ")
                        .speed(0.05)
                        .range(-360.0..=360.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.z)
                        .prefix("Z: ")
                        .speed(0.05)
                        .range(-360.0..=360.0),
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

            // Border
            painter.rect_stroke(
                rect,
                5.0,
                egui::Stroke::new(2.0, egui::Color32::GREEN),
                egui::StrokeKind::Middle,
            );

            self.render_frame(&rect, &painter);
            self.hud(&rect, &painter, fps);
        });
    }
}
