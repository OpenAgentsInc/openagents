// Verse physical scene: lit surfaces, the sky at infinity, stars, glows, and
// guide lines, all in pre-exposed luminance (cd/m² × exposure).
//
// Shading follows Karis, "Real Shading in Unreal Engine 4" (2013); Walter et
// al. (2007) and Heitz (2014) for GGX with height-correlated Smith masking;
// Kulla and Conty (2017) for multiple scattering; Fernando (2005) for
// percentage-closer soft shadows; Tokuyoshi and Kaplanyan (2019) for specular
// antialiasing; Estevez and Kulla (2017) for sheen; Belcour and Barla (2017)
// for thin films; Cox and Munk (1954) for ocean glint; and Chandrasekhar's
// single-scattering solution for the Earth's Rayleigh haze.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light: mat4x4<f32>,
    // xyz eye; w exposure.
    eye: vec4<f32>,
    // xyz toward the Sun; w illuminance in lux.
    sun: vec4<f32>,
    // x angular radius; y mean disc luminance; z visible fraction; w shadow texel (m).
    sun_disc: vec4<f32>,
    // xyz direction; w angular radius.
    earth: vec4<f32>,
    earth_x: vec4<f32>,
    earth_y: vec4<f32>,
    // xyz north pole; w distance (m).
    earth_z: vec4<f32>,
    moon: vec4<f32>,
    moon_x: vec4<f32>,
    moon_y: vec4<f32>,
    // xyz north; w distance (m).
    moon_z: vec4<f32>,
    celestial_x: vec4<f32>,
    celestial_y: vec4<f32>,
    celestial_z: vec4<f32>,
    // rgb illuminance from the Earth at the station, lux; w shadow depth range (m).
    earth_light: vec4<f32>,
    // width, height, 1 / width, 1 / height.
    viewport: vec4<f32>,
    // xyz origin; w cell size.
    probe_origin: vec4<f32>,
    // probe counts; w 1 when probes are present.
    probe_dims: vec4<f32>,
    // x star gain; y time (s); z pixel angle (rad); w legacy color scale.
    params: vec4<f32>,
    // x metering target; y, z gain bounds; w 1 when exposure adapts.
    metering: vec4<f32>,
    // Neon stage: x fog start, y fog end (m), z line width (px), w mode
    // (0 space, 1 neon).
    neon: vec4<f32>,
    // rgb field color; w unused.
    field: vec4<f32>,
};

@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_compare: sampler_comparison;
@group(0) @binding(3) var probe_r: texture_3d<f32>;
@group(0) @binding(4) var probe_g: texture_3d<f32>;
@group(0) @binding(5) var probe_b: texture_3d<f32>;
@group(0) @binding(6) var linear_clamp: sampler;
@group(0) @binding(7) var earth_day: texture_2d<f32>;
@group(0) @binding(8) var earth_clouds: texture_2d<f32>;
@group(0) @binding(9) var earth_water: texture_2d<f32>;
@group(0) @binding(10) var moon_albedo: texture_2d<f32>;
@group(0) @binding(11) var milky_way: texture_2d<f32>;
@group(0) @binding(12) var linear_repeat: sampler;
// The most recent adapted scene luminance, for display-referred guides.
@group(1) @binding(0) var adapted: texture_2d<f32>;

// Guides and the amber companion are display colors, not light: undo the
// exposure adaptation that post-processing will apply to them.
fn guide_scale() -> f32 {
    if f.metering.w < 0.5 {
        return f.params.w;
    }
    let average = textureLoad(adapted, vec2<i32>(0, 0), 0).r;
    if average <= 0.0 {
        return f.params.w;
    }
    return f.params.w / clamp(f.metering.x / average, f.metering.y, f.metering.z);
}

const PI: f32 = 3.14159265;
const MAX_SIGNAL: f32 = 60000.0;

// True when the adapter cannot render a floating-point scene: each draw then
// tone-maps its own output and post-processing is skipped.
override DIRECT: bool = false;
// Development views (VERSE_PHOTO_DEBUG): 1 direct light, 2 probe diffuse,
// 3 probe specular, 4 ambient occlusion, 5 sun shadow.
override DEBUG: u32 = 0u;

// Khronos PBR Neutral, duplicated from post.wgsl for the direct path.
fn neutral(color: vec3<f32>) -> vec3<f32> {
    let start = 0.76;
    let x = min(color.r, min(color.g, color.b));
    let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
    var c = color - offset;
    let peak = max(c.r, max(c.g, c.b));
    if peak < start {
        return c;
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    c *= new_peak / peak;
    let g = 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0);
    return mix(c, vec3<f32>(new_peak), g);
}

// Hue-preserving shoulder: scales a color by its peak, so an amber line stays
// on the amber hue however bright its core.
fn hue_shoulder(color: vec3<f32>) -> vec3<f32> {
    let peak = max(color.r, max(color.g, color.b));
    let start = 0.76;
    if peak < start {
        return color;
    }
    let d = 1.0 - start;
    return color * ((1.0 - d * d / (peak + d - start)) / peak);
}

fn expose(luminance: vec3<f32>) -> vec3<f32> {
    let signal = min(luminance * f.eye.w, vec3<f32>(MAX_SIGNAL));
    if DIRECT {
        if f.neon.w > 0.5 {
            return hue_shoulder(max(signal, vec3<f32>(0.0)));
        }
        return neutral(max(signal, vec3<f32>(0.0)));
    }
    return signal;
}

// Distance fog toward the field, as the amber world's own shader applies it.
fn neon_fog(color: vec3<f32>, world: vec3<f32>, weight: f32) -> vec3<f32> {
    if f.neon.w < 0.5 {
        return color;
    }
    let d = distance(world.xz, f.eye.xz);
    let t = clamp((d - f.neon.x) / max(f.neon.y - f.neon.x, 1e-3), 0.0, 1.0);
    return mix(color, f.field.rgb, t * t * weight);
}


