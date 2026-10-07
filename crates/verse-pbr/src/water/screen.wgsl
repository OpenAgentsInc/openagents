// ---------------------------------------------------------------------------
// Water in screen space (`verse_pbr::water::screen`), spliced after the
// shared `water.wgsl` into a renderer whose Medium and High tiers copy the
// opaque scene before the water draws: refraction through the scene color
// copy, absorption and contact foam from the depth copy, the planar mirror,
// and screen-space reflection. It implements public techniques,
// reimplemented here:
//
// - Refraction: the refracted view ray carried as far as the depth copy
//   says the scene lies behind the surface and projected back to the
//   screen, falling back to the unbent pixel where something in front of
//   the water would show through (Sousa, "Generic Refraction Simulation",
//   GPU Gems 2, chapter 19, 2005).
// - Planar reflection: the scene drawn once from the eye mirrored in the
//   water's plane, clipped at the plane by an oblique near plane
//   (Lengyel, "Oblique View Frustum Depth Projection and Clipping", JGT
//   2005), and sampled where the bent reflected ray lands.
// - Screen-space reflection: a ray marched across the screen with its
//   reciprocal depth interpolated linearly in screen space, tested against
//   the depth copy with a thickness, and refined by bisection (McGuire and
//   Mara, "Efficient GPU Screen-Space Ray Tracing", JCGT 2014), jittered by
//   interleaved gradient noise (Jimenez, "Next Generation Post Processing
//   in Call of Duty: Advanced Warfare", SIGGRAPH 2014).
//
// The host declares these bindings and hooks:
//
//   var water_scene: texture_2d<f32>;        // the opaque scene, resolved
//   var water_scene_depth: texture_2d<f32>;  // its view depth (clip w), m
//   var water_mirror: texture_2d<f32>;       // the planar mirror
//   var water_screen_sampler: sampler;       // linear, clamped
//   fn water_host_view_proj() -> mat4x4<f32>      // world to clip
//   fn water_host_inv_view_proj() -> mat4x4<f32>  // its inverse
//   fn water_host_eye() -> vec3<f32>
//
// The copies are in the host's output units: exposed and fogged.

// The depth copy's value where nothing was drawn (the sky), m
// (`screen::FAR`).
const WATER_FAR: f32 = 60000.0;
// How far behind the surface refraction looks for the bent ray's end, m.
const WATER_REFRACT_REACH: f32 = 3.0;

// The clip w of a world point: its depth along the view, m.
fn water_view_w(p: vec3<f32>) -> f32 {
    let m = water_host_view_proj();
    return dot(vec4<f32>(m[0].w, m[1].w, m[2].w, m[3].w), vec4<f32>(p, 1.0));
}

// A world point's texture coordinate on the screen (xy) and its view
// depth (z).
fn water_project(p: vec3<f32>) -> vec3<f32> {
    let c = water_host_view_proj() * vec4<f32>(p, 1.0);
    let w = max(c.w, 1e-4);
    return vec3<f32>(c.x / w * 0.5 + 0.5, 0.5 - c.y / w * 0.5, c.w);
}

fn water_on_screen(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
}

// The opaque scene's view depth at `uv`, m; `WATER_FAR` for the sky.
fn water_scene_w(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(water_scene_depth));
    let p = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(water_scene_depth, p, 0).r;
}

// The world point on the ray through `uv` at view depth `w`.
fn water_unproject(uv: vec2<f32>, w: f32) -> vec3<f32> {
    let m = water_host_view_proj();
    let inv = water_host_inv_view_proj();
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let a = inv * vec4<f32>(ndc, 1.0, 1.0);
    let b = inv * vec4<f32>(ndc, 0.5, 1.0);
    let d = normalize(b.xyz / b.w - a.xyz / a.w);
    let row = vec3<f32>(m[0].w, m[1].w, m[2].w);
    let eye = water_host_eye();
    return eye + d * (w / max(dot(row, d), 1e-5));
}

// What the surface lets through, read from the scene copy.
struct WaterRefraction {
    // Where to read the scene color copy; one coordinate per channel, for
    // dispersion.
    uv_r: vec2<f32>,
    uv_g: vec2<f32>,
    uv_b: vec2<f32>,
    // The water the refracted view crosses (m): the depth over the scene
    // point it shows, along the refracted ray as the baked path runs
    // (`water_transmittance`); and the straight view's distance to the
    // scene behind (m), where foam gathers.
    path: f32,
    contact: f32,
    // 1 when the depth copy found the scene behind; 0 over the sky.
    found: f32,
};

