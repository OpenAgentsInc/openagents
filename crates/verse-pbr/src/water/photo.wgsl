// ---------------------------------------------------------------------------
// The physical renderer's water pass (`water`, spliced after the shared
// `water.wgsl` into `pbr/photo.wgsl`): the shared surface shading through
// this renderer's sky cube, sun, shadows, exposure, and fog, plus what only
// this renderer draws so far: the Water Lab's spells on the sea, falling
// sheets, free orbs of water, and the light under the sea on every lit
// surface (Beer–Lambert extinction and in-scatter along the view and sun
// paths), and, since W7 (`water::under`), the view from under any body:
// the per-pixel split at the waterline, underwater fog, sun shafts, and
// caustics on every lit surface under a listed body, from the refracted
// light field's area ratio (Wallace, "Rendering Realtime Caustics in
// WebGL", 2016), attenuated by depth and blocked by the sun's shadow map
// (Guardado and Sánchez-Crespo, "Rendering Water Caustics", GPU Gems,
// chapter 2, 2004).

@group(2) @binding(3) var<uniform> water: WaterUniform;
@group(2) @binding(4) var water_tile: texture_2d<f32>;
@group(2) @binding(5) var water_tile_sampler: sampler;
@group(2) @binding(6) var water_waves: texture_2d_array<f32>;
// The ocean's streamed field (`water::field`): depth, shore distance, and
// current a texel, read where the clipmap places each vertex. The vertex
// stage alone reads it, so it takes none of the fragment stage's slots.
@group(2) @binding(7) var water_field: texture_2d<f32>;
@group(2) @binding(8) var water_field_sampler: sampler;
// The scene copies and the planar mirror (`water::screen`, Medium and
// High), the water pipelines' group 1; placeholders on Low, whose water
// never reads them.
@group(1) @binding(2) var water_scene: texture_2d<f32>;
@group(1) @binding(3) var water_scene_depth: texture_2d<f32>;
@group(1) @binding(4) var water_mirror: texture_2d<f32>;
@group(1) @binding(5) var water_screen_sampler: sampler;

// What `water_fragment` found in the copies for the fragment it shaded,
// for `fs_water_screen` to read after it.
var<private> water_screen_on: bool;
var<private> water_refraction: WaterRefraction;
var<private> water_shading_normal: vec3<f32>;
var<private> water_roughness: f32;

// ---- Hooks the shared water shading calls.

fn water_host_control() -> vec4<f32> {
    return f.water_control;
}

fn water_key_tint() -> vec3<f32> {
    return select(vec3<f32>(1.0), f.key_tint.rgb, f.key_tint.w > 0.5);
}

// The sky reflected along `dir`, at a blur of 0 (sharp) to 1 (roughest).
fn water_host_sky(dir: vec3<f32>, level: f32) -> vec3<f32> {
    let up = normalize(vec3<f32>(dir.x, max(dir.y, 0.015), dir.z));
    if f.sky_light.x > 0.5 {
        return textureSampleLevel(sky_cube, linear_clamp, up, level * f.sky_light.y).rgb;
    }
    if f.sky_zenith.w > 0.5 {
        return daylight_air(up);
    }
    return f.field.rgb;
}

fn water_host_sun() -> vec4<f32> {
    return vec4<f32>(f.sun.xyz, select(0.0, 1.0, f.sun.w > 0.0));
}

fn water_host_sun_light() -> vec3<f32> {
    return f.sun.w * water_key_tint();
}

// Diffuse light on a white surface facing `n`, such as foam.
fn water_host_light(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let sun = f.sun.w * max(dot(n, f.sun.xyz), 0.0) * water_key_tint() * sun_shadow(world, n, pixel);
    var sky = vec3<f32>(f.sun.w * 0.15);
    if f.sky_light.x > 0.5 {
        sky = sky_irradiance(n);
    }
    return (sun + sky) / PI;
}

fn water_host_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    return sun_shadow(world, vec3<f32>(0.0, 1.0, 0.0), pixel);
}

fn water_host_view_proj() -> mat4x4<f32> {
    return f.view_proj;
}

fn water_host_inv_view_proj() -> mat4x4<f32> {
    return f.inv_view_proj;
}

fn water_host_eye() -> vec3<f32> {
    return f.eye.xyz;
}

// ---- Spells on the water (`water::Controls`), read from the frame.

// How much of `p` lies in Part Water's trench, its walls sloping over
// `slope` m.
fn water_trench(p: vec2<f32>, slope: f32) -> f32 {
    let amount = f.water_part_size.z;
    if amount <= 0.0 {
        return 0.0;
    }
    let local = p - f.water_part.xy;
    let dir = f.water_part.zw;
    let along = abs(dot(local, dir));
    let across = abs(local.x * dir.y - local.y * dir.x);
    let ends = 1.0 - smoothstep(f.water_part_size.x - slope, f.water_part_size.x, along);
    let width = f.water_part_size.y * amount;
    let sides = 1.0 - smoothstep(width, width + slope * max(amount, 0.05), across);
    return ends * sides * min(amount, 1.0);
}

// How far the spells pull the surface down at `p` (`Controls::drop`).
fn water_drop(p: vec2<f32>, depth: f32) -> f32 {
    var drop = water_trench(p, 2.5) * (max(depth, 0.0) + 1.0);
    if f.water_whirl.w > 0.0 {
        let r = distance(p, f.water_whirl.xy) / max(f.water_whirl.z, 0.1);
        drop += f.water_whirl.w * 3.2 * exp(-(r * 2.2) * (r * 2.2));
    }
    return drop;
}

