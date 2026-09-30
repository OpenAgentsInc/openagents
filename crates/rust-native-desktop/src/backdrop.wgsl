// Lays the views over the backdrop: the backdrop, softened with a small
// blur and covered by the window's background at `dim`, then the views'
// premultiplied pixel over it. Values stay sRGB-encoded, as the software
// painter blends them (`composite_pixel` in backdrop.rs).

struct Look {
    background: vec4<f32>,
    texel: vec2<f32>,
    dim: f32,
    blur: f32,
};

@group(0) @binding(0) var backdrop: texture_2d<f32>;
@group(0) @binding(1) var filtered: sampler;
@group(0) @binding(2) var views: texture_2d<f32>;
@group(0) @binding(3) var<uniform> look: Look;

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Varying {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Varying;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    // A 3x3 tent blur, `blur` texels apart, over the bilinear upscale.
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let w = (2.0 - f32(abs(x))) * (2.0 - f32(abs(y)));
            let offset = vec2<f32>(f32(x), f32(y)) * look.texel * look.blur;
            sum += w * textureSampleLevel(backdrop, filtered, in.uv + offset, 0.0).rgb;
            weight += w;
        }
    }
    let base = mix(sum / weight, look.background.rgb, look.dim);
    let view = textureLoad(views, vec2<i32>(in.position.xy), 0);
    return vec4<f32>(view.rgb + base * (1.0 - view.a), 1.0);
}
