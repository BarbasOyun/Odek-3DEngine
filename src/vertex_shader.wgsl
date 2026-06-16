// DATA STRUCTS

struct UniformData {
    vertex_count: u32, // Global vertices
}

struct VertexInput {
    position: vec3<f32>,
    model_index: u32,
}

// BINDINGS

@group(0) @binding(0)
var<uniform> uniforms: UniformData;

@group(0) @binding(1)
var<storage, read> mvps: array<mat4x4<f32>>; // MVP matrix of each model

@group(0) @binding(2)
var<storage, read> input_vertices: array<VertexInput>;

@group(0) @binding(3)
var<storage, read_write> output_vertices: array<vec4<f32>>;

// LOGIC

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= uniforms.vertex_count) {
        return;
    }

    let vertex = input_vertices[index];
    let model_index = vertex.model_index;
    let mvp = mvps[model_index];

    let raw_pos = vertex.position;
    let clip_space = mvp * vec4<f32>(raw_pos, 1.0);

    output_vertices[index] = clip_space;
}