// How far the bent view moves on the screen from the flat surface's: the
// refracted ray through `n` and through a level surface, each carried
// `reach` m into water of index `ior` and projected. A level surface
// keeps the straight view, which the copy already holds, so only the
// surface's slope moves what shows through it.
fn water_refract_shift(world: vec3<f32>, v: vec3<f32>, n: vec3<f32>, ior: f32, reach: f32) -> vec2<f32> {
    let bent = water_project(world + water_refracted(v, n, ior) * reach).xy;
    let level = water_project(world + water_refracted(v, vec3<f32>(0.0, 1.0, 0.0), ior) * reach).xy;
    return bent - level;
}

// Refraction at a surface fragment at `world`, view depth `w`, on screen
// at `uv0`, with shading normal `n`, where `shown` of the surface shows
// (`water_edge`: none where it fades out at the very edge, which bends
// nothing). `dispersion` spreads the channels' indices (High): water's
// index runs from about 1.331 in red to 1.343 in blue.
fn water_screen_refraction(world: vec3<f32>, v: vec3<f32>, n: vec3<f32>, w: f32, uv0: vec2<f32>, shown: f32, dispersion: f32) -> WaterRefraction {
    var o: WaterRefraction;
    o.uv_r = uv0;
    o.uv_g = uv0;
    o.uv_b = uv0;
    let w0 = water_scene_w(uv0);
    if w0 >= WATER_FAR * 0.5 {
        o.found = 0.0;
        o.contact = WATER_FAR;
        return o;
    }
    o.found = 1.0;
    let behind = water_unproject(uv0, w0);
    o.contact = select(0.0, distance(world, behind), w0 > w);
    // The refracted ray runs down at its own angle whatever the view's, so
    // a grazing view still crosses only the depth over what it shows.
    let slant = 1.0 / max(water_cos_refracted(clamp(v.y, 0.0, 1.0)), 0.05);
    o.path = max(world.y - behind.y, 0.0) * slant;
    // Thin water bends little: the shift grows with the water behind.
    let reach = min(o.contact, WATER_REFRACT_REACH) * clamp(shown, 0.0, 1.0);
    let uv = uv0 + water_refract_shift(world, v, n, WATER_IOR, reach);
    let w1 = water_scene_w(uv);
    // Whatever stands in front of the surface must not show through it.
    if !water_on_screen(uv) || w1 < w || w1 >= WATER_FAR * 0.5 {
        return o;
    }
    o.uv_g = uv;
    o.uv_r = uv;
    o.uv_b = uv;
    o.path = max(world.y - water_unproject(uv, w1).y, 0.0) * slant;
    if dispersion > 0.0 {
        let r = uv0 + water_refract_shift(world, v, n, WATER_IOR - 0.004 * dispersion, reach);
        let b = uv0 + water_refract_shift(world, v, n, WATER_IOR + 0.012 * dispersion, reach);
        o.uv_r = select(uv, r, water_on_screen(r) && water_scene_w(r) >= w);
        o.uv_b = select(uv, b, water_on_screen(b) && water_scene_w(b) >= w);
    }
    return o;
}

// The scene behind the surface through `r`, in the host's output units.
fn water_scene_through(r: WaterRefraction) -> vec3<f32> {
    let red = textureSampleLevel(water_scene, water_screen_sampler, r.uv_r, 0.0).r;
    let green = textureSampleLevel(water_scene, water_screen_sampler, r.uv_g, 0.0).g;
    let blue = textureSampleLevel(water_scene, water_screen_sampler, r.uv_b, 0.0).b;
    return vec3<f32>(red, green, blue);
}

// How much of the shading normal's slope bends what the mirror and the
// march reflect. The fine detail that shades the sky's reflection would
// scatter a sharp image into noise at full strength; real ripples on
// calm water bend a reflection by a fraction of that.
const WATER_REFLECT_BEND: f32 = 0.3;

// The normal reflections are traced along: `n` with its slope scaled by
// `WATER_REFLECT_BEND`.
fn water_reflect_normal(n: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(n.x * WATER_REFLECT_BEND, n.y, n.z * WATER_REFLECT_BEND));
}