// The spells' current at `p` (`Controls::flow_at`), xz only.
fn water_spell_flow(p: vec2<f32>) -> vec2<f32> {
    var v = vec2<f32>(0.0);
    let radius = f.water_part_size.w;
    if radius > 0.0 {
        let r = distance(p, f.water_flow.xy);
        v += f.water_flow.zw * (1.0 - smoothstep(0.7 * radius, radius, r));
    }
    if f.water_whirl.w > 0.0 {
        let to = f.water_whirl.xy - p;
        let r = max(length(to), 0.3);
        let k = f.water_whirl.w * (1.0 - smoothstep(0.6 * f.water_whirl.z, 1.6 * f.water_whirl.z, r));
        v += (vec2<f32>(-to.y, to.x) / r * 3.2 + to / r * 0.9) * k;
    }
    return v;
}

// How frozen the water at `p` is (`Controls::ice_at`), with a ragged rim.
fn water_ice(p: vec2<f32>) -> f32 {
    if f.water_ice.w <= 0.0 {
        return 0.0;
    }
    let r = distance(p, f.water_ice.xy) + (value_noise(vec3<f32>(p * 0.35, 1.0)) - 0.5) * 2.5;
    return f.water_ice.w * (1.0 - smoothstep(f.water_ice.z - 1.5, f.water_ice.z, r));
}

// How wet the ground at `p` is, 0 to 1: the weather's wetness everywhere
// (`f.weather.x`, `water::rain`), and Create Water's rain on its disc.
fn water_wetness(p: vec2<f32>) -> f32 {
    var wet = f.weather.x;
    if f.water_wet.w > 0.0 {
        let r = distance(p, f.water_wet.xy);
        wet = max(wet, f.water_wet.w * (1.0 - smoothstep(f.water_wet.z - 1.0, f.water_wet.z + 0.5, r)));
    }
    return wet;
}

// How wet a character's body at `world` is, 0 to 1: one who swam or stood
// in the rain darkens for a while (`water::rain::Rain::figure`), within
// 0.6 m of its feet's axis and 2.1 m above them.
fn water_figure_wetness(world: vec3<f32>) -> f32 {
    let fig = f.weather_figure;
    if fig.w <= 0.0 {
        return 0.0;
    }
    let h = world.y - fig.y;
    if h < 0.03 || h > 2.1 {
        return 0.0;
    }
    return fig.w * (1.0 - smoothstep(0.45, 0.65, distance(world.xz, fig.xz)));
}

// ---- Light under the water (read by every lit fragment through `shade`,
// and by the sky; `water::under`, phase W7).

// Light scattered back toward the eye from inside the water: the frame's
// in-scatter color times the sun's and the sky's light falling on it.
fn water_sea_inscatter() -> vec3<f32> {
    let sun = f.sun.w * max(f.sun.y, 0.0) * water_key_tint();
    var sky = vec3<f32>(f.sun.w * 0.15);
    if f.sky_light.x > 0.5 {
        sky = sky_irradiance(vec3<f32>(0.0, 1.0, 0.0));
    }
    return f.water_scatter.rgb * (sun + sky) / PI;
}

// How far the near-plane point behind normalized device coordinates
// `ndc` stands over the water's surface, m (`water::under::line`): the
// pixel looks out from under the water where it is negative. Positive
// everywhere when the view has no waterline.
fn water_line_at(ndc: vec2<f32>) -> f32 {
    if f.water_line.w < 0.5 {
        return 1.0;
    }
    return f.water_line.x + f.water_line.y * ndc.x + f.water_line.z * ndc.y;
}

// Whether the pixel at framebuffer position `pixel` looks out from under
// the water: the per-pixel split where the near plane straddles the
// surface.
fn water_under_pixel(pixel: vec2<f32>) -> bool {
    let uv = pixel * f.viewport.zw;
    return water_line_at(vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0)) < 0.0;
}

// Whether the eye sees `world` from under the water: through the pixel it
// projects to.
fn water_under_world(world: vec3<f32>) -> bool {
    if f.water_line.w < 0.5 {
        return false;
    }
    let c = f.view_proj * vec4<f32>(world, 1.0);
    return water_line_at(c.xy / max(c.w, 1e-5)) < 0.0;
}

// How far above the eye body's level a point seen from under the water
// still counts as in it: a crest's height. Above that the surface, seen
// from below, carries the water up to it.
const WATER_THROUGH_MARGIN: f32 = 0.3;

// Whether the eye sees `world` through water alone: from under the
// surface, at a point under it.
fn water_through(world: vec3<f32>) -> bool {
    return water_under_world(world) && world.y < f.water_eye.w + WATER_THROUGH_MARGIN;
}

// The sea's depth over a lit point, m: zero in Part Water's trench and
// under the whirlpool's eye, where the water has drawn away.
fn water_depth(world: vec3<f32>) -> f32 {
    let depth = f.water.x - world.y;
    if depth <= 0.0 {
        return depth;
    }
    let open = 1.0 - water_trench(world.xz, 0.6);
    return depth * open - water_drop(world.xz, depth) * (1.0 - open);
}

// What lies over a lit point: the first body in the list (`water::under`)
// whose bounds hold it and whose plane stands over it by more than its
// residual.
struct WaterBed {
    found: bool,
    depth: f32,
    residual: f32,
    extinction: vec3<f32>,
    strength: f32,
};

fn water_bed(world: vec3<f32>) -> WaterBed {
    var o: WaterBed;
    o.found = false;
    o.depth = 0.0;
    o.residual = 0.0;
    o.extinction = vec3<f32>(0.0);
    o.strength = 0.0;
    let n = i32(f.water_under.x);
    for (var i = 0; i < 8; i++) {
        if i >= n {
            break;
        }
        let b = f.water_list[i * 3];
        if world.x < b.x || world.z < b.y || world.x > b.z || world.z > b.w {
            continue;
        }
        let plane = f.water_list[i * 3 + 1];
        var depth = plane.x + plane.y * world.x + plane.z * world.z - world.y;
        var residual = plane.w;
        if plane.w < -0.5 {
            // The sea, with its spells.
            depth = water_depth(world);
            residual = 0.0;
        }
        if depth <= residual {
            continue;
        }
        let optics = f.water_list[i * 3 + 2];
        o.found = true;
        o.depth = depth;
        o.residual = residual;
        o.extinction = optics.rgb;
        o.strength = optics.w;
        return o;
    }
    return o;
}

