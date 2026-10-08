// ---------------------------------------------------------------------------
// Shared water shading (`verse_pbr::water`), spliced by
// `verse_pbr::shading::source` into the physical renderer's water pass
// (`water/photo.wgsl`) and the imported renderer's (`water/imported.wgsl`).
// It implements public techniques, reimplemented here:
//
// - Gerstner waves: Fournier and Reeves, "A Simple Model of Ocean Waves"
//   (SIGGRAPH 1986), in Finch's parameterization, "Effective Water
//   Simulation from Physical Models" (GPU Gems, chapter 1, 2004). The terms
//   are `physics::water::WaveSet`'s, evaluated in f32: the angle is
//   k D·p0 − 2π·count/period + φ, with the last two summed on the CPU from
//   the integer phase counter, and the horizontal factor q = Q / (k A N).
// - Fresnel: Schlick's approximation (1994) with F0 from an index of
//   refraction of 1.33, above and below the surface, with Snell's window and
//   total internal reflection from below.
// - Sun glints: the GGX distribution (Walter et al., 2007) with the
//   height-correlated Smith visibility (Heitz, 2014), widened by the sun's
//   disc and faded with distance.
// - Crest scattering: single scattering through a crest's thickness with the
//   Henyey–Greenstein phase function (Henyey and Greenstein, 1941).
// - Absorption: per-channel Beer–Lambert transmittance along the refracted
//   view path over the vertex's baked depth, with coefficients from Jerlov's
//   water types (Jerlov, Marine Optics, 1976).
// - Flow: two-phase flow-map advection of normals and foam (Vlachos, "Water
//   Flow in Portal 2", SIGGRAPH 2010).
// - The spectral sea (body 0): Tessendorf's FFT cascades ("Simulating Ocean
//   Water", SIGGRAPH 2001) of a JONSWAP spectrum, synthesized on the CPU
//   (`water::ocean`) and read from `water_waves`: displacement and slopes,
//   whitecaps where the Jacobian folds the surface, linear shoaling capped
//   by McCowan's breaker limit (1894), and surf where waves break.
// - Depth from a scene copy (Medium and High, `water/screen.wgsl`): the
//   absorption path is the distance to the scene behind the surface, and
//   foam gathers where that distance is short, as soft particles fade
//   (Lorach, "Soft Particles", NVIDIA, 2007).
// - The ripple and foam field around the camera (`water::ripple`): the
//   damped wave equation (Bridson and Müller-Fischer, SIGGRAPH 2007 course
//   notes) or Tessendorf's iWave ("Interactive Water Surfaces", 2004),
//   with Kelvin's wake wedge drawn in, stepped on the CPU and read from
//   the last layer of `water_waves`: height, slopes, and foam.
//
// The host shader declares the bindings and four hooks:
//
//   var<uniform> water: WaterUniform;
//   var water_tile: texture_2d<f32>;        // `water::tile`, RG slopes
//   var water_tile_sampler: sampler;         // also samples the cascades
//   var water_waves: texture_2d_array<f32>; // `water::ocean`, 2 layers a cascade
//   fn water_host_control() -> vec4<f32>     // `water::control`
//   fn water_host_sky(dir: vec3<f32>, roughness: f32) -> vec3<f32>
//   fn water_host_sun() -> vec4<f32>         // toward the sun; w 1 when lit
//   fn water_host_sun_light() -> vec3<f32>   // the sun's colored illuminance
//   fn water_host_light(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> vec3<f32>
//   fn water_host_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32
//
// Radiance comes back in the host's units, before its exposure and fog.

const WATER_IOR: f32 = 1.33;
// ((n - 1) / (n + 1))², reflectance at normal incidence.
const WATER_F0: f32 = 0.0200593;
const WATER_RIPPLE_LIFE: f32 = 4.0;
const WATER_RIPPLE_SPEED: f32 = 1.1;
// `water::tile::SIZE`, m, and the slope its texels' full range stands for.
const WATER_TILE_METERS: f32 = 8.0;
const WATER_TILE_SLOPE: f32 = 0.5;
// The whitecap cover below which no foam shows, and above which it is
// solid (`water::ocean::WHITECAP_EDGE` and `WHITECAP_CORE`).
const WATER_WHITECAP_EDGE: f32 = 0.3;
const WATER_WHITECAP_CORE: f32 = 0.7;
// The flow maps' cycle, s (Vlachos 2010 uses a similar period).
const WATER_FLOW_PERIOD: f32 = 1.6;

// One body of water's terms (`water::Body`).
struct WaterBody {
    // Per Gerstner term: direction (x, z), wavenumber (rad/m), amplitude
    // (m); then q, the angle's constant part now (rad), ω (rad/s), and 0.
    waves: array<vec4<f32>, 16>,
    // rgb absorption, 1/m; w the level now, m.
    absorb: vec4<f32>,
    // rgb in-scatter color; w foam amount, 0 to 1.
    scatter: vec4<f32>,
    // Term count, roughness, swell gain, and 1 when the eye is inside.
    params: vec4<f32>,
    // The level the vertices were built at (m), the shore foam band (m),
    // crest scattering's strength, and 0.
    rest: vec4<f32>,
};