fn hash3(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

// Values that must interpolate linearly in screen space, not with perspective
// correction. GLSL ES has no `noperspective` qualifier, so its variant sends
// the value times the vertex's clip w with perspective correction and
// multiplies by the fragment's interpolated 1 / w, which cancels the division
// (Heckbert and Moreton 1991).
//#if GLES
fn linear_out(v: vec2<f32>, w: f32) -> vec2<f32> {
    return v * w;
}
fn linear_in(v: vec2<f32>, position: vec4<f32>) -> vec2<f32> {
    return v * position.w;
}
fn linear_out1(v: f32, w: f32) -> f32 {
    return v * w;
}
fn linear_in1(v: f32, position: vec4<f32>) -> f32 {
    return v * position.w;
}
//#else
fn linear_out(v: vec2<f32>, w: f32) -> vec2<f32> {
    return v;
}
fn linear_in(v: vec2<f32>, position: vec4<f32>) -> vec2<f32> {
    return v;
}
fn linear_out1(v: f32, w: f32) -> f32 {
    return v;
}
fn linear_in1(v: f32, position: vec4<f32>) -> f32 {
    return v;
}
//#endif

fn noise_ign(pixel: vec2<f32>) -> f32 {
    // Interleaved gradient noise (Jimenez 2014).
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

// ---------------------------------------------------------------------------
// Shadow depth pass.

struct LitIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) local: vec3<f32>,
    @location(4) color: vec3<f32>,
    @location(5) params: vec4<f32>,
};

@vertex
fn vs_shadow(v: LitIn) -> @builtin(position) vec4<f32> {
    return f.light * vec4<f32>(v.pos, 1.0);
}

// ---------------------------------------------------------------------------
// Lit surfaces.

struct LitOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) local: vec3<f32>,
    @location(4) color: vec3<f32>,
    @location(5) params: vec4<f32>,
};

@vertex
fn vs_lit(v: LitIn) -> LitOut {
    var o: LitOut;
    o.clip = f.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.tangent = v.tangent;
    o.local = v.local;
    o.color = v.color;
    o.params = v.params;
    return o;
}

fn d_ggx(noh: f32, a2: f32) -> f32 {
    let d = noh * noh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Height-correlated Smith visibility, V = G / (4 NoL NoV).
fn v_smith(nov: f32, nol: f32, a2: f32) -> f32 {
    let gv = nol * sqrt(nov * nov * (1.0 - a2) + a2);
    let gl = nov * sqrt(nol * nol * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 1e-6);
}

fn f_schlick(f0: vec3<f32>, voh: f32) -> vec3<f32> {
    let k = pow(1.0 - voh, 5.0);
    return f0 + (vec3<f32>(1.0) - f0) * k;
}

fn f_schlick1(f0: f32, voh: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - voh, 5.0);
}

// Anisotropic GGX with the height-correlated visibility (Heitz 2014).
fn aniso_spec(n: vec3<f32>, t: vec3<f32>, b: vec3<f32>, v: vec3<f32>, l: vec3<f32>, h: vec3<f32>, at: f32, ab: f32) -> f32 {
    let toh = dot(t, h);
    let boh = dot(b, h);
    let noh = dot(n, h);
    let a2 = at * ab;
    let w = vec3<f32>(ab * toh, at * boh, a2 * noh);
    let w2 = a2 / max(dot(w, w), 1e-12);
    let d = a2 * w2 * w2 / PI;
    let nov = max(dot(n, v), 1e-4);
    let nol = max(dot(n, l), 1e-4);
    let lv = nol * length(vec3<f32>(at * dot(t, v), ab * dot(b, v), nov));
    let ll = nov * length(vec3<f32>(at * dot(t, l), ab * dot(b, l), nol));
    return d * 0.5 / max(lv + ll, 1e-6);
}

// Analytic fit of the split-sum environment BRDF for mobile (Karis 2014,
// "Physically Based Shading on Mobile"): scale and bias applied to F0.
fn env_brdf(nov: f32, roughness: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * nov)) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

// Charlie sheen distribution with Neubelt's visibility.
fn sheen(noh: f32, nov: f32, nol: f32, roughness: f32) -> f32 {
    let inv = 1.0 / max(roughness, 0.07);
    let sin2 = max(1.0 - noh * noh, 1e-4);
    let d = (2.0 + inv) * pow(sin2, inv * 0.5) / (2.0 * PI);
    let vis = 1.0 / (4.0 * (nol + nov - nol * nov) + 1e-4);
    return d * vis;
}

// Reflectance of a single thin film of index `n1` and thickness `d` (nm) on a
// substrate of index `n2`, at red, green, and blue wavelengths, with both
// polarizations averaged (two-beam Airy summation).
fn thin_film(cos_i: f32, n1: f32, n2: f32, d: f32) -> vec3<f32> {
    let sin_i2 = 1.0 - cos_i * cos_i;
    let cos_t = sqrt(max(1.0 - sin_i2 / (n1 * n1), 0.0));
    let cos_s = sqrt(max(1.0 - sin_i2 / (n2 * n2), 0.0));
    // Fresnel amplitudes at air-film and film-substrate interfaces.
    let r01s = (cos_i - n1 * cos_t) / (cos_i + n1 * cos_t);
    let r01p = (n1 * cos_i - cos_t) / (n1 * cos_i + cos_t);
    let r12s = (n1 * cos_t - n2 * cos_s) / (n1 * cos_t + n2 * cos_s);
    let r12p = (n2 * cos_t - n1 * cos_s) / (n2 * cos_t + n1 * cos_s);
    let lambda = vec3<f32>(650.0, 550.0, 450.0);
    let phase = 4.0 * PI * n1 * d * cos_t / lambda;
    let c = cos(phase);
    let rs = (r01s * r01s + r12s * r12s + 2.0 * r01s * r12s * c)
        / (1.0 + r01s * r01s * r12s * r12s + 2.0 * r01s * r12s * c);
    let rp = (r01p * r01p + r12p * r12p + 2.0 * r01p * r12p * c)
        / (1.0 + r01p * r01p * r12p * r12p + 2.0 * r01p * r12p * c);
    return 0.5 * (rs + rp);
}