// The sun's light under a level surface, travelling down: refracted into
// the water by Snell's law.
fn water_sun_below() -> vec3<f32> {
    let d = refract(-f.sun.xyz, vec3<f32>(0.0, 1.0, 0.0), 1.0 / WATER_IOR);
    if dot(d, d) < 1e-6 || d.y > -0.05 {
        return vec3<f32>(0.0, -1.0, 0.0);
    }
    return normalize(d);
}

// Refraction's lever over water `depth` deep: a slope g at the surface
// moves the light landing below by about (1 − 1/n)·depth·g, capped at
// `water::under::LEVER_DEPTH`.
fn water_lever(depth: f32) -> f32 {
    return (1.0 - 1.0 / WATER_IOR) * min(depth, 3.5);
}

// The refracted light field's focusing for one layer of the caustic waves
// (every `layers`th wave from `layer`), entering at `p` with lever
// `lever`: the area the light left over the area it lands on, the
// Jacobian of the landing point from the waves' curvature (Wallace 2016,
// in closed form). `water::under::focus` is its CPU mirror.
fn water_focus(p: vec2<f32>, lever: f32, layer: i32, layers: i32, most: i32) -> f32 {
    let n = min(i32(f.water_under.y), most);
    let t = f.water_scatter.w;
    var hxx = 0.0;
    var hzz = 0.0;
    var hxz = 0.0;
    var variance = 0.0;
    for (var i = 0; i < 8; i++) {
        if i >= n {
            break;
        }
        if i % layers != layer {
            continue;
        }
        let a = f.water_caustic[i * 2];
        let w = f.water_caustic[i * 2 + 1];
        let k = a.z;
        let theta = k * dot(a.xy, p) - w.x * t + w.y;
        let curvature = a.w * k * k;
        let m = -curvature * sin(theta);
        hxx += m * a.x * a.x;
        hzz += m * a.y * a.y;
        hxz += m * a.x * a.y;
        variance += 0.5 * curvature * curvature;
    }
    let det = (1.0 + lever * hxx) * (1.0 + lever * hzz) - lever * lever * hxz * hxz;
    // Read at the light's source, the ratio averages 1 + lever²·E[tr H²];
    // dividing that out keeps the bed's mean light.
    return min(1.0 / max(abs(det), 0.12), 6.0) / (1.0 + lever * lever * variance);
}

// The caustics on a point `depth` m under the surface, past `residual`:
// one where the light arrives evenly, more where the surface's curvature
// gathers it into lines, less between them. On High a second layer of
// waves multiplies in. They fade where the water thins at the edge and,
// slowly, with depth, where the light field folds too often to show.
fn water_caustic(world: vec3<f32>, depth: f32, residual: f32, strength: f32, footprint: f32) -> f32 {
    if strength <= 0.0 || f.sun.w <= 0.0 || f.water_under.y < 0.5 {
        return 1.0;
    }
    let d = water_sun_below();
    let p = world.xz - d.xz * (depth / max(-d.y, 0.2));
    let lever = water_lever(depth);
    let layers = max(i32(f.water_under.z), 1);
    var focus = water_focus(p, lever, 0, layers, 8);
    if layers > 1 {
        focus = min(focus * water_focus(p, lever, 1, layers, 8), 8.0);
    }
    // A pixel wider than a fraction of the shortest caustic wave would
    // alias the lines into moiré; they fade to their mean instead.
    let resolved = 1.0 - smoothstep(0.03, 0.12, footprint);
    let fade = smoothstep(residual, residual + 0.3, depth) * exp(-depth * 0.12) * resolved;
    return mix(1.0, focus, clamp(strength, 0.0, 1.0) * fade);
}

// The sunlight reaching a lit point under any listed body, as a factor:
// extinction along the refracted sun's path down from the surface, times
// the caustics. The sun's shadow map then blocks it as it does in air.
// `footprint` is the pixel's width on the surface, m.
fn water_sun(world: vec3<f32>, footprint: f32) -> vec3<f32> {
    let bed = water_bed(world);
    if !bed.found {
        return vec3<f32>(1.0);
    }
    let mu = max(-water_sun_below().y, 0.2);
    return exp(-bed.extinction * bed.depth / mu) * water_caustic(world, bed.depth, bed.residual, bed.strength, footprint);
}

// The sky's light reaching a point under any listed body, as a factor.
fn water_ambient(world: vec3<f32>) -> vec3<f32> {
    let bed = water_bed(world);
    if !bed.found {
        return vec3<f32>(1.0);
    }
    return exp(-bed.extinction * bed.depth * 1.4);
}

// Henyey–Greenstein's phase function for cosine `c` and asymmetry `g`,
// over the isotropic phase, so one is isotropic.
fn water_phase(c: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / pow(max(1.0 + g2 - 2.0 * g * c, 1e-3), 1.5);
}

// How far the shafts march along a view under the water, m.
const WATER_SHAFT_REACH: f32 = 24.0;
// The shafts' strength over the single-scattered light they stand for:
// the eye adapts to the dim water, so their contrast reads stronger than
// the bare in-scatter suggests.
const WATER_SHAFT_GAIN: f32 = 16.0;
// The longest caustic waves the shafts take: they make broad beams, and
// the shorter ones only add noise a sample's step apart.
const WATER_SHAFT_WAVES: i32 = 3;

