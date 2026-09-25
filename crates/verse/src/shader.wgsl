// Verse: one shader for amber lines and near-black faces.
//
// Color arrives in linear light. Fog fades a vertex toward the near-black
// field with ground distance from the eye; vertices with fog weight zero
// (the horizon ridge) ignore it.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: eye position. w: distance where fog starts.
    eye: vec4<f32>,
    // rgb: the field color. a: distance where fog is total.
    fog: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) fog: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) fog: f32,
};

@vertex
fn vs(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(in.pos, 1.0);
    out.color = in.color;
    out.world = in.pos;
    out.fog = in.fog;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let d = distance(in.world.xz, g.eye.xz);
    let t = clamp((d - g.eye.w) / (g.fog.a - g.eye.w), 0.0, 1.0);
    let f = t * t * in.fog;
    return vec4<f32>(mix(in.color, g.fog.rgb, f), 1.0);
}