// Crinkled foil: two octaves of smooth value noise tilt the normal, so the
// blanket shows soft wrinkles that catch the Sun in broken streaks.
fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let u = fract(p);
    let s = u * u * (3.0 - 2.0 * u);
    let a = hash3(i).x;
    let b = hash3(i + vec3<f32>(1.0, 0.0, 0.0)).x;
    let c = hash3(i + vec3<f32>(0.0, 1.0, 0.0)).x;
    let d = hash3(i + vec3<f32>(1.0, 1.0, 0.0)).x;
    let e = hash3(i + vec3<f32>(0.0, 0.0, 1.0)).x;
    let g = hash3(i + vec3<f32>(1.0, 0.0, 1.0)).x;
    let h = hash3(i + vec3<f32>(0.0, 1.0, 1.0)).x;
    let k = hash3(i + vec3<f32>(1.0, 1.0, 1.0)).x;
    return mix(
        mix(mix(a, b, s.x), mix(c, d, s.x), s.y),
        mix(mix(e, g, s.x), mix(h, k, s.x), s.y),
        s.z
    );
}

fn wrinkle(p: vec3<f32>) -> f32 {
    return value_noise(p) + 0.5 * value_noise(p * 2.7 + 17.0);
}

fn crinkle(p: vec3<f32>, cell: f32) -> vec2<f32> {
    let q = p / cell;
    let e = 0.2;
    let base = wrinkle(q);
    // Finite-difference slope along two object axes.
    let dx = wrinkle(q + vec3<f32>(e, 0.0, 0.0)) - base;
    let dy = wrinkle(q + vec3<f32>(0.0, e, 0.0)) - base;
    let dz = wrinkle(q + vec3<f32>(0.0, 0.0, e)) - base;
    return vec2<f32>(dx - dz * 0.5, dy + dz * 0.5) / e * 0.22;
}

fn probe_irradiance(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if f.probe_dims.w < 0.5 {
        return vec3<f32>(0.0);
    }
    // Offset along the normal so thin walls do not sample the far side.
    let q = (p + n * f.probe_origin.w * 0.5 - f.probe_origin.xyz) / f.probe_origin.w;
    let uvw = (q + 0.5) / f.probe_dims.xyz;
    let r = textureSampleLevel(probe_r, linear_clamp, uvw, 0.0);
    let g = textureSampleLevel(probe_g, linear_clamp, uvw, 0.0);
    let b = textureSampleLevel(probe_b, linear_clamp, uvw, 0.0);
    return max(vec3<f32>(r.x + dot(r.yzw, n), g.x + dot(g.yzw, n), b.x + dot(b.yzw, n)), vec3<f32>(0.0));
}

const POISSON: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216), vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870), vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432), vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845), vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554), vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023), vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507), vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367), vec2<f32>(0.14383161, -0.14100790)
);

//#if GLES
// GLSL ES cannot read the values of a depth texture that is also sampled with
// comparison, so this variant has no blocker search: every penumbra assumes
// an occluder 1 m from the receiver. Returns the filter radius in texels.
fn penumbra(uv: vec2<f32>, depth: f32, size: vec2<f32>, rot: mat2x2<f32>, tan_r: f32, texel: f32) -> f32 {
    return clamp(tan_r / texel, 0.8, 12.0);
}
//#else
// The blocker search of a percentage-closer soft shadow: the filter radius in
// texels, or 0 when nothing occludes the point.
fn penumbra(uv: vec2<f32>, depth: f32, size: vec2<f32>, rot: mat2x2<f32>, tan_r: f32, texel: f32) -> f32 {
    let depth_range = f.earth_light.w;
    // Search as far as a 60 m occluder distance could blur, in texels.
    let search = clamp(60.0 * tan_r / texel, 1.5, 12.0);
    var blockers = 0.0;
    var sum = 0.0;
    for (var i = 0; i < 16; i++) {
        let o = rot * POISSON[i] * search / size;
        let t = vec2<i32>(clamp((uv + o) * size, vec2<f32>(0.0), size - 1.0));
        let d = textureLoad(shadow_map, t, 0);
        if d < depth - 0.0005 {
            blockers += 1.0;
            sum += d;
        }
    }
    if blockers < 0.5 {
        return 0.0;
    }
    let gap = max(depth - sum / blockers, 0.0) * depth_range;
    // Penumbra width is the occluder gap times the disc's full angle.
    return max(gap * tan_r / texel, 0.8);
}
//#endif

// Percentage-closer soft shadow whose penumbra follows the Sun's disc.
fn sun_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    let texel = f.sun_disc.w;
    let p = world + n * texel * 1.5;
    let c = f.light * vec4<f32>(p, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || c.z > 1.0 {
        return 1.0;
    }
    let size = vec2<f32>(textureDimensions(shadow_map));
    let tan_r = tan(f.sun_disc.x);
    let angle = noise_ign(pixel) * 2.0 * PI;
    let rot = mat2x2<f32>(cos(angle), sin(angle), -sin(angle), cos(angle));
    let radius = penumbra(uv, c.z, size, rot, tan_r, texel);
    if radius <= 0.0 {
        return 1.0;
    }
    var lit = 0.0;
    for (var i = 0; i < 16; i++) {
        let o = rot * POISSON[i] * radius / size;
        lit += textureSampleCompareLevel(shadow_map, shadow_compare, uv + o, c.z - 0.0003);
    }
    return lit / 16.0;
}

struct Lobe {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
};

