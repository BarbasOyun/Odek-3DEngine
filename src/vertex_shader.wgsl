// DATA STRUCTS

struct UniformData {
    mvp: mat4x4<f32>,
    vertex_count: u32,
}

// BINDINGS

@group(0) @binding(0)
var<uniform> uniforms: UniformData;

@group(0) @binding(1)
var<storage, read> input_vertices: array<vec4<f32>>;

@group(0) @binding(2)
var<storage, read_write> output_vertices: array<vec4<f32>>;

// LOGIC

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= uniforms.vertex_count) {
        return;
    }

    let raw_pos = input_vertices[index].xyz;
    let clip_space = uniforms.mvp * vec4<f32>(raw_pos, 1.0);

    output_vertices[index] = clip_space;
}