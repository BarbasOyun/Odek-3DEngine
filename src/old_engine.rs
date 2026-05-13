use crate::Bindings;

use eframe::egui::*;
use glam::Vec3;

// Import file
use rfd::{FileDialog, FileHandle};
use std::path::PathBuf;
use tobj::LoadOptions;

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
    vertices: Vec<glam::Vec3>,
    faces: Vec<Vec<u16>>, // TODO : Triangulate + Flatten
}

impl ThreeDEngine {
    fn new() -> Self {
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
            vertices: Vec::new(),
            faces: Vec::new(),
        }
    }

    // OLD ENGINE

    // Old Load OBJ File : synchronous
    fn pick_obj_file() -> Option<PathBuf> {
        let file = FileDialog::new()
            .add_filter("Object Files", &["obj"]) // Filter for .obj files
            .set_directory("/") // Starting directory
            .pick_file();

        return file;
    }

    fn load_obj_custom(&mut self, path: &str) {
        // Load the file
        let (models, _) = tobj::load_obj(
            path,
            &LoadOptions {
                triangulate: true, // Converts quads to triangles automatically
                single_index: true,
                ..Default::default()
            },
        )
        .expect("Failed to load OBJ file");

        let mesh = &models[0].mesh;
        self.load_mesh(mesh);
    }

    // Engine 1
    fn old_frame_image(
        &self,
        rect: &egui::Rect,
        projection_function: &dyn Fn(&Self, &Vec3) -> Vec2,
    ) -> Vec<Option<egui::Vec2>> {
        let rotation_matrix_x = glam::Mat3::from_rotation_x(self.model_rotation.x.to_radians());
        let rotation_matrix_y = glam::Mat3::from_rotation_y(self.model_rotation.y.to_radians());
        let rotation_matrix_z = glam::Mat3::from_rotation_z(self.model_rotation.z.to_radians());
        let scale_matrix = glam::Mat3::from_diagonal(self.model_scale);

        return self
            .vertices
            .iter()
            .map(|v| {
                // 1] Model + Transformations -> World Space
                let mut world_v =
                    scale_matrix * rotation_matrix_z * rotation_matrix_y * rotation_matrix_x * *v;
                world_v += self.model_position;

                // 2] World Space -> View Space (Camera)
                // View Position
                world_v = self.relative_vertex(&world_v);

                // View Rotation = Camera Rotation inverse
                let cam_quat = glam::Quat::from_euler(
                    glam::EulerRot::YXZ,
                    self.camera_rotation.y.to_radians(),
                    self.camera_rotation.x.to_radians(),
                    self.camera_rotation.z.to_radians(),
                );

                let view_quat = cam_quat.inverse();
                let view_matrix = glam::Mat3::from_quat(view_quat);

                world_v = view_matrix * world_v;

                // 3] Projection
                return (world_v.z - self.camera_position.z > 0.1).then(|| {
                    Self::proj_to_screen(
                        &projection_function(&self, &world_v),
                        rect.width(),
                        rect.height(),
                    )
                });
            })
            .collect();
    }

    fn relative_vertex(&self, vertex: &Vec3) -> Vec3 {
        return Vec3::new(
            vertex.x - self.camera_position.x,
            vertex.y - self.camera_position.y,
            vertex.z - self.camera_position.z,
        );
    }

    fn calc_fov(&self) -> f32 {
        let fov_rad = self.fov.to_radians();
        return 1.0 / (fov_rad * 0.5).tan();
    }

    fn old_perspective_project(&self, vertex: &Vec3) -> Vec2 {
        // let aspect_ratio = 1.0;
        let f = self.calc_fov();

        return Vec2::new(
            vertex.x * f / vertex.z,  // / aspect_ratio
            -vertex.y * f / vertex.z, // - = Flip Y -> 0, 0 = Top Left in Screen Space
        );
    }

    fn old_orthographic_project(&self, vertex: &Vec3) -> Vec2 {
        let f = self.calc_fov();
        return Vec2::new(vertex.x * f, -vertex.y * f);
    }

    // Engine 0 : Tsoding Video
    fn old_engine(
        &mut self,
        dt: f32,
        rect: &egui::Rect,
        painter: &egui::Painter,
        projection_function: &dyn Fn(&Self, &Vec3) -> Vec2,
    ) {
        let angle = std::f32::consts::PI * dt; // 180 degrees per second
        let sin_angle = angle.sin();
        let cos_angle = angle.cos();

        // Render Vertices
        for vertex in &mut self.vertices {
            if self.rotate {
                // Maybe : StateMachine for automatic transformations
                // Self::rotate_y(vertex, angle); // Rotate
                Self::rotate_y_computed(vertex, sin_angle, cos_angle); // Rotate
            }

            if self.display_vertices {
                let vertex_world_pos = self.model_position + *vertex;

                if vertex_world_pos.z <= 0.0 {
                    continue; // Skip vertices behind the camera
                }

                let vertex_pos = Self::project_simple(&vertex_world_pos);
                let vertex_rect = Rect::from_center_size(
                    rect.left_top()
                        + Self::proj_to_screen(&vertex_pos, rect.width(), rect.height()),
                    vec2(10.0, 10.0),
                );
                painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
            }
        }

        for face in &self.faces {
            for i in 0..face.len() {
                let v1_world_pos = self.model_position + self.vertices[face[i] as usize];
                let v2_world_pos =
                    self.model_position + self.vertices[face[(i + 1) % face.len()] as usize];

                if v1_world_pos.z <= 0.0 || v2_world_pos.z <= 0.0 {
                    continue; // Skip vertices behind the camera
                }

                let p1 = Self::proj_to_screen(
                    &projection_function(&self, &v1_world_pos),
                    rect.width(),
                    rect.height(),
                );
                let p2 = Self::proj_to_screen(
                    &projection_function(&self, &v2_world_pos),
                    rect.width(),
                    rect.height(),
                );

                painter.line_segment([rect.left_top() + p1, rect.left_top() + p2], self.stroke);
            }
        }
    }

    fn project_simple(vertex: &Vec3) -> Vec2 {
        return Vec2::new(vertex.x / vertex.z, vertex.y / vertex.z);
    }

    // Transformations
    // Rotations -> angle = radians
    fn rotate_y(vertex: &mut Vec3, angle: f32) {
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();
        let x = vertex.x * cos_angle - vertex.z * sin_angle;
        let z = vertex.x * sin_angle + vertex.z * cos_angle;
        vertex.x = x;
        vertex.z = z;
    }

    fn rotate_y_computed(vertex: &mut Vec3, sin_angle: f32, cos_angle: f32) {
        let x = vertex.x * cos_angle - vertex.z * sin_angle;
        let z = vertex.x * sin_angle + vertex.z * cos_angle;
        vertex.x = x;
        vertex.z = z;
    }
}

impl eframe::App for ThreeDEngine {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ui.request_repaint();
            ui.request_repaint_after(std::time::Duration::from_millis(16)); // 60 FPS
            let dt = ui.input(|i| i.stable_dt); // DeltaTime in second
            let fps = 1.0 / dt;

            // USER INTERFACE

            // Settings : Import OBJ, Reset Scene, Perspective, Render Vertices, Bindings
            ui.horizontal(|ui| {
                // Import OBJ
                if ui.button("Import OBJ").clicked() {
                    let file = Self::pick_obj_file();

                    if let Some(path) = file {
                        self.load_obj_custom(path.to_str().unwrap());
                    }
                }

                // Reset Scene
                if ui.button("Reset Scene").clicked() {
                    *self = Self::new();
                    // self.cube();
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