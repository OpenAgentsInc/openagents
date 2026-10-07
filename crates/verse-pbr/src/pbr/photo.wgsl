// VERSE_SHARED_SHADING
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
    // The cascade a shadow pass draws: world to map clip space.
    light: mat4x4<f32>,
    // xyz eye; w exposure.
    eye: vec4<f32>,
    // xyz toward the Sun; w illuminance in lux.
    sun: vec4<f32>,
    // x angular radius; y mean disc luminance; z visible fraction; w unused.
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
    // rgb illuminance from the Earth at the station, lux; w unused.
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
    // rgb field color; w the daylight sky's dusk glow from 0 to 1.
    field: vec4<f32>,
    // Neon stage daylight sky: rgb zenith, w 1 when the sky is drawn.
    sky_zenith: vec4<f32>,
    // rgb horizon haze; w cloud cover from 0 to 1.
    sky_horizon: vec4<f32>,
    // rgb Sun tint; w the disc's angular radius (rad).
    sky_sun: vec4<f32>,
    // Daylight sky light: x 1 when lit surfaces take it; y the reflection
    // cube's last level, which roughness 1 reads; z lightning's flash on
    // the sky, 0 to 1.
    sky_light: vec4<f32>,
    // The sky light's irradiance as order-two spherical harmonics (rgb), in
    // `sky_irradiance`'s order with each band's cosine weight folded in.
    sky_sh: array<vec4<f32>, 9>,
    // Height fog: density (1/m), base height (m), falloff (1/m), and start
    // distance (m).
    fog_shape: vec4<f32>,
    // x opacity cap; y Sun lobe strength; z its exponent; w 1 when present.
    fog_lobe: vec4<f32>,
    // The sun's shadow cascades (`verse_engine::lighting::Cascades`), near to
    // far, one layer of `shadow_map` each: world to map clip space.
    cascades: array<mat4x4<f32>, 4>,
    // Per cascade: the texel edge (m).
    cascade_texel: vec4<f32>,
    // Per cascade: the depth range its map spans along the light (m).
    cascade_depth: vec4<f32>,
    // Per cascade: the view depth where it hands over to the next (m).
    cascade_end: vec4<f32>,
    // x cascade count; y blend band as a fraction of each slice; z view depth
    // where the shadow starts to fade; w view depth where it ends (m).
    cascade_params: vec4<f32>,
    // xyz the camera's view axis, along which view depth is measured.
    view_forward: vec4<f32>,
    // x lamp count; y the scale from emitted luminance to the shaded signal.
    lamp_params: vec4<f32>,
    // Per lamp (`pbr::Lamp`): position and range (m), then pre-exposed color
    // times candela.
    lamps: array<vec4<f32>, 64>,
    // rgb a neon stage's key light color; w 1 when set, white otherwise.
    key_tint: vec4<f32>,
    fire_control: vec4<f32>,
    // What this tier's water draws (`verse_pbr::water::control`): analytic
    // detail waves (0 for the baked tile), the glints' fade distance (m),
    // foam noise octaves, and 1.
    water_control: vec4<f32>,
    // The sea (`verse_pbr::water`, body 0): x its level (m), y 1 when the stage has it, z 1
    // when the eye is under it, w the caustics' strength.
    water: vec4<f32>,
    // rgb the water's extinction (1/m).
    water_extinction: vec4<f32>,
    // rgb the water's in-scatter; w the water clock (s).
    water_scatter: vec4<f32>,
    // Spells on the water (`water::Water::control_terms`): Part Water's
    // trench center and direction; its half length, half width, and how
    // far it is open, with Redirect Flow's radius; Redirect Flow's center
    // and velocity; a whirlpool's center, radius, and strength; rain's
    // wetness and ice, each a center, a radius, and an amount.
    water_part: vec4<f32>,
    water_part_size: vec4<f32>,
    water_flow: vec4<f32>,
    water_whirl: vec4<f32>,
    water_wet: vec4<f32>,
    water_ice: vec4<f32>,
    // Medium and High (`water::screen::Plan::uniform`): x 1 when the scene
    // copies hold this frame, y the screen-space reflection's steps (0 for
    // none), z refraction's dispersion, w the particles' soft fade depth
    // (m, 0 for none).
    water_screen: vec4<f32>,
    // The planar mirror: x 1 when drawn this frame, y its plane's level
    // (m), z the body it mirrors, w unused.
    water_mirror: vec4<f32>,
};

