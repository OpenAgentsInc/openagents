// ---------------------------------------------------------------------------
// Water (`pbr::water`): the sea's swell and detail, streams, falls, and the
// light under the surface. The module docs list the public techniques this
// follows: Gerstner waves (Tessendorf 2001; Finch 2004), flow-map advection
// (Vlachos 2010), Schlick's Fresnel term, Beer–Lambert extinction, and
// caustics from the refracted light field's area ratio (Wallace 2011).
// Everything here is procedural, with no textures of its own, so WebGL2 and
// GLES 3.0 draw the same water with fewer waves.

struct WaterUniform {
    // Per swell wave: dir.xz, k, amplitude; then steepness, omega, phase.
    waves: array<vec4<f32>, 16>,
    // Per detail wave: dir.xz, k, amplitude; then omega, phase, wavelength.
    detail: array<vec4<f32>, 32>,
    // Per ripple: x, z, start, strength.
    ripples: array<vec4<f32>, 24>,
    // rgb extinction (1/m); w the sea's level.
    extinction: vec4<f32>,
    // rgb in-scatter; w foam.
    scatter: vec4<f32>,
    // Time, swell count, detail count, ripple count.
    params: vec4<f32>,
    // Roughness, caustics, the flood's rise over the rest level, and the
    // swell's gain.
    look: vec4<f32>,
};

// Group 2 is the textured material's slot in this module; the water's
// uniform takes a binding none of the material's use.
@group(2) @binding(3) var<uniform> water: WaterUniform;

const WATER_IOR: f32 = 1.333;
const RIPPLE_LIFE: f32 = 4.0;
const RIPPLE_SPEED: f32 = 1.1;

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

// ---- Light under the surface (read by every lit fragment through `shade`).

fn water_key_tint() -> vec3<f32> {
    return select(vec3<f32>(1.0), f.key_tint.rgb, f.key_tint.w > 0.5);
}

// Light scattered back toward the eye from inside the water, pre-exposed:
// the in-scatter color times the sun's and the sky's light falling on it.
fn water_inscatter() -> vec3<f32> {
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

// The sunlight reaching a point under the sea, as a factor: extinction
// along the sun's path down from the surface, times the caustics.
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
    if f.water.y < 0.5 {
        return radiance;
    }
    if f.water.z > 0.5 {
        // The eye is under the sea: everything it sees lies through water,
        // whatever stands above the surface through the surface too.
        let t = exp(-f.water_extinction.rgb * distance(world, f.eye.xyz));
        return radiance * t + water_fog() * (1.0 - t);
    }
    let depth = water_depth(world);
    if depth <= 0.0 {
        return radiance;
    }
    let v = normalize(f.eye.xyz - world);
    let ci = clamp(v.y, 0.0, 1.0);
    let ct = sqrt(max(1.0 - (1.0 - ci * ci) / (WATER_IOR * WATER_IOR), 0.0));
    let path = depth / max(ct, 0.05);
    let t = exp(-f.water_extinction.rgb * path);
    return radiance * t + water_inscatter() * (1.0 - t);
}

// The water's color around an eye under the surface: the in-scatter of
// the whole water column around it, brighter than the deep color seen
// from above, which is dimmed by the column over the bed.
fn water_fog() -> vec3<f32> {
    return water_inscatter() * 2.4;
}

// ---- The surface.

struct WaterIn {
    @location(0) pos: vec3<f32>,
    @location(1) depth: f32,
    @location(2) flow: vec2<f32>,
    @location(3) foam: f32,
    @location(4) kind: f32,
};

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // The undisplaced position on the plane, where waves are evaluated.
    @location(1) rest: vec2<f32>,
    @location(2) depth: f32,
    @location(3) flow: vec2<f32>,
    @location(4) foam: f32,
    @location(5) kind: f32,
    // How tightly the swell squeezes the surface here: crests near one.
    @location(6) crest: f32,
};

