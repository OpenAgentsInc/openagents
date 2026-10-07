// ---------------------------------------------------------------------------
// The physical renderer's water pass (`water`, spliced after the shared
// `water.wgsl` into `pbr/photo.wgsl`): the shared surface shading through
// this renderer's sky cube, sun, shadows, exposure, and fog, plus what only
// this renderer draws so far: the Water Lab's spells on the sea, falling
// sheets, free orbs of water, and the light under the sea on every lit
// surface (Beer–Lambert extinction and in-scatter along the view and sun
// paths, and analytic caustics from the refracted light field's area
// ratio, after Evan Wallace's "WebGL Water", 2011).

@group(2) @binding(3) var<uniform> water: WaterUniform;
@group(2) @binding(4) var water_tile: texture_2d<f32>;
@group(2) @binding(5) var water_tile_sampler: sampler;
@group(2) @binding(6) var water_waves: texture_2d_array<f32>;

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

// Rain's wetness on the ground at `p`, 0 to 1.
fn water_wetness(p: vec2<f32>) -> f32 {
    if f.water_wet.w <= 0.0 {
        return 0.0;
    }
    let r = distance(p, f.water_wet.xy);
    return f.water_wet.w * (1.0 - smoothstep(f.water_wet.z - 1.0, f.water_wet.z + 0.5, r));
}

// ---- Light under the sea (read by every lit fragment through `shade`).

// Light scattered back toward the eye from inside the sea: the frame's
// in-scatter color times the sun's and the sky's light falling on it.
fn water_sea_inscatter() -> vec3<f32> {
    let sun = f.sun.w * max(f.sun.y, 0.0) * water_key_tint();
    var sky = vec3<f32>(f.sun.w * 0.15);
    if f.sky_light.x > 0.5 {
        sky = sky_irradiance(vec3<f32>(0.0, 1.0, 0.0));
    }
    return f.water_scatter.rgb * (sun + sky) / PI;
}

// The analytic caustics' focusing at a point `depth` under the sea: one
// where the light arrives evenly, more where the surface's curvature
// gathers it into lines, less between them.
const CAUSTIC_WAVES: array<vec4<f32>, 5> = array<vec4<f32>, 5>(
    vec4<f32>(0.80, 0.60, 4.6, 0.020),
    vec4<f32>(-0.45, 0.89, 5.9, 0.016),
    vec4<f32>(0.10, -0.99, 3.7, 0.022),
    vec4<f32>(-0.93, -0.36, 7.3, 0.011),
    vec4<f32>(0.62, -0.78, 9.1, 0.008)
);
fn water_caustics(world: vec3<f32>, depth: f32) -> f32 {
    let strength = f.water.w;
    if strength <= 0.0 {
        return 1.0;
    }
    let t = f.water_scatter.w;
    // Where the light reaching this point crossed the surface.
    let p = world.xz - f.sun.xz / max(f.sun.y, 0.25) * depth;
    // Refraction's lever: (1 - 1/n) of the depth, held short so deep water
    // doesn't fold into noise.
    let lever = 0.25 * min(depth, 2.8);
    var hxx = 0.0;
    var hzz = 0.0;
    var hxz = 0.0;
    for (var i = 0; i < 5; i++) {
        let w = CAUSTIC_WAVES[i];
        let k = w.z;
        let theta = k * dot(w.xy, p) - sqrt(9.81 * k) * t + f32(i) * 1.7;
        let m = -w.w * k * k * sin(theta);
        hxx += m * w.x * w.x;
        hzz += m * w.y * w.y;
        hxz += m * w.x * w.y;
    }
    let det = (1.0 + lever * hxx) * (1.0 + lever * hzz) - lever * lever * hxz * hxz;
    let focus = clamp(0.75 / max(abs(det), 0.1), 0.0, 5.0);
    let fade = smoothstep(0.0, 0.35, depth) * exp(-depth * 0.12);
    return mix(1.0, focus, strength * fade);
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

// The sunlight reaching a point under the sea, as a factor: extinction
// along the sun's path down from the surface, times the caustics.
fn water_sun(world: vec3<f32>) -> vec3<f32> {
    if f.water.y < 0.5 {
        return vec3<f32>(1.0);
    }
    let depth = water_depth(world);
    if depth <= 0.0 {
        return vec3<f32>(1.0);
    }
    let mu = max(f.sun.y, 0.2);
    return exp(-f.water_extinction.rgb * depth / mu) * water_caustics(world, depth);
}

// The sky's light reaching a point under the sea, as a factor.
fn water_ambient(world: vec3<f32>) -> vec3<f32> {
    if f.water.y < 0.5 {
        return vec3<f32>(1.0);
    }
    let depth = water_depth(world);
    if depth <= 0.0 {
        return vec3<f32>(1.0);
    }
    return exp(-f.water_extinction.rgb * depth * 1.4);
}

// A lit point's radiance as the eye sees it through the sea: extinction
// along the view path inside the water, and the water's own in-scatter.
// From above the surface the path runs from the point up to the surface
// along the refracted ray; from under it, from the point to the eye.
fn water_view(world: vec3<f32>, radiance: vec3<f32>) -> vec3<f32> {
    if f.water.z > 0.5 {
        // The eye is under water, the sea's or the body the zone says it
        // is in: everything it sees lies through water, whatever stands
        // above the surface through the surface too.
        let t = exp(-f.water_extinction.rgb * distance(world, f.eye.xyz));
        return radiance * t + water_fog() * (1.0 - t);
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

@vertex
fn vs_water(v: WaterIn) -> WaterOut {
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
        moved = water_move(v, water_kind_scale(v.kind) * calm, rise);
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
        moved = water_move(v, water_kind_scale(v.kind), 0.0);
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

    // The eye under this body's surface: the sea knows from the frame,
    // other bodies from their own flag. A body the eye is beside rather
    // than in is too thin to hide anything from below.
    let below = f.eye.y < world.y;
    let inside = select(water.bodies[b].params.w > 0.5, f.water.z > 0.5, sea);
    if below {
        if inside {
            return water_below(b, wn.n, v, distance(f.eye.xyz, world), wn.slope);
        }
        var none: WaterShade;
        none.emit = vec3<f32>(0.0);
        none.transmit = vec3<f32>(1.0);
        return none;
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
    }
    return o;
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