@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var shadow_map: texture_depth_2d_array;
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
// The daylight sky prefiltered by roughness, one GGX lobe per level.
@group(0) @binding(13) var sky_cube: texture_cube<f32>;
// The most recent adapted scene luminance, for display-referred guides.
@group(1) @binding(0) var adapted: texture_2d<f32>;
// The high tier's screen-space terms at full resolution (`pbr::screen`): red
// ambient occlusion, green the sun's contact shadow. One white texel on the
// other tiers, which never read it.
@group(1) @binding(1) var screen_occlusion: texture_2d<f32>;

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
// The quality tier (`verse_engine::quality`). PCSS: a blocker search sets each
// penumbra; without it the penumbra is fixed, as on GLSL ES. DETAIL:
// procedural surface normals, such as crinkled foil facets.
override PCSS: bool = true;
override DETAIL: bool = true;
// SCREEN: the tier traced the screen-space terms this frame's lit surfaces
// read. Without it they are exactly one.
override SCREEN: bool = false;

// The screen-space ambient occlusion (x) and contact shadow (y) at a pixel.
fn screen_terms(pixel: vec2<f32>) -> vec2<f32> {
    if SCREEN {
        return textureLoad(screen_occlusion, vec2<i32>(pixel), 0).rg;
    }
    return vec2<f32>(1.0);
}

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
// Under a daylight sky the fog takes the sky's color along the view ray, a
// single-scattering aerial perspective: fully fogged ground matches the sky
// behind it, so the horizon has no seam. A stage with height fog uses it in
// place of the ramp, and sunlight scattered toward the eye brightens the fog
// on the Sun's side.
fn neon_fog(color: vec3<f32>, world: vec3<f32>, weight: f32) -> vec3<f32> {
    // Under water the water's own fog (`water_view`) stands in for the
    // air's.
    if f.neon.w < 0.5 || f.water.z > 0.5 {
        return color;
    }
    let d = distance(world.xz, f.eye.xz);
    let t = clamp((d - f.neon.x) / max(f.neon.y - f.neon.x, 1e-3), 0.0, 1.0);
    let ray = world - f.eye.xyz;
    let dir = ray / max(length(ray), 1e-4);
    var air = f.field.rgb;
    if f.sky_zenith.w > 0.5 {
        air = daylight_air(dir);
    }
    var amount = t * t;
    if f.fog_lobe.w > 0.5 {
        // The ramp's end stays the limit: over its last 30% the fog closes
        // in, so the world's edge stays hidden (`HeightFog::stage_opacity`).
        let end = f.neon.y;
        let edge = smoothstep(0.0, 1.0, (d - 0.7 * end) / max(0.3 * end, 1e-3));
        amount = max(height_fog(world), edge);
        air += f.sky_sun.rgb * f.fog_lobe.y * pow(max(dot(dir, f.sun.xyz), 0.0), f.fog_lobe.z);
    }
    return mix(color, air, amount * weight);
}

// Exponential height fog (`verse_engine::lighting::HeightFog`): the opacity
// between the eye and `world`. Density falls exponentially with height, so
// the optical depth along the ray from the start distance has a closed form
// (Wenzel 2007; Quilez, "Better Fog").
fn height_fog(world: vec3<f32>) -> f32 {
    let ray = world - f.eye.xyz;
    let len = max(length(ray), 1e-4);
    let travel = len - f.fog_shape.w;
    if travel <= 0.0 || f.fog_shape.x <= 0.0 {
        return 0.0;
    }
    let first = f.eye.y + ray.y * (f.fog_shape.w / len);
    let rise = ray.y * (travel / len);
    let at_start = f.fog_shape.x * exp(clamp(-f.fog_shape.z * (first - f.fog_shape.y), -80.0, 80.0));
    // The mean density relative to the start: (1 - e^-k) / k.
    let k = clamp(f.fog_shape.z * rise, -80.0, 80.0);
    var shape = 1.0 - 0.5 * k;
    if abs(k) > 1e-4 {
        shape = (1.0 - exp(-k)) / k;
    }
    return min(1.0 - exp(-at_start * travel * shape), f.fog_lobe.x);
}

// ---------------------------------------------------------------------------
// Daylight sky for a neon stage, in display-linear color. A cheap stand-in
// for a physical atmosphere: the horizon-to-zenith gradient plays the part of
// Rayleigh scattering's growing optical depth toward the horizon, and a
// forward lobe around the Sun plays Mie scattering's. No textures, no
// derivatives, and no branches on varying values, so every backend, WebGL2
// included, runs it in uniform control flow.

// The shape of the Cornette-Shanks phase function (without its 1 / 4π
// normalization): a forward lobe whose width `g` sets.
fn mie_lobe(mu: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = pow(max(1.0 + g2 - 2.0 * g * mu, 1e-4), 1.5);
    return (1.0 - g2) * (1.0 + mu * mu) / (2.0 * denom);
}