// One fragment of a lit surface: the lit vertex's channels and the pixel it
// covers. Lit triangles and textured meshes shade through the same function.
struct Shading {
    world: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
    local: vec3<f32>,
    color: vec3<f32>,
    params: vec4<f32>,
    pixel: vec2<f32>,
};

@fragment
fn fs_lit(i: LitOut) -> @location(0) vec4<f32> {
    return vec4<f32>(shade(Shading(i.world, i.normal, i.tangent, i.local, i.color, i.params, i.clip.xy)), 1.0);
}

// The exposed, fogged color of one lit fragment.
fn shade(i: Shading) -> vec3<f32> {
    // Surfaces are two-sided: thin panels show whichever face the eye sees.
    let facing = select(-1.0, 1.0, dot(i.normal, f.eye.xyz - i.world) >= 0.0);
    let geometric = normalize(i.normal) * facing;
    var n = geometric;
    var t = normalize(i.tangent - n * dot(i.tangent, n));
    let b = cross(n, t);
    let code = i32(round(i.params.z));
    var metallic = clamp(i.params.x, 0.0, 1.0);
    var roughness = clamp(i.params.y, 0.03, 1.0);
    let ao = clamp(i.params.w, 0.0, 1.0);
    var base = i.color;

    // Aluminized Kapton: tilted facets a few centimeters across.
    if code == 2 {
        // Facets smaller than about two pixels would sparkle; fold their
        // tilt into roughness instead (LEAN mapping, Olano and Baker 2010).
        let cell = 0.09;
        let footprint = length(fwidth(i.local));
        let resolved = clamp(cell / max(footprint, 1e-5) * 0.5 - 0.5, 0.0, 1.0);
        let tilt = crinkle(i.local, cell);
        n = normalize(n + (t * tilt.x + b * tilt.y) * resolved);
        t = normalize(t - n * dot(t, n));
        roughness = sqrt(roughness * roughness + (1.0 - resolved) * 0.13);
    }
    let bb = cross(n, t);
    let v = normalize(f.eye.xyz - i.world);
    let nov = max(dot(n, v), 1e-4);

    // Specular antialiasing from screen-space normal variance.
    let dn = fwidth(n);
    let variance = min(2.0 * dot(dn, dn) * 0.25, 0.18);
    var a = roughness * roughness;
    let a2 = min(a * a + variance, 1.0);
    a = sqrt(a2);

    // A solar cell's glint is the cover glass, drawn as the coat below.
    // Beneath the glass, textured silicon under its antireflection coating
    // returns about 1% of the light, scattered widely, so a broad glossy
    // lobe there would wash the blue cells out into a gray sheen.
    let dielectric = select(0.04, 0.01, code == 3);
    let f0 = mix(vec3<f32>(dielectric), base, metallic);
    let diffuse_color = base * (1.0 - metallic);
    let env = env_brdf(nov, roughness);
    let e_spec = f0 * env.x + env.y;
    // Multiple-scattering compensation for rough metal (Kulla and Conty 2017).
    let energy = vec3<f32>(1.0) + f0 * (1.0 / max(env.x + env.y, 0.05) - 1.0);

    // Coat parameters by material.
    var coat_f0 = 0.0;
    var coat_rough = 0.05;
    var coat_tint = vec3<f32>(1.0);
    if code == 2 {
        coat_f0 = 0.05;
        coat_rough = 0.12;
        coat_tint = vec3<f32>(0.95, 0.70, 0.30);
    } else if code == 3 || code == 4 || code == 6 {
        coat_f0 = 0.04;
        coat_rough = 0.03;
    }

    var radiance = vec3<f32>(0.0);
    var direct_part = vec3<f32>(0.0);
    var shadow_seen = 1.0;
    let pixel = i.pixel;

    // Direct light from the Sun and the Earth, each a small disc.
    for (var k = 0; k < 2; k++) {
        var l: vec3<f32>;
        var e: vec3<f32>;
        var angular: f32;
        if k == 0 {
            l = f.sun.xyz;
            e = vec3<f32>(f.sun.w);
            angular = f.sun_disc.x;
        } else {
            l = f.earth.xyz;
            e = f.earth_light.xyz;
            angular = f.earth.w;
        }
        let nol = dot(n, l);
        if nol <= 0.0 {
            continue;
        }
        var shadow = 1.0;
        if k == 0 {
            shadow = sun_shadow(i.world, geometric, pixel);
            if shadow <= 0.0 {
                continue;
            }
        }
        let h = normalize(l + v);
        let noh = max(dot(n, h), 0.0);
        let voh = max(dot(v, h), 0.0);
        // Widen the lobe by the source's angular radius (Karis 2013).
        let as_ = min(a + angular * 0.5, 1.0);
        let as2 = as_ * as_;
        var spec: vec3<f32>;
        if code == 1 {
            let at = min(max(as_ * 1.5, 0.002), 1.0);
            let ab = min(max(as_ / 1.5, 0.002), 1.0);
            spec = aniso_spec(n, t, bb, v, l, h, at, ab) * f_schlick(f0, voh);
        } else {
            spec = d_ggx(noh, as2) * v_smith(nov, nol, as2) * f_schlick(f0, voh);
        }
        spec *= energy;
        var diffuse = diffuse_color / PI;
        if code == 5 {
            // Fabric: a sheen lobe and a slightly flattened diffuse.
            spec += vec3<f32>(0.3) * sheen(noh, nov, nol, 0.5);
        }
        var layer = diffuse + spec;
        if coat_f0 > 0.0 {
            let fc_v = f_schlick1(coat_f0, nov);
            let fc_l = f_schlick1(coat_f0, nol);
            let tint = pow(coat_tint, vec3<f32>(1.0 / nov + 1.0 / nol) * 0.5);
            var cf = vec3<f32>(f_schlick1(coat_f0, voh));
            if code == 3 {
                // Magnesium fluoride on cover glass.
                cf = thin_film(voh, 1.38, 1.52, 110.0);
            }
            let ac = min(coat_rough * coat_rough + angular * 0.5, 1.0);
            let coat = d_ggx(noh, ac * ac) * v_smith(nov, nol, ac * ac) * cf;
            layer = layer * (1.0 - fc_v) * (1.0 - fc_l) * tint + coat;
        }
        radiance += layer * e * nol * shadow;
        if k == 0 {
            shadow_seen = shadow;
        }
    }
    direct_part = radiance;

    // Bounce light from nearby surfaces through the probe grid.
    let irr = probe_irradiance(i.world, n) * ao;
    radiance += diffuse_color / PI * irr;
    // Glossy bounce: the probes' radiance toward the reflection direction.
    let r = reflect(-v, n);
    let so = clamp(pow(nov + ao, exp2(-16.0 * roughness - 1.0)) - 1.0 + ao, 0.0, 1.0);
    let lr = probe_irradiance(i.world, r) / PI;
    radiance += lr * e_spec * so;

    if DEBUG == 1u {
        radiance = direct_part;
    } else if DEBUG == 2u {
        radiance = diffuse_color / PI * irr;
    } else if DEBUG == 3u {
        radiance = lr * e_spec * so;
    } else if DEBUG == 4u {
        return vec3<f32>(ao);
    } else if DEBUG == 5u {
        return vec3<f32>(shadow_seen);
    } else if DEBUG == 6u {
        let hs = normalize(f.sun.xyz + v);
        return vec3<f32>(nov, max(dot(n, f.sun.xyz), 0.0), max(dot(n, hs), 0.0));
    }
    var shaded = expose(radiance);
    // A stage floor fades by its occlusion channel into the field behind it.
    if code == 7 {
        shaded = mix(f.field.rgb, shaded, ao);
    }
    // On a neon stage, lit geometry fades into the field like the lines.
    return neon_fog(shaded, i.world, 1.0);
}