// The swell's displacement (xyz) and squeeze (w) at a rest point.
fn water_swell(p: vec2<f32>, depth: f32, scale: f32) -> vec4<f32> {
    var d = vec3<f32>(0.0);
    var squeeze = 0.0;
    let t = water.params.x;
    let count = i32(water.params.y);
    for (var i = 0; i < count; i++) {
        let a = water.waves[i * 2];
        let b = water.waves[i * 2 + 1];
        let k = a.z;
        let amp = a.w * scale * water.look.w * sqrt(tanh(k * max(depth, 0.0)));
        let theta = k * dot(a.xy, p) - b.y * t + b.z;
        let c = cos(theta);
        let s = sin(theta);
        d.x += b.x * amp * a.x * c;
        d.z += b.x * amp * a.y * c;
        d.y += amp * s;
        squeeze += b.x * amp * k * s;
    }
    return vec4<f32>(d, squeeze);
}

@vertex
fn vs_water(v: WaterIn) -> WaterOut {
    var o: WaterOut;
    var world = v.pos;
    var crest = 0.0;
    var depth = v.depth;
    if v.kind < 1.5 {
        // A flood lifts the sea; ice stills it; spells pull it down.
        depth += water.look.z;
        world.y += water.look.z;
        let calm = 1.0 - water_ice(v.pos.xz);
        let swell = water_swell(v.pos.xz, depth, v.kind * calm);
        world += swell.xyz;
        crest = swell.w;
        world.y -= water_drop(v.pos.xz, depth);
    } else if v.kind > 3.5 {
        // An orb: its center rides in the depth and flow channels, and the
        // vertex is its offset at rest, moved by the wobble.
        let center = vec3<f32>(v.flow.x, v.depth, v.flow.y);
        let r = length(v.pos);
        let d = v.pos / max(r, 1e-5);
        world = center + d * r * orb_shape(d, water.params.x, v.kind - 4.0);
        crest = r;
    }
    o.clip = f.view_proj * vec4<f32>(world, 1.0);
    o.world = world;
    o.rest = select(v.pos.xz, world.xz, v.kind > 3.5);
    o.depth = depth;
    o.flow = v.flow;
    o.foam = v.foam;
    o.kind = v.kind;
    o.crest = crest;
    return o;
}

// The surface slope (d height / dx, d height / dz) of the swell at a rest
// point, after Finch (2004), equation 12, without the squeeze's change of
// the normal's y, which `water_normal` restores.
fn water_swell_slope(p: vec2<f32>, depth: f32, scale: f32) -> vec3<f32> {
    var g = vec3<f32>(0.0);
    let t = water.params.x;
    let count = i32(water.params.y);
    for (var i = 0; i < count; i++) {
        let a = water.waves[i * 2];
        let b = water.waves[i * 2 + 1];
        let k = a.z;
        let amp = a.w * scale * water.look.w * sqrt(tanh(k * max(depth, 0.0)));
        let theta = k * dot(a.xy, p) - b.y * t + b.z;
        let wa = k * amp;
        g.x += a.x * wa * cos(theta);
        g.y += a.y * wa * cos(theta);
        g.z += b.x * wa * sin(theta);
    }
    return g;
}