// The sky without its Sun disc or clouds: also the fog color along `d`.
fn daylight_air(d: vec3<f32>) -> vec3<f32> {
    let sun = f.sun.xyz;
    let up = max(d.y, 0.0);
    let glow = f.field.w;
    // Optical depth grows toward the horizon; the haze band is narrow, and
    // wider in the long light of dusk.
    let haze = pow(1.0 - up, mix(5.0, 3.5, glow));
    var c = mix(f.sky_zenith.rgb, f.sky_horizon.rgb, haze);
    // The horizon is warmer and brighter on the Sun's side of the sky.
    let flat_d = normalize(vec3<f32>(d.x, 0.0, d.z) + vec3<f32>(1e-4, 0.0, 0.0));
    let flat_s = normalize(vec3<f32>(sun.x, 0.0, sun.z) + vec3<f32>(1e-4, 0.0, 0.0));
    let toward = dot(flat_d, flat_s) * 0.5 + 0.5;
    c = c * (1.0 + 0.12 * haze * (toward - 0.5));
    c = mix(c, c * f.sky_sun.rgb * 1.08, haze * toward * (0.35 + 0.5 * glow));
    // Below the horizon the haze stays: distant fogged ground matches it.
    let below = clamp(-d.y * 4.0, 0.0, 1.0);
    c = mix(c, f.sky_horizon.rgb * 0.94, below);
    // Forward scattering around the Sun: a wide halo and a tight glow, and
    // at dusk a broad glow and a band of fire along the Sun's horizon.
    let mu = dot(d, sun);
    let halo = mie_lobe(mu, 0.76);
    c += f.sky_sun.rgb * (0.025 * halo + 0.22 * pow(max(mu, 0.0), 48.0));
    let band = pow(toward, 3.0) * pow(1.0 - up, 8.0) * (1.0 - below);
    c += f.sky_sun.rgb * glow * (0.3 * pow(max(mu, 0.0), 6.0) + 0.08 * halo + 0.35 * band);
    return c;
}

// Smooth value noise on the plane.
fn sky_hash(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn sky_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let u = fract(p);
    let s = u * u * (3.0 - 2.0 * u);
    let a = sky_hash(i);
    let b = sky_hash(i + vec2<f32>(1.0, 0.0));
    let c = sky_hash(i + vec2<f32>(0.0, 1.0));
    let e = sky_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, s.x), mix(c, e, s.x), s.y);
}

// Four octaves, rotated between octaves to hide the lattice.
fn sky_fbm(p: vec2<f32>) -> f32 {
    let r = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
    var q = p;
    var sum = 0.0;
    var amp = 0.5;
    for (var o = 0; o < 4; o += 1) {
        sum += amp * sky_noise(q);
        q = r * q * 2.03 + vec2<f32>(17.1, 9.2);
        amp *= 0.5;
    }
    return sum / 0.9375;
}