// ---------------------------------------------------------------------------
// Textured static meshes: the base color is an image times the material's
// factor, and shading is the lit surfaces' own. Opaque, alpha-masked, and
// blended materials each have a fragment entry, so only masked draws discard
// and opaque draws keep early depth rejection on tiled phone GPUs.

struct TexturedMaterial {
    // Linear base color factor; alpha multiplies the image's alpha.
    base: vec4<f32>,
    // x metallic; y perceptual roughness; z alpha cutoff; w unused.
    params: vec4<f32>,
};

// Group 1 stays the guides' adapted luminance, so textured draws share the
// pass's bindings with the legacy faces.
@group(2) @binding(0) var base_color: texture_2d<f32>;
@group(2) @binding(1) var base_sampler: sampler;
@group(2) @binding(2) var<uniform> material: TexturedMaterial;

struct TexturedIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // Linear vertex color, glTF's COLOR_0.
    @location(3) color: vec4<f32>,
};

struct TexturedOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_textured(v: TexturedIn) -> TexturedOut {
    var o: TexturedOut;
    o.clip = f.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.uv = v.uv;
    o.color = v.color;
    return o;
}

fn textured_base(uv: vec2<f32>, color: vec4<f32>) -> vec4<f32> {
    return textureSample(base_color, base_sampler, uv) * material.base * color;
}

// Shades a textured fragment as a generic metallic-roughness surface
// (material code 0) without baked occlusion.
fn textured_shade(world: vec3<f32>, normal: vec3<f32>, pixel: vec2<f32>, base: vec3<f32>) -> vec3<f32> {
    let n = normalize(normal);
    // Code 0 has no anisotropy; any tangent across the normal will do.
    let across = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.x) > 0.9);
    let params = vec4<f32>(material.params.x, material.params.y, 0.0, 1.0);
    return shade(Shading(world, n, cross(n, across), world, base, params, pixel));
}

@fragment
fn fs_textured(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb), 1.0);
}

// glTF's MASK mode: a fragment is fully opaque when its alpha reaches the
// cutoff and absent otherwise.
@fragment
fn fs_textured_masked(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    if base.a < material.params.z {
        discard;
    }
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb), 1.0);
}

// glTF's BLEND mode, premultiplied, for glass and other thin transparency.
@fragment
fn fs_textured_blend(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    let alpha = clamp(base.a, 0.0, 1.0);
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb) * alpha, alpha);
}

struct TexturedShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_shadow_textured(v: TexturedIn) -> TexturedShadowOut {
    var o: TexturedShadowOut;
    o.clip = f.light * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.alpha = v.color.a;
    return o;
}

// Masked surfaces cast the shadow of their visible texels only.
@fragment
fn fs_shadow_masked(i: TexturedShadowOut) {
    let alpha = textureSample(base_color, base_sampler, i.uv).a * material.base.a * i.alpha;
    if alpha < material.params.z {
        discard;
    }
}

// ---------------------------------------------------------------------------
// Sky at infinity.

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> SkyOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.ndc = p;
    return o;
}

fn view_ray(ndc: vec2<f32>) -> vec3<f32> {
    // Reversed depth: 1 is the near plane.
    let near = f.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let far = f.inv_view_proj * vec4<f32>(ndc, 0.5, 1.0);
    return normalize(far.xyz / far.w - near.xyz / near.w);
}

fn celestial_of(d: vec3<f32>) -> vec3<f32> {
    // The celestial columns map equatorial to scene; transpose maps back.
    return vec3<f32>(dot(d, f.celestial_x.xyz), dot(d, f.celestial_y.xyz), dot(d, f.celestial_z.xyz));
}