// Sun shafts under the water along the view `dir` (unit, from the eye)
// over `reach` m: the light the water scatters toward the eye at a few
// jittered samples, each lit by the refracted sun through the water over
// it and focused or thinned by the caustics there. Focusing grows with
// depth along each refracted ray, so the light lies in streaks along the
// sun's direction. Only the caustics' departure from even light is
// added: the fog already holds the mean. The samples sit at fixed
// fractions of the path: a per-pixel jitter would show as a fine
// crosshatch at so few samples.
fn water_shafts(dir: vec3<f32>, reach: f32) -> vec3<f32> {
    let n = i32(f.water_under.w);
    if n <= 0 || f.sun.w <= 0.0 || f.water_under.y < 0.5 {
        return vec3<f32>(0.0);
    }
    let level = f.water_eye.w;
    let d = water_sun_below();
    let mu = max(-d.y, 0.2);
    let ext = f.water_extinction.rgb;
    let span = min(reach, WATER_SHAFT_REACH);
    let step = span / f32(n);
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 16; i++) {
        if i >= n {
            break;
        }
        let s = (f32(i) + 0.5) * step;
        let x = f.eye.xyz + dir * s;
        let depth = level - x.y;
        if depth <= 0.05 {
            continue;
        }
        let p = x.xz - d.xz * (depth / mu);
        let focus = water_focus(p, water_lever(depth), 0, 1, WATER_SHAFT_WAVES);
        sum += (focus - 1.0) * exp(-ext * (s + depth / mu)) * step;
    }
    let sun = f.sun.w * max(f.sun.y, 0.0) * water_key_tint() / PI;
    let phase = min(water_phase(dot(d, -dir), 0.55), 6.0);
    return f.water_scatter.rgb * sun * ext * sum * phase * WATER_SHAFT_GAIN;
}

// A lit point's radiance as the eye sees it through the water. From under
// the surface: extinction and in-scatter along the view to the point, and
// the sun's shafts; a point above the surface is left to the surface seen
// from below, which carries the water up to it. From above the sea: along
// the refracted ray from the point up to the surface; other bodies' own
// surfaces carry their water from above.
fn water_view(world: vec3<f32>, radiance: vec3<f32>) -> vec3<f32> {
    if water_under_world(world) {
        if world.y >= f.water_eye.w + WATER_THROUGH_MARGIN && f.eye.y < f.water_eye.w {
            return radiance;
        }
        let ray = world - f.eye.xyz;
        let dist = length(ray);
        let t = exp(-f.water_extinction.rgb * dist);
        let shafts = water_shafts(ray / max(dist, 1e-4), dist);
        return radiance * t + water_fog() * (1.0 - t) + shafts;
    }
    if f.water.y < 0.5 {
        return radiance;
    }
    let depth = water_depth(world);
    if depth <= 0.0 {
        return radiance;
    }
    let v = normalize(f.eye.xyz - world);
    let ci = clamp(v.y, 0.0, 1.0);
    let path = depth / max(water_cos_refracted(ci), 0.05);
    let t = exp(-f.water_extinction.rgb * path);
    return radiance * t + water_sea_inscatter() * (1.0 - t);
}

// The water's color around an eye under the surface: the in-scatter of
// the whole water column around it, brighter than the deep color seen
// from above, which is dimmed by the column over the bed.
fn water_fog() -> vec3<f32> {
    return water_sea_inscatter() * 2.4;
}

// ---- The surface.

// A `water::Kind` code's swell scale: the sea's (0 to 1), a body's
// (2 plus up to 0.98), or none.
fn water_kind_scale(kind: f32) -> f32 {
    if kind < 1.5 {
        return clamp(kind, 0.0, 1.0);
    }
    if kind < 2.99 {
        return clamp((kind - 2.0) / 0.98, 0.0, 1.0);
    }
    return 0.0;
}

// ---- The ocean's clipmap (`water::clipmap`, phase W10).

// The share of a level's half width over which it morphs into the next
// (`water::clipmap::MORPH`).
const WATER_CLIP_MORPH: f32 = 0.25;
// Rings of the apron (`water::clipmap::APRON_RINGS`).
const WATER_CLIP_APRON: f32 = 8.0;
// Texels along a page of the field (`water::field::PAGE`).
const WATER_FIELD_PAGE: f32 = 64.0;

// Where clipmap vertex `v` lies at rest (x, z): its level's snapped center
// plus its grid coordinates, an odd coordinate sliding onto its even
// neighbor as the eye's distance grows toward the rim (Losasso and Hoppe's
// transition regions, with Strugar's vertex morph), and the apron's outer
// rings stretched out geometrically to the clipmap's reach.
// `clipmap::position` is its CPU mirror.
fn water_clip_rest(v: WaterIn) -> vec2<f32> {
    let level = min(u32(max(v.pos.y, 0.0) + 0.5), 4u);
    let row = water.clip[level];
    let half = water.clip[5].y;
    let s = row.z;
    let g = v.pos.xz;
    let d = abs(row.xy + g * s - f.eye.xz);
    let alpha = clamp((max(d.x, d.y) - row.w) / (WATER_CLIP_MORPH * half * s), 0.0, 1.0);
    let odd = g - 2.0 * floor(g * 0.5);
    let morphed = g - odd * alpha;
    var reach = 1.0;
    if v.depth > 0.5 {
        reach = pow(water.clip[5].z / (half * s), v.depth / WATER_CLIP_APRON);
    }
    return row.xy + morphed * s * reach;
}