@fragment
fn fs_daylight(i: SkyOut) -> @location(0) vec4<f32> {
    let d = view_ray(i.ndc);
    let sun = f.sun.xyz;
    var c = daylight_air(d);
    // Clouds on a plane overhead; the offset keeps the projection finite at
    // the horizon, where they thin into the haze.
    let up = max(d.y, 0.0);
    let plane = d.xz / (up + 0.12);
    let wind = vec2<f32>(0.004, 0.0015) * f.params.y;
    // Offset from the origin, where the hash's lattice is least varied.
    let p = plane * 2.4 + wind + vec2<f32>(41.3, 87.9);
    let n = sky_fbm(p);
    let cover = f.sky_horizon.w;
    // The octaves' sum has a median near 0.38 and a 95th percentile near
    // 0.62; the cover moves the threshold through that range.
    let low = mix(0.56, 0.24, cover);
    let density = smoothstep(low, low + 0.2, n) * smoothstep(0.02, 0.3, up);
    // Light from the Sun's side: a sample nudged toward the Sun is denser
    // when this point faces away from the light.
    let toward = normalize(sun.xz + vec2<f32>(1e-4, 0.0)) * 0.12;
    let lee = sky_fbm(p + toward);
    let lit = clamp(0.62 + (n - lee) * 3.0, 0.0, 1.0);
    let mu = dot(d, sun);
    let glow = f.field.w;
    // At dusk the shadowed undersides take the zenith's violet and the lit
    // edges the Sun's fire.
    let shadowed = mix(f.sky_horizon.rgb * vec3<f32>(0.80, 0.84, 0.95), f.sky_zenith.rgb * 1.3, glow * 0.6);
    let sunlit = vec3<f32>(0.97, 0.95, 0.92) * mix(vec3<f32>(1.0), f.sky_sun.rgb, 0.35 + 0.5 * glow);
    let shade = mix(shadowed, sunlit, lit);
    // Silver lining where thin cloud crosses the Sun's glow.
    let rim = f.sky_sun.rgb * pow(max(mu, 0.0), 12.0 - 8.0 * glow) * (1.0 - density) * (0.6 + 1.2 * glow);
    // Distant clouds fade into the haze, as aerial perspective would.
    let cloud = mix(shade + rim, daylight_air(d), pow(1.0 - up, 6.0) * 0.6);
    c = mix(c, cloud, density * 0.92);
    // Crepuscular rays at dusk: streaks in the Sun's glow, fanning out from
    // it, brighter through the gaps between clouds.
    if glow > 0.0 {
        let side = normalize(cross(sun, vec3<f32>(0.0, 1.0, 0.0)) + vec3<f32>(1e-4, 0.0, 0.0));
        let over = cross(side, sun);
        let fan = atan2(dot(d, over), dot(d, side));
        let streak = sky_noise(vec2<f32>(fan * 9.0, f.params.y * 0.03)) * sky_noise(vec2<f32>(fan * 23.0 + 7.0, 3.1));
        let rays = smoothstep(0.12, 0.5, streak) - 0.35;
        c += f.sky_sun.rgb * glow * rays * pow(max(mu, 0.0), 5.0) * (0.45 - 0.25 * density);
    }
    // Lightning lights the sky blue-white, the clouds most.
    let flash = f.sky_light.z;
    c += vec3<f32>(0.6, 0.68, 0.95) * flash * (0.6 + 1.2 * density);
    // The Sun's disc with a soft edge, dimmed but not hidden behind cloud.
    let r = max(f.sky_sun.w, 1e-3);
    let angle = acos(clamp(mu, -1.0, 1.0));
    let disc = 1.0 - smoothstep(r * 0.75, r * 1.15, angle);
    c += f.sky_sun.rgb * disc * 6.0 * (1.0 - 0.8 * density);
    // Under water, only Snell's window above shows the sky; below the
    // horizon the eye looks into the water's own color.
    if f.water.z > 0.5 && d.y < 0.05 {
        c = water_fog();
    }
    // Dither below one 8-bit step against banding in the gradient.
    c += (noise_ign(i.clip.xy) - 0.5) / 255.0;
    return vec4<f32>(expose(max(c, vec3<f32>(0.0))), 1.0);
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

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The daylight sky's irradiance on a surface facing `n`: the nine
// spherical-harmonic terms of `verse_engine::environment::basis`, with the
// coefficients' cosine weights already folded in.
fn sky_irradiance(n: vec3<f32>) -> vec3<f32> {
    var e = f.sky_sh[0].rgb;
    e += f.sky_sh[1].rgb * n.y + f.sky_sh[2].rgb * n.z + f.sky_sh[3].rgb * n.x;
    e += f.sky_sh[4].rgb * (n.x * n.y) + f.sky_sh[5].rgb * (n.y * n.z);
    e += f.sky_sh[6].rgb * (3.0 * n.z * n.z - 1.0) + f.sky_sh[7].rgb * (n.x * n.z);
    e += f.sky_sh[8].rgb * (n.x * n.x - n.y * n.y);
    return max(e, vec3<f32>(0.0));
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
fn penumbra(uv: vec2<f32>, layer: i32, depth: f32, size: vec2<f32>, rot: mat2x2<f32>, tan_r: f32, texel: f32, depth_range: f32) -> f32 {
    return clamp(tan_r / texel, 0.8, 12.0);
}
//#else
// The blocker search of a percentage-closer soft shadow in one cascade's map:
// the filter radius in texels, or 0 when nothing occludes the point.
fn penumbra(uv: vec2<f32>, layer: i32, depth: f32, size: vec2<f32>, rot: mat2x2<f32>, tan_r: f32, texel: f32, depth_range: f32) -> f32 {
    // Search as far as a 60 m occluder distance could blur, in texels.
    let search = clamp(60.0 * tan_r / texel, 1.5, 12.0);
    // A blocker stands at least 8 cm above the receiver.
    let gap_min = 0.08 / max(depth_range, 1.0);
    var blockers = 0.0;
    var sum = 0.0;
    for (var i = 0; i < 16; i++) {
        let o = rot * POISSON[i] * search / size;
        let t = vec2<i32>(clamp((uv + o) * size, vec2<f32>(0.0), size - 1.0));
        let d = textureLoad(shadow_map, t, layer, 0);
        if d < depth - gap_min {
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

// The sun's visibility in one cascade, from 1 (lit) to 0 (shadowed): a
// percentage-closer filter whose penumbra follows the Sun's disc. The width
// is in meters, divided by each cascade's own texel, so penumbrae match
// across cascades. Only the nearest cascade searches for blockers; the
// others keep the fixed penumbra. Comparison samples take an explicit level,
// so they are valid in the non-uniform control flow they run in.
fn cascade_shadow(layer: i32, world: vec3<f32>, n: vec3<f32>, rot: mat2x2<f32>, tan_r: f32) -> f32 {
    let texel = f.cascade_texel[layer];
    let depth_range = f.cascade_depth[layer];
    let p = world + n * texel * 1.5;
    let c = f.cascades[layer] * vec4<f32>(p, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || c.z > 1.0 {
        return 1.0;
    }
    let size = vec2<f32>(textureDimensions(shadow_map));
    // A fixed penumbra, as if every occluder stood 1 m from the receiver.
    var radius = clamp(tan_r / texel, 0.8, 12.0);
    if PCSS && layer == 0 {
        radius = penumbra(uv, layer, c.z, size, rot, tan_r, texel, depth_range);
    }
    if radius <= 0.0 {
        return 1.0;
    }
    // A 5 cm bias along the light, in the map's depth units.
    let reference = c.z - 0.05 / max(depth_range, 1.0);
    var lit = 0.0;
    for (var i = 0; i < 16; i++) {
        let o = rot * POISSON[i] * radius / size;
        lit += textureSampleCompareLevel(shadow_map, shadow_compare, uv + o, layer, reference);
    }
    return lit / 16.0;
}

// The Sun's visibility at a world point. The cascade is the first whose
// slice reaches the point's view depth; across the band at the end of each
// cascade the next one blends in, and the last fades out at the shadow
// distance. `verse_engine::lighting::Cascades::select` makes the same choice.
fn sun_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    let count = i32(f.cascade_params.x + 0.5);
    if count < 1 {
        return 1.0;
    }
    let depth = dot(world - f.eye.xyz, f.view_forward.xyz);
    if depth >= f.cascade_params.w {
        return 1.0;
    }
    var layer = 0;
    for (var k = 0; k < count - 1; k++) {
        if depth > f.cascade_end[k] {
            layer = k + 1;
        }
    }
    let tan_r = tan(f.sun_disc.x);
    let angle = noise_ign(pixel) * 2.0 * PI;
    let rot = mat2x2<f32>(cos(angle), sin(angle), -sin(angle), cos(angle));
    var lit = cascade_shadow(layer, world, n, rot, tan_r);
    if layer + 1 < count {
        let end = f.cascade_end[layer];
        var start = 0.0;
        if layer > 0 {
            start = f.cascade_end[layer - 1];
        }
        let band = (end - start) * f.cascade_params.y;
        let weight = clamp((depth - (end - band)) / max(band, 1e-4), 0.0, 1.0);
        if weight > 0.0 {
            lit = mix(lit, cascade_shadow(layer + 1, world, n, rot, tan_r), weight);
        }
    }
    let fade = clamp(
        (depth - f.cascade_params.z) / max(f.cascade_params.w - f.cascade_params.z, 1e-4),
        0.0,
        1.0
    );
    return mix(lit, 1.0, fade);
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
    // Multiplies the probes' diffuse irradiance per channel: the lit
    // vertex's occlusion, or a textured vertex's baked ambient light.
    ambient: vec3<f32>,
    // `screen_terms`: x scales ambient light only, y the sun's direct light
    // only. Both are one where nothing traced them.
    screen: vec2<f32>,
    // Emitted luminance (cd/m²), added before exposure and fog.
    emit: vec3<f32>,
};

@fragment
fn fs_lit(i: LitOut) -> @location(0) vec4<f32> {
    let ao = vec3<f32>(clamp(i.params.w, 0.0, 1.0));
    let screen = screen_terms(i.clip.xy);
    return vec4<f32>(shade(Shading(i.world, i.normal, i.tangent, i.local, i.color, i.params, i.clip.xy, ao, screen, vec3<f32>(0.0))), 1.0);
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
    // Rain darkens and glosses what it wets and pools on flat ground;
    // sleet frosts the ground under its ice (`pbr::water`).
    if f.water.y > 0.5 {
        let wet = water_wetness(i.world.xz);
        if wet > 0.0 {
            base *= 1.0 - 0.45 * wet;
            roughness = mix(roughness, 0.18, wet);
            let flat = smoothstep(0.93, 0.99, geometric.y);
            let pool = smoothstep(0.52, 0.6, value_noise(vec3<f32>(i.world.xz * 0.45, 3.0)) + 0.25 * wet);
            let puddle = flat * pool * wet;
            base = mix(base, base * 0.25, puddle);
            roughness = mix(roughness, 0.03, puddle);
        }
        if f.water_ice.w > 0.0 && i.world.y > f.water.x - 0.2 {
            let frost = water_ice(i.world.xz) * smoothstep(0.3, 0.8, geometric.y);
            base = mix(base, vec3<f32>(0.78, 0.84, 0.9), frost * 0.7);
            roughness = mix(roughness, 0.45, frost);
        }
    }
    // Derivatives need uniform control flow, which WGSL requires and a
    // browser's WebGPU enforces, so take the footprint before branching on
    // the per-fragment material code.
    let footprint = length(fwidth(i.local));

    // Aluminized Kapton: tilted facets a few centimeters across.
    if code == 2 {
        // Facets smaller than about two pixels would sparkle; fold their
        // tilt into roughness instead (LEAN mapping, Olano and Baker 2010).
        // A tier without detail normals folds all of it.
        let cell = 0.09;
        var resolved = 0.0;
        if DETAIL {
            resolved = clamp(cell / max(footprint, 1e-5) * 0.5 - 0.5, 0.0, 1.0);
        }
        if resolved > 0.0 {
            let tilt = crinkle(i.local, cell);
            n = normalize(n + (t * tilt.x + b * tilt.y) * resolved);
            t = normalize(t - n * dot(t, n));
        }
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
            // Under the sea the sun arrives dimmed and gathered into caustics.
            e = vec3<f32>(f.sun.w) * select(vec3<f32>(1.0), f.key_tint.rgb, f.key_tint.w > 0.5) * water_sun(i.world);
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
            shadow = sun_shadow(i.world, geometric, pixel) * i.screen.y;
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
    // Lamps: unshadowed point lights, inverse square windowed to zero at
    // their range (Karis 2013).
    let lamp_count = i32(f.lamp_params.x);
    for (var k = 0; k < lamp_count; k++) {
        let at = f.lamps[k * 2];
        let to = at.xyz - i.world;
        let d2 = max(dot(to, to), 1e-4);
        let l = to * inverseSqrt(d2);
        let nol = dot(n, l);
        let falloff = verse_point_falloff(dot(to, to), at.w, true);
        if nol <= 0.0 || falloff <= 0.0 {
            continue;
        }
        let h = normalize(l + v);
        let noh = max(dot(n, h), 0.0);
        let voh = max(dot(v, h), 0.0);
        let spec = d_ggx(noh, a2) * v_smith(nov, nol, a2) * f_schlick(f0, voh) * energy;
        let e = f.lamps[k * 2 + 1].rgb * falloff;
        radiance += (diffuse_color / PI + spec) * e * nol;
    }
    direct_part = radiance;

    let r = reflect(-v, n);
    // Screen-space occlusion darkens ambient light, diffuse and glossy,
    // and never direct light.
    let ambient_ao = min(ao, i.screen.x);
    let ambient = i.ambient * i.screen.x;
    let so = clamp(pow(nov + ambient_ao, exp2(-16.0 * roughness - 1.0)) - 1.0 + ambient_ao, 0.0, 1.0);
    var irr: vec3<f32>;
    var lr: vec3<f32>;
    if f.sky_light.x > 0.5 {
        // The daylight sky's light: its irradiance times the surface's local
        // ambient (vertex occlusion or the baked multiplier), and the
        // prefiltered sky toward the reflection at the antialiased
        // roughness's level, read with an explicit level of detail. Glossy
        // light is scaled by local over open-sky irradiance, so covered
        // surfaces stop reflecting the open sky (Lagarde and Zanuttini 2012).
        let open = sky_irradiance(n);
        irr = open * ambient;
        let normalization = clamp(luma(irr) / max(luma(open), 1e-4), 0.0, 1.0);
        lr = textureSampleLevel(sky_cube, linear_clamp, r, sqrt(a) * f.sky_light.y).rgb * normalization;
    } else {
        // Bounce light from nearby surfaces through the probe grid, and the
        // probes' radiance toward the reflection direction as glossy bounce.
        irr = probe_irradiance(i.world, n) * ambient;
        lr = probe_irradiance(i.world, r) / PI;
    }
    let wet = water_ambient(i.world);
    radiance += diffuse_color / PI * irr * wet;
    radiance += lr * e_spec * so * wet;
    radiance += i.emit * f.lamp_params.y;
    // The sea between this point and the eye.
    radiance = water_view(i.world, radiance);

    if DEBUG == 1u {
        radiance = direct_part;
    } else if DEBUG == 2u {
        radiance = diffuse_color / PI * irr;
    } else if DEBUG == 3u {
        radiance = lr * e_spec * so;
    } else if DEBUG == 4u {
        return vec3<f32>(ambient_ao);
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
    // x metallic; y perceptual roughness; z alpha cutoff; w emitted
    // luminance per unit of base color (cd/m²).
    params: vec4<f32>,
};

// Group 1 stays the guides' adapted luminance, so textured draws share the
// pass's bindings with the legacy faces.
@group(2) @binding(0) var base_color: texture_2d<f32>;
@group(2) @binding(1) var base_sampler: sampler;
@group(2) @binding(2) var<uniform> material: TexturedMaterial;

// Baked ambient light (`pbr::textured_bake`), one texel a vertex
// (`pbr::instanced`): rgb encodes a diffuse multiplier as 4 × value², alpha
// the open sky fraction; zero alpha means no bake reached the vertex. Group
// 3's bindings 0 and 1 are the fx sheets'.
@group(3) @binding(2) var light_map: texture_2d_array<f32>;

// A vertex (`pbr::instanced::GpuVertex`) and its instance
// (`pbr::instanced::Instance`). A merged cell is one instance whose
// transform is the identity and whose light starts at zero.
struct TexturedIn {
    // Mesh space for an instance, world space for a merged cell.
    @location(0) pos: vec3<f32>,
    // Octahedrally encoded unit normal.
    @location(1) normal: vec2<f32>,
    @location(2) uv: vec2<f32>,
    // Linear vertex color, glTF's COLOR_0.
    @location(3) color: vec4<f32>,
    // The top three rows of the instance's transform.
    @location(4) row0: vec4<f32>,
    @location(5) row1: vec4<f32>,
    @location(6) row2: vec4<f32>,
    // The light texel of this instance's vertex 0, modulo 2^32.
    @location(7) light: u32,
};

fn instance_world(v: TexturedIn) -> vec3<f32> {
    let p = vec4<f32>(v.pos, 1.0);
    return vec3<f32>(dot(v.row0, p), dot(v.row1, p), dot(v.row2, p));
}

// The normal through the instance's transform: its cofactor matrix, which is
// the inverse transpose times the determinant, so a scaled or mirrored
// instance shades as its merged copy does.
fn instance_normal(v: TexturedIn) -> vec3<f32> {
    let e = v.normal;
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    let t = max(-n.z, 0.0);
    n.x = n.x + select(t, -t, n.x >= 0.0);
    n.y = n.y + select(t, -t, n.y >= 0.0);
    let c0 = vec3<f32>(v.row0.x, v.row1.x, v.row2.x);
    let c1 = vec3<f32>(v.row0.y, v.row1.y, v.row2.y);
    let c2 = vec3<f32>(v.row0.z, v.row1.z, v.row2.z);
    let cofactor = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1));
    let flip = select(1.0, -1.0, dot(c0, cross(c1, c2)) < 0.0);
    return normalize(cofactor * n) * flip;
}

// The vertex's baked light: texel `light + index` of the light texture, read
// across its 2048-texel rows (`instanced::LIGHT_WIDTH`) and its layers.
fn instance_light(v: TexturedIn, index: u32) -> vec4<f32> {
    let texel = v.light + index;
    let rows = textureDimensions(light_map, 0).y;
    let row = texel >> 11u;
    let layer = row / rows;
    return textureLoad(light_map, vec2<i32>(i32(texel & 2047u), i32(row - layer * rows)), i32(layer), 0);
}

struct TexturedOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    // rgb the diffuse ambient multiplier; w the specular occlusion.
    @location(4) ambient: vec4<f32>,
};

// Decodes a vertex's baked light; an unbaked vertex keeps the ambient as is.
fn baked_ambient(light: vec4<f32>) -> vec4<f32> {
    if light.a < 0.5 / 255.0 {
        return vec4<f32>(1.0);
    }
    return vec4<f32>(light.rgb * light.rgb * 4.0, light.a);
}

@vertex
fn vs_textured(v: TexturedIn, @builtin(vertex_index) index: u32) -> TexturedOut {
    var o: TexturedOut;
    let world = instance_world(v);
    o.clip = f.view_proj * vec4<f32>(world, 1.0);
    o.world = world;
    o.normal = instance_normal(v);
    o.uv = v.uv;
    o.color = v.color;
    o.ambient = baked_ambient(instance_light(v, index));
    return o;
}

fn textured_base(uv: vec2<f32>, color: vec4<f32>) -> vec4<f32> {
    return textureSample(base_color, base_sampler, uv) * material.base * color;
}

// Shades a textured fragment as a generic metallic-roughness surface
// (material code 0) under its baked ambient light: the diffuse multiplier
// scales the probes' irradiance, and the open sky fraction occludes ambient
// reflections.
fn textured_shade(world: vec3<f32>, normal: vec3<f32>, pixel: vec2<f32>, base: vec3<f32>, ambient: vec4<f32>, screen: vec2<f32>) -> vec3<f32> {
    let n = normalize(normal);
    // Code 0 has no anisotropy; any tangent across the normal will do.
    let across = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.x) > 0.9);
    let params = vec4<f32>(material.params.x, material.params.y, 0.0, clamp(ambient.w, 0.0, 1.0));
    let emit = base * material.params.w;
    return shade(Shading(world, n, cross(n, across), world, base, params, pixel, max(ambient.rgb, vec3<f32>(0.0)), screen, emit));
}

@fragment
fn fs_textured(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    let screen = screen_terms(i.clip.xy);
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb, i.ambient, screen), 1.0);
}

