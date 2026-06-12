// DATA STRUCTS

// struct VertexInput {
//     @location(0) position: vec3<f32>,
// };

// struct VertexOutput {
//     @builtin(position) clip_position: vec4<f32>,
// };

struct UniformData {
    mvp: mat4x4<f32>,
    vertex_count: u32,
}

// struct VertexInput {
//     position: vec3<f32>,
// }

// BINDINGS

// @group(0) @binding(0)
// var<uniform> mvp: mat4x4<f32>;

@group(0) @binding(0)
var<uniform> uniforms: UniformData;

@group(0) @binding(1)
var<storage, read> input_vertices: array<vec4<f32>>;
// @group(0) @binding(1) 
// var<storage, read> input_vertices: array<VertexInput>;

@group(0) @binding(2)
var<storage, read_write> output_vertices: array<vec4<f32>>;

// LOGIC

// @vertex
// @compute @workgroup_size(64, 1, 1)
// fn vs_main(model: VertexInput) -> VertexOutput {
//     var out: VertexOutput;
//     out.clip_position = mvp * vec4<f32>(model.position, 1.0);
//     return out;
// }

// @fragment
// fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
//     return vec4<f32>(0.0, 1.0, 0.0, 1.0);
// }

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= uniforms.vertex_count) {
        return;
    }

    // let position = input_vertices[index].position;
    // let processed_pos = uniforms.mvp * vec4<f32>(position, 1.0);

    // output_vertices[index] = vec4<f32>(processed_pos.xyz, 12.34);
    // output_vertices[index] = processed_pos;

    let raw_pos = input_vertices[index].xyz;
    let clip_space = uniforms.mvp * vec4<f32>(raw_pos, 1.0);

    output_vertices[index] = clip_space;
}