struct WaterUniform {
    bodies: array<WaterBody, 8>,
    // Per detail wave: direction, wavenumber, amplitude; then ω, phase,
    // wavelength, and 0.
    detail: array<vec4<f32>, 32>,
    // Per ripple: x, z, start, strength.
    ripples: array<vec4<f32>, 24>,
    // Time (s), detail count, ripple count, and body count.
    params: vec4<f32>,
    // Caustics' strength, the angle one pixel spans at the eye (rad, from
    // the host renderer; 0 when unknown), the rain falling on the water (0
    // to 1, `water::rain`), and one spare.
    look: vec4<f32>,
    // A sky and a sun for a host that has neither (the imported renderer):
    // zenith (w 1 when set), horizon, the direction toward the sun with its
    // illuminance, and the sun's color.
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    sun: vec4<f32>,
    sun_color: vec4<f32>,
    // The spectral sea (`water::ocean::rows`): per cascade 1 / tile (1/m),
    // the shortest wavelength (m), the wavenumber it shoals by (rad/m), and
    // its gain; then the cascade count, the cascades that move vertices, 1
    // for surf, and the significant height (m); half a texel, the peak's
    // angular frequency (rad/s), and the gains on the sea's ripples and on
    // its cascades' slopes; and each cascade's slope variance.
    ocean: array<vec4<f32>, 6>,
    // The ocean's clipmap (`water::clipmap::rows`): per level its center
    // (x, z, m), spacing (m), and the distance from the eye where its morph
    // starts (m); then the level count, a level's half width in cells, the
    // apron's reach (m), and 1 when it draws the sea.
    clip: array<vec4<f32>, 6>,
    // The streamed field (`water::field::Stream::rows`): its origin (x, z,
    // m), texel (m), and window (pages); its pages along x and z, a page's
    // side (m), and 1 when present; and the depth, shore distance, and
    // current outside it.
    field: array<vec4<f32>, 3>,
    // The page each slot of the field's atlas holds (x × 4096 + z), four
    // slots a row; −1 for none.
    field_pages: array<vec4<f32>, 16>,
    // The ripple field's window: its corner (x, z), side (m; 0 for none),
    // and its layer in `water_waves`.
    ripple: vec4<f32>,
    // Per boat hull the water is masked out of: its middle (x, z) and its
    // heading; then its half beam (0 for none), half length, and gunwale.
    hulls: array<vec4<f32>, 8>,
};

// A vertex of a water surface at rest (`water::WaterVertex`).
struct WaterIn {
    @location(0) pos: vec3<f32>,
    @location(1) depth: f32,
    @location(2) flow: vec2<f32>,
    @location(3) foam: f32,
    @location(4) kind: f32,
    @location(5) shore: f32,
    @location(6) body: f32,
};

// Four inter-stage locations, well under WebGL2's eight.
struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    // The displaced position, and the depth under it at rest.
    @location(0) world_depth: vec4<f32>,
    // The rest position on the plane, where waves are evaluated, and the
    // flow (m/s).
    @location(1) rest_flow: vec4<f32>,
    // Extra foam, kind, crest squeeze, and shore distance (m).
    @location(2) look: vec4<f32>,
    // Body index, height over the level (m), and two spare.
    @location(3) extra: vec4<f32>,
};

fn water_body_index(code: f32) -> u32 {
    return min(u32(max(code, 0.0) + 0.5), 7u);
}

// The share of a wave's height left over water `depth` deep, √tanh(k d):
// one in deep water. Visual only; `physics::water` is deep water. The
// argument stops at 10, where tanh is one in f32, because some GPUs'
// tanh of a large argument is inf / inf.
fn water_shoal(k: f32, depth: f32) -> f32 {
    return sqrt(tanh(clamp(k * depth, 0.0, 10.0)));
}

// The Gerstner displacement (xyz, m) of rest point `p0` of body `b`, and how
// tightly the waves squeeze the surface there (w, crests near one). Each
// amplitude is scaled by `scale` and the shallow-water factor.
fn water_gerstner(b: u32, p0: vec2<f32>, depth: f32, scale: f32) -> vec4<f32> {
    var d = vec3<f32>(0.0);
    var squeeze = 0.0;
    let count = i32(water.bodies[b].params.x);
    let gain = water.bodies[b].params.z * scale;
    for (var i = 0; i < count; i++) {
        let a = water.bodies[b].waves[i * 2];
        let c = water.bodies[b].waves[i * 2 + 1];
        let k = a.z;
        let amp = a.w * gain * water_shoal(k, depth);
        let theta = k * dot(a.xy, p0) + c.y;
        let co = cos(theta);
        let s = sin(theta);
        d.x += c.x * amp * a.x * co;
        d.z += c.x * amp * a.y * co;
        d.y += amp * s;
        squeeze += c.x * amp * k * s;
    }
    return vec4<f32>(d, squeeze);
}

// The waves' slope at rest point `p0` (dh/dx, dh/dz) and the squeeze's
// change of the normal's y (Finch 2004, equation 12).
fn water_gerstner_slope(b: u32, p0: vec2<f32>, depth: f32, scale: f32) -> vec3<f32> {
    var g = vec3<f32>(0.0);
    let count = i32(water.bodies[b].params.x);
    let gain = water.bodies[b].params.z * scale;
    for (var i = 0; i < count; i++) {
        let a = water.bodies[b].waves[i * 2];
        let c = water.bodies[b].waves[i * 2 + 1];
        let k = a.z;
        let wa = k * a.w * gain * water_shoal(k, depth);
        let theta = k * dot(a.xy, p0) + c.y;
        let co = cos(theta);
        g.x += a.x * wa * co;
        g.y += a.y * wa * co;
        g.z += c.x * wa * sin(theta);
    }
    return g;
}

// ---- The spectral sea: body 0's cascades.

// Cascades body `b` carries: the sea's, or none.
fn water_ocean_count(b: u32) -> i32 {
    return select(0, i32(water.ocean[3].x), b == 0u);
}

// Rest point `p` in cascade `c`'s tile, as a texture coordinate: texel
// (i, j) holds the rest point (i, j) times the tile over the texels.
fn water_ocean_uv(c: i32, p: vec2<f32>) -> vec2<f32> {
    return p * water.ocean[c].x + vec2<f32>(water.ocean[4].x);
}

