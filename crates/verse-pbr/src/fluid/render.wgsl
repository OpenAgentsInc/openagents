// Screen-space liquid (#11660). Particles are drawn as spheres into a
// view-depth target and summed into a thickness target; the depth is
// smoothed with a separable bilateral filter, and a full-screen pass
// rebuilds normals from it and shades the surface: Fresnel reflection of
// a sky, refraction of the scene behind with Beer-Lambert absorption, and
// a sun highlight. After van der Laan, Green and Sainz, "Screen Space
// Fluid Rendering with Curvature Flow" (I3D 2009), using the bilateral
// smoothing most later work substitutes for curvature flow.

struct View {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    size: vec2<f32>,
    radius: f32,
    far: f32,
    sun: vec3<f32>,
    blur_direction: u32,
    absorb: vec3<f32>,
    blur_scale: f32,
    tint: vec3<f32>,
    thickness_scale: f32,
}

@group(0) @binding(0) var<uniform> u: View;

struct Sprite {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) center: vec3<f32>,
}

@vertex
fn sprite_vs(@builtin(vertex_index) vertex: u32, @location(0) particle: vec4<f32>) -> Sprite {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vertex];
    let center = (u.view * vec4<f32>(particle.xyz, 1.0)).xyz;
    let at = center + vec3<f32>(corner * u.radius, 0.0);
    var out: Sprite;
    out.clip = u.proj * vec4<f32>(at, 1.0);
    out.corner = corner;
    out.center = center;
    return out;
}

struct DepthOut {
    @location(0) depth: f32,
    @builtin(frag_depth) frag: f32,
}

// View-space distance along -z of the sphere's front surface.
@fragment
fn depth_fs(in: Sprite) -> DepthOut {
    let r2 = dot(in.corner, in.corner);
    if r2 > 1.0 {
        discard;
    }
    let front = in.center + vec3<f32>(in.corner * u.radius, sqrt(1.0 - r2) * u.radius);
    let clip = u.proj * vec4<f32>(front, 1.0);
    var out: DepthOut;
    out.depth = -front.z;
    out.frag = clip.z / clip.w;
    return out;
}

@fragment
fn thickness_fs(in: Sprite) -> @location(0) vec4<f32> {
    let r2 = dot(in.corner, in.corner);
    if r2 > 1.0 {
        discard;
    }
    let t = 2.0 * u.radius * sqrt(1.0 - r2);
    return vec4<f32>(t, 0.0, 0.0, 0.0);
}

struct Screen {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn screen_vs(@builtin(vertex_index) vertex: u32) -> Screen {
    let xy = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    var out: Screen;
    out.clip = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return out;
}

@group(1) @binding(0) var source: texture_2d<f32>;

// One direction of a bilateral blur of view depth: taps further in depth
// than about two particle radii are left out, so separate sheets stay
// separate and silhouettes stay sharp.
@fragment
fn blur_fs(in: Screen) -> @location(0) f32 {
    let p = vec2<i32>(in.clip.xy);
    let d0 = textureLoad(source, p, 0).r;
    if d0 >= u.far {
        return d0;
    }
    // The blur covers about two particle radii on screen.
    let pixels = clamp(u.blur_scale * u.radius / d0, 1.0, 24.0);
    let step = select(vec2<i32>(0, 1), vec2<i32>(1, 0), u.blur_direction == 0u);
    var sum = 0.0;
    var weight = 0.0;
    let sigma = pixels * 0.5;
    let range = 2.0 * u.radius;
    let limit = vec2<i32>(u.size) - vec2<i32>(1);
    for (var k = -12; k <= 12; k = k + 1) {
        let offset = f32(k) * pixels / 12.0;
        let q = clamp(p + step * i32(round(offset)), vec2<i32>(0), limit);
        let d = textureLoad(source, q, 0).r;
        if d >= u.far {
            continue;
        }
        let dz = (d - d0) / range;
        let w = exp(-offset * offset / (2.0 * sigma * sigma)) * exp(-dz * dz);
        sum = sum + d * w;
        weight = weight + w;
    }
    return sum / max(weight, 1e-6);
}

@group(1) @binding(1) var thickness: texture_2d<f32>;
@group(1) @binding(2) var background: texture_2d<f32>;
@group(1) @binding(3) var scene_depth: texture_2d<f32>;
@group(1) @binding(4) var linear: sampler;

fn view_position(p: vec2<i32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(
        (f32(p.x) + 0.5) / u.size.x * 2.0 - 1.0,
        1.0 - (f32(p.y) + 0.5) / u.size.y * 2.0,
    );
    let ray = u.inv_proj * vec4<f32>(ndc, 1.0, 1.0);
    let dir = ray.xyz / ray.w;
    return dir * (depth / -dir.z);
}

fn sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    let horizon = vec3<f32>(0.78, 0.84, 0.9);
    let zenith = vec3<f32>(0.32, 0.5, 0.78);
    let ground = vec3<f32>(0.32, 0.3, 0.28);
    var c = mix(horizon, zenith, pow(up, 0.6));
    if dir.y < 0.0 {
        c = mix(horizon, ground, min(-dir.y * 3.0, 1.0));
    }
    let s = max(dot(dir, normalize(u.sun)), 0.0);
    return c + vec3<f32>(1.0, 0.95, 0.85) * pow(s, 400.0) * 8.0;
}