// The streamed field at `p` (x, z): depth, shore distance, and current;
// the field's outside values where its page is not in the atlas.
fn water_field_at(p: vec2<f32>) -> vec4<f32> {
    let outside = water.field[2];
    if water.field[1].w < 0.5 {
        return outside;
    }
    let window = water.field[0].w;
    let local = (p - water.field[0].xy) / water.field[0].z;
    let page = floor(local / WATER_FIELD_PAGE);
    if page.x < 0.0 || page.y < 0.0 || page.x >= water.field[1].x || page.y >= water.field[1].y {
        return outside;
    }
    let slot2 = page - window * floor(page / window);
    let slot = min(u32(slot2.x + slot2.y * window + 0.5), 63u);
    if abs(water.field_pages[slot / 4u][slot % 4u] - (page.x * 4096.0 + page.y)) > 0.5 {
        return outside;
    }
    return textureSampleLevel(water_field, water_field_sampler, local / (WATER_FIELD_PAGE * window), 0.0);
}

@vertex
fn vs_water(v_in: WaterIn) -> WaterOut {
    var v = v_in;
    var spacing = 0.0;
    if v.kind > 5.5 {
        // A clipmap vertex: placed around the eye, given the field's depth,
        // shore distance, and current there, then moved as the sea or a
        // body at full swell.
        let row = water.clip[min(u32(max(v.pos.y, 0.0) + 0.5), 4u)];
        let distance = abs(row.xy + v.pos.xz * row.z - f.eye.xz);
        let alpha = clamp((max(distance.x, distance.y) - row.w)
            / (WATER_CLIP_MORPH * water.clip[5].y * row.z), 0.0, 1.0);
        spacing = row.z * (1.0 + alpha);
        if v.depth > 0.5 {
            spacing *= pow(water.clip[5].z / (water.clip[5].y * row.z), v.depth / WATER_CLIP_APRON);
        }
        let rest = water_clip_rest(v);
        let field = water_field_at(rest);
        let level = water.bodies[water_body_index(v.body)].rest.x;
        v.pos = vec3<f32>(rest.x, level, rest.y);
        v.depth = field.x;
        v.shore = field.y;
        v.flow = field.zw;
        v.foam = 0.0;
        v.kind = select(2.98, 1.0, water.clip[5].w > 0.5);
    }
    let b = water_body_index(v.body);
    var moved: WaterMoved;
    moved.world = v.pos;
    moved.crest = 0.0;
    var depth = v.depth;
    var rest = v.pos.xz;
    if v.kind < 1.5 {
        // The sea: a flood lifts it, ice stills it, spells pull it down.
        let rise = water.bodies[b].absorb.w - water.bodies[b].rest.x;
        depth += rise;
        let calm = 1.0 - water_ice(v.pos.xz);
        moved = water_move_spaced(v, water_kind_scale(v.kind) * calm, rise, spacing);
        moved.world.y -= water_drop(v.pos.xz, depth);
    } else if v.kind > 3.5 {
        // An orb: its center rides in the depth and flow channels, and the
        // vertex is its offset at rest, moved by the wobble.
        let center = vec3<f32>(v.flow.x, v.depth, v.flow.y);
        let r = length(v.pos);
        let d = v.pos / max(r, 1e-5);
        moved.world = center + d * r * orb_shape(d, water.params.x, v.kind - 4.0);
        moved.crest = r;
        rest = moved.world.xz;
    } else if v.kind < 2.99 {
        moved = water_move_spaced(v, water_kind_scale(v.kind), 0.0, spacing);
    }
    return water_out(v, moved, rest, depth, f.view_proj * vec4<f32>(moved.world, 1.0));
}

// What a fragment of free or falling water adds and lets through, from its
// straight color `rgb` (premultiplied) covering `alpha`.
fn water_cover(rgb: vec3<f32>, alpha: f32) -> WaterShade {
    var o: WaterShade;
    let a = clamp(alpha, 0.0, 1.0);
    o.emit = rgb;
    o.transmit = vec3<f32>(1.0 - a);
    return o;
}