// Cascade `c`'s gain over water `depth` deep: linear shoaling,
// 1 / √(tanh(kd) (1 + 2kd / sinh 2kd)), capped so no wave stands higher
// than 0.78 of the depth (`water::ocean::gain`).
fn water_ocean_gain(c: i32, depth: f32) -> f32 {
    let d = max(depth, 0.0);
    let x = clamp(water.ocean[c].z * d, 0.02, 10.0);
    let shoal = min(1.0 / sqrt(tanh(x) * (1.0 + 2.0 * x / sinh(2.0 * x))), 2.0);
    let cap = clamp(0.78 * d / max(water.ocean[3].w * shoal, 1e-3), 0.0, 1.0);
    return water.ocean[c].w * shoal * cap;
}

// The cascades' displacement of rest point `p0` over water `depth` deep
// (xyz, m) and their crest squeeze, 1 − J (w).
fn water_ocean_move(b: u32, p0: vec2<f32>, depth: f32, scale: f32) -> vec4<f32> {
    var d = vec4<f32>(0.0);
    let count = min(water_ocean_count(b), i32(water.ocean[3].y));
    for (var c = 0; c < count; c++) {
        let uv = water_ocean_uv(c, p0);
        let g = water_ocean_gain(c, depth) * scale;
        let a = textureSampleLevel(water_waves, water_tile_sampler, uv, c * 2, 0.0);
        let s = textureSampleLevel(water_waves, water_tile_sampler, uv, c * 2 + 1, 0.0);
        d += vec4<f32>(a.xyz * g, s.z * g);
    }
    return d;
}

// The cascades' slope at rest point `p` (xy), each cascade faded once its
// shortest wave spans only a few of the pixel's `footprint` (m), the slope
// variance the fading left behind (z), and the whitecaps' foam (w).
fn water_ocean_detail(b: u32, p: vec2<f32>, depth: f32, scale: f32, footprint: f32, dpx: vec2<f32>, dpy: vec2<f32>) -> vec4<f32> {
    var o = vec4<f32>(0.0);
    var coarse = 0.0;
    var squeeze = 0.0;
    let count = water_ocean_count(b);
    for (var c = 0; c < count; c++) {
        let row = water.ocean[c];
        let uv = water_ocean_uv(c, p);
        let g = water_ocean_gain(c, depth) * scale;
        let keep = clamp(row.y / (6.0 * max(footprint, 1e-4)) - 0.5, 0.0, 1.0);
        let a = textureSampleGrad(water_waves, water_tile_sampler, uv, c * 2, dpx * row.x, dpy * row.x);
        let s = textureSampleGrad(water_waves, water_tile_sampler, uv, c * 2 + 1, dpx * row.x, dpy * row.x);
        // The cascades' slopes follow the wind (`water::ocean::slope_gains`).
        let sg = g * water.ocean[4].w;
        o += vec4<f32>(s.xy * (sg * keep), water.ocean[5][c] * sg * sg * (1.0 - keep * keep), 0.0);
        let foam = a.w * clamp(g, 0.0, 1.0);
        if c == 0 {
            coarse = foam;
        } else {
            o.w = max(o.w, foam);
        }
        if c == 1 {
            squeeze = s.z * keep;
        }
    }
    // Cascade 0's texels span metres to tens of metres, so on its own its
    // foam would whiten whole crests of the longest waves. Where a finer
    // cascade is drawn, its foam gathers only on that cascade's crests
    // inside the breaking zone, in streaks the size of the waves breaking.
    // Low has no finer cascade, so two octaves of value noise, metres
    // across, break it into ragged patches instead.
    if count > 1 {
        coarse *= smoothstep(0.0, 0.5, squeeze);
    } else {
        let t = water.params.x * 0.15;
        let n = 0.6 * water_noise(vec3<f32>(p * 0.37, t)) + 0.4 * water_noise(vec3<f32>(p * 1.3, t));
        coarse *= smoothstep(0.5, 0.75, n);
    }
    o.w = max(o.w, coarse);
    return o;
}

// ---- The ripple and foam field.

// The field at `p`: height (m), its slopes along x and z, and foam, faded
// out over the window's outer cells; zero outside it or without one.
fn water_ripple_field(p: vec2<f32>) -> vec4<f32> {
    let side = water.ripple.z;
    if side <= 0.0 {
        return vec4<f32>(0.0);
    }
    let uv = (p - water.ripple.xy) / side;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let fade = smoothstep(0.0, 0.08, edge);
    let layer = i32(water.ripple.w + 0.5);
    return textureSampleLevel(water_waves, water_tile_sampler, uv, layer, 0.0) * fade;
}

// Whether `world` lies inside a boat's hull, under its gunwale: the hull
// keeps the water out, so the surface draws nothing there (an analytic
// footprint mask on every tier, in place of a screen-space one).
fn water_in_hull(world: vec3<f32>) -> bool {
    for (var i = 0; i < 4; i++) {
        let a = water.hulls[i * 2];
        let b = water.hulls[i * 2 + 1];
        if b.x <= 0.0 {
            continue;
        }
        let d = world.xz - a.xy;
        let along = dot(d, a.zw);
        let across = d.x * a.w - d.y * a.z;
        if abs(along) < b.y && abs(across) < b.x && world.y < b.z {
            return true;
        }
    }
    return false;
}

// Displaces one vertex: `world` the position, `crest` the squeeze.
struct WaterMoved {
    world: vec3<f32>,
    crest: f32,
};

// A surface vertex moved by its body's waves at `scale` (0 for still
// water), on a level raised by `rise` (m).
fn water_move(v: WaterIn, scale: f32, rise: f32) -> WaterMoved {
    var o: WaterMoved;
    let b = water_body_index(v.body);
    var world = v.pos;
    world.y += rise;
    o.crest = 0.0;
    if scale > 0.0 && water.bodies[b].params.x > 0.5 {
        let wave = water_gerstner(b, v.pos.xz, v.depth + rise, scale);
        world += wave.xyz;
        o.crest = wave.w;
    }
    if scale > 0.0 && water_ocean_count(b) > 0 {
        let sea = water_ocean_move(b, v.pos.xz, v.depth + rise, scale);
        world += sea.xyz;
        o.crest = max(o.crest, sea.w);
    }
    world.y += water_ripple_field(v.pos.xz).x;
    o.world = world;
    return o;
}