// glTF's MASK mode: a fragment is fully opaque when its alpha reaches the
// cutoff and absent otherwise.
@fragment
fn fs_textured_masked(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    if base.a < material.params.z {
        discard;
    }
    let screen = screen_terms(i.clip.xy);
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb, i.ambient, screen), 1.0);
}

// glTF's BLEND mode, premultiplied, for glass and other thin transparency.
// The prepass holds the opaque surface behind it, so its screen-space terms
// are not this surface's.
@fragment
fn fs_textured_blend(i: TexturedOut) -> @location(0) vec4<f32> {
    let base = textured_base(i.uv, i.color);
    let alpha = verse_coverage(base.a, 2.0);
    return vec4<f32>(textured_shade(i.world, i.normal, i.clip.xy, base.rgb, i.ambient, vec2<f32>(1.0)) * alpha, alpha);
}

struct TexturedShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_shadow_textured(v: TexturedIn) -> TexturedShadowOut {
    var o: TexturedShadowOut;
    o.clip = f.light * vec4<f32>(instance_world(v), 1.0);
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

// ---------------------------------------------------------------------------
// Particle sprites (`crate::fx`): flipbook frames from the fx sheets, one
// texture-array layer each, in premultiplied alpha. Group 2 is the textured
// material's slot, so the sheets bind at group 3.

@group(3) @binding(0) var fx_sheets: texture_2d_array<f32>;
@group(3) @binding(1) var fx_sampler: sampler;
@group(3) @binding(3) var fire_noise: texture_3d<f32>;
@group(3) @binding(4) var fire_lut: texture_2d<f32>;
@group(3) @binding(5) var fire_sampler: sampler;
// The water's depth copy (`water::screen`): each pixel's view depth, read
// for the particles' soft fade on Medium and High.
@group(3) @binding(6) var fx_scene_depth: texture_2d<f32>;

// How much of a particle at `world` on `pixel` shows in front of the
// opaque scene: it fades over the last `f.water_screen.w` m before what it
// meets instead of cutting a hard line (Lorach, "Soft Particles", 2007).
fn soft_particle(world: vec3<f32>, pixel: vec2<f32>) -> f32 {
    if f.water_screen.x < 0.5 || f.water_screen.w <= 0.0 {
        return 1.0;
    }
    let size = vec2<i32>(textureDimensions(fx_scene_depth));
    let p = clamp(vec2<i32>(pixel), vec2<i32>(0), size - vec2<i32>(1));
    let scene = textureLoad(fx_scene_depth, p, 0).r;
    return clamp((scene - water_view_w(world)) / f.water_screen.w, 0.0, 1.0);
}

// Fire Pro volume-march.wgsl: bounded emission/absorption integration.
// Copyright 2026 Daniel Greenheck, MIT (assets/verse/fx/LICENSE-fire-pro.txt).
// Verse supplies a turbulent procedural field inside the existing particle proxy;
// this is not Fire Pro's sparse voxel fluid solver.

struct SpriteIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv_a: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
    // x frame blend, y sheet layer, z additive, w 1 when lit.
    @location(4) params: vec4<f32>,
};

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv_a: vec2<f32>,
    @location(2) uv_b: vec2<f32>,
    @location(3) params: vec4<f32>,
    @location(4) world: vec3<f32>,
};

