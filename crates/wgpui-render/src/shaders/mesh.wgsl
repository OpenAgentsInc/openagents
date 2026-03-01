struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let ndc = vec2<f32>(
        (input.position.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (input.position.y / uniforms.viewport.y) * 2.0,
    );
    // Keep Z in a small range so layered 2D rendering remains predictable.
    out.position = vec4<f32>(ndc, input.position.z * 0.0001, 1.0);
    out.normal = normalize(input.normal);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.35, -0.45, 0.82));
    let ambient = 0.18;
    let diffuse = max(dot(input.normal, light_dir), 0.0);
    let lit = ambient + diffuse * 0.82;
    let rgb = input.color.rgb * lit;
    return vec4<f32>(rgb, input.color.a);
}