// The varyings for a moved vertex.
fn water_out(v: WaterIn, moved: WaterMoved, rest: vec2<f32>, depth: f32, clip: vec4<f32>) -> WaterOut {
    var o: WaterOut;
    let b = water_body_index(v.body);
    o.clip = clip;
    o.world_depth = vec4<f32>(moved.world, depth);
    o.rest_flow = vec4<f32>(rest, v.flow);
    o.look = vec4<f32>(v.foam, v.kind, moved.crest, v.shore);
    o.extra = vec4<f32>(v.body, moved.world.y - water.bodies[b].absorb.w, 0.0, 0.0);
    return o;
}

// ---- Detail: what bends the normal but never moves the surface.

// A lattice hash: the pcg3d permutation from Jarzynski and Olano, "Hash
// Functions for GPU Rendering" (JCGT, 2020), on the cell's integer corner.
fn water_hash(p: vec3<f32>) -> f32 {
    var v = bitcast<vec3<u32>>(vec3<i32>(p)) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    return f32(v.x >> 8u) / 16777215.0;
}

// Smooth value noise over the hashed lattice.
fn water_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let u = fract(p);
    let s = u * u * (3.0 - 2.0 * u);
    let a = water_hash(i);
    let b = water_hash(i + vec3<f32>(1.0, 0.0, 0.0));
    let c = water_hash(i + vec3<f32>(0.0, 1.0, 0.0));
    let d = water_hash(i + vec3<f32>(1.0, 1.0, 0.0));
    let e = water_hash(i + vec3<f32>(0.0, 0.0, 1.0));
    let g = water_hash(i + vec3<f32>(1.0, 0.0, 1.0));
    let h = water_hash(i + vec3<f32>(0.0, 1.0, 1.0));
    let k = water_hash(i + vec3<f32>(1.0, 1.0, 1.0));
    return mix(
        mix(mix(a, b, s.x), mix(c, d, s.x), s.y),
        mix(mix(e, g, s.x), mix(h, k, s.x), s.y),
        s.z
    );
}

// The analytic detail waves' slope at `p`, each faded once it is shorter
// than a few of the pixel's `footprint` (m), and the slope variance the
// faded waves would have added (z), which becomes roughness (after Toksvig,
// "Mipmapping Normal Maps", 2005).
fn water_detail_waves(p: vec2<f32>, footprint: f32, gain: f32, count: i32) -> vec3<f32> {
    var g = vec2<f32>(0.0);
    var lost = 0.0;
    let t = water.params.x;
    let n = min(count, i32(water.params.y));
    for (var i = 0; i < n; i++) {
        let a = water.detail[i * 2];
        let b = water.detail[i * 2 + 1];
        let k = a.z;
        let slope = a.w * k * gain;
        let keep = clamp(b.z / (6.0 * max(footprint, 1e-4)) - 0.5, 0.0, 1.0);
        let theta = k * dot(a.xy, p) - b.x * t + b.y;
        g += a.xy * (slope * keep * cos(theta));
        lost += 0.5 * slope * slope * (1.0 - keep * keep);
    }
    return vec3<f32>(g, lost);
}

// The baked tile's slope at `p` (Low): two layers of the looping tile
// scrolled across each other, sampled with the pixel's own gradients so
// distant water filters instead of aliasing.
fn water_tile_detail(p: vec2<f32>, dpx: vec2<f32>, dpy: vec2<f32>, gain: f32) -> vec3<f32> {
    let t = water.params.x;
    let s1 = 1.0 / WATER_TILE_METERS;
    let s2 = 1.0 / (WATER_TILE_METERS * 0.61);
    let r = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    let uv1 = p * s1 + vec2<f32>(t * 0.021, t * 0.013);
    let uv2 = (r * p) * s2 - vec2<f32>(t * 0.017, -t * 0.024);
    let a = textureSampleGrad(water_tile, water_tile_sampler, uv1, dpx * s1, dpy * s1);
    let b = textureSampleGrad(water_tile, water_tile_sampler, uv2, (r * dpx) * s2, (r * dpy) * s2);
    let ga = (a.xy * 2.0 - 1.0) * WATER_TILE_SLOPE;
    // The second layer's slope turns back into world axes.
    let gb = transpose(r) * ((b.xy * 2.0 - 1.0) * WATER_TILE_SLOPE);
    // The tile's alpha keeps its slope variance lost to filtering.
    let lost = (a.z + b.z) * 0.5 * WATER_TILE_SLOPE * WATER_TILE_SLOPE * 0.25;
    return vec3<f32>((ga * 0.6 + gb * 0.4) * gain, lost * gain * gain);
}

// A pixel's footprint on the surface (m) and its gradients in the rest
// plane. From the eye's distance times the pixel's angle, stretched along
// the view at grazing angles, it stays smooth across the triangles of a
// coarse, displaced mesh, where screen derivatives of the rest position
// jump; without a pixel angle, the screen derivatives.
struct WaterFootprint {
    size: f32,
    dpx: vec2<f32>,
    dpy: vec2<f32>,
};

fn water_footprint(v: vec3<f32>, distance: f32, dpx: vec2<f32>, dpy: vec2<f32>) -> WaterFootprint {
    var o: WaterFootprint;
    let angle = water.look.y;
    if angle <= 0.0 {
        o.size = max(length(abs(dpx) + abs(dpy)), 1e-4);
        o.dpx = dpx;
        o.dpy = dpy;
        return o;
    }
    let size = max(distance * angle, 1e-4);
    let flat = vec2<f32>(v.x, v.z);
    let reach = length(flat);
    let toward = select(vec2<f32>(0.0, 1.0), flat / max(reach, 1e-6), reach > 1e-4);
    let stretch = 1.0 / max(abs(v.y), 0.08);
    o.dpx = vec2<f32>(-toward.y, toward.x) * size;
    o.dpy = toward * size * stretch;
    o.size = size * sqrt(stretch);
    return o;
}

