// ---------------------------------------------------------------------------
// The imported renderer's water pass (`water`, spliced after the shared
// `water.wgsl` into `imported/scene.wgsl`): the shared surface shading under
// this renderer's ambient light, point lights, exposure, and height fog.
// The chamber has no sky or sun of its own, so the water uniform carries a
// sky gradient and a sun for it; without them the sky is the fog's color
// over the ambient light. Point lights add their own glints.

@group(1) @binding(7) var<uniform> water: WaterUniform;
@group(1) @binding(8) var water_tile: texture_2d<f32>;
@group(1) @binding(9) var water_tile_sampler: sampler;
@group(1) @binding(10) var water_waves: texture_2d_array<f32>;

fn water_host_control() -> vec4<f32> {
    return frame.water_control;
}

// The sky reflected along `dir`, at a blur of 0 (sharp) to 1 (roughest):
// the uniform's gradient, or the fog over the ambient light.
fn water_host_sky(dir: vec3<f32>, level: f32) -> vec3<f32> {
    var zenith = frame.ambient.rgb * 1.6;
    var horizon = frame.fog.rgb;
    if water.sky_zenith.w > 0.5 {
        zenith = water.sky_zenith.rgb;
        horizon = water.sky_horizon.rgb;
    }
    let up = clamp(dir.y, 0.0, 1.0);
    let sharp = mix(horizon, zenith, smoothstep(0.0, 0.55, up));
    let blurred = mix(horizon, zenith, 0.55);
    return mix(sharp, blurred, level * level);
}

fn water_host_sun() -> vec4<f32> {
    return vec4<f32>(water.sun.xyz, select(0.0, 1.0, water.sun.w > 0.0));
}

fn water_host_sun_light() -> vec3<f32> {
    return water.sun_color.rgb * water.sun.w;
}

// Diffuse light on a white surface facing `n`: the ambient, the sun, and
// the point lights in range.
fn water_host_light(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    var light = frame.ambient.rgb + water_host_sun_light() * max(dot(n, water.sun.xyz), 0.0) / 3.14159265;
    for (var i = 0u; i < u32(frame.settings.x); i++) {
        let source = frame.lights[i * 2u];
        let radiance = frame.lights[i * 2u + 1u];
        let delta = source.xyz - world;
        let d2 = dot(delta, delta);
        if d2 >= source.w * source.w || radiance.w <= 0.0 {
            continue;
        }
        let l = delta / max(sqrt(d2), 0.001);
        light += radiance.rgb * radiance.w * verse_point_falloff(d2, source.w, false) * max(dot(n, l), 0.0);
    }
    return light;
}

fn water_host_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    return 1.0;
}

@vertex
fn vs_water(v: WaterIn) -> WaterOut {
    var moved: WaterMoved;
    moved.world = v.pos;
    moved.crest = 0.0;
    var scale = 0.0;
    if v.kind < 1.5 {
        scale = clamp(v.kind, 0.0, 1.0);
    } else if v.kind < 2.99 {
        scale = clamp((v.kind - 2.0) / 0.98, 0.0, 1.0);
    }
    if v.kind < 2.99 {
        moved = water_move(v, scale, 0.0);
    }
    return water_out(v, moved, v.pos.xz, v.depth, frame.view * vec4<f32>(moved.world, 1.0));
}

fn water_fragment(i: WaterOut) -> WaterShade {
    if water_in_hull(i.world_depth.xyz) {
        discard;
    }
    let world = i.world_depth.xyz;
    let rest = i.rest_flow.xy;
    let dpx = dpdx(rest);
    let dpy = dpdy(rest);
    let v = normalize(frame.eye.xyz - world);
    let kind = i.look.y;
    let b = water_body_index(i.extra.x);
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
    s.scale = 0.0;
    if kind < 1.5 {
        s.scale = clamp(kind, 0.0, 1.0);
    } else if kind < 2.99 {
        s.scale = clamp((kind - 2.0) / 0.98, 0.0, 1.0);
    }
    s.v = v;
    s.eye_distance = distance(frame.eye.xyz, world);
    let fp = water_footprint(v, s.eye_distance, dpx, dpy);
    s.footprint = fp.size;
    s.dpx = fp.dpx;
    s.dpy = fp.dpy;
    s.pixel = i.clip.xy;
    s.calm = 0.0;
    s.slope = vec2<f32>(0.0);
    s.extra_foam = 0.0;
    // This renderer never tints a bed under the water itself.
    s.column = 1.0;
    s.disc = 0.03;
    let wn = water_normal(s);
    if frame.eye.y < world.y {
        if water.bodies[b].params.w > 0.5 {
            return water_below(b, wn.n, v, s.eye_distance, wn.slope);
        }
        var none: WaterShade;
        none.emit = vec3<f32>(0.0);
        none.transmit = vec3<f32>(1.0);
        return none;
    }
    var o = water_shade(s, wn);
    // Each point light in range glints on the water too.
    let roughness = wn.roughness;
    for (var k = 0u; k < u32(frame.settings.x); k++) {
        let source = frame.lights[k * 2u];
        let radiance = frame.lights[k * 2u + 1u];
        let delta = source.xyz - world;
        let d2 = dot(delta, delta);
        if d2 >= source.w * source.w || radiance.w <= 0.0 {
            continue;
        }
        let l = delta / max(sqrt(d2), 0.001);
        let glint = water_glint(wn.n, v, l, roughness, 0.05, s.eye_distance);
        o.emit += radiance.rgb * radiance.w * verse_point_falloff(d2, source.w, false) * glint;
    }
    return o;
}

// The emitted half, added over the scene after `fs_water_transmit` dims it:
// fogged and exposed so that both halves sum to the fogged surface.
@fragment
fn fs_water(i: WaterOut) -> @location(0) vec4<f32> {
    let shade = water_fragment(i);
    let fog = fog_amount(i.world_depth.xyz);
    let color = (shade.emit * (1.0 - fog) + frame.fog.rgb * fog * (vec3<f32>(1.0) - shade.transmit)) * frame.ambient.w;
    return vec4<f32>(min(max(color, vec3<f32>(0.0)), vec3<f32>(60000.0)), 0.0);
}

// The transmitted half: what of the scene behind comes through, per
// channel, multiplied into it.
@fragment
fn fs_water_transmit(i: WaterOut) -> @location(0) vec4<f32> {
    let shade = water_fragment(i);
    return vec4<f32>(clamp(shade.transmit, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

fn water_host_rain_open(world: vec3<f32>) -> f32 { return 1.0; }