// The planar mirror's reflection at a fragment on screen at `uv0`: the
// mirror drew the scene as seen along the flat surface's reflected rays,
// so the bent normal `n` moves the lookup by where its reflected ray lands
// a few meters out compared with the flat one's.
fn water_mirror_at(world: vec3<f32>, v: vec3<f32>, n: vec3<f32>, uv0: vec2<f32>) -> vec3<f32> {
    let reach = 4.0;
    let flat = water_project(world + reflect(-v, vec3<f32>(0.0, 1.0, 0.0)) * reach).xy;
    let bent = water_project(world + reflect(-v, water_reflect_normal(n)) * reach).xy;
    let uv = clamp(uv0 + (bent - flat), vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSampleLevel(water_mirror, water_screen_sampler, uv, 0.0).rgb;
}

// Interleaved gradient noise at a pixel (Jimenez 2014), 0 to 1.
fn water_ign(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

// A screen-space reflection's hit: where to read the scene copy and how
// much to trust it, 0 to 1.
struct WaterHit {
    uv: vec2<f32>,
    weight: f32,
};

// Marches the reflected ray `r` from the surface point `world` across the
// screen in up to `steps` steps (McGuire and Mara 2014): the ray's
// reciprocal view depth is linear in screen space, so each step reads its
// depth without a matrix. A hit is a step whose depth span reaches behind
// the depth copy by less than a thickness, refined by bisection; the hit
// must stand above the surface, which a reflection can only see.
fn water_ssr(world: vec3<f32>, r: vec3<f32>, steps: i32, pixel: vec2<f32>) -> WaterHit {
    var o: WaterHit;
    o.weight = 0.0;
    let size = vec2<f32>(textureDimensions(water_scene_depth));
    let m = water_host_view_proj();
    let row = vec4<f32>(m[0].w, m[1].w, m[2].w, m[3].w);
    let w0 = dot(row, vec4<f32>(world, 1.0));
    let dw = dot(row.xyz, r);
    var span = 60.0;
    // Stop short of the near plane when the ray turns toward the eye.
    if dw < 0.0 {
        span = min(span, (w0 - 0.2) / -dw);
    }
    if span <= 0.05 {
        return o;
    }
    let h0 = m * vec4<f32>(world, 1.0);
    let h1 = m * vec4<f32>(world + r * span, 1.0);
    let k0 = 1.0 / h0.w;
    let k1 = 1.0 / h1.w;
    let p0 = vec2<f32>(h0.x * k0 * 0.5 + 0.5, 0.5 - h0.y * k0 * 0.5) * size;
    var p1 = vec2<f32>(h1.x * k1 * 0.5 + 0.5, 0.5 - h1.y * k1 * 0.5) * size;
    let delta = p1 - p0;
    let pixels = max(max(abs(delta.x), abs(delta.y)), 1.0);
    let count = max(steps, 1);
    let stride = max(pixels / f32(count), 1.0) / pixels;
    let jitter = water_ign(pixel);
    var prev_t = 0.0;
    var prev_w = w0;
    var hit_t = -1.0;
    for (var i = 1; i <= count; i++) {
        let t = min((f32(i) - 1.0 + jitter) * stride + stride * 0.5, 1.0);
        let p = p0 + delta * t;
        let uv = p / size;
        if !water_on_screen(uv) {
            break;
        }
        let ray_w = 1.0 / mix(k0, k1, t);
        let scene = water_scene_w(uv);
        let far = max(ray_w, prev_w);
        let near = min(ray_w, prev_w);
        let thickness = 0.35 + 0.03 * ray_w;
        if far >= scene && near <= scene + thickness && scene < WATER_FAR * 0.5 {
            hit_t = t;
            break;
        }
        prev_t = t;
        prev_w = ray_w;
        if t >= 1.0 {
            break;
        }
    }
    if hit_t < 0.0 {
        return o;
    }
    // Bisect between the last step in front and the hit.
    var lo = prev_t;
    var hi = hit_t;
    for (var j = 0; j < 4; j++) {
        let mid = 0.5 * (lo + hi);
        let uv = (p0 + delta * mid) / size;
        let ray_w = 1.0 / mix(k0, k1, mid);
        if ray_w >= water_scene_w(uv) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let uv = (p0 + delta * hi) / size;
    let scene = water_scene_w(uv);
    let at = water_unproject(uv, scene);
    if at.y < world.y - 0.05 {
        return o;
    }
    // Trust fades toward the screen's edges, the ray's end, and rays that
    // turn back toward the eye.
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let fade = smoothstep(0.0, 0.08, edge) * (1.0 - smoothstep(0.7, 1.0, hi)) * smoothstep(-0.6, 0.0, dw);
    o.uv = uv;
    o.weight = clamp(fade, 0.0, 1.0);
    return o;
}