// Detail at `p` for this tier: the baked tile on Low, analytic waves above.
fn water_detail_at(p: vec2<f32>, footprint: f32, gain: f32, dpx: vec2<f32>, dpy: vec2<f32>) -> vec3<f32> {
    let count = i32(water_host_control().x);
    if count <= 0 {
        return water_tile_detail(p, dpx, dpy, gain);
    }
    return water_detail_waves(p, footprint, gain, count);
}

// Detail carried along `flow` (m/s) in two phases half a cycle apart,
// blended so neither phase's reset shows (Vlachos 2010). Still water gets
// the standing detail.
fn water_flow_detail(p: vec2<f32>, flow: vec2<f32>, footprint: f32, dpx: vec2<f32>, dpy: vec2<f32>) -> vec3<f32> {
    if length(flow) <= 0.02 {
        return water_detail_at(p, footprint, 1.0, dpx, dpy);
    }
    let t = water.params.x;
    let phase_a = fract(t / WATER_FLOW_PERIOD);
    let phase_b = fract(t / WATER_FLOW_PERIOD + 0.5);
    let da = water_detail_at(p - flow * phase_a * WATER_FLOW_PERIOD, footprint, 1.4, dpx, dpy);
    let db = water_detail_at(p - flow * phase_b * WATER_FLOW_PERIOD + vec2<f32>(3.7, 1.3), footprint, 1.4, dpx, dpy);
    let wa = 1.0 - abs(2.0 * phase_a - 1.0);
    return da * wa + db * (1.0 - wa);
}

// The ripples' slope at `p`: damped wave packets riding expanding rings.
fn water_ripples(p: vec2<f32>) -> vec2<f32> {
    var g = vec2<f32>(0.0);
    let t = water.params.x;
    let count = i32(water.params.z);
    let k = 6.2831853 / 0.45;
    for (var i = 0; i < count; i++) {
        let r = water.ripples[i];
        let age = t - r.z;
        let to = p - r.xy;
        let dist = length(to);
        let x = dist - WATER_RIPPLE_SPEED * age;
        let envelope = exp(-x * x / 0.18) * exp(-age * 1.1) / (1.0 + 1.5 * dist);
        let slope = r.w * envelope * k * cos(k * x);
        g += to / max(dist, 1e-3) * slope;
    }
    return g;
}

// Rain ripples (`water::rain`): analytic rings from drops landing on a
// jittered grid, after Lagarde's "Water drop 2b: dynamic rain and its
// effects" (2013), here computed in the shader rather than read from an
// animated texture. Each of two layers has one drop per cell per cycle,
// where a hash under the rain's intensity says a drop lands; its ring
// spreads at 0.4 m/s and fades over the cycle. Returns the surface slope
// at `p` for rain `rain` (0 to 1) at time `time` (s), faded once the rings
// are smaller than the pixel's `footprint` (m). No textures, no
// derivatives, so the lit pass's puddles call it too.
fn water_rain_slope(p: vec2<f32>, time: f32, rain: f32, footprint: f32) -> vec2<f32> {
    if rain <= 0.0 {
        return vec2<f32>(0.0);
    }
    let fade = 1.0 - smoothstep(0.03, 0.12, footprint);
    if fade <= 0.0 {
        return vec2<f32>(0.0);
    }
    var g = vec2<f32>(0.0);
    for (var layer = 0; layer < 2; layer++) {
        let cell = select(0.42, 0.29, layer == 1);
        let q = p / cell + vec2<f32>(f32(layer) * 0.37, f32(layer) * 0.71);
        let id = floor(q);
        let period = 0.85 + 0.3 * water_hash(vec3<f32>(id, 41.0 + f32(layer)));
        let offset = water_hash(vec3<f32>(id, 53.0 + f32(layer))) * period;
        let cycle = floor((time + offset) / period);
        let age = (time + offset) - cycle * period;
        let lands = water_hash(vec3<f32>(id, cycle * 0.618 + 7.0 + f32(layer)));
        if lands > rain {
            continue;
        }
        let jitter = vec2<f32>(
            water_hash(vec3<f32>(id, cycle + 17.0)),
            water_hash(vec3<f32>(id, cycle + 29.0)),
        );
        let center = id + 0.2 + 0.6 * jitter;
        let to = (q - center) * cell;
        let d = length(to);
        let s = d - 0.4 * age;
        let k = 6.2831853 / 0.045;
        let life = 1.0 - age / period;
        let envelope = exp(-s * s / 0.0006) * life * life;
        // d/dd of 0.0035 cos(k s) under the envelope.
        let slope = -0.0035 * k * sin(k * s) * envelope;
        g += to / max(d, 1e-4) * slope;
    }
    return g * fade * (0.6 + 0.4 * rain);
}

// Foam riding the rings of young ripples.
fn water_ripple_foam(p: vec2<f32>) -> f32 {
    var foam = 0.0;
    let t = water.params.x;
    let count = i32(water.params.z);
    for (var i = 0; i < count; i++) {
        let r = water.ripples[i];
        let age = t - r.z;
        let x = distance(p, r.xy) - WATER_RIPPLE_SPEED * age;
        foam = max(foam, exp(-x * x / 0.08) * exp(-age * 1.6) * clamp(r.w * 25.0, 0.0, 1.0));
    }
    return foam;
}