@fragment
fn fs_background(i: SkyOut) -> @location(0) vec4<f32> {
    let d = view_ray(i.ndc);
    let e = celestial_of(d);
    let ra = atan2(e.y, e.x);
    let dec = asin(clamp(e.z, -1.0, 1.0));
    // Equirectangular in right ascension, 0h at the center, increasing left.
    let uv = vec2<f32>(0.5 - ra / (2.0 * PI), 0.5 - dec / PI);
    let lod = log2(max(f.params.z * f32(textureDimensions(milky_way).x) / (2.0 * PI), 1.0));
    let c = textureSampleLevel(milky_way, linear_repeat, uv, lod).rgb;
    // The brightest Milky Way regions are about 5 × 10⁻⁴ cd/m².
    return vec4<f32>(expose(c * 6.0e-4 * f.params.x), 1.0);
}

struct StarIn {
    @location(0) dir: vec3<f32>,
    @location(1) illuminance: f32,
    @location(2) color: vec3<f32>,
};

const QUAD: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0)
);

struct StarOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) corner: vec2<f32>,
};

const STAR_SIGMA: f32 = 0.7;

@vertex
fn vs_star(s: StarIn, @builtin(vertex_index) index: u32) -> StarOut {
    let corner = QUAD[index];
    let d = f.celestial_x.xyz * s.dir.x + f.celestial_y.xyz * s.dir.y + f.celestial_z.xyz * s.dir.z;
    var c = f.view_proj * vec4<f32>(d, 0.0);
    let extent = 3.0 * STAR_SIGMA;
    c.x += corner.x * extent * 2.0 * f.viewport.z * c.w;
    c.y += corner.y * extent * 2.0 * f.viewport.w * c.w;
    c.z = 0.0;
    var o: StarOut;
    o.clip = c;
    // Spread the star's illuminance over a Gaussian footprint of pixels.
    let footprint = 2.0 * PI * STAR_SIGMA * STAR_SIGMA * f.params.z * f.params.z;
    o.color = s.color * s.illuminance / footprint * f.params.x;
    o.corner = corner * extent;
    return o;
}

@fragment
fn fs_star(i: StarOut) -> @location(0) vec4<f32> {
    let g = exp(-dot(i.corner, i.corner) / (2.0 * STAR_SIGMA * STAR_SIGMA));
    return vec4<f32>(expose(i.color * g), 0.0);
}

struct BodyOut {
    @builtin(position) clip: vec4<f32>,
//#if GLES
    @location(0) ndc: vec2<f32>,
//#else
    @location(0) @interpolate(linear) ndc: vec2<f32>,
//#endif
    @location(1) @interpolate(flat) kind: u32,
};

fn body_dir(kind: u32) -> vec4<f32> {
    if kind == 0u {
        return vec4<f32>(f.sun.xyz, f.sun_disc.x);
    } else if kind == 1u {
        return f.earth;
    }
    return f.moon;
}

fn tangent_basis(d: vec3<f32>) -> mat3x3<f32> {
    var helper = vec3<f32>(0.0, 1.0, 0.0);
    if abs(d.y) > 0.9 {
        helper = vec3<f32>(1.0, 0.0, 0.0);
    }
    let u = normalize(cross(d, helper));
    let v = cross(d, u);
    return mat3x3<f32>(u, v, d);
}

@vertex
fn vs_body(@builtin(vertex_index) index: u32, @builtin(instance_index) kind: u32) -> BodyOut {
    let body = body_dir(kind);
    let m = tangent_basis(body.xyz);
    // Two pixels of margin for the antialiased limb.
    let r = tan(body.w) + 3.0 * f.params.z;
    let k = QUAD[index];
    let d = body.xyz + (m[0] * k.x + m[1] * k.y) * r;
    var c = f.view_proj * vec4<f32>(d, 0.0);
    c.z = 0.0;
    var o: BodyOut;
    o.clip = c;
    o.ndc = linear_out(c.xy / c.w, c.w);
    o.kind = kind;
    return o;
}

// Limb darkening by a quadratic law per channel, from the Neckel and Labs
// center-to-limb ratios at 650, 550, and 450 nm.
fn limb(mu: f32) -> vec3<f32> {
    let a = vec3<f32>(0.52, 0.64, 0.81);
    let b = vec3<f32>(0.08, 0.07, 0.08);
    let x = 1.0 - mu;
    let i = vec3<f32>(1.0) - a * x - b * x * x;
    let mean = vec3<f32>(1.0) - a / 3.0 - b / 6.0;
    return i / mean;
}

fn disc_overlap(d: f32, r1: f32, r2: f32) -> f32 {
    // Area of intersection of two discs of radii r1 and r2 at distance d.
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= abs(r1 - r2) {
        let r = min(r1, r2);
        return PI * r * r;
    }
    let a = r1 * r1 * acos(clamp((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1), -1.0, 1.0));
    let b = r2 * r2 * acos(clamp((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2), -1.0, 1.0));
    let c = 0.5 * sqrt(max((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2), 0.0));
    return a + b - c;
}