@vertex
fn vs_sprite(v: SpriteIn) -> SpriteOut {
    var o: SpriteOut;
    o.clip = f.view_proj * vec4<f32>(v.pos, 1.0);
    o.color = v.color;
    o.uv_a = v.uv_a;
    o.uv_b = v.uv_b;
    o.params = v.params;
    o.world = v.pos;
    return o;
}

@fragment
fn fs_sprite(i: SpriteOut) -> @location(0) vec4<f32> {
    // Both samples come first, in uniform control flow.
    let layer = i32(i.params.y + 0.5);
    let a = textureSample(fx_sheets, fx_sampler, i.uv_a, layer);
    let b = textureSample(fx_sheets, fx_sampler, i.uv_b, layer);
    // Premultiplied color and coverage.
    var texel = mix(a, b, clamp(i.params.x, 0.0, 1.0));
    if layer == 0 && f.fire_control.x > 0.0 && f.fire_control.y > 0.5 {
        let volume = verse_fire_volume(fract(i.uv_a * 4.0) * 2.0 - 1.0, i.world, f.params.y, texel.a, u32(f.fire_control.x), fire_noise, fire_lut, fire_sampler, fx_sampler);
        // Keep the authored flipbook silhouette while adding optical hot cores.
        texel = vec4<f32>(volume.rgb, volume.a);
    }
    let alpha = i.color.a;
    // Emitted light in luminance, through the exposure.
    let emitted = expose(texel.rgb * i.color.rgb);
    // A lit surface in the scene's display scale, fogged like the faces
    // around it: unpremultiplied for the fog, premultiplied again after.
    let coverage = max(texel.a, 1.0 / 255.0);
    let straight = texel.rgb / coverage * i.color.rgb * guide_scale();
    let lit = neon_fog(straight, i.world, 1.0) * texel.a;
    let soft = soft_particle(i.world, i.clip.xy);
    let rgb = mix(emitted, lit, i.params.w) * alpha * soft;
    let cover = texel.a * alpha * (1.0 - clamp(i.params.z, 0.0, 1.0)) * soft;
    return vec4<f32>(rgb, cover);
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

// VERSE_WATER