// Foam's mottled cover at `p`: one octave on Low, two above, carried along
// the flow in two phases like the detail.
fn water_foam_noise(p: vec2<f32>, flow: vec2<f32>) -> f32 {
    let t = water.params.x;
    let octaves = water_host_control().z;
    let phase_a = fract(t / WATER_FLOW_PERIOD);
    let phase_b = fract(t / WATER_FLOW_PERIOD + 0.5);
    let wa = 1.0 - abs(2.0 * phase_a - 1.0);
    let pa = p - flow * phase_a * WATER_FLOW_PERIOD;
    let pb = p - flow * phase_b * WATER_FLOW_PERIOD + vec2<f32>(5.3, 2.9);
    var a = water_noise(vec3<f32>(pa * 1.3, t * 0.25));
    var b = water_noise(vec3<f32>(pb * 1.3, t * 0.25));
    if octaves > 1.5 {
        a = a * 0.6 + 0.4 * water_noise(vec3<f32>(pa * 4.1 + vec2<f32>(t * 0.07, 0.0), t * 0.5));
        b = b * 0.6 + 0.4 * water_noise(vec3<f32>(pb * 4.1 + vec2<f32>(t * 0.07, 0.0), t * 0.5));
    }
    return a * wa + b * (1.0 - wa);
}

// ---- Optics.

// Schlick's Fresnel reflectance for an air–water interface at cosine `c`.
fn water_fresnel(c: f32) -> f32 {
    let m = clamp(1.0 - c, 0.0, 1.0);
    let m2 = m * m;
    return WATER_F0 + (1.0 - WATER_F0) * m2 * m2 * m;
}

// The cosine of the refracted angle under the surface for a view at cosine
// `ci` above it.
fn water_cos_refracted(ci: f32) -> f32 {
    return sqrt(max(1.0 - (1.0 - ci * ci) / (WATER_IOR * WATER_IOR), 0.0));
}

// Per-channel Beer–Lambert transmittance through `depth` m of body `b`'s
// water seen at cosine `ci`: the path runs along the refracted ray.
fn water_transmittance(b: u32, depth: f32, ci: f32) -> vec3<f32> {
    let path = max(depth, 0.0) / max(water_cos_refracted(ci), 0.05);
    return exp(-water.bodies[b].absorb.rgb * path);
}

// Light scattered back toward the eye from inside body `b`'s water: its
// in-scatter color under the sun and sky.
fn water_inscatter(b: u32) -> vec3<f32> {
    let sun = water_host_sun();
    let sky = water_host_sky(vec3<f32>(0.0, 1.0, 0.0), 1.0);
    let lit = water_host_sun_light() * (sun.w * max(sun.y, 0.0) / 3.14159265);
    return water.bodies[b].scatter.rgb * (lit + sky);
}

// The sky's mip level for a roughness: rough, small, or distant water
// reflects a blurrier sky. The host maps 0..1 onto its own levels.
fn water_sky_roughness(roughness: f32, footprint: f32) -> f32 {
    return clamp(sqrt(roughness) + 0.15 * log2(1.0 + footprint * 4.0), 0.0, 1.0);
}

fn water_ggx(noh: f32, a2: f32) -> f32 {
    let d = noh * noh * (a2 - 1.0) + 1.0;
    return a2 / (3.14159265 * d * d);
}

fn water_smith(nov: f32, nol: f32, a2: f32) -> f32 {
    let gv = nol * sqrt(nov * nov * (1.0 - a2) + a2);
    let gl = nov * sqrt(nol * nol * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 1e-6);
}

// The sun's glint as reflected radiance per unit illuminance: GGX widened
// by the sun's disc (`disc`, rad), fading out by the tier's distance, where
// glitter smaller than a pixel would only shimmer.
fn water_glint(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32, disc: f32, distance: f32) -> f32 {
    let nol = dot(n, l);
    let nov = max(dot(n, v), 1e-3);
    if nol <= 0.0 {
        return 0.0;
    }
    let h = normalize(l + v);
    let fade_at = max(water_host_control().y, 1.0);
    let far = smoothstep(0.4 * fade_at, fade_at, distance);
    let a = min(roughness * roughness + disc * 0.5 + far * 0.08, 1.0);
    let a2 = a * a;
    let spec = water_ggx(max(dot(n, h), 0.0), a2) * water_smith(nov, nol, a2) * water_fresnel(max(dot(v, h), 0.0));
    return spec * nol * (1.0 - 0.75 * far);
}

// Light scattered through a thin crest toward an eye looking into the sun:
// a crest `height` m over the level, at most a meter thick, attenuated by
// the water's absorption and weighted by the Henyey–Greenstein phase for
// forward scattering (g = 0.6).
fn water_crest_scatter(b: u32, v: vec3<f32>, l: vec3<f32>, n: vec3<f32>, height: f32, squeeze: f32) -> vec3<f32> {
    let strength = water.bodies[b].rest.z;
    if strength <= 0.0 {
        return vec3<f32>(0.0);
    }
    let g = 0.6;
    let cos_theta = dot(-l, v);
    let hg = (1.0 - g * g) / (4.0 * 3.14159265 * pow(1.0 + g * g - 2.0 * g * cos_theta, 1.5));
    let thin = clamp(height * 2.0, 0.0, 1.0) * clamp(squeeze * 1.5 + 0.2, 0.0, 1.0);
    let side = clamp(1.0 - n.y * 0.6 + dot(n, v) * 0.4, 0.0, 1.0);
    let through = exp(-water.bodies[b].absorb.rgb * (1.0 - 0.6 * thin));
    return water.bodies[b].scatter.rgb * through * hg * thin * side * strength * 12.0;
}

// The foam at a surface point: a lace along the shore from the baked
// distance (m), crests the waves squeeze, the vertex's own white water,
// and young ripples, all through the mottled noise.
fn water_foam(b: u32, rest: vec2<f32>, shore: f32, crest: f32, extra: f32, noise: f32) -> f32 {
    let band = max(water.bodies[b].rest.y, 0.05);
    var foam = (1.0 - smoothstep(0.0, band, shore)) * smoothstep(0.3, 0.62, noise + 0.15 * (1.0 - shore / band));
    foam = max(foam, smoothstep(0.62, 0.95, crest) * smoothstep(0.35, 0.7, noise));
    foam = max(foam, clamp(extra, 0.0, 1.0) * smoothstep(0.25, 0.65, noise + 0.25 * extra));
    foam = max(foam, water_ripple_foam(rest) * smoothstep(0.2, 0.6, noise));
    // Trails and splash foam from the field: mottled, and solid where
    // thick. They are churned water, as white on a still pond as on the
    // sea, so the body's foam amount does not thin them.
    let trail = water_ripple_field(rest).w;
    let churned = trail * smoothstep(0.1, 0.5, noise + 0.5 * trail) * 0.9;
    return max(clamp(foam * water.bodies[b].scatter.w, 0.0, 1.0), clamp(churned, 0.0, 0.9));
}

