struct Vertex {
    position: vec3<f32>,
    padding: f32,
}

@group(0) @binding(0)
var<uniform> mvp: mat4x4<f32>;

@group(0) @binding(1)
var<storage, read> input_vertices: array<Vertex>;

@group(0) @binding(2)
var<storage, read_write> output_vertices: array<vec3<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    let raw_vertex = input_vertices[index];
    
    let projected = mvp * vec4<f32>(raw_vertex.position, 1.0);
    
    if (projected.w != 0.0) {
        output_vertices[index] = projected.xyz / projected.w;
    } else {
        output_vertices[index] = projected.xyz;
    }
}