// The detail waves' slope at `p`, each faded once it is shorter than a few
// of the pixel's `footprint` (m), and the slope variance the faded waves
// would have added (z), which becomes roughness.
fn water_detail(p: vec2<f32>, footprint: f32, gain: f32) -> vec3<f32> {
    var g = vec2<f32>(0.0);
    var lost = 0.0;
    let t = water.params.x;
    let count = i32(water.params.z);
    for (var i = 0; i < count; i++) {
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

// The ripples' slope at `p`.
fn water_ripples(p: vec2<f32>) -> vec2<f32> {
    var g = vec2<f32>(0.0);
    let t = water.params.x;
    let count = i32(water.params.w);
    let k = 6.2831853 / 0.45;
    for (var i = 0; i < count; i++) {
        let r = water.ripples[i];
        let age = t - r.z;
        let to = p - r.xy;
        let dist = length(to);
        let x = dist - RIPPLE_SPEED * age;
        let envelope = exp(-x * x / 0.18) * exp(-age * 1.1) / (1.0 + 1.5 * dist);
        let slope = r.w * envelope * k * cos(k * x);
        g += to / max(dist, 1e-3) * slope;
    }
    return g;
}

// Foam's mottled cover at `p`: two octaves of drifting value noise.
fn water_foam_noise(p: vec2<f32>, t: f32) -> f32 {
    let a = value_noise(vec3<f32>(p * 1.3, t * 0.25));
    let b = value_noise(vec3<f32>(p * 4.1 + vec2<f32>(t * 0.07, 0.0), t * 0.5));
    return a * 0.6 + b * 0.4;
}

// The sky reflected along `r`, pre-exposed.
fn water_sky(r: vec3<f32>, roughness: f32) -> vec3<f32> {
    let up = normalize(vec3<f32>(r.x, max(r.y, 0.015), r.z));
    if f.sky_light.x > 0.5 {
        return textureSampleLevel(sky_cube, linear_clamp, up, sqrt(roughness) * f.sky_light.y).rgb;
    }
    if f.sky_zenith.w > 0.5 {
        return daylight_air(up);
    }
    return f.field.rgb;
}

// Diffuse light on a white surface facing up, such as foam, pre-exposed.
fn water_diffuse_light(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let sun = f.sun.w * max(dot(n, f.sun.xyz), 0.0) * water_key_tint() * sun_shadow(world, n, pixel);
    var sky = vec3<f32>(f.sun.w * 0.15);
    if f.sky_light.x > 0.5 {
        sky = sky_irradiance(n);
    }
    return (sun + sky) / PI;
}

// Premultiplied, fogged output for a surface fragment of straight color
// `rgb` covering `alpha`.
fn water_out(rgb_premultiplied: vec3<f32>, alpha: f32, world: vec3<f32>) -> vec4<f32> {
    let a = clamp(alpha, 0.0, 1.0);
    let straight = rgb_premultiplied / max(a, 1e-3);
    let fogged = neon_fog(expose(straight), world, 1.0);
    return vec4<f32>(fogged * a, a);
}

@fragment
fn fs_water(i: WaterOut) -> @location(0) vec4<f32> {
    // Derivatives first, in uniform control flow.
    let geometric = normalize(cross(dpdx(i.world), dpdy(i.world)));
    let footprint = max(length(fwidth(i.rest)), 1e-4);
    let t = water.params.x;
    let pixel = i.clip.xy;
    let v = normalize(f.eye.xyz - i.world);
    let kind = i.kind;

    if kind > 3.5 {
        return water_orb(i, v, pixel, t);
    }
    if kind > 2.5 {
        // A falling sheet: aerated white water streaked along its fall,
        // thinner where it tears.
        let heading = normalize(i.flow + vec2<f32>(1e-4, 0.0));
        let across = dot(i.rest, vec2<f32>(-heading.y, heading.x));
        let streak = value_noise(vec3<f32>(across * 2.6, i.world.y * 0.35 + t * 2.2, 0.0));
        let fine = value_noise(vec3<f32>(across * 9.0, i.world.y * 1.4 + t * 5.5, 3.0));
        let body = clamp(0.45 + 0.6 * streak + 0.3 * fine - 0.25, 0.0, 1.0);
        var n = geometric;
        if dot(n, v) < 0.0 {
            n = -n;
        }
        let light = water_diffuse_light(i.world, n, pixel);
        let white = vec3<f32>(0.86, 0.93, 0.96) * light;
        let blue = water_inscatter() * 3.0;
        let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
        let refl = water_sky(reflect(-v, n), 0.2) * fresnel;
        let alpha = clamp(0.35 + 0.6 * body + i.foam * 0.3, 0.0, 0.97);
        let rgb = mix(blue, white, 0.35 + 0.65 * body) * alpha + refl * (1.0 - alpha);
        return water_out(rgb, alpha, i.world);
    }

    let stream = kind > 1.5;
    let scale = select(clamp(kind, 0.0, 1.0), 0.0, stream);
    var slope = vec2<f32>(0.0);
    var squeeze_y = 0.0;
    if !stream {
        let swell = water_swell_slope(i.rest, i.depth, scale);
        slope = swell.xy;
        squeeze_y = swell.z;
    }
    // Detail: carried along the flow in two phases (Vlachos 2010) where the
    // water moves, else standing.
    var detail = vec3<f32>(0.0);
    var flow = i.flow;
    if !stream {
        flow += water_spell_flow(i.rest);
    }
    let speed = length(flow);
    if speed > 0.02 {
        let period = 1.6;
        let phase_a = fract(t / period);
        let phase_b = fract(t / period + 0.5);
        let da = water_detail(i.rest - flow * phase_a * period, footprint, 1.4);
        let db = water_detail(i.rest - flow * phase_b * period + vec2<f32>(3.7, 1.3), footprint, 1.4);
        let wa = 1.0 - abs(2.0 * phase_a - 1.0);
        detail = da * wa + db * (1.0 - wa);
    } else {
        detail = water_detail(i.rest, footprint, 1.0);
    }
    let ice = select(water_ice(i.rest), 0.0, stream);
    slope = (slope + detail.xy + water_ripples(i.rest)) * (1.0 - ice);
    var n = normalize(vec3<f32>(-slope.x, 1.0 - squeeze_y, -slope.y));
    let roughness = clamp(sqrt(water.look.x * water.look.x + detail.z), 0.02, 0.6);

    // Foam: along the shore, a band that runs up and back with the swash;
    // on crests the swell squeezes; around rocks and under falls.
    let noise = water_foam_noise(i.rest, t);
    // The water over the bed here, with the swell's own height, so the
    // foam runs up the beach with each wave and back.
    let wet = i.depth + select(i.world.y - water.extinction.w, 0.0, stream);
    let shore = 1.0 - smoothstep(0.0, 0.32, wet);
    let swash = 0.5 + 0.5 * sin(wet * 14.0 - t * 1.4 + noise * 3.0);
    var foam = shore * smoothstep(0.72, 0.95, swash * 0.6 + noise * 0.5);
    // A lace of foam right at the water's edge.
    foam = max(foam, (1.0 - smoothstep(0.0, 0.14, wet)) * smoothstep(0.3, 0.62, noise));
    foam = max(foam, smoothstep(0.62, 0.95, i.crest) * smoothstep(0.35, 0.7, noise));
    foam = max(foam, clamp(i.foam, 0.0, 1.0) * smoothstep(0.25, 0.65, noise + 0.25 * i.foam));
    foam = max(foam, water_ripple_foam(i.rest) * smoothstep(0.2, 0.6, noise));
    if !stream {
        // White water on a trench's walls and in a whirlpool's spiral.
        let wall = water_trench(i.rest, 2.5) - water_trench(i.rest, 0.4);
        foam = max(foam, clamp(wall * 2.0, 0.0, 1.0) * smoothstep(0.2, 0.5, noise));
        if f.water_whirl.w > 0.0 {
            let to = i.rest - f.water_whirl.xy;
            let r = length(to) / max(f.water_whirl.z, 0.1);
            let arms = sin(atan2(to.y, to.x) * 3.0 + r * 9.0 - t * 4.0);
            foam = max(foam, f.water_whirl.w * smoothstep(0.3, 0.9, arms * 0.5 + 0.5) * smoothstep(1.6, 0.4, r) * smoothstep(0.3, 0.7, noise + 0.3));
        }
    }
    foam = clamp(foam * water.scatter.w, 0.0, 1.0);

    // Only the sea can hold the eye: streams and pools are too shallow.
    let below = f.eye.y < i.world.y;
    if stream && below {
        // A stream seen from under its own level, as from the foot of the
        // falls: it is too thin to hide anything.
        return vec4<f32>(0.0);
    }
    let under = !stream && f.water.z > 0.5 && below;
    if under {
        // Seen from below: Snell's window, total internal reflection
        // outside it, and the water between the eye and the surface.
        let nd = -n;
        let ci = clamp(dot(v, nd), 0.0, 1.0);
        let st2 = WATER_IOR * WATER_IOR * (1.0 - ci * ci);
        var reflectance = 1.0;
        if st2 < 1.0 {
            let ct = sqrt(1.0 - st2);
            reflectance = 0.02 + 0.98 * pow(1.0 - ct, 5.0);
        }
        let fog = water_fog();
        // Outside the window the surface mirrors the lit water below it,
        // rippling with the waves.
        let shimmer = 1.1 + 1.6 * clamp(length(slope), 0.0, 0.6);
        let path = distance(f.eye.xyz, i.world);
        let trans = exp(-f.water_extinction.rgb * path);
        let rgb = fog * shimmer * reflectance * trans + fog * (1.0 - trans);
        let through = (1.0 - reflectance) * dot(trans, vec3<f32>(0.3333));
        let alpha = 1.0 - through;
        let a = clamp(alpha, 0.0, 1.0);
        return vec4<f32>(expose(rgb / max(a, 1e-3)) * a, a);
    }

    let nov = max(dot(n, v), 1e-3);
    let fresnel = 0.02 + 0.98 * pow(1.0 - nov, 5.0);
    let r = reflect(-v, n);
    let sky = water_sky(r, roughness);
    // The sun's glint: the renderer's GGX lobe, widened by the disc.
    let l = f.sun.xyz;
    let nol = dot(n, l);
    var glint = vec3<f32>(0.0);
    if nol > 0.0 && f.sun.w > 0.0 {
        let h = normalize(l + v);
        let a = min(roughness * roughness + f.sun_disc.x * 0.5, 1.0);
        let a2 = a * a;
        let spec = d_ggx(max(dot(n, h), 0.0), a2) * v_smith(nov, nol, a2) * f_schlick1(0.02, max(dot(v, h), 0.0));
        glint = vec3<f32>(spec) * f.sun.w * water_key_tint() * nol * sun_shadow(i.world, geometric, pixel);
    }
    let foam_rgb = vec3<f32>(0.9, 0.94, 0.95) * water_diffuse_light(i.world, vec3<f32>(0.0, 1.0, 0.0), pixel);
    if ice > 0.01 {
        // Ice: frosted, cracked, and nearly opaque, over the water's own
        // look where it thins at the rim.
        let cracks = value_noise(vec3<f32>(i.rest * 1.8, 7.0));
        let line = 1.0 - smoothstep(0.0, 0.05, abs(cracks - 0.5));
        let frost = 0.55 + 0.25 * value_noise(vec3<f32>(i.rest * 6.0, 2.0)) - 0.25 * line;
        let ice_light = water_diffuse_light(i.world, vec3<f32>(0.0, 1.0, 0.0), pixel);
        let ice_rgb = vec3<f32>(0.62, 0.78, 0.88) * frost * ice_light + sky * 0.08;
        let rgb_w = sky * fresnel + glint;
        let a_w = fresnel;
        let rgb = mix(rgb_w, ice_rgb + glint * 0.3, ice);
        let alpha = mix(a_w, 0.94, ice);
        return water_out(rgb, alpha, i.world);
    }

    var body = vec3<f32>(0.0);
    var body_alpha = 0.0;
    if stream {
        // A stream or pool tints what lies under it itself.
        let ci = clamp(v.y, 0.0, 1.0);
        let ct = sqrt(max(1.0 - (1.0 - ci * ci) / (WATER_IOR * WATER_IOR), 0.0));
        let path = clamp(i.depth, 0.0, 2.5) / max(ct, 0.05);
        let trans = exp(-water.extinction.rgb * path);
        body = water_inscatter_local() * (1.0 - trans);
        body_alpha = 1.0 - dot(trans, vec3<f32>(0.3333));
    }
    // Premultiplied: reflection and glint over whatever lies under the
    // surface, which shows through the transmitted share; foam covers.
    let clear = sky * fresnel + glint + body * (1.0 - fresnel);
    let clear_alpha = fresnel + body_alpha * (1.0 - fresnel);
    let rgb = clear * (1.0 - foam) + foam_rgb * foam;
    let alpha = clear_alpha * (1.0 - foam) + foam;
    // Thin water at the very edge fades out instead of ending in a line.
    let edge = smoothstep(-0.05, 0.12, i.depth);
    return water_out(rgb * edge, alpha * edge, i.world);
}

// Foam riding the rings of young ripples: a footfall's or a splash's
// white ring, which fades as the ring spreads.
fn water_ripple_foam(p: vec2<f32>) -> f32 {
    var foam = 0.0;
    let t = water.params.x;
    let count = i32(water.params.w);
    for (var i = 0; i < count; i++) {
        let r = water.ripples[i];
        let age = t - r.z;
        let x = distance(p, r.xy) - RIPPLE_SPEED * age;
        foam = max(foam, exp(-x * x / 0.08) * exp(-age * 1.6) * clamp(r.w * 25.0, 0.0, 1.0));
    }
    return foam;
}

// A stream's own in-scatter: the uniform's color under the frame's light.
fn water_inscatter_local() -> vec3<f32> {
    let sun = f.sun.w * max(f.sun.y, 0.0) * water_key_tint();
    var sky = vec3<f32>(f.sun.w * 0.15);
    if f.sky_light.x > 0.5 {
        sky = sky_irradiance(vec3<f32>(0.0, 1.0, 0.0));
    }
    return water.scatter.rgb * (sun + sky) / PI;
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
    let sky = water_sky(d, 0.15);
    let ground = vec3<f32>(0.46, 0.4, 0.3) * water_diffuse_light(world, vec3<f32>(0.0, 1.0, 0.0), pixel);
    return mix(ground, sky, smoothstep(-0.08, 0.06, d.y));
}

fn water_orb(i: WaterOut, v: vec3<f32>, pixel: vec2<f32>, t: f32) -> vec4<f32> {
    let center = vec3<f32>(i.flow.x, i.depth, i.flow.y);
    let r = max(i.crest, 0.01);
    let wobble = i.kind - 4.0;
    let charge = clamp(i.foam, 0.0, 1.0);
    let rel = i.world - center;
    let d = normalize(rel);
    // Only the near face draws, or the far face when the eye is inside.
    let inside = distance(f.eye.xyz, center) < r;
    let facing = dot(rel, f.eye.xyz - i.world) > 0.0;
    if facing == inside {
        return vec4<f32>(0.0);
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
    let fresnel = 0.02 + 0.98 * pow(1.0 - nov, 5.0);
    let roughness = clamp(water.look.x, 0.02, 0.3);
    let sky = water_sky(reflect(-v, n), roughness);
    let l = f.sun.xyz;
    let nol = dot(n, l);
    var glint = vec3<f32>(0.0);
    if nol > 0.0 && f.sun.w > 0.0 {
        let h = normalize(l + v);
        let lobe = min(roughness * roughness + f.sun_disc.x * 0.5, 1.0);
        let a2 = lobe * lobe;
        let spec = d_ggx(max(dot(n, h), 0.0), a2) * v_smith(nov, nol, a2) * f_schlick1(0.02, max(dot(v, h), 0.0));
        glint = vec3<f32>(spec) * f.sun.w * water_key_tint() * nol;
    }
    let sunlit = water_diffuse_light(i.world, l, pixel);

    var rgb = vec3<f32>(0.0);
    var alpha = 0.0;
    if inside {
        // From within: the water all around, and the sky through the skin.
        let fog = water_inscatter_local() * 2.4;
        rgb = fog * 0.6 + sky * fresnel * 0.4;
        alpha = 0.35 + 0.4 * fresnel;
    } else {
        // Through the ball: in along the refracted ray, across the chord,
        // and out again, which turns the world behind it upside down.
        let d1 = refract(-v, n, 1.0 / WATER_IOR);
        let chord = 2.0 * r * clamp(dot(d1, -d), 0.05, 1.0);
        let exit = i.world + d1 * chord;
        let nq = normalize(exit - center);
        var d2 = refract(d1, -nq, WATER_IOR);
        if dot(d2, d2) < 0.25 {
            d2 = reflect(d1, -nq);
        }
        let trans = exp(-water.extinction.rgb * 0.55 * chord);
        let through = dot(trans, vec3<f32>(0.3333));
        let lens = 0.55 * smoothstep(0.08, 0.6, r);
        let body = water_inscatter_local() * 1.8 * (1.0 - trans);
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
        let scale = water_diffuse_light(i.world, vec3<f32>(0.0, 1.0, 0.0), pixel);
        let glow = vec3<f32>(0.4, 0.62, 1.0) * (0.15 * pulse) + vec3<f32>(0.9, 0.95, 1.0) * crack * 3.0;
        rgb += glow * scale * charge;
        alpha = max(alpha, charge * (0.05 + 0.6 * crack));
    }
    return water_out(rgb, alpha, i.world);
}