// The shading of one water fragment, before exposure and fog. Called first
// thing in both entry points, so its derivatives stay in uniform control
// flow.
fn water_fragment(i: WaterOut) -> WaterShade {
    if water_in_hull(i.world_depth.xyz) {
        discard;
    }
    let world = i.world_depth.xyz;
    let rest = i.rest_flow.xy;
    let geometric = normalize(cross(dpdx(world), dpdy(world)));
    let dpx = dpdx(rest);
    let dpy = dpdy(rest);
    let t = water.params.x;
    let pixel = i.clip.xy;
    let v = normalize(f.eye.xyz - world);
    let kind = i.look.y;
    let b = water_body_index(i.extra.x);
    let uv0 = pixel * f.viewport.zw;
    water_refraction.uv_r = uv0;
    water_refraction.uv_g = uv0;
    water_refraction.uv_b = uv0;
    water_refraction.found = 0.0;
    water_shading_normal = vec3<f32>(0.0, 1.0, 0.0);
    water_roughness = 1.0;

    if kind > 3.5 {
        return water_orb(i, v, pixel, t);
    }
    if kind > 2.99 {
        // A falling sheet: aerated white water streaked along its fall,
        // thinner where it tears.
        let heading = normalize(i.rest_flow.zw + vec2<f32>(1e-4, 0.0));
        let across = dot(rest, vec2<f32>(-heading.y, heading.x));
        let streak = value_noise(vec3<f32>(across * 2.6, world.y * 0.35 + t * 2.2, 0.0));
        let fine = value_noise(vec3<f32>(across * 9.0, world.y * 1.4 + t * 5.5, 3.0));
        let body = clamp(0.45 + 0.6 * streak + 0.3 * fine - 0.25, 0.0, 1.0);
        var n = geometric;
        if dot(n, v) < 0.0 {
            n = -n;
        }
        let light = water_host_light(world, n, pixel);
        let white = vec3<f32>(0.86, 0.93, 0.96) * light;
        let blue = water_inscatter(b) * 3.0;
        let fresnel = water_fresnel(max(dot(n, v), 0.0));
        let refl = water_host_sky(reflect(-v, n), 0.45) * fresnel;
        let alpha = clamp(0.35 + 0.6 * body + i.look.x * 0.3, 0.0, 0.97);
        let rgb = mix(blue, white, 0.35 + 0.65 * body) * alpha + refl * (1.0 - alpha);
        return water_cover(rgb, alpha);
    }

    let sea = kind < 1.5;
    var s: WaterFragment;
    s.world = world;
    s.rest = rest;
    s.depth = i.world_depth.w;
    s.flow = i.rest_flow.zw;
    s.foam = i.look.x;
    s.crest = i.look.z;
    s.shore = i.look.w;
    s.body = b;
    s.height = i.extra.y;
    s.scale = water_kind_scale(kind);
    s.v = v;
    s.eye_distance = distance(f.eye.xyz, world);
    let fp = water_footprint(v, s.eye_distance, dpx, dpy);
    s.footprint = fp.size;
    s.dpx = fp.dpx;
    s.dpy = fp.dpy;
    s.pixel = pixel;
    s.calm = 0.0;
    s.slope = vec2<f32>(0.0);
    s.extra_foam = 0.0;
    s.column = select(1.0, 0.0, sea);
    s.disc = f.sun_disc.x;
    if sea {
        s.flow += water_spell_flow(rest);
        s.calm = water_ice(rest);
        // The water over the bed here, with the swell's own height, so the
        // foam runs up the beach with each wave and back.
        let noise = water_foam_noise(rest, s.flow);
        let wet = s.depth + s.height;
        let shore = 1.0 - smoothstep(0.0, 0.32, wet);
        let swash = 0.5 + 0.5 * sin(wet * 14.0 - t * 1.4 + noise * 3.0);
        var foam = shore * smoothstep(0.72, 0.95, swash * 0.6 + noise * 0.5);
        // White water on a trench's walls and in a whirlpool's spiral.
        let wall = water_trench(rest, 2.5) - water_trench(rest, 0.4);
        foam = max(foam, clamp(wall * 2.0, 0.0, 1.0) * smoothstep(0.2, 0.5, noise));
        if f.water_whirl.w > 0.0 {
            let to = rest - f.water_whirl.xy;
            let r = length(to) / max(f.water_whirl.z, 0.1);
            let arms = sin(atan2(to.y, to.x) * 3.0 + r * 9.0 - t * 4.0);
            foam = max(foam, f.water_whirl.w * smoothstep(0.3, 0.9, arms * 0.5 + 0.5) * smoothstep(1.6, 0.4, r) * smoothstep(0.3, 0.7, noise + 0.3));
        }
        s.extra_foam = foam * water.bodies[b].scatter.w;
    }
    let wn = water_normal(s);
    water_shading_normal = wn.n;
    water_roughness = wn.roughness;

    // The eye under this body's surface. The body the eye is in or at
    // splits the view per pixel where the near plane straddles it
    // (`water::under`); another body is seen from below only when the
    // eye is lower than the fragment and the zone marks it inside. A body
    // the eye is beside rather than in is too thin to hide anything from
    // below.
    let own = f.water_line.w > 0.5 && i32(f.water_eye.x) == i32(b);
    let below = select(f.eye.y < world.y, water_under_pixel(pixel), own);
    let inside = own || water.bodies[b].params.w > 0.5;
    if below {
        if inside {
            let path = distance(f.eye.xyz, world);
            var o = water_below(b, wn.n, v, path, wn.slope);
            o.emit += water_shafts(-v, path);
            if water_screen_on {
                // Medium and High bend the copy through the waves as the
                // view leaves the water (W5 left it straight).
                let shift = water_refract_up(world, v, wn.n, WATER_REFRACT_REACH);
                let uv = clamp(uv0 + shift, vec2<f32>(0.0), vec2<f32>(1.0));
                water_refraction.uv_r = uv;
                water_refraction.uv_g = uv;
                water_refraction.uv_b = uv;
                water_refraction.found = 1.0;
            }
            return o;
        }
        var none: WaterShade;
        none.emit = vec3<f32>(0.0);
        none.transmit = vec3<f32>(1.0);
        return none;
    }
    if water_screen_on {
        // What lies behind, from the copies: the refracted view's end, the
        // water it crosses, and how near the scene stands to the surface.
        let r = water_screen_refraction(world, v, wn.n, water_view_w(world), uv0, water_edge(s.depth), f.water_screen.z);
        water_refraction = r;
        s.screen = r.found;
        s.path = r.path;
        s.contact = r.contact;
    }
    var o = water_shade(s, wn);
    if s.calm > 0.01 {
        // Ice: frosted, cracked, and nearly opaque, over the water's own
        // look where it thins at the rim.
        let ice = s.calm;
        let cracks = value_noise(vec3<f32>(rest * 1.8, 7.0));
        let line = 1.0 - smoothstep(0.0, 0.05, abs(cracks - 0.5));
        let frost = 0.55 + 0.25 * value_noise(vec3<f32>(rest * 6.0, 2.0)) - 0.25 * line;
        let light = water_host_light(world, vec3<f32>(0.0, 1.0, 0.0), pixel);
        let sky = water_host_sky(vec3<f32>(0.0, 1.0, 0.0), 0.6);
        let ice_rgb = vec3<f32>(0.62, 0.78, 0.88) * frost * light + sky * 0.08;
        o.emit = mix(o.emit, ice_rgb, ice);
        o.transmit = mix(o.transmit, vec3<f32>(0.06), ice);
        o.reflect *= 1.0 - ice;
    }
    return o;
}

// How far the view leaving the water upward through normal `n` (the
// surface's, facing up) moves on the screen from the view through a level
// surface, each carried `reach` m into the air: the waves' bending of
// what Snell's window shows. Through total internal reflection the ray
// turns back down, and the shift follows it.
fn water_refract_up(world: vec3<f32>, v: vec3<f32>, n: vec3<f32>, reach: f32) -> vec2<f32> {
    let into_air = 1.0 / WATER_IOR;
    let bent = water_project(world + water_refracted(v, -n, into_air) * reach).xy;
    let level = water_project(world + water_refracted(v, vec3<f32>(0.0, -1.0, 0.0), into_air) * reach).xy;
    return bent - level;
}

