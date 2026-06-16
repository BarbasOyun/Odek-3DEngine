use std::fmt;
use glam::Vec3;

// DATA STRUCTS
pub struct Bindings {
    pub forward: egui::Key,
    pub left: egui::Key,
    pub backward: egui::Key,
    pub right: egui::Key,
}

impl Bindings {
    pub fn qwerty() -> Self {
        Self {
            forward: egui::Key::W,
            left: egui::Key::A,
            backward: egui::Key::S,
            right: egui::Key::D,
        }
    }

    pub fn azerty() -> Self {
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
    pub model_index: u32,
    // pub _padding: f32, // 12 + 4 = 16 bytes
}

impl Vertex {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            model_index: 0,
            // _padding: 0.0,
        }
    }
}

pub struct ModelData {
    // TODO : array
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Vec<u16>>,
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

impl ModelData {
    pub fn new(vertices: Vec<Vertex>, faces: Vec<Vec<u16>>,) -> Self {
        return Self {
            vertices,
            faces,
        }
    }
}

pub struct Model {
    // pub id: usize, // using id / vertex instead
    // Model
    pub vertex_count: u32,
    pub face_count: u32,
    // Transform
    pub position: Vec3,
    pub rotation: Vec3, // Degrees
    pub scale: Vec3,
    // Transformations
    pub is_translating: bool,
    pub is_rotating: bool,
    pub is_scaling: bool,
    pub translate_osciallator: f32,
    pub scale_osciallator: f32,
}

impl Model {
    pub fn new(
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

    pub fn default(vertex_count: u32, face_count: u32) -> Self {
        return Self {
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