// What one surface fragment adds and lets through: `emit` the radiance it
// adds (premultiplied, host units, before exposure), `transmit` the share
// of what lies behind it that comes through, per channel. `emit` includes
// the sky's reflection, `sky` times `reflect`, so a host with a sharper
// reflection (a planar mirror or a screen-space trace) can swap it out.
struct WaterShade {
    emit: vec3<f32>,
    transmit: vec3<f32>,
    sky: vec3<f32>,
    reflect: f32,
};

// The surface seen from below, from inside body `b`: Snell's window, with
// total internal reflection outside it mirroring the lit water below, and
// the water between the eye and the surface (`path`, m).
fn water_below(b: u32, n: vec3<f32>, v: vec3<f32>, path: f32, slope: f32) -> WaterShade {
    var o: WaterShade;
    let nd = -n;
    let ci = clamp(dot(v, nd), 0.0, 1.0);
    let st2 = WATER_IOR * WATER_IOR * (1.0 - ci * ci);
    var reflectance = 1.0;
    if st2 < 1.0 {
        reflectance = water_fresnel(sqrt(1.0 - st2));
    }
    let fog = water_inscatter(b) * 2.4;
    let shimmer = 1.1 + 1.6 * clamp(slope, 0.0, 0.6);
    let trans = exp(-water.bodies[b].absorb.rgb * path);
    o.emit = fog * shimmer * reflectance * trans + fog * (vec3<f32>(1.0) - trans);
    o.transmit = trans * (1.0 - reflectance);
    return o;
}

// Inputs to `water_shade` the host gathers.
struct WaterFragment {
    world: vec3<f32>,
    rest: vec2<f32>,
    // Depth at rest under the point, m.
    depth: f32,
    flow: vec2<f32>,
    foam: f32,
    crest: f32,
    shore: f32,
    body: u32,
    // Height over the level, m.
    height: f32,
    // Swell scale (0 for still water).
    scale: f32,
    // Toward the eye, unit.
    v: vec3<f32>,
    eye_distance: f32,
    // The pixel's footprint (`water_footprint`).
    footprint: f32,
    dpx: vec2<f32>,
    dpy: vec2<f32>,
    pixel: vec2<f32>,
    // How frozen or calmed the surface is, 0 to 1: slopes fade with it.
    calm: f32,
    // Extra slope and foam the host adds (spells).
    slope: vec2<f32>,
    extra_foam: f32,
    // 1 when the water tints what lies under it here; 0 when the host
    // tints the bed itself (the Water Lab's sea).
    column: f32,
    // The sun's disc, rad.
    disc: f32,
    // 1 when the host read its scene's depth behind the surface (Medium and
    // High): `path` is then the water the refracted view crosses to the
    // scene behind (m), and `contact` how far the straight view runs
    // through the water before it meets anything (m). 0 on Low, where the
    // baked depth stands in.
    screen: f32,
    path: f32,
    contact: f32,
};

// The normal at a surface fragment and the roughness its lost detail adds.
struct WaterNormal {
    n: vec3<f32>,
    roughness: f32,
    slope: f32,
    // The spectral sea's whitecaps, 0 to 1.
    whitecap: f32,
};

fn water_normal(s: WaterFragment) -> WaterNormal {
    var o: WaterNormal;
    var slope = vec2<f32>(0.0);
    var squeeze_y = 0.0;
    if s.scale > 0.0 {
        let swell = water_gerstner_slope(s.body, s.rest, s.depth, s.scale);
        slope = swell.xy;
        squeeze_y = swell.z;
    }
    var lost = 0.0;
    o.whitecap = 0.0;
    if s.scale > 0.0 && water_ocean_count(s.body) > 0 {
        let sea = water_ocean_detail(s.body, s.rest, s.depth, s.scale, s.footprint, s.dpx, s.dpy);
        slope += sea.xy;
        lost = sea.z;
        o.whitecap = sea.w * (1.0 - s.calm);
    }
    var detail = water_flow_detail(s.rest, s.flow, s.footprint, s.dpx, s.dpy);
    if water_ocean_count(s.body) > 0 {
        // The sea's ripples follow its wind (`water::ocean::slope_gains`).
        let rg = water.ocean[4].z;
        detail = vec3<f32>(detail.xy * rg, detail.z * rg * rg);
    }
    let field = water_ripple_field(s.rest);
    let rain = water_rain_slope(s.rest, water.params.x, water.look.z, s.footprint);
    slope = (slope + detail.xy + water_ripples(s.rest) + field.yz + rain + s.slope) * (1.0 - s.calm);
    o.n = normalize(vec3<f32>(-slope.x, 1.0 - squeeze_y * (1.0 - s.calm), -slope.y));
    let base = water.bodies[s.body].params.y;
    o.roughness = clamp(sqrt(base * base + detail.z + lost), 0.02, 0.6);
    o.slope = length(slope);
    return o;
}