@fragment
fn shade_fs(in: Screen) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.clip.xy);
    let back = textureLoad(background, p, 0).rgb;
    let depth = textureLoad(source, p, 0).r;
    let scene = textureLoad(scene_depth, p, 0).r;
    if depth >= u.far || depth > scene {
        return vec4<f32>(back, 1.0);
    }
    let limit = vec2<i32>(u.size) - vec2<i32>(1);
    let pos = view_position(p, depth);
    // Normals from the nearer of each pair of neighbours, so edges do not
    // bend toward the background.
    let xr = clamp(p + vec2<i32>(1, 0), vec2<i32>(0), limit);
    let xl = clamp(p - vec2<i32>(1, 0), vec2<i32>(0), limit);
    let yd = clamp(p + vec2<i32>(0, 1), vec2<i32>(0), limit);
    let yu = clamp(p - vec2<i32>(0, 1), vec2<i32>(0), limit);
    let dr = view_position(xr, textureLoad(source, xr, 0).r) - pos;
    let dl = pos - view_position(xl, textureLoad(source, xl, 0).r);
    let dd = view_position(yd, textureLoad(source, yd, 0).r) - pos;
    let du = pos - view_position(yu, textureLoad(source, yu, 0).r);
    let ddx = select(dl, dr, abs(dr.z) < abs(dl.z));
    let ddy = select(du, dd, abs(dd.z) < abs(du.z));
    var n = normalize(cross(ddy, ddx));
    if n.z < 0.0 {
        n = -n;
    }
    let to_eye = normalize(-pos);
    let world_n = normalize((u.inv_view * vec4<f32>(n, 0.0)).xyz);
    let world_v = normalize((u.inv_view * vec4<f32>(to_eye, 0.0)).xyz);
    let t = textureLoad(thickness, p, 0).r * u.thickness_scale;
    // Refraction: the scene behind, shifted along the normal and dimmed
    // by the liquid it passes through.
    let shift = n.xy * min(t, 0.2) * 0.25 * vec2<f32>(1.0, -1.0);
    let refracted = textureSampleLevel(background, linear, in.uv + shift, 0.0).rgb;
    let through = refracted * exp(-u.absorb * t) + u.tint * (1.0 - exp(-t * 6.0)) * 0.12;
    let cos_theta = max(dot(world_n, world_v), 0.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - cos_theta, 5.0);
    let reflected = sky(reflect(-world_v, world_n));
    let half_vector = normalize(normalize(u.sun) + world_v);
    let highlight = pow(max(dot(world_n, half_vector), 0.0), 600.0) * 3.0;
    let color = mix(through, reflected, fresnel) + vec3<f32>(highlight);
    return vec4<f32>(color, 1.0);
}