fn earth_radiance(n: vec3<f32>, to_eye: vec3<f32>, lod: f32) -> vec3<f32> {
    let s = f.sun.xyz;
    let local = vec3<f32>(dot(n, f.earth_x.xyz), dot(n, f.earth_y.xyz), dot(n, f.earth_z.xyz));
    let lon = atan2(local.y, local.x);
    let lat = asin(clamp(local.z, -1.0, 1.0));
    let uv = vec2<f32>(0.5 + lon / (2.0 * PI), 0.5 - lat / PI);
    let ground = textureSampleLevel(earth_day, linear_repeat, uv, lod).rgb;
    let cloud = textureSampleLevel(earth_clouds, linear_repeat, uv, lod).r;
    let water = textureSampleLevel(earth_water, linear_repeat, uv, max(lod - 1.0, 0.0)).r;
    let mu0 = dot(n, s);
    let mu = max(dot(n, to_eye), 0.02);
    if mu0 <= 0.0 {
        return vec3<f32>(0.0);
    }
    let e = f.sun.w;
    // Blue Marble colors are already visible reflectance; scale to albedo.
    let surface = ground * 0.85;
    let albedo = mix(surface, vec3<f32>(0.75), cloud * 0.95);
    let mu0c = max(mu0, 0.02);
    // Rayleigh optical depth at 650, 550, and 450 nm, trimmed slightly for
    // the ocean's own absorption so disc color matches DSCOVR EPIC frames.
    let tau = vec3<f32>(0.045, 0.09, 0.17);
    let air = 1.0 / mu0c + 1.0 / mu;
    let trans = exp(-tau * air);
    var l = albedo / PI * e * mu0 * trans;
    // Ocean glint through clear sky (Cox and Munk, 5 m/s wind).
    let h = normalize(s + to_eye);
    let cosb = max(dot(n, h), 1e-3);
    let tan2 = (1.0 - cosb * cosb) / (cosb * cosb);
    let sigma2 = 0.0286;
    let p = exp(-tan2 / sigma2) / (PI * sigma2 * pow(cosb, 4.0));
    let fres = f_schlick1(0.02, max(dot(to_eye, h), 0.0));
    l += vec3<f32>(e * fres * p / (4.0 * mu)) * water * (1.0 - cloud) * trans;
    // Single-scattered Rayleigh haze (Chandrasekhar), backscatter at L1.
    let cos_theta = dot(-s, -to_eye);
    let phase = 0.75 * (1.0 + cos_theta * cos_theta);
    l += e * mu0c / (mu0c + mu) * phase / (4.0 * PI) * (vec3<f32>(1.0) - exp(-tau * air));
    // The Moon's shadow during a solar eclipse.
    let earth_center = f.earth.xyz * f.earth_z.w;
    let point = earth_center + n * 6.371e6;
    let to_moon = f.moon.xyz * f.moon_z.w - point;
    let moon_r = asin(clamp(1.7374e6 / length(to_moon), 0.0, 1.0));
    let sep = acos(clamp(dot(normalize(to_moon), s), -1.0, 1.0));
    let sun_r = f.sun_disc.x;
    let covered = disc_overlap(sep, sun_r, moon_r) / (PI * sun_r * sun_r);
    return l * (1.0 - clamp(covered, 0.0, 1.0));
}

fn moon_radiance(n: vec3<f32>, to_eye: vec3<f32>, lod: f32) -> vec3<f32> {
    let s = f.sun.xyz;
    let local = vec3<f32>(dot(n, f.moon_x.xyz), dot(n, f.moon_y.xyz), dot(n, f.moon_z.xyz));
    let lon = atan2(local.y, local.x);
    let lat = asin(clamp(local.z, -1.0, 1.0));
    let uv = vec2<f32>(0.5 + lon / (2.0 * PI), 0.5 - lat / PI);
    let a = textureSampleLevel(moon_albedo, linear_repeat, uv, lod).r;
    let mu0 = dot(n, s);
    let mu = max(dot(n, to_eye), 1e-3);
    if mu0 <= 0.0 {
        return vec3<f32>(0.0);
    }
    // Lunar-Lambert reflectance (McEwen 1991) with an opposition surge
    // (Hapke 1981): the Moon is flatter and brighter near full than a Lambert
    // sphere.
    let alpha = acos(clamp(dot(s, to_eye), -1.0, 1.0));
    let lw = clamp(1.0 - 0.019 * degrees(alpha) / 1.0 * 0.5, 0.0, 1.0);
    let lunar = 2.0 * lw * mu0 / (mu0 + mu) + (1.0 - lw) * mu0;
    let surge = 1.0 + 0.8 / (1.0 + tan(alpha * 0.5) / 0.06);
    // Normal albedo: map texture 0–1 around a 0.12 mean with a warm tint.
    let albedo = a * 0.26 * vec3<f32>(1.02, 1.0, 0.95);
    return albedo / PI * f.sun.w * lunar * surge / 1.8;
}

@fragment
fn fs_body(i: BodyOut) -> @location(0) vec4<f32> {
    let body = body_dir(i.kind);
    let ray = view_ray(linear_in(i.ndc, i.clip));
    let m = tangent_basis(body.xyz);
    let r = tan(body.w);
    let p = vec2<f32>(dot(ray, m[0]), dot(ray, m[1])) / (dot(ray, m[2]) * r);
    let rho = length(p);
    // Antialiased limb: distance to the edge in pixels.
    let edge = (1.0 - rho) * r / f.params.z;
    let coverage = clamp(edge + 0.5, 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    let z = sqrt(max(1.0 - min(rho, 1.0) * min(rho, 1.0), 0.0));
    var color: vec3<f32>;
    if i.kind == 0u {
        color = f.sun_disc.y * limb(z);
    } else {
        let n = normalize(m[0] * p.x + m[1] * p.y - m[2] * z);
        let to_eye = -body.xyz;
        // Texture footprint: radians of longitude per pixel near the center.
        let per_pixel = f.params.z / max(r, 1e-6);
        // Foreshortening toward the limb widens each pixel's footprint.
        let slant = 1.0 / max(z, 0.08);
        if i.kind == 1u {
            let lod = log2(max(per_pixel * slant / (2.0 * PI) * f32(textureDimensions(earth_day).x), 1.0));
            color = earth_radiance(n, to_eye, lod);
        } else {
            let lod = log2(max(per_pixel * slant / (2.0 * PI) * f32(textureDimensions(moon_albedo).x), 1.0));
            color = moon_radiance(n, to_eye, lod);
        }
    }
    return vec4<f32>(expose(color) * coverage, coverage);
}

// Diffraction spikes of a six-blade aperture around the Sun, added after the
// scene when the Sun is in view (Ritschel et al. 2009 describe the physics).
@vertex
fn vs_flare(@builtin(vertex_index) index: u32) -> BodyOut {
    let center = f.view_proj * vec4<f32>(f.sun.xyz, 0.0);
    var o: BodyOut;
    let k = QUAD[index];
    // A fixed screen-space size: 22% of the frame height.
    let size = 0.22 * 2.0;
    o.clip = vec4<f32>(center.xy / center.w + k * vec2<f32>(size * f.viewport.y * f.viewport.z, size), 0.0, 1.0);
    if center.w <= 0.0 {
        o.clip = vec4<f32>(2.0, 2.0, 0.0, 1.0);
    }
    o.ndc = linear_out(k, o.clip.w);
    o.kind = 0u;
    return o;
}

@fragment
fn fs_flare(i: BodyOut) -> @location(0) vec4<f32> {
    let p = linear_in(i.ndc, i.clip);
    let r = length(p);
    var spikes = 0.0;
    for (var k = 0; k < 3; k++) {
        let a = f32(k) * PI / 3.0 + 0.26;
        let dir = vec2<f32>(cos(a), sin(a));
        let across = abs(dot(p, vec2<f32>(-dir.y, dir.x)));
        spikes += exp(-across * 180.0) * pow(max(1.0 - r, 0.0), 2.0);
    }
    let halo = 0.02 / (1.0 + r * r * 400.0);
    // Scaled from the exposed disc so the spikes follow exposure.
    let peak = min(f.sun_disc.y * f.eye.w, MAX_SIGNAL) * 1e-3 * f.sun_disc.z;
    let tint = vec3<f32>(1.0, 0.97, 0.92);
    return vec4<f32>(tint * peak * (spikes + halo), 0.0);
}

// ---------------------------------------------------------------------------
// Emissive glows and legacy geometry.

struct GlowIn {
    @location(0) pos: vec3<f32>,
    @location(1) radiance: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct GlowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) radiance: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_glow(g: GlowIn) -> GlowOut {
    var o: GlowOut;
    o.clip = f.view_proj * vec4<f32>(g.pos, 1.0);
    o.radiance = g.radiance;
    o.uv = g.uv;
    return o;
}

@fragment
fn fs_glow(i: GlowOut) -> @location(0) vec4<f32> {
    let k = max(1.0 - dot(i.uv, i.uv), 0.0);
    return vec4<f32>(expose(i.radiance * k * k), 0.0);
}

struct LegacyIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) fog: f32,
};