// The emitted half of the pass, added over the scene after
// `fs_water_transmit` has dimmed it: exposed, and fogged so that the sum
// of both halves is the fogged surface.
@fragment
fn fs_water(i: WaterOut) -> @location(0) vec4<f32> {
    let shade = water_fragment(i);
    let world = i.world_depth.xyz;
    let fogged = neon_fog(expose(shade.emit), world, 1.0) - neon_fog(vec3<f32>(0.0), world, 1.0) * shade.transmit;
    return vec4<f32>(max(fogged, vec3<f32>(0.0)), 0.0);
}

// The transmitted half: what of the scene behind comes through, per
// channel, multiplied into it.
@fragment
fn fs_water_transmit(i: WaterOut) -> @location(0) vec4<f32> {
    let shade = water_fragment(i);
    return vec4<f32>(clamp(shade.transmit, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// Medium and High (`water::screen`): the whole surface in one draw over
// the scene, what lies behind read from the color copy along the refracted
// view, and the sky's reflection replaced by the planar mirror on the
// mirrored body or, on High, a screen-space reflection where its march
// hits. The copies are already exposed and fogged; the mirror and the
// march carry the fog of the whole reflected path, so only the surface's
// own light is fogged here. Without this frame's copies it shades as the
// two halves would over the copy.
@fragment
fn fs_water_screen(i: WaterOut) -> @location(0) vec4<f32> {
    water_screen_on = f.water_screen.x > 0.5;
    let shade = water_fragment(i);
    let world = i.world_depth.xyz;
    let v = normalize(f.eye.xyz - world);
    let n = water_shading_normal;
    let uv0 = i.clip.xy * f.viewport.zw;
    let b = water_body_index(i.extra.x);
    let sharp = 1.0 - smoothstep(0.08, 0.35, water_roughness);
    var reflected = vec3<f32>(0.0);
    var weight = 0.0;
    let level = water.bodies[b].absorb.w;
    let mirrored = f.water_mirror.x > 0.5 && u32(f.water_mirror.z + 0.5) == b && abs(level - f.water_mirror.y) < 0.02;
    if water_screen_on && shade.reflect > 0.0 && f.eye.y > world.y {
        if mirrored {
            reflected = water_mirror_at(world, v, n, uv0);
            weight = sharp;
        } else if f.water_screen.y > 0.5 {
            let hit = water_ssr(world, reflect(-v, water_reflect_normal(n)), i32(f.water_screen.y), i.clip.xy);
            if hit.weight > 0.0 {
                reflected = textureSampleLevel(water_scene, water_screen_sampler, hit.uv, 0.0).rgb;
                weight = hit.weight * sharp;
            }
        }
    }
    // Where nothing of the surface shows (dry ground the patch runs over),
    // leave the scene and its depth alone.
    if all(shade.transmit >= vec3<f32>(0.999)) && all(abs(shade.emit) <= vec3<f32>(1e-6)) {
        discard;
    }
    let swapped = shade.reflect * weight;
    let own = shade.emit - shade.sky * swapped;
    let behind = water_scene_through(water_refraction);
    let air = neon_fog(vec3<f32>(0.0), world, 1.0);
    let transmit = clamp(shade.transmit, vec3<f32>(0.0), vec3<f32>(1.0));
    let lit = neon_fog(expose(max(own, vec3<f32>(0.0))), world, 1.0) - air * (transmit + swapped);
    let rgb = behind * transmit + reflected * swapped + lit;
    return vec4<f32>(max(rgb, vec3<f32>(0.0)), 1.0);
}

// ---- Orbs (`water::Kind::Orb`): free bodies of water, such as the Water
// Lab's Water Orb and the streams that feed it. Each is a closed surface
// around a center, wobbling by `orb_shape`. Seen from outside it is a ball
// lens: Fresnel-weighted sky and the sun's glint on the skin, Beer–Lambert
// extinction and in-scatter along the refracted ray's chord, the world
// behind it turned upside down through the two refractions, and caustics
// gathering on the side away from the sun. Lightning makes it glow and
// crackle with arcs (`foam` carries the charge).

// The surface's radius, as a multiple of the rest radius, along unit
// direction `d` at time `t`.
fn orb_shape(d: vec3<f32>, t: f32, wobble: f32) -> f32 {
    let a = sin(2.1 * d.x + 1.3 * t) * cos(1.9 * d.z - 1.1 * t);
    let b = sin(3.2 * d.y + 2.0 * t + 1.7 * d.x);
    let c = sin(6.5 * (d.x + d.z) - 3.6 * t + 2.0 * d.y);
    let amp = 0.025 + 0.14 * clamp(wobble, 0.0, 1.0);
    return 1.0 + amp * (0.55 * a + 0.3 * b + 0.15 * c);
}

// What a ray leaving the orb along `d` sees: the sky above the horizon
// and lit sand below it.
fn orb_environment(d: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let sky = water_host_sky(d, 0.4);
    let ground = vec3<f32>(0.46, 0.4, 0.3) * water_host_light(world, vec3<f32>(0.0, 1.0, 0.0), pixel);
    return mix(ground, sky, smoothstep(-0.08, 0.06, d.y));
}

fn water_orb(i: WaterOut, v: vec3<f32>, pixel: vec2<f32>, t: f32) -> WaterShade {
    let world = i.world_depth.xyz;
    let center = vec3<f32>(i.rest_flow.z, i.world_depth.w, i.rest_flow.w);
    let r = max(i.look.z, 0.01);
    let wobble = i.look.y - 4.0;
    let charge = clamp(i.look.x, 0.0, 1.0);
    let b = water_body_index(i.extra.x);
    let rel = world - center;
    let d = normalize(rel);
    // Only the near face draws, or the far face when the eye is inside.
    let inside = distance(f.eye.xyz, center) < r;
    let facing = dot(rel, f.eye.xyz - world) > 0.0;
    if facing == inside {
        return water_cover(vec3<f32>(0.0), 0.0);
    }
    // The wobbling surface's normal, from its shape a step to either side.
    var ta = cross(d, vec3<f32>(0.0, 1.0, 0.0));
    if dot(ta, ta) < 1e-4 {
        ta = cross(d, vec3<f32>(1.0, 0.0, 0.0));
    }
    ta = normalize(ta);
    let tb = cross(d, ta);
    let e = 0.03;
    let p0 = d * orb_shape(d, t, wobble);
    let da = normalize(d + ta * e);
    let db = normalize(d + tb * e);
    var n = normalize(cross(da * orb_shape(da, t, wobble) - p0, db * orb_shape(db, t, wobble) - p0));
    if dot(n, d) < 0.0 {
        n = -n;
    }
    // Fine ripples running over the skin.
    let q = d * r;
    let ripple = vec3<f32>(
        sin(q.y * 5.0 + t * 3.1) + sin(q.z * 7.3 - t * 2.3),
        sin(q.z * 6.1 + t * 2.7) + sin(q.x * 5.7 + t * 3.3),
        sin(q.x * 6.7 - t * 2.9) + sin(q.y * 7.1 + t * 2.1)
    ) * (0.025 + 0.04 * wobble);
    n = normalize(n + ripple - d * dot(ripple, d));
    if inside {
        n = -n;
    }

    let nov = max(dot(n, v), 1e-3);
    let fresnel = water_fresnel(nov);
    let roughness = clamp(water.bodies[b].params.y, 0.02, 0.3);
    let sky = water_host_sky(reflect(-v, n), sqrt(roughness));
    let l = f.sun.xyz;
    var glint = vec3<f32>(0.0);
    if f.sun.w > 0.0 {
        glint = water_glint(n, v, l, roughness, f.sun_disc.x, 0.0) * water_host_sun_light();
    }
    let sunlit = water_host_light(world, l, pixel);
    let extinction = water.bodies[b].absorb.rgb;

    var rgb = vec3<f32>(0.0);
    var alpha = 0.0;
    if inside {
        // From within: the water all around, and the sky through the skin.
        let fog = water_inscatter(b) * 2.4;
        rgb = fog * 0.6 + sky * fresnel * 0.4;
        alpha = 0.35 + 0.4 * fresnel;
    } else {
        // Through the ball: in along the refracted ray, across the chord,
        // and out again, which turns the world behind it upside down.
        let d1 = refract(-v, n, 1.0 / WATER_IOR);
        let chord = 2.0 * r * clamp(dot(d1, -d), 0.05, 1.0);
        let exit = world + d1 * chord;
        let nq = normalize(exit - center);
        var d2 = refract(d1, -nq, WATER_IOR);
        if dot(d2, d2) < 0.25 {
            d2 = reflect(d1, -nq);
        }
        let trans = exp(-extinction * 0.55 * chord);
        let through = dot(trans, vec3<f32>(0.3333));
        let lens = 0.55 * smoothstep(0.08, 0.6, r);
        let body = water_inscatter(b) * 1.8 * (1.0 - trans);
        // Caustics: light gathered on the side away from the sun, and
        // bright bands drifting through the water.
        let focus = pow(max(dot(d, -l), 0.0), 10.0) * 2.5;
        let band = value_noise(q * 1.4 + vec3<f32>(0.0, t * 0.6, t * 0.4));
        let lines = pow(1.0 - abs(band * 2.0 - 1.0), 8.0) * 1.2;
        let caustic = (focus + lines) * sunlit * 0.35 * through;
        let seen = orb_environment(d2, exit, pixel) * trans * lens;
        rgb = sky * fresnel + glint + (body + caustic + seen) * (1.0 - fresnel);
        alpha = fresnel + (1.0 - fresnel) * ((1.0 - through) + through * lens);
        // A thin stream or a droplet is aerated, white water, which shows
        // against the sea where clear water would vanish.
        let aerated = 1.0 - smoothstep(0.08, 0.7, r);
        let white = vec3<f32>(0.86, 0.93, 0.96) * sunlit * (0.35 + 0.25 * lines);
        rgb = rgb * (1.0 - aerated * 0.5) + white * aerated * 0.5;
        alpha = alpha * (1.0 - aerated * 0.5) + aerated * 0.5;
    }
    if charge > 0.0 {
        // Electrified: a blue glow through the water and white arcs
        // crawling over the skin, redrawn many times a second.
        let flick = floor(t * 24.0);
        let c1 = value_noise(q * 1.3 + vec3<f32>(flick * 1.7, flick * 0.9, 0.0));
        let c2 = value_noise(q * 2.9 + vec3<f32>(0.0, flick * 2.3, flick * 1.1));
        let width = 0.03 + 0.05 * charge;
        let crack = max(
            1.0 - smoothstep(0.0, width, abs(c1 - 0.5)),
            1.0 - smoothstep(0.0, width * 0.7, abs(c2 - 0.5))
        );
        let pulse = 0.75 + 0.25 * sin(t * 61.0 + q.y * 3.0);
        let scale = water_host_light(world, vec3<f32>(0.0, 1.0, 0.0), pixel);
        let glow = vec3<f32>(0.4, 0.62, 1.0) * (0.15 * pulse) + vec3<f32>(0.9, 0.95, 1.0) * crack * 3.0;
        rgb += glow * scale * charge;
        alpha = max(alpha, charge * (0.05 + 0.6 * crack));
    }
    return water_cover(rgb, alpha);
}
