struct Frame {
    viewProjection: mat4x4<f32>,
    inverseViewProjection: mat4x4<f32>,
    nearShadow: mat4x4<f32>,
    farShadow: mat4x4<f32>,
    eye: vec4<f32>,
    sun: vec4<f32>,
    sunColor: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,
    settings: vec4<f32>,
}
struct Draw {
    model: mat4x4<f32>,
    atlasRect: vec4<f32>,
    uv: vec4<f32>,
    baseColor: vec4<f32>,
    material: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var baseImages: texture_2d_array<f32>;
@group(0) @binding(2) var imageSampler: sampler;
@group(0) @binding(3) var normalImages: texture_2d_array<f32>;
@group(0) @binding(4) var mraImages: texture_2d_array<f32>;
@group(0) @binding(5) var nearShadowMap: texture_depth_2d;
@group(0) @binding(6) var farShadowMap: texture_depth_2d;
@group(0) @binding(7) var shadowSampler: sampler_comparison;
@group(1) @binding(0) var<uniform> draw: Draw;
struct Instance {
    model: mat4x4<f32>,
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
}
@group(1) @binding(1) var<storage, read> instances: array<Instance>;
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
}
struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) @interpolate(flat) orientation: f32,
}
@vertex fn worldVertex(v: Vertex, @builtin(instance_index) index: u32) -> Varying {
    var out: Varying;
    let instance = instances[index];
    let model = draw.model * instance.model;
    let normalMatrix = mat3x3(instance.normal0.xyz, instance.normal1.xyz, instance.normal2.xyz);
    let world = model * vec4(v.position, 1.0);
    out.clip = frame.viewProjection * world;
    out.position = world.xyz;
    out.normal = normalize((draw.model * vec4(normalMatrix * v.normal, 0.0)).xyz);
    out.tangent = vec4(normalize((model * vec4(v.tangent.xyz, 0.0)).xyz), v.tangent.w * instance.normal0.w);
    out.orientation = instance.normal0.w;
    out.uv = v.uv;
    return out;
}
fn materialUV(uv: vec2<f32>) -> vec2<f32> {
    // Array layers repeat without atlas seams. The rectangle also supports old packages.
    return draw.atlasRect.xy + fract(uv * draw.uv.xy) * draw.atlasRect.zw;
}
struct DepthVarying {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn nearShadowVertex(v: Vertex, @builtin(instance_index) index: u32) -> DepthVarying {
    return DepthVarying(frame.nearShadow * draw.model * instances[index].model * vec4(v.position, 1.0), v.uv);
}
@vertex fn farShadowVertex(v: Vertex, @builtin(instance_index) index: u32) -> DepthVarying {
    return DepthVarying(frame.farShadow * draw.model * instances[index].model * vec4(v.position, 1.0), v.uv);
}
@fragment fn shadowFragment(v: DepthVarying) {
    let alpha = textureSample(baseImages, imageSampler, materialUV(v.uv), i32(draw.uv.w)).a * draw.baseColor.a;
    if alpha < draw.uv.z { discard; }
}
struct ShadowProjection { uv: vec2<f32>, depth: f32, edge: f32, inside: bool }
fn projectShadow(projection: mat4x4<f32>, p: vec3<f32>) -> ShadowProjection {
    let clip = projection * vec4(p, 1.0);
    let ndc = clip.xyz / clip.w;
    return ShadowProjection(ndc.xy * vec2(0.5, -0.5) + vec2(0.5), ndc.z,
        max(abs(ndc.x), abs(ndc.y)), clip.w > 0.0 && all(abs(ndc.xy) <= vec2(1.0)) && ndc.z >= 0.0 && ndc.z <= 1.0);
}
fn shadowPcf(map: texture_depth_2d, projection: mat4x4<f32>, p: vec3<f32>, normal: vec3<f32>, offset: f32) -> f32 {
    let q = projectShadow(projection, p + normal * offset);
    if !q.inside { return 1.0; }
    let dimensions = vec2<f32>(textureDimensions(map));
    let rowX = vec3(projection[0].x, projection[1].x, projection[2].x);
    let rowY = vec3(projection[0].y, projection[1].y, projection[2].y);
    let rowZ = vec3(projection[0].z, projection[1].z, projection[2].z);
    let texelMeters = max(2.0 / (dimensions.x * length(rowX)), 2.0 / (dimensions.y * length(rowY)));
    let slope = 1.0 - clamp(dot(normal, frame.sun.xyz), 0.0, 1.0);
    let bias = texelMeters * length(rowZ) * (0.2 + 1.2 * slope);
    var visibility = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            visibility += textureSampleCompareLevel(map, shadowSampler, q.uv + vec2(f32(x), f32(y)) / dimensions, q.depth - bias);
        }
    }
    return visibility / 9.0;
}
fn sunVisibility(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    let q = projectShadow(frame.nearShadow, p);
    if !q.inside { return shadowPcf(farShadowMap, frame.farShadow, p, normal, 0.03); }
    let near = shadowPcf(nearShadowMap, frame.nearShadow, p, normal, 0.008);
    if q.edge < 0.8 { return near; }
    return mix(near, shadowPcf(farShadowMap, frame.farShadow, p, normal, 0.03), smoothstep(0.8, 1.0, q.edge));
}
fn skyRadiance(direction: vec3<f32>) -> vec3<f32> {
    let height = max(direction.y, 0.0);
    let horizon = vec3(0.48, 0.65, 0.85);
    let zenith = vec3(0.075, 0.24, 0.52);
    var color = mix(horizon, zenith, pow(height, 0.45));
    let sunAngle = max(dot(direction, frame.sun.xyz), 0.0);
    color += frame.sunColor.rgb * (pow(sunAngle, 40.0) * 0.025 + smoothstep(0.99990, 0.99997, sunAngle) * 3.5);
    return mix(vec3(0.17, 0.20, 0.24), color, smoothstep(-0.10, 0.02, direction.y));
}
fn displayColor(linear: vec3<f32>) -> vec3<f32> {
    let x = max(linear * frame.settings.x, vec3(0.0));
    // Filmic shoulder preserves bright stone and sun highlights before the sRGB surface conversion.
    return clamp((x * (2.51 * x + vec3(0.03))) / (x * (2.43 * x + vec3(0.59)) + vec3(0.14)), vec3(0.0), vec3(1.0));
}
fn fresnel(cosine: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3(1.0) - f0) * pow(1.0 - cosine, 5.0);
}
@fragment fn worldFragment(v: Varying, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let uv = materialUV(v.uv);
    let layer = i32(draw.uv.w);
    // Gradients are computed before wrapping, so tiled surfaces choose the correct mip at seams.
    let dx = dpdx(v.uv * draw.uv.xy) * draw.atlasRect.zw;
    let dy = dpdy(v.uv * draw.uv.xy) * draw.atlasRect.zw;
    let sampled = textureSampleGrad(baseImages, imageSampler, uv, layer, dx, dy) * draw.baseColor;
    let mapXY = textureSampleGrad(normalImages, imageSampler, uv, layer, dx, dy).rg * 2.0 - vec2(1.0);
    let packed = textureSampleGrad(mraImages, imageSampler, uv, layer, dx, dy).rgb;
    if sampled.a < draw.uv.z { discard; }
    let face = select(-1.0, 1.0, front == (v.orientation > 0.0));
    let sourceNormal = normalize(v.normal);
    let geometricNormal = sourceNormal * face;
    let tangent = normalize(v.tangent.xyz - sourceNormal * dot(sourceNormal, v.tangent.xyz));
    let bitangent = cross(sourceNormal, tangent) * v.tangent.w;
    let xy = mapXY * draw.material.z;
    let tangentNormal = normalize(vec3(xy, sqrt(max(1.0 - dot(xy, xy), 0.0001))));
    let normal = normalize(mat3x3(tangent, bitangent, sourceNormal) * tangentNormal) * face;
    let base = sampled.rgb;
    let roughness = clamp(packed.g * draw.material.x, 0.16, 1.0);
    let metallic = clamp(packed.r * draw.material.y, 0.0, 1.0);
    let ao = clamp(packed.b, 0.0, 1.0);
    let view = normalize(frame.eye.xyz - v.position);
    let light = frame.sun.xyz;
    let halfVector = normalize(view + light);
    let nDotV = max(dot(normal, view), 0.0001);
    let nDotL = max(dot(normal, light), 0.0);
    let nDotH = max(dot(normal, halfVector), 0.0);
    let vDotH = max(dot(view, halfVector), 0.0);
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denominator = nDotH * nDotH * (alpha2 - 1.0) + 1.0;
    let distribution = alpha2 / max(3.14159265 * denominator * denominator, 0.0001);
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let geometryV = nDotV / (nDotV * (1.0 - k) + k);
    let geometryL = nDotL / (nDotL * (1.0 - k) + k);
    let f0 = mix(vec3(0.04), base, metallic);
    let f = fresnel(vDotH, f0);
    let specular = distribution * geometryV * geometryL * f / max(4.0 * nDotV * nDotL, 0.0001);
    let diffuse = (vec3(1.0) - f) * (1.0 - metallic) * base / 3.14159265;
    let direct = (diffuse + specular) * frame.sunColor.rgb * nDotL * sunVisibility(v.position, geometricNormal);
    let skyFill = mix(frame.ambient.rgb * 0.32, frame.ambient.rgb, normal.y * 0.5 + 0.5);
    let environment = skyRadiance(reflect(-view, normal)) * fresnel(nDotV, f0) * (1.0 - roughness * 0.75) * 0.32;
    var linear = direct + (base * (1.0 - metallic) * skyFill + environment) * ao;
    let distance = length(v.position - frame.eye.xyz);
    let density = frame.fog.x * exp(-max(v.position.y - frame.fog.y, 0.0) * frame.fog.z);
    let transmittance = exp(-max(distance - frame.fog.w, 0.0) * density);
    let air = skyRadiance(normalize(v.position - frame.eye.xyz));
    linear = mix(air, linear, transmittance);
    return vec4(displayColor(linear), 1.0);
}
struct Sky { @builtin(position) position: vec4<f32>, @location(0) ndc: vec2<f32> }
@vertex fn skyVertex(@builtin(vertex_index) i: u32) -> Sky {
    let p = array<vec2<f32>, 3>(vec2(-1.0,-1.0), vec2(3.0,-1.0), vec2(-1.0,3.0));
    return Sky(vec4(p[i], 1.0, 1.0), p[i]);
}
@fragment fn skyFragment(v: Sky) -> @location(0) vec4<f32> {
    let world = frame.inverseViewProjection * vec4(v.ndc, 1.0, 1.0);
    let direction = normalize(world.xyz / world.w - frame.eye.xyz);
    return vec4(displayColor(skyRadiance(direction)), 1.0);
}