struct LegacyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) fog: f32,
};

@vertex
fn vs_legacy(v: LegacyIn) -> LegacyOut {
    var o: LegacyOut;
    o.clip = f.view_proj * vec4<f32>(v.pos, 1.0);
    // Faces keep the field color; only lines carry the emission gain.
    o.color = v.color;
    o.world = v.pos;
    o.fog = v.fog;
    return o;
}

@fragment
fn fs_legacy(i: LegacyOut) -> @location(0) vec4<f32> {
    var c = i.color;
    if f.neon.w < 0.5 {
        c = c * guide_scale();
    }
    return vec4<f32>(neon_fog(c, i.world, i.fog), 1.0);
}


// Guide lines as antialiased screen-space quads (Chan and Durand 2005).
struct WideIn {
    @location(0) a: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) b: vec3<f32>,
    @location(3) fog: f32,
};

struct WideOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // Pixels from the line's center: a screen-space quantity, so it must not
    // be perspective-corrected across a quad whose ends differ greatly in depth.
//#if GLES
    @location(1) across: f32,
//#else
    @location(1) @interpolate(linear) across: f32,
//#endif
    @location(2) world: vec3<f32>,
    @location(3) fog: f32,
    @location(4) width: f32,
};

@vertex
fn vs_wide(w: WideIn, @builtin(vertex_index) index: u32) -> WideOut {
    let ends = array<f32, 6>(0.0, 1.0, 1.0, 0.0, 1.0, 0.0);
    let sides = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
    var ca = f.view_proj * vec4<f32>(w.a, 1.0);
    var cb = f.view_proj * vec4<f32>(w.b, 1.0);
    // Clip the segment to the near plane so both ends project.
    // The camera's near plane is 0.1 m; clip there, not at w = 0, so a clipped
    // end projects to a finite point.
    let near = 0.1;
    if ca.w < near && cb.w < near {
        var o: WideOut;
        o.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    // Clip to the near plane, moving the world endpoints with the clip ones
    // so fog is evaluated where the visible fragment really is.
    var wa = w.a;
    var wb = w.b;
    if ca.w < near {
        let t = (near - ca.w) / (cb.w - ca.w);
        ca = mix(ca, cb, t);
        wa = mix(w.a, w.b, t);
    } else if cb.w < near {
        let t = (near - cb.w) / (ca.w - cb.w);
        cb = mix(cb, ca, t);
        wb = mix(w.b, w.a, t);
    }
    let sa = ca.xy / ca.w * f.viewport.xy * 0.5;
    let sb = cb.xy / cb.w * f.viewport.xy * 0.5;
    var dir = sb - sa;
    if dot(dir, dir) < 1e-8 {
        dir = vec2<f32>(1.0, 0.0);
    }
    let normal = normalize(vec2<f32>(-dir.y, dir.x));
    let end = ends[index];
    let side = sides[index];
    var c = mix(ca, cb, end);
    let width = max(f.neon.z, 1.0);
    let extent = width * 0.5 + 1.0;
    c.x += normal.x * side * extent * 2.0 * f.viewport.z * c.w;
    c.y += normal.y * side * extent * 2.0 * f.viewport.w * c.w;
    var o: WideOut;
    o.clip = c;
    o.color = w.color * guide_scale();
    o.across = linear_out1(side * extent, c.w);
    o.world = mix(wa, wb, end);
    o.fog = w.fog;
    o.width = width;
    return o;
}

@fragment
fn fs_wide(i: WideOut) -> @location(0) vec4<f32> {
    let across = linear_in1(i.across, i.clip);
    let coverage = clamp(i.width * 0.5 + 0.5 - abs(across), 0.0, 1.0);
    var c = i.color;
    if f.neon.w > 0.5 {
        c = expose(neon_fog(c, i.world, i.fog));
    }
    return vec4<f32>(c * coverage, coverage);
}
