// Verse presentation on OpenGL ES: copies a finished frame from an sRGB
// texture, which decodes it to linear light, into a linear window surface,
// applying the sRGB transfer function here. Some EGL drivers, including the
// Android emulator's, ignore an sRGB window colorspace, so the renderer does
// not rely on it.

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Out {
    let q = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var o: Out;
    o.clip = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(q.x, 1.0 - q.y);
    return o;
}

// The sRGB transfer function (IEC 61966-2-1).
fn encode(linear: vec3<f32>) -> vec3<f32> {
    let c = clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0));
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(frame, nearest, i.uv, 0.0);
    return vec4<f32>(encode(c.rgb), 1.0);
}