// A surface fragment seen from above: Fresnel-weighted sky, the sun's
// glint, crest scattering, foam, and the water column's per-channel
// transmittance over the baked depth.
fn water_shade(s: WaterFragment, wn: WaterNormal) -> WaterShade {
    var o: WaterShade;
    let b = s.body;
    let n = wn.n;
    let v = s.v;
    let nov = max(dot(n, v), 1e-3);
    let fresnel = water_fresnel(nov);
    let level = water_sky_roughness(wn.roughness, s.footprint);
    let sky = water_host_sky(reflect(-v, n), level);
    let sun = water_host_sun();
    var glint = vec3<f32>(0.0);
    var crest = vec3<f32>(0.0);
    if sun.w > 0.0 {
        let l = sun.xyz;
        // The shading normal, not the triangle's: the shadow lookup's
        // offset must point out of the water whatever the screen's winding.
        let light = water_host_sun_light() * water_host_shadow(s.world, n, s.pixel);
        glint = water_glint(n, v, l, wn.roughness, s.disc, s.eye_distance) * light;
        if s.scale > 0.0 {
            crest = water_crest_scatter(b, v, l, n, s.height, s.crest) * light;
        }
    }
    let noise = water_foam_noise(s.rest, s.flow);
    var foam = max(water_foam(b, s.rest, s.shore, s.crest, s.foam, noise), clamp(s.extra_foam, 0.0, 1.0));
    if water_ocean_count(b) > 0 && s.scale > 0.0 {
        let amount = water.bodies[b].scatter.w;
        // Whitecaps: the foam field's dense patches, broken up by the
        // mottled cover. Its thin edges, where old foam has faded or a
        // coarse cascade's texels blur it, stay clear rather than veil
        // the sea.
        let core = smoothstep(WATER_WHITECAP_EDGE, WATER_WHITECAP_CORE, wn.whitecap);
        let caps = core * smoothstep(0.25, 0.6, noise + 0.25 * core);
        foam = max(foam, clamp(caps * amount, 0.0, 1.0));
        // Surf: where the shoaling waves reach their breaking height
        // (Medium and High), broken waves run up the shore as bands of
        // white water, each a bore moving at the shallow-water speed
        // √(g d), and the crests in the zone spill.
        if water.ocean[3].z > 0.5 {
            let hs = water.ocean[3].w;
            let d = max(s.depth, 0.0);
            let limit = 0.78 * d;
            let breaking = smoothstep(0.55, 1.1, hs / max(limit, 1e-3));
            let local = max(min(hs, limit), 0.02);
            let crest = smoothstep(-0.1 * local, 0.3 * local, s.height);
            // The bores of the peak's waves and of waves at about twice its
            // frequency, which break too.
            let phase = water.ocean[4].y * (water.params.x + s.shore / sqrt(9.81 * max(d, 0.2)));
            let first = smoothstep(0.35, 0.85, 0.5 + 0.5 * sin(phase + noise * 2.5));
            let second = smoothstep(0.45, 0.9, 0.5 + 0.5 * sin(phase * 2.13 + 1.7 + noise * 3.1));
            let bore = max(first, 0.75 * second);
            let surf = breaking * max(bore, crest) * smoothstep(0.2, 0.6, noise + 0.25 * breaking);
            foam = max(foam, clamp(surf * amount, 0.0, 1.0));
        }
    }
    if s.screen > 0.5 {
        // Only where something stands nearer than the bed: over a shallow
        // bed the view meets the bed itself soon after the surface, and
        // the shore's own lace covers that.
        let bed = max(s.depth, 0.02) / max(s.v.y, 0.1);
        let standing = 1.0 - smoothstep(0.35, 0.75, s.contact / bed);
        foam = max(foam, water_contact_foam(b, s.contact, noise) * standing);
    }
    let foam_rgb = vec3<f32>(0.9, 0.94, 0.95) * water_host_light(s.world, vec3<f32>(0.0, 1.0, 0.0), s.pixel);
    // The column under the surface: its in-scatter, and what of the bed
    // comes through, per channel.
    var body = vec3<f32>(0.0);
    var through = vec3<f32>(1.0);
    if s.column > 0.5 {
        let ci = clamp(dot(v, vec3<f32>(0.0, 1.0, 0.0)), 0.0, 1.0);
        through = water_transmittance(b, s.depth, ci);
        if s.screen > 0.5 {
            through = exp(-water.bodies[b].absorb.rgb * max(s.path, 0.0));
        }
        body = water_inscatter(b) * (vec3<f32>(1.0) - through);
    }
    let clear = sky * fresnel + glint + crest + body * (1.0 - fresnel);
    // Thin water at the very edge fades out instead of ending in a line;
    // the shore's foam runs on to the waterline itself.
    let edge = water_edge(s.depth);
    let cover = foam * smoothstep(-0.08, 0.0, s.depth);
    o.emit = clear * edge * (1.0 - cover) + foam_rgb * cover;
    let passed = mix(vec3<f32>(1.0), through * (1.0 - fresnel), edge);
    o.transmit = passed * (1.0 - cover);
    o.sky = sky;
    o.reflect = fresnel * edge * (1.0 - cover);
    return o;
}

// How much of the surface shows over water `depth` m deep at rest: thin
// water at the very edge fades out instead of ending in a line.
fn water_edge(depth: f32) -> f32 {
    return smoothstep(-0.05, 0.12, depth);
}

// The width of the foam where the surface meets what stands in it, m.
const WATER_CONTACT_BAND: f32 = 0.22;

// Foam where the surface meets what stands in it, `contact` m along the
// view from the surface to the scene behind it, broken by the mottled
// noise like the shore's lace.
fn water_contact_foam(b: u32, contact: f32, noise: f32) -> f32 {
    let near = 1.0 - smoothstep(0.0, WATER_CONTACT_BAND, contact);
    return near * smoothstep(0.3, 0.6, noise + 0.25 * near) * water.bodies[b].scatter.w;
}

// The direction a view ray toward the surface takes into water of index
// `ior` through normal `n` (`v` toward the eye); straight down the
// reflected ray where the view cannot enter.
fn water_refracted(v: vec3<f32>, n: vec3<f32>, ior: f32) -> vec3<f32> {
    let d = refract(-v, n, 1.0 / ior);
    if dot(d, d) < 1e-6 {
        return reflect(-v, n);
    }
    return d;
}
