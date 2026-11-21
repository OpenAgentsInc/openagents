// Firm reason: 6 (AppKit/Metal 3D Metaverse Grid Flythrough viewport over CAMetalLayer driven by Metaverse Engine Coax flyover spline + rh-math.coax).
import AppKit
import Metal
import QuartzCore
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
import simd

// MARK: - Coax Scene Payload Models (from oa_dc_rh3d_scene_json)

private struct Rh3dStationData {
    let index: Int
    let pos: SIMD3<Float>
    let eye: SIMD3<Float>
    let timeS: Float
    let shortName: String
    let title: String
    let subtitle: String
    let plainEnglish: String
    let moduleName: String
    let accent: SIMD4<Float>
}

private struct Rh3dLabelAnchor {
    let worldPos: SIMD3<Float>
    let text: String
    let subtext: String
    let color: NSColor
    let stationIndex: Int
    let isStationBanner: Bool
    let maxDist: Float
}

private struct Rh3dPickable {
    let center: SIMD3<Float>
    let radius: Float
    let title: String
    let detail: String
    let moduleName: String
    let stationIndex: Int
}

// MARK: - OpenAgents Grid Visual Tokens (from metaverse-engine/ui/hud-frame.coax & apps/openagents-web/newland.coax)

private enum GridTokens {
    // 90% Monochrome Obsidian & Luminous Silver-White Architecture
    static let greyWhite = SIMD4<Float>(0.94, 0.95, 0.96, 1.0)
    static let greyLight = SIMD4<Float>(0.78, 0.80, 0.82, 1.0)
    static let greyMid   = SIMD4<Float>(0.56, 0.59, 0.62, 1.0)
    static let greyDim   = SIMD4<Float>(0.36, 0.39, 0.42, 1.0)

    // Deep obsidian charcoal faces (exact pack_grid.py DARK = 0.008 linear matching the-grid.jpg)
    static let obsidianDeep  = SIMD4<Float>(0.0045, 0.0052, 0.0064, 1.0)
    static let obsidianBody  = SIMD4<Float>(0.0075, 0.0086, 0.0105, 1.0)
    static let obsidianSlate = SIMD4<Float>(0.0115, 0.0130, 0.0160, 1.0)
    static let obsidianGlass = SIMD4<Float>(0.010, 0.013, 0.018, 0.22)

    // Signature OpenAgents Electric Cyan Accent (HUD_ACCENT = hsl(180, 68.07%, 53.33%))
    static let cyan      = SIMD4<Float>(0.216, 0.851, 0.851, 1.0)
    static let cyanSoft  = SIMD4<Float>(0.216, 0.851, 0.851, 0.72)
    // Surgical Warm Amber-Gold Accent (aligned with OpenAgents Bitcoin orange / milestone highlight)
    static let amber     = SIMD4<Float>(1.000, 0.650, 0.180, 1.0)
    static let amberHot  = SIMD4<Float>(1.000, 0.800, 0.340, 1.0)
    // Surgical Alert Crimson-Amber (ONLY for the 2/3 classical wall & pre-5040 violations)
    static let alertRed  = SIMD4<Float>(0.960, 0.260, 0.240, 1.0)

    static let nsWhite = NSColor(calibratedRed: 0.94, green: 0.95, blue: 0.96, alpha: 1.0)
    static let nsLight = NSColor(calibratedRed: 0.78, green: 0.80, blue: 0.82, alpha: 1.0)
    static let nsMid   = NSColor(calibratedRed: 0.58, green: 0.60, blue: 0.63, alpha: 1.0)
    static let nsCyan  = NSColor(calibratedRed: 0.216, green: 0.851, blue: 0.851, alpha: 1.0)
    static let nsAmber = NSColor(calibratedRed: 1.000, green: 0.680, blue: 0.220, alpha: 1.0)
    static let nsRed   = NSColor(calibratedRed: 0.960, green: 0.300, blue: 0.280, alpha: 1.0)
}

// MARK: - Metal Vertex / Instance / Uniform Structs (16-byte aligned across Swift & Metal)

private struct Rh3dMeshVert {
    var pos: SIMD4<Float>    // xyz = local pos, w = capFlag (1 on cylinder caps, 0 on walls)
    var normal: SIMD4<Float> // xyz = local normal, w = 0
    var uv: SIMD4<Float>     // xy = face uv [0,1], zw = barycentric (for octahedron facets)
}

private struct Rh3dTerrainVert {
    var posHeight: SIMD4<Float>  // xyz = world pos, w = zeta |z| height
    var normalCrit: SIMD4<Float> // xyz = world normal, w = distance from Re(s)=1/2
    var uvPad: SIMD4<Float>      // x = sigma_u [0,1], y = t_v [0,1], z = 0, w = 0
}

private struct Rh3dQuadLineSeg {
    var p0Width: SIMD4<Float> // xyz = start world pos, w = core half-width in px
    var p1Glow: SIMD4<Float>  // xyz = end world pos, w = halo radius in px
    var color: SIMD4<Float>   // rgb = HDR linear color, a = base light multiplier
    var anim: SIMD4<Float>    // x = pulseSpeed, y = pulsePhase, z = mode (0=solid, 1=pulse conduit, 2=soft link), w = pad
}

private struct Rh3dInstance {
    var model: simd_float4x4
    var faceColor: SIMD4<Float>
    var edgeColor: SIMD4<Float> // rgb = linear edge color, a = HDR edge intensity
    var params: SIMD4<Float>    // x = mode (0=box, 1=skyscraper, 2=cyl, 3=crystal, 4=laserPlane), y = edgePx, z = floorBands, w = pulsePhase
}

private struct Rh3dUniforms {
    var viewProj: simd_float4x4
    var invViewProj: simd_float4x4
    var cameraPos: SIMD4<Float>
    var viewport: SIMD4<Float>  // x = width, y = height, z = scale, w = time_s
    var st0: SIMD4<Float>       // x = station X, y = station Z, z = accentMode (0=cyan, 1=amber, 2=red, 3=white), w = activeWeight
    var st1: SIMD4<Float>
    var st2: SIMD4<Float>
    var st3: SIMD4<Float>
    var st4: SIMD4<Float>
    var st5: SIMD4<Float>
    var st6: SIMD4<Float>
    var st7: SIMD4<Float>
}

private struct Rh3dPostUniforms {
    var texelDir: SIMD4<Float>  // xy = 1/texSize, zw = blur direction (1,0) or (0,1)
    var params: SIMD4<Float>    // x = bloomThreshold, y = bloomKnee, z = halfWeight, w = quarterWeight
}

// MARK: - OpenAgents Grid Metal Shaders (Sky + Analytical Infinite Grid + Obsidian Wireframe + Quad Lines + HDR Bloom + metaverse_tone)

private let rh3dMetalShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct MeshVert {
    float4 pos;
    float4 normal;
    float4 uv;
};

struct TerrainVert {
    float4 posHeight;
    float4 normalCrit;
    float4 uvPad;
};

struct QuadLineSeg {
    float4 p0Width;
    float4 p1Glow;
    float4 color;
    float4 anim;
};

struct Instance {
    float4x4 model;
    float4 faceColor;
    float4 edgeColor;
    float4 params;
};

struct Uniforms {
    float4x4 viewProj;
    float4x4 invViewProj;
    float4 cameraPos;
    float4 viewport;
    float4 stations[8];
};

struct PostUniforms {
    float4 texelDir;
    float4 params;
};

// Shared lighting from metaverse/light.coax (metaverse_lit & metaverse_tone)
static inline float grid_lit(float3 N) {
    float3 sun = normalize(float3(0.30, 0.90, 0.35));
    float d = max(dot(N, sun), 0.0);
    float hemi = clamp(N.y * 0.5 + 0.5, 0.0, 1.0);
    return 0.90 * d + 0.05 + 0.27 * hemi;
}

static inline float grid_fog(float dist) {
    // Matches metaverse/grid.coax smoothstep recession into obsidian void
    float t = clamp((dist - 16.0) / 118.0, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

static inline float3 station_accent(float mode) {
    if (mode > 2.5) return float3(0.94, 0.95, 0.96);
    if (mode > 1.5) return float3(0.96, 0.26, 0.24);
    if (mode > 0.5) return float3(1.00, 0.65, 0.18);
    return float3(0.216, 0.851, 0.851);
}

// Hash & smooth fBM matching apps/openagents-web/gpu/newland.coax
static inline float nhash(float2 p) {
    return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453);
}

static inline float nnoise(float2 p) {
    float2 i = floor(p);
    float2 f = fract(p);
    float2 s = f * f * (3.0 - 2.0 * f);
    float a = mix(nhash(i), nhash(i + float2(1.0, 0.0)), s.x);
    float b = mix(nhash(i + float2(0.0, 1.0)), nhash(i + float2(1.0, 1.0)), s.x);
    return mix(a, b, s.y);
}

static inline float nfbm(float2 q) {
    float2 p = q;
    float v = 0.0;
    float a = 0.5;
    for (int k = 0; k < 4; ++k) {
        v += a * nnoise(p);
        p = p * 2.03 + float2(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}

// ---- 1. Fullscreen OpenAgents Grid Obsidian Void Sky (newland_bg_fs inspired) ----

struct SkyOut {
    float4 position [[position]];
    float2 ndc;
};

vertex SkyOut rh3d_sky_vs(uint vid [[vertex_id]]) {
    float2 verts[3] = { float2(-1.0, -3.0), float2(-1.0, 1.0), float2(3.0, 1.0) };
    SkyOut o;
    o.ndc = verts[vid];
    o.position = float4(verts[vid], 0.9999, 1.0);
    return o;
}

fragment float4 rh3d_sky_fs(SkyOut in [[stage_in]], constant Uniforms &u [[buffer(0)]]) {
    // Reconstruct world-space ray direction from NDC so horizon & stars stay anchored in 3D
    float4 farClip = u.invViewProj * float4(in.ndc, 1.0, 1.0);
    float3 farWorld = farClip.xyz / max(abs(farClip.w), 1e-5);
    float3 rd = normalize(farWorld - u.cameraPos.xyz);

    // Pure pitch-black obsidian base (#020203 linear) seamlessly matching distant floor fade
    float3 col = float3(0.0014, 0.0016, 0.0020);

    // Subtle monochrome newland grey nebula ONLY high above the horizon so the Grid floor melts seamlessly into pure black
    float t = u.viewport.w * 0.018;
    float2 skyUV = float2(atan2(rd.z, rd.x) * 0.65, rd.y * 2.4);
    float2 w = float2(nfbm(skyUV * 1.4 + float2(t, -t)), nfbm(skyUV * 1.4 + float2(-t, t * 0.7) + 4.0));
    float n = nfbm(skyUV * 2.0 + 1.6 * w);
    float aboveHorizon = smoothstep(0.02, 0.28, rd.y);

    col += float3(0.0065, 0.0072, 0.0088) * (n * n) * aboveHorizon;

    // Sparse crisp pinhole stars high in the void
    if (rd.y > 0.04) {
        float2 cell = floor(in.position.xy * 0.5);
        float h = nhash(cell);
        float star = step(0.9986, h) * (0.45 + 0.55 * sin(u.viewport.w * 1.4 + h * 60.0));
        col += float3(0.75, 0.80, 0.88) * star * 0.28 * smoothstep(0.04, 0.28, rd.y);
    }

    return float4(col, 1.0);
}

// ---- 2. Infinite Analytical Anti-Aliased OpenAgents Grid Floor ----

struct FloorOut {
    float4 position [[position]];
    float3 worldPos;
};

vertex FloorOut rh3d_floor_vs(uint vid [[vertex_id]], constant Uniforms &u [[buffer(0)]]) {
    float2 corners[6] = {
        float2(-260.0, -260.0), float2( 260.0, -260.0), float2( 260.0,  260.0),
        float2(-260.0, -260.0), float2( 260.0,  260.0), float2(-260.0,  260.0)
    };
    float2 p = corners[vid];
    // Keep floor centered near camera X/Z so horizon is truly infinite as you fly
    float cx = floor(u.cameraPos.x / 12.0) * 12.0;
    float cz = floor(u.cameraPos.z / 12.0) * 12.0;
    float3 wp = float3(p.x + cx, 0.0, p.y + cz);
    FloorOut o;
    o.position = u.viewProj * float4(wp, 1.0);
    o.worldPos = wp;
    return o;
}

static inline float analytical_grid_line(float2 coord, float2 deriv, float lineWidthPx) {
    float2 grid = abs(fract(coord - 0.5) - 0.5) / max(deriv, float2(1e-5));
    float d = min(grid.x, grid.y);
    float aa = 1.0 - smoothstep(lineWidthPx * 0.5 - 0.5, lineWidthPx * 0.5 + 0.65, d);
    // Moire suppression at extreme grazing distances
    float2 fade = clamp(1.0 - max(deriv - 0.25, 0.0) * 1.6, 0.0, 1.0);
    return aa * min(fade.x, fade.y);
}

fragment float4 rh3d_floor_fs(FloorOut in [[stage_in]], constant Uniforms &u [[buffer(0)]]) {
    float3 wp = in.worldPos;
    float dist = length(u.cameraPos.xyz - wp);
    float fog = grid_fog(dist);

    // Pure pitch-black obsidian floor base (matches the-grid.jpg)
    float3 col = float3(0.0014, 0.0016, 0.0020);

    // 1. Minor 2.0 m Grid lines (crisp monochrome silver-grey, zero aliasing)
    float2 minorCoord = wp.xz / 2.0;
    float2 minorDeriv = fwidth(minorCoord);
    float minorLine = analytical_grid_line(minorCoord, minorDeriv, 1.05);

    // 2. Major 12.0 m Street Grid lines (aligned so each station at x = -42+12k sits centered in its 12x12m Grid bay!)
    float2 majorCoord = (wp.xz + float2(0.0, 6.0)) / 12.0;
    float2 majorDeriv = fwidth(majorCoord);
    float majorLine = analytical_grid_line(majorCoord, majorDeriv, 1.45);

    // Subtle intersection cross-ticks at every 12m street corner
    float2 cellD = abs(fract(majorCoord - 0.5) - 0.5);
    float crossTick = (cellD.x < 0.035 && cellD.y < 0.035) ? 1.0 : 0.0;

    // 3. Central Proof Spine along Z = 0 between Station 1 (-42) and Station 8 (+42)
    float spineMask = step(-48.0, wp.x) * step(wp.x, 48.0);
    float zDistPx = abs(wp.z) / max(fwidth(wp.z), 1e-4);
    float spineCore = (1.0 - smoothstep(0.6, 1.8, zDistPx)) * spineMask;
    float spineHalo = exp(-wp.z * wp.z * 3.5) * 0.065 * spineMask;

    // Combine Grid floor lines (exact monochrome palette from metaverse/grid.coax: MINOR 0.24, STREET 0.55)
    float lineFade = (1.0 - fog);
    col += float3(0.24, 0.26, 0.29) * minorLine * 0.42 * lineFade;
    col += float3(0.76, 0.80, 0.85) * majorLine * 0.68 * lineFade;
    col += float3(0.94, 0.96, 0.98) * crossTick * majorLine * 0.35 * lineFade;
    col += float3(0.216, 0.851, 0.851) * (spineCore * 0.52 + spineHalo) * lineFade;

    // 4. Concentric Grid Station Bay Rings + Subtle Obsidian Floor Reflection Pool around each of the 8 stations
    for (int i = 0; i < 8; ++i) {
        float2 sc = u.stations[i].xy;
        float2 rel = wp.xz - sc;
        float r = length(rel);
        if (r < 9.5) {
            float3 acc = station_accent(u.stations[i].z);
            float active = u.stations[i].w;
            float dr = max(fwidth(r), 1e-4);
            // Outer architectural bay ring (r = 5.05m) & inner precision ring (r = 4.65m)
            float ringOuter = 1.0 - smoothstep(0.5, 1.65, abs(r - 5.05) / dr);
            float ringInner = 1.0 - smoothstep(0.4, 1.35, abs(r - 4.65) / dr);
            // Subtle compass ticks on outer ring
            float ang = atan2(rel.y, rel.x);
            float tick = step(0.96, abs(cos(ang * 4.0))) * (1.0 - smoothstep(0.0, 0.32, abs(r - 5.20)));

            float3 ringCol = mix(float3(0.74, 0.78, 0.84), acc, 0.38 + 0.48 * active);
            col += ringCol * (ringOuter * (0.62 + 0.55 * active) + ringInner * 0.28 + tick * 0.55) * lineFade;

            // Subtle obsidian floor glow reflection underneath each glowing landmark
            float pool = exp(-r * r * 0.068) * (0.048 + 0.045 * active);
            col += mix(float3(0.26, 0.30, 0.36), acc, 0.65) * pool * lineFade;
        }
    }

    // Fade smoothly to obsidian void (exact match with sky base so zero horizon seam!)
    float3 voidCol = float3(0.0014, 0.0016, 0.0020);
    col = mix(col, voidCol, fog);
    return float4(col, 1.0);
}

// ---- 3. Station 1 Holographic LiDAR Riemann Zeta 3D Terrain ----

struct TerrainOut {
    float4 position [[position]];
    float3 worldPos;
    float3 worldNormal;
    float2 uv;
    float height;
    float critDist;
};

vertex TerrainOut rh3d_terrain_vs(
    const device TerrainVert *verts [[buffer(0)]],
    constant Uniforms &u [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    TerrainVert v = verts[vid];
    TerrainOut o;
    o.position = u.viewProj * float4(v.posHeight.xyz, 1.0);
    o.worldPos = v.posHeight.xyz;
    o.worldNormal = normalize(v.normalCrit.xyz);
    o.uv = v.uvPad.xy;
    o.height = v.posHeight.w;
    o.critDist = v.normalCrit.w;
    return o;
}

fragment float4 rh3d_terrain_fs(TerrainOut in [[stage_in]], constant Uniforms &u [[buffer(1)]]) {
    float3 N = normalize(in.worldNormal);
    float3 V = normalize(u.cameraPos.xyz - in.worldPos);
    float fresnel = pow(1.0 - max(dot(N, V), 0.0), 2.8);
    float lit = grid_lit(N);

    // Deep obsidian-charcoal sculpted manifold so glowing mesh & contours pop sharply
    float3 base = float3(0.014, 0.020, 0.028) * (0.55 + 0.85 * lit);
    base += float3(0.04, 0.14, 0.16) * fresnel;

    // Crisp holographic (sigma, t) coordinate mesh via fwidth
    float2 meshCoord = in.uv * float2(18.0, 26.0);
    float meshLine = analytical_grid_line(meshCoord, fwidth(meshCoord), 1.15);

    // Crisp topographic elevation contour lines
    float contCoord = in.height * 2.6;
    float contD = abs(fract(contCoord - 0.5) - 0.5) / max(fwidth(contCoord), 1e-4);
    float contour = 1.0 - smoothstep(0.45, 1.35, contD);

    // Highlight the Critical Line Re(s) = 1/2 (u = 0.5) right on the surface
    float critPx = abs(in.uv.x - 0.5) / max(fwidth(in.uv.x), 1e-4);
    float critLine = 1.0 - smoothstep(0.6, 2.0, critPx);

    // Outer perimeter border of terrain patch
    float2 dEdge = min(in.uv, 1.0 - in.uv) / max(fwidth(in.uv), float2(1e-4));
    float border = 1.0 - smoothstep(0.6, 1.9, min(dEdge.x, dEdge.y));

    // Radiant amber-gold glow inside actual Riemann zero wells along Re(s) = 1/2
    float inZeroWell = smoothstep(0.42, 0.05, in.height) * smoothstep(0.09, 0.015, in.critDist);

    float3 col = base;
    col += float3(0.46, 0.54, 0.62) * meshLine * 0.42;
    col += float3(0.216, 0.851, 0.851) * contour * (0.55 + 0.35 * smoothstep(2.4, 0.2, in.height));
    col += float3(1.00, 0.72, 0.24) * critLine * 1.15;
    col += float3(1.00, 0.76, 0.26) * inZeroWell * 1.45;
    col += float3(0.84, 0.90, 0.94) * border * 0.85;

    float fog = grid_fog(length(u.cameraPos.xyz - in.worldPos));
    col = mix(col, float3(0.0022, 0.0025, 0.0032), fog);
    return float4(col, 1.0);
}

// ---- 4. Architectural Obsidian Solid + Procedural Crisp Wireframe Edge Shader ----

struct MeshOut {
    float4 position [[position]];
    float3 worldPos;
    float3 worldNormal;
    float2 uv;
    float2 bary;
    float capFlag;
    float4 faceColor;
    float4 edgeColor;
    float4 params;
};

vertex MeshOut rh3d_mesh_vs(
    const device MeshVert *verts [[buffer(0)]],
    const device Instance *insts [[buffer(1)]],
    constant Uniforms &u [[buffer(2)]],
    uint vid [[vertex_id]],
    uint iid [[instance_id]]
) {
    MeshVert v = verts[vid];
    Instance inst = insts[iid];
    float4 wp = inst.model * float4(v.pos.xyz, 1.0);
    float3x3 nmat = float3x3(inst.model[0].xyz, inst.model[1].xyz, inst.model[2].xyz);

    MeshOut o;
    o.position = u.viewProj * wp;
    o.worldPos = wp.xyz;
    o.worldNormal = normalize(nmat * v.normal.xyz);
    o.uv = v.uv.xy;
    o.bary = v.uv.zw;
    o.capFlag = v.pos.w;
    o.faceColor = inst.faceColor;
    o.edgeColor = inst.edgeColor;
    o.params = inst.params;
    return o;
}

fragment float4 rh3d_mesh_fs(MeshOut in [[stage_in]], constant Uniforms &u [[buffer(2)]]) {
    float3 N = normalize(in.worldNormal);
    float3 V = normalize(u.cameraPos.xyz - in.worldPos);
    float ndv = max(dot(N, V), 0.0);
    float fresnel = pow(1.0 - ndv, 3.0);
    float lit = grid_lit(N);
    float mode = in.params.x;
    float edgeW = max(in.params.y, 0.9);
    float edgeInt = in.edgeColor.a;
    float3 eCol = in.edgeColor.rgb;
    float dist = length(u.cameraPos.xyz - in.worldPos);
    float fog = grid_fog(dist);

    // Distance-attenuated edge intensity so far landmarks stay crisp without turning into solid blobs
    float distScale = mix(1.0, 0.62, clamp((dist - 12.0) / 55.0, 0.0, 1.0));

    // ---- Mode 4: Translucent Laser-Etched Glass Plane (Critical Wall, Robin 1.0 Ceiling, 2/3 Barrier) ----
    if (mode > 3.5) {
        float2 dEdge = min(in.uv, 1.0 - in.uv) / max(fwidth(in.uv), float2(1e-4));
        float frame = 1.0 - smoothstep(edgeW * 0.55, edgeW * 0.55 + 1.1, min(dEdge.x, dEdge.y));

        // Crisp world-space laser grid etched onto glass plane
        float2 planeCoord = (abs(N.y) > 0.5) ? in.worldPos.xz : ((abs(N.x) > 0.5) ? in.worldPos.zy : in.worldPos.xy);
        float laserGrid = analytical_grid_line(planeCoord * 1.0, fwidth(planeCoord * 1.0), 1.05);

        // Subtle animated scan sweep across the plane
        float scan = exp(-pow(fract(planeCoord.x * 0.12 - u.viewport.w * 0.16) - 0.5, 2.0) * 90.0) * 0.35;

        float3 glassRGB = in.faceColor.rgb * (0.45 + 0.55 * fresnel)
                        + eCol * (laserGrid * 0.42 + scan + frame * 1.45) * edgeInt * distScale;
        float glassAlpha = clamp(in.faceColor.a + laserGrid * 0.24 + frame * 0.78 + fresnel * 0.14, 0.0, 0.96);
        glassRGB = mix(glassRGB, float3(0.0022, 0.0025, 0.0032), fog);
        return float4(glassRGB, glassAlpha * (1.0 - fog * 0.85));
    }

    // ---- Mode 3: Radiant HDR Wireframe Crystal / Core / Beacon ----
    if (mode > 2.5 && mode < 3.5) {
        float3 b3 = float3(in.bary.x, in.bary.y, 1.0 - in.bary.x - in.bary.y);
        float3 bpx = b3 / max(fwidth(b3), float3(1e-4));
        float facetEdge = 1.0 - smoothstep(edgeW * 0.45, edgeW * 0.45 + 1.0, min(min(bpx.x, bpx.y), bpx.z));
        float pulse = 0.88 + 0.12 * sin(u.viewport.w * 2.8 + in.params.w);
        // Dark luminous faceted gem interior + blazing white-hot wireframe edges (so it blooms cleanly!)
        float3 core = in.faceColor.rgb * (0.35 + 0.65 * lit + 0.65 * fresnel) * pulse;
        float3 col = core + mix(eCol, float3(1.0, 0.99, 0.96), 0.35) * facetEdge * edgeInt * 1.35 * pulse;
        col = mix(col, float3(0.0022, 0.0025, 0.0032), fog);
        return float4(col, in.faceColor.a);
    }

    // ---- Mode 2: Sleek Obsidian Cylinder / Fluted Column ----
    if (mode > 1.5 && mode < 2.5) {
        float edgeMask = 0.0;
        if (in.capFlag > 0.5) {
            float r = length(in.uv - 0.5) * 2.0;
            float rimPx = abs(1.0 - r) / max(fwidth(r), 1e-4);
            edgeMask = 1.0 - smoothstep(edgeW * 0.45, edgeW * 0.45 + 1.0, rimPx);
        } else {
            // Top & bottom rim rings + crisp vertical architectural rib lines
            float vPx = min(in.uv.y, 1.0 - in.uv.y) / max(fwidth(in.uv.y), 1e-4);
            float rim = 1.0 - smoothstep(edgeW * 0.45, edgeW * 0.45 + 1.0, vPx);
            float uPx = min(in.uv.x, 1.0 - in.uv.x) / max(fwidth(in.uv.x), 1e-4);
            float rib = (1.0 - smoothstep(edgeW * 0.35, edgeW * 0.35 + 0.9, uPx)) * 0.45;
            edgeMask = max(rim, rib);
        }
        float3 base = in.faceColor.rgb * (0.38 + 0.58 * lit) + eCol * fresnel * 0.12;
        float3 col = base + eCol * edgeMask * edgeInt * distScale;
        col = mix(col, float3(0.0014, 0.0016, 0.0020), fog);
        return float4(col, in.faceColor.a);
    }

    // ---- Mode 0 (Grid Obsidian Wireframe Box) & Mode 1 (Station 5 Obsidian Glass Skyscraper) ----
    float2 dEdge = min(in.uv, 1.0 - in.uv) / max(fwidth(in.uv), float2(1e-4));
    float boxEdge = 1.0 - smoothstep(edgeW * 0.45, edgeW * 0.45 + 1.0, min(dEdge.x, dEdge.y));

    // Deep pitch-charcoal Grid obsidian face (matching the-grid.jpg Pylon terminal) + crisp Fresnel sheen
    float3 base = in.faceColor.rgb * (0.38 + 0.58 * lit) + float3(0.025, 0.030, 0.038) * fresnel;
    float3 col = base + eCol * boxEdge * edgeInt * distScale;

    if (mode > 0.5 && mode < 1.5 && abs(N.y) < 0.5) {
        // Architectural glowing horizontal floor-tier bands on Station 5 skyscrapers
        float bands = max(in.params.z, 4.0);
        float fy = in.uv.y * bands;
        float fDist = abs(fract(fy - 0.5) - 0.5) / max(fwidth(fy), 1e-4);
        float floorLine = (1.0 - smoothstep(0.35, 1.15, fDist)) * smoothstep(0.06, 0.14, in.uv.y) * smoothstep(0.96, 0.90, in.uv.y);
        // Subtle inner inset margin so floor lines look like precision recessed glass tiers
        float margin = smoothstep(0.06, 0.12, in.uv.x) * smoothstep(0.94, 0.88, in.uv.x);
        float topGlow = smoothstep(0.84, 1.0, in.uv.y) * 0.16;
        col += eCol * (floorLine * margin * 0.44 + topGlow) * edgeInt * distScale;
    }

    col = mix(col, float3(0.0014, 0.0016, 0.0020), fog);
    return float4(col, in.faceColor.a);
}

// ---- 5. Screen-Space Expanded Glowing Vector Lines & Data Pulses (gridcrowd.wgsl + newland.coax) ----

struct QLineOut {
    float4 position [[position]];
    float2 place;  // x = along (px), y = across (px)
    float4 shape;  // x = length (px), y = core half-width (px), z = light, w = glow radius (px)
    float4 color;  // rgb = HDR color, a = mode
    float4 anim;   // x = pulseSpeed, y = pulsePhase, z = alongT [0,1], w = fog
};

vertex QLineOut rh3d_qline_vs(
    const device QuadLineSeg *segs [[buffer(0)]],
    constant Uniforms &u [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    uint segIdx = vid / 6;
    uint corner = vid % 6;
    QuadLineSeg s = segs[segIdx];

    float3 a = s.p0Width.xyz;
    float3 b = s.p1Glow.xyz;
    float4 pa = u.viewProj * float4(a, 1.0);
    float4 pb = u.viewProj * float4(b, 1.0);

    QLineOut o;
    o.place = float2(0.0);
    o.shape = float4(0.0);
    o.color = float4(0.0);
    o.anim = float4(0.0);

    float nearW = 0.18;
    if (pa.w < nearW && pb.w < nearW) {
        o.position = float4(0.0, 0.0, 2.0, 1.0);
        return o;
    }
    if (pa.w < nearW) {
        float t = (nearW - pa.w) / max(pb.w - pa.w, 1e-5);
        pa = mix(pa, pb, t);
        a = mix(a, b, t);
    }
    if (pb.w < nearW) {
        float t = (nearW - pb.w) / max(pa.w - pb.w, 1e-5);
        pb = mix(pb, pa, t);
        b = mix(b, a, t);
    }

    float2 vp = u.viewport.xy;
    float2 sa = (pa.xy / pa.w * 0.5 + 0.5) * vp;
    float2 sb = (pb.xy / pb.w * 0.5 + 0.5) * vp;
    float2 dir = sb - sa;
    float lenPx = length(dir);
    float2 ux = (lenPx > 0.001) ? (dir / lenPx) : float2(1.0, 0.0);
    float2 nx = float2(-ux.y, ux.x);

    float dpr = max(u.viewport.z, 1.0);
    float corePx = s.p0Width.w * dpr;
    float glowPx = s.p1Glow.w * dpr;
    float pad = corePx + glowPx;

    float alongTable[6] = { 0.0, 1.0, 1.0, 0.0, 1.0, 0.0 };
    float sideTable[6]  = { -1.0, -1.0, 1.0, -1.0, 1.0, 1.0 };
    float endPadTable[6] = { -1.0, 1.0, 1.0, -1.0, 1.0, -1.0 };

    float alongT = alongTable[corner];
    float side = sideTable[corner];
    float endPad = endPadTable[corner];

    float4 clip = mix(pa, pb, alongT);
    float3 worldP = mix(a, b, alongT);
    float2 offsetPx = nx * (side * pad) + ux * (endPad * pad);
    // Convert screen-pixel offset back to clip-space delta
    float2 clipDelta = (offsetPx / vp) * 2.0 * clip.w;

    o.position = float4(clip.xy + clipDelta, clip.z - 0.00018 * clip.w, clip.w);
    o.place = float2(alongT * lenPx + endPad * pad, side * pad);
    o.shape = float4(lenPx, corePx, s.color.a, glowPx);
    o.color = float4(s.color.rgb, s.anim.z);
    o.anim = float4(s.anim.x, s.anim.y, alongT, grid_fog(length(u.cameraPos.xyz - worldP)));
    return o;
}

fragment float4 rh3d_qline_fs(QLineOut in [[stage_in]], constant Uniforms &u [[buffer(1)]]) {
    float along = max(max(-in.place.x, in.place.x - in.shape.x), 0.0);
    float d = length(float2(along, in.place.y));
    float core = 1.0 - smoothstep(in.shape.y - 0.45, in.shape.y + 0.65, d);
    float g = d / max(in.shape.w, 1.0);
    float halo = exp(-5.0 * g * g) * 0.34;
    float light = in.shape.z * (1.0 - in.anim.w * 0.92);
    float mode = in.color.a;

    float3 col = in.color.rgb * (core + halo) * light;

    // Mode 1: Animated traveling white-hot data pulse along proof conduits (newland.coax MODE_PULSE)
    if (mode > 0.5 && mode < 1.5) {
        float head = fract(u.viewport.w * in.anim.x + in.anim.y);
        float delta = fract(head - in.anim.z);
        float trail = exp(-delta * 10.5) * smoothstep(0.0, 0.02, delta);
        float hot = exp(-delta * 38.0) * smoothstep(0.0, 0.015, delta);
        col += in.color.rgb * (core + halo * 1.8) * trail * 1.35 * (1.0 - in.anim.w);
        col += float3(1.0, 0.98, 0.94) * core * hot * 1.85 * (1.0 - in.anim.w);
    }

    return float4(col, 1.0);
}

// ---- 6. Post-Processing: Soft-Knee HDR Bloom Extract, Separable Gaussian Blur & metaverse_tone Composite ----

struct PostOut {
    float4 position [[position]];
    float2 uv;
};

vertex PostOut rh3d_post_vs(uint vid [[vertex_id]]) {
    float2 verts[3] = { float2(-1.0, -3.0), float2(-1.0, 1.0), float2(3.0, 1.0) };
    PostOut o;
    o.position = float4(verts[vid], 0.0, 1.0);
    o.uv = float2(verts[vid].x * 0.5 + 0.5, 0.5 - verts[vid].y * 0.5);
    return o;
}

fragment float4 rh3d_bloom_extract_fs(
    PostOut in [[stage_in]],
    texture2d<float> hdrTex [[texture(0)]],
    constant PostUniforms &p [[buffer(0)]]
) {
    constexpr sampler lin(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 d = p.texelDir.xy;
    // 4-tap box downsample to suppress single-pixel shimmer in bloom
    float3 c = 0.25 * (
        hdrTex.sample(lin, in.uv + float2(-d.x, -d.y)).rgb +
        hdrTex.sample(lin, in.uv + float2( d.x, -d.y)).rgb +
        hdrTex.sample(lin, in.uv + float2(-d.x,  d.y)).rgb +
        hdrTex.sample(lin, in.uv + float2( d.x,  d.y)).rgb
    );
    float luma = max(max(c.r, c.g), c.b);
    float thresh = p.params.x;
    float knee = max(p.params.y, 0.001);
    float rq = clamp(luma - thresh + knee, 0.0, 2.0 * knee);
    float contrib = max(rq * rq / (4.0 * knee), luma - thresh) / max(luma, 1e-4);
    return float4(c * max(contrib, 0.0), 1.0);
}

fragment float4 rh3d_bloom_blur_fs(
    PostOut in [[stage_in]],
    texture2d<float> srcTex [[texture(0)]],
    constant PostUniforms &p [[buffer(0)]]
) {
    constexpr sampler lin(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 stepUV = p.texelDir.xy * p.texelDir.zw;
    // 9-tap separable Gaussian (weights normalized to 1.0)
    const float w[5] = { 0.227027, 0.194595, 0.121622, 0.054054, 0.016216 };
    float3 acc = srcTex.sample(lin, in.uv).rgb * w[0];
    for (int i = 1; i < 5; ++i) {
        float2 off = stepUV * float(i) * 1.45;
        acc += srcTex.sample(lin, in.uv + off).rgb * w[i];
        acc += srcTex.sample(lin, in.uv - off).rgb * w[i];
    }
    return float4(acc, 1.0);
}

static inline float3 srgb_encode(float3 c) {
    float3 x = clamp(c, 0.0, 1.0);
    float3 lo = x * 12.92;
    float3 hi = 1.055 * pow(max(x, float3(1e-6)), float3(1.0 / 2.4)) - 0.055;
    return select(hi, lo, x <= 0.0031308);
}

fragment float4 rh3d_composite_fs(
    PostOut in [[stage_in]],
    texture2d<float> hdrTex [[texture(0)]],
    texture2d<float> bloomHalf [[texture(1)]],
    texture2d<float> bloomQuarter [[texture(2)]],
    constant PostUniforms &p [[buffer(0)]]
) {
    constexpr sampler lin(coord::normalized, filter::linear, address::clamp_to_edge);
    float3 scene = hdrTex.sample(lin, in.uv).rgb;
    float3 b1 = bloomHalf.sample(lin, in.uv).rgb;
    float3 b2 = bloomQuarter.sample(lin, in.uv).rgb;

    float3 hdr = scene + b1 * p.params.z + b2 * p.params.w;

    // Exact OpenAgents Metaverse exponential tone curve (metaverse/light.coax metaverse_tone)
    float3 toned = 1.0 - exp(-max(hdr, float3(0.0)) * 1.35);

    // Subtle newland.coax edge vignette to keep eyes locked on the Grid horizon
    float2 q = in.uv * 2.0 - 1.0;
    float r2 = q.x * q.x * 0.75 + q.y * q.y * 1.15;
    float vig = clamp(1.12 - 0.36 * r2, 0.56, 1.0);
    toned *= vig;

    return float4(srgb_encode(toned), 1.0);
}
"""

// MARK: - Matrix Helpers (Right-Handed Y-Up matching Metaverse Engine)

private func rh3dLookAt(eye: SIMD3<Float>, target: SIMD3<Float>) -> simd_float4x4 {
    let f = simd_normalize(target - eye)
    let upRef: SIMD3<Float> = abs(f.y) > 0.992 ? SIMD3<Float>(0, 0, -1) : SIMD3<Float>(0, 1, 0)
    let r = simd_normalize(simd_cross(f, upRef))
    let u = simd_cross(r, f)
    return simd_float4x4(
        SIMD4<Float>(r.x, u.x, -f.x, 0),
        SIMD4<Float>(r.y, u.y, -f.y, 0),
        SIMD4<Float>(r.z, u.z, -f.z, 0),
        SIMD4<Float>(-simd_dot(r, eye), -simd_dot(u, eye), simd_dot(f, eye), 1)
    )
}

private func rh3dPerspective(fovY: Float, aspect: Float, nearZ: Float, farZ: Float) -> simd_float4x4 {
    let ys = 1.0 / tan(fovY * 0.5)
    let xs = ys / max(0.05, aspect)
    let zs = farZ / (nearZ - farZ)
    return simd_float4x4(
        SIMD4<Float>(xs, 0, 0, 0),
        SIMD4<Float>(0, ys, 0, 0),
        SIMD4<Float>(0, 0, zs, -1),
        SIMD4<Float>(0, 0, nearZ * zs, 0)
    )
}

private func rh3dModelBox(center: SIMD3<Float>, size: SIMD3<Float>, rotY: Float = 0) -> simd_float4x4 {
    let c = cos(rotY), s = sin(rotY)
    return simd_float4x4(
        SIMD4<Float>(c * size.x, 0, -s * size.x, 0),
        SIMD4<Float>(0, size.y, 0, 0),
        SIMD4<Float>(s * size.z, 0, c * size.z, 0),
        SIMD4<Float>(center.x, center.y, center.z, 1)
    )
}

private func rh3dModelBeam(from a: SIMD3<Float>, to b: SIMD3<Float>, radius: Float) -> simd_float4x4 {
    let diff = b - a
    let len = simd_length(diff)
    guard len > 0.0005 else { return rh3dModelBox(center: a, size: SIMD3<Float>(repeating: radius)) }
    let up = diff / len
    let ref: SIMD3<Float> = abs(up.y) > 0.96 ? SIMD3<Float>(1, 0, 0) : SIMD3<Float>(0, 1, 0)
    let right = simd_normalize(simd_cross(ref, up))
    let fwd = simd_cross(up, right)
    let mid = (a + b) * 0.5
    return simd_float4x4(
        SIMD4<Float>(right.x * radius * 2, right.y * radius * 2, right.z * radius * 2, 0),
        SIMD4<Float>(up.x * len, up.y * len, up.z * len, 0),
        SIMD4<Float>(fwd.x * radius * 2, fwd.y * radius * 2, fwd.z * radius * 2, 0),
        SIMD4<Float>(mid.x, mid.y, mid.z, 1)
    )
}

// MARK: - OpenAgents Grid Chamfered HUD Frame Helper (exact metaverse-engine/ui/hud-frame.coax geometry)

private func gridHudChamferPath(rect: CGRect, cut: CGFloat) -> CGPath {
    let c = min(cut, min(rect.width, rect.height) * 0.35)
    let p = CGMutablePath()
    p.move(to: CGPoint(x: rect.minX, y: rect.minY))
    p.addLine(to: CGPoint(x: rect.maxX - c, y: rect.minY))
    p.addLine(to: CGPoint(x: rect.maxX, y: rect.minY + c))
    p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
    p.addLine(to: CGPoint(x: rect.minX + c, y: rect.maxY))
    p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY - c))
    p.closeSubpath()
    return p
}

private func drawGridHudFrame(in ctx: CGContext, rect: CGRect, cut: CGFloat, fillAlpha: CGFloat = 0.88, drawBrackets: Bool = true, drawScanlines: Bool = true) {
    let path = gridHudChamferPath(rect: rect, cut: cut)
    ctx.saveGState()
    // Obsidian dark glass fill
    ctx.setFillColor(NSColor(calibratedRed: 0.014, green: 0.016, blue: 0.020, alpha: fillAlpha).cgColor)
    ctx.addPath(path)
    ctx.fillPath()

    // Ultra-faint Grid horizontal scanlines (matches newland_panel)
    if drawScanlines {
        ctx.saveGState()
        ctx.addPath(path)
        ctx.clip()
        ctx.setFillColor(NSColor(white: 1.0, alpha: 0.018).cgColor)
        var y = rect.minY
        while y < rect.maxY {
            ctx.fill(CGRect(x: rect.minX, y: y, width: rect.width, height: 1.0))
            y += 3.0
        }
        ctx.restoreGState()
    }

    // Hairline monochrome outline (HUD_OUTLINE = 0.30)
    ctx.setStrokeColor(NSColor(white: 0.94, alpha: 0.28).cgColor)
    ctx.setLineWidth(1.0)
    ctx.addPath(path)
    ctx.strokePath()

    // Iconic bright crisp L-brackets on top-left and bottom-right (the-grid.jpg & hud-frame.coax)
    if drawBrackets {
        let arm = min(22.0, rect.width * 0.22)
        let armDown = min(arm * 0.6, rect.height * 0.45)
        ctx.setStrokeColor(NSColor(white: 0.96, alpha: 0.94).cgColor)
        ctx.setLineWidth(1.5)
        ctx.beginPath()
        // Top-left L bracket
        ctx.move(to: CGPoint(x: rect.minX, y: rect.minY + armDown))
        ctx.addLine(to: CGPoint(x: rect.minX, y: rect.minY))
        ctx.addLine(to: CGPoint(x: rect.minX + arm, y: rect.minY))
        // Bottom-right L bracket
        ctx.move(to: CGPoint(x: rect.maxX - arm, y: rect.maxY))
        ctx.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
        ctx.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - armDown))
        ctx.strokePath()
    }
    ctx.restoreGState()
}

private final class GridHudFrameView: NSView {
    var cutSize: CGFloat = 12.0
    var showBrackets: Bool = true
    var showScanlines: Bool = true
    var fillAlpha: CGFloat = 0.88

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setShouldAntialias(true)
        drawGridHudFrame(in: ctx, rect: bounds.insetBy(dx: 0.5, dy: 0.5), cut: cutSize, fillAlpha: fillAlpha, drawBrackets: showBrackets, drawScanlines: showScanlines)
    }
}

// MARK: - 3D-to-2D Projected Callout Labels Overlay (Minimalist OpenAgents Grid Style)

private struct ProjectedLabel {
    let rect: CGRect
    let anchorPt: CGPoint
    let title: String
    let subtext: String
    let color: NSColor
    let isBanner: Bool
    let alpha: CGFloat
    let stationIndex: Int
}

private final class RhLabelsOverlayView: NSView {
    var labels: [ProjectedLabel] = []
    var onClickStation: ((Int) -> Void)?

    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let pt = convert(point, from: superview)
        for lbl in labels where lbl.isBanner && lbl.rect.contains(pt) {
            return self
        }
        return nil
    }

    override func mouseDown(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        for lbl in labels where lbl.isBanner && lbl.rect.contains(pt) {
            onClickStation?(lbl.stationIndex)
            return
        }
        super.mouseDown(with: event)
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setShouldAntialias(true)
        Self.drawLabels(labels, in: ctx)
    }

    static func drawLabels(_ labels: [ProjectedLabel], in ctx: CGContext) {
        let titleBannerFont = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .semibold)
        let titlePartFont = NSFont.monospacedSystemFont(ofSize: 9.8, weight: .medium)
        let subFont = NSFont.monospacedSystemFont(ofSize: 8.8, weight: .regular)

        for lbl in labels {
            ctx.saveGState()
            ctx.setAlpha(lbl.alpha)

            // Crisp hairline leader stem straight down to 3D projected anchor
            let tagBottom = CGPoint(x: lbl.rect.midX, y: lbl.rect.maxY)
            ctx.setStrokeColor(NSColor(white: 0.90, alpha: lbl.isBanner ? 0.34 : 0.24).cgColor)
            ctx.setLineWidth(1.0)
            ctx.beginPath()
            ctx.move(to: lbl.anchorPt)
            ctx.addLine(to: tagBottom)
            ctx.strokePath()

            // Crisp architectural diamond reticle on the exact 3D anchor
            let dR: CGFloat = lbl.isBanner ? 3.8 : 2.8
            ctx.beginPath()
            ctx.move(to: CGPoint(x: lbl.anchorPt.x, y: lbl.anchorPt.y - dR))
            ctx.addLine(to: CGPoint(x: lbl.anchorPt.x + dR, y: lbl.anchorPt.y))
            ctx.addLine(to: CGPoint(x: lbl.anchorPt.x, y: lbl.anchorPt.y + dR))
            ctx.addLine(to: CGPoint(x: lbl.anchorPt.x - dR, y: lbl.anchorPt.y))
            ctx.closePath()
            ctx.setFillColor(NSColor(calibratedRed: 0.01, green: 0.01, blue: 0.02, alpha: 0.9).cgColor)
            ctx.fillPath()
            ctx.setStrokeColor(lbl.color.withAlphaComponent(0.95).cgColor)
            ctx.setLineWidth(1.2)
            ctx.strokePath()

            // Exact Grid chamfered obsidian glass plate (cut = 7 for banners, 5 for part tags)
            drawGridHudFrame(
                in: ctx,
                rect: lbl.rect,
                cut: lbl.isBanner ? 7.0 : 5.0,
                fillAlpha: lbl.isBanner ? 0.84 : 0.74,
                drawBrackets: lbl.isBanner,
                drawScanlines: false
            )

            // Signature newland.coax crisp accent rule right under the title
            let ruleY = lbl.rect.minY + 17.5
            ctx.setFillColor(NSColor(white: 0.94, alpha: 0.16).cgColor)
            ctx.fill(CGRect(x: lbl.rect.minX + 8, y: ruleY, width: max(12, lbl.rect.width - 16), height: 1.0))
            ctx.setFillColor(lbl.color.withAlphaComponent(0.92).cgColor)
            ctx.fill(CGRect(x: lbl.rect.minX + 8, y: ruleY, width: min(26, max(12, lbl.rect.width * 0.36)), height: 1.5))

            let tAttrs: [NSAttributedString.Key: Any] = [
                .font: lbl.isBanner ? titleBannerFont : titlePartFont,
                .foregroundColor: GridTokens.nsWhite,
                .kern: lbl.isBanner ? 0.7 : 0.3,
            ]
            (lbl.title as NSString).draw(at: CGPoint(x: lbl.rect.minX + 8, y: lbl.rect.minY + 3.5), withAttributes: tAttrs)

            if !lbl.subtext.isEmpty {
                let sAttrs: [NSAttributedString.Key: Any] = [
                    .font: subFont,
                    .foregroundColor: GridTokens.nsLight.withAlphaComponent(0.82),
                ]
                (lbl.subtext as NSString).draw(at: CGPoint(x: lbl.rect.minX + 8, y: lbl.rect.minY + 20.5), withAttributes: sAttrs)
            }
            ctx.restoreGState()
        }
    }
}

// MARK: - Main RhFlythroughView (OpenAgents Grid Metal 3D + HDR Bloom + Metaverse Engine Coax Spline)

final class RhFlythroughView: NSView {
    var onInspectModule: ((String) -> Void)?

    private let device: MTLDevice?
    private let commandQueue: MTLCommandQueue?
    private let metalHost = NSView()
    private var metalLayer: CAMetalLayer?

    // MSAA + HDR + Multi-Scale Bloom Render Targets
    private var sampleCount: Int = 4
    private var rtSize: (Int, Int) = (0, 0)
    private var msaaColorTex: MTLTexture?
    private var msaaDepthTex: MTLTexture?
    private var hdrResolveTex: MTLTexture?
    private var bloomA1: MTLTexture?
    private var bloomB1: MTLTexture?
    private var bloomA2: MTLTexture?
    private var bloomB2: MTLTexture?

    // Pipelines
    private var skyPipe: MTLRenderPipelineState?
    private var floorPipe: MTLRenderPipelineState?
    private var terrainPipe: MTLRenderPipelineState?
    private var opaqueMeshPipe: MTLRenderPipelineState?
    private var translucentMeshPipe: MTLRenderPipelineState?
    private var qlinePipe: MTLRenderPipelineState?
    private var bloomExtractPipe: MTLRenderPipelineState?
    private var bloomBlurPipe: MTLRenderPipelineState?
    private var compositePipe: MTLRenderPipelineState?

    private var depthNone: MTLDepthStencilState?
    private var depthOpaque: MTLDepthStencilState?
    private var depthReadOnly: MTLDepthStencilState?

    // Shared primitive geometry buffers
    private var boxVB: MTLBuffer?
    private var boxVCount = 0
    private var cylVB: MTLBuffer?
    private var cylVCount = 0
    private var octVB: MTLBuffer?
    private var octVCount = 0

    // World buffers
    private var terrainVB: MTLBuffer?
    private var terrainVCount = 0
    private var quadLinesVB: MTLBuffer?
    private var quadLineSegCount = 0

    private var opaqueBoxes: [Rh3dInstance] = []
    private var opaqueCyls: [Rh3dInstance] = []
    private var opaqueOcts: [Rh3dInstance] = []
    private var transBoxes: [Rh3dInstance] = []
    private var transCyls: [Rh3dInstance] = []

    // Pre-allocated GPU instance buffers (uploaded once on scene build)
    private var opaqueBoxIB: MTLBuffer?
    private var opaqueCylIB: MTLBuffer?
    private var opaqueOctIB: MTLBuffer?
    private var transBoxIB: MTLBuffer?
    private var transCylIB: MTLBuffer?

    // Coax-driven scene data
    private var durationS: Float = 62.0
    private var stations: [Rh3dStationData] = []
    private var labelAnchors: [Rh3dLabelAnchor] = []
    private var pickables: [Rh3dPickable] = []

    // Camera state (Y-up right-handed matching metaverse-engine/cam/flyover.coax)
    private var eye = SIMD3<Float>(-49, 8.5, 15)
    private var target = SIMD3<Float>(-42, 2.6, 0)
    private var flyPlaying = true
    private var flyTimeS: Float = 0.0
    private var showLabels = true
    private var activeStation = 0
    private var selectedPick: Rh3dPickable?

    // Smooth camera glide when clicking a station pill or Overview
    private var glideFromEye = SIMD3<Float>.zero
    private var glideToEye = SIMD3<Float>.zero
    private var glideFromTarget = SIMD3<Float>.zero
    private var glideToTarget = SIMD3<Float>.zero
    private var glideElapsed: Float = 0
    private var glideDuration: Float = 0

    // Manual orbit / pan / WASD flight state
    private var isOrbitDragging = false
    private var isPanDragging = false
    private var dragLast = CGPoint.zero
    private var pressedKeys = Set<UInt16>()

    // HUD views (Grid chamfered frames with white L-brackets)
    private let labelsOverlay = RhLabelsOverlayView()
    private let topLeftFrame = GridHudFrameView()
    private let topLeftBadge = NSTextField(labelWithString: "")
    private let playPauseBtn = NSButton(title: "PAUSE", target: nil, action: nil)
    private let overviewBtn = NSButton(title: "OVERVIEW", target: nil, action: nil)
    private let labelsBtn = NSButton(title: "LABELS: ON", target: nil, action: nil)
    private let inspectBtn = NSButton(title: "SOURCE →", target: nil, action: nil)

    private let storyCard = GridHudFrameView()
    private let storyKicker = NSTextField(labelWithString: "OPENAGENTS / VERSE  ·  THE GRID  ·  RIEMANN HYPOTHESIS")
    private let storyTitle = NSTextField(labelWithString: "")
    private let storySubtitle = NSTextField(labelWithString: "")
    private let storyBody = NSTextField(wrappingLabelWithString: "")
    private let prevBtn = NSButton(title: "← PREV", target: nil, action: nil)
    private let nextBtn = NSButton(title: "NEXT →", target: nil, action: nil)
    private let progressTrack = NSView()
    private let progressFill = NSView()
    private var progressFillWidth: NSLayoutConstraint?
    private let stationStack = NSStackView()
    private var stationButtons: [NSButton] = []

    private var frameTimer: Timer?
    private var lastTick: CFTimeInterval = CACurrentMediaTime()
    private var totalElapsedS: Float = 0

    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        self.device = MTLCreateSystemDefaultDevice()
        self.commandQueue = self.device?.makeCommandQueue()
        if let dev = self.device {
            self.sampleCount = dev.supportsTextureSampleCount(4) ? 4 : 1
        } else {
            self.sampleCount = 1
        }
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor

        setupMetalPipelines()
        buildUnitPrimitives()
        loadSceneFromCoaxAndAtlas()
        setupViews()
        syncStoryHUD()
        startLoop()
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        frameTimer?.invalidate()
    }

    // MARK: - Metal Setup (4x MSAA HDR + Bloom + metaverse_tone)

    private func setupMetalPipelines() {
        guard let dev = device else { return }
        guard let lib = try? dev.makeLibrary(source: rh3dMetalShaderSource, options: nil) else { return }

        // 1. Sky Dome (HDR 4x MSAA)
        let skyDesc = MTLRenderPipelineDescriptor()
        skyDesc.vertexFunction = lib.makeFunction(name: "rh3d_sky_vs")
        skyDesc.fragmentFunction = lib.makeFunction(name: "rh3d_sky_fs")
        skyDesc.rasterSampleCount = sampleCount
        skyDesc.colorAttachments[0].pixelFormat = .rgba16Float
        skyDesc.depthAttachmentPixelFormat = .depth32Float
        skyPipe = try? dev.makeRenderPipelineState(descriptor: skyDesc)

        // 2. Infinite Analytical Anti-Aliased Grid Floor (HDR 4x MSAA)
        let flDesc = MTLRenderPipelineDescriptor()
        flDesc.vertexFunction = lib.makeFunction(name: "rh3d_floor_vs")
        flDesc.fragmentFunction = lib.makeFunction(name: "rh3d_floor_fs")
        flDesc.rasterSampleCount = sampleCount
        flDesc.colorAttachments[0].pixelFormat = .rgba16Float
        flDesc.depthAttachmentPixelFormat = .depth32Float
        floorPipe = try? dev.makeRenderPipelineState(descriptor: flDesc)

        // 3. Holographic LiDAR Zeta Terrain (HDR 4x MSAA)
        let terDesc = MTLRenderPipelineDescriptor()
        terDesc.vertexFunction = lib.makeFunction(name: "rh3d_terrain_vs")
        terDesc.fragmentFunction = lib.makeFunction(name: "rh3d_terrain_fs")
        terDesc.rasterSampleCount = sampleCount
        terDesc.colorAttachments[0].pixelFormat = .rgba16Float
        terDesc.depthAttachmentPixelFormat = .depth32Float
        terrainPipe = try? dev.makeRenderPipelineState(descriptor: terDesc)

        // 4. Opaque Obsidian + Crisp Wireframe Edge Monoliths (HDR 4x MSAA)
        let opDesc = MTLRenderPipelineDescriptor()
        opDesc.vertexFunction = lib.makeFunction(name: "rh3d_mesh_vs")
        opDesc.fragmentFunction = lib.makeFunction(name: "rh3d_mesh_fs")
        opDesc.rasterSampleCount = sampleCount
        opDesc.colorAttachments[0].pixelFormat = .rgba16Float
        opDesc.depthAttachmentPixelFormat = .depth32Float
        opaqueMeshPipe = try? dev.makeRenderPipelineState(descriptor: opDesc)

        // 5. Translucent Laser-Etched Glass Planes (HDR 4x MSAA)
        let trDesc = MTLRenderPipelineDescriptor()
        trDesc.vertexFunction = lib.makeFunction(name: "rh3d_mesh_vs")
        trDesc.fragmentFunction = lib.makeFunction(name: "rh3d_mesh_fs")
        trDesc.rasterSampleCount = sampleCount
        trDesc.colorAttachments[0].pixelFormat = .rgba16Float
        trDesc.colorAttachments[0].isBlendingEnabled = true
        trDesc.colorAttachments[0].rgbBlendOperation = .add
        trDesc.colorAttachments[0].alphaBlendOperation = .add
        trDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        trDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        trDesc.colorAttachments[0].sourceAlphaBlendFactor = .one
        trDesc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        trDesc.depthAttachmentPixelFormat = .depth32Float
        translucentMeshPipe = try? dev.makeRenderPipelineState(descriptor: trDesc)

        // 6. Screen-Space Expanded Glowing Vector Lines & Pulses (HDR 4x MSAA, Additive)
        let qlDesc = MTLRenderPipelineDescriptor()
        qlDesc.vertexFunction = lib.makeFunction(name: "rh3d_qline_vs")
        qlDesc.fragmentFunction = lib.makeFunction(name: "rh3d_qline_fs")
        qlDesc.rasterSampleCount = sampleCount
        qlDesc.colorAttachments[0].pixelFormat = .rgba16Float
        qlDesc.colorAttachments[0].isBlendingEnabled = true
        qlDesc.colorAttachments[0].rgbBlendOperation = .add
        qlDesc.colorAttachments[0].alphaBlendOperation = .add
        qlDesc.colorAttachments[0].sourceRGBBlendFactor = .one
        qlDesc.colorAttachments[0].destinationRGBBlendFactor = .one
        qlDesc.colorAttachments[0].sourceAlphaBlendFactor = .one
        qlDesc.colorAttachments[0].destinationAlphaBlendFactor = .one
        qlDesc.depthAttachmentPixelFormat = .depth32Float
        qlinePipe = try? dev.makeRenderPipelineState(descriptor: qlDesc)

        // 7. Post-processing pipelines (1x)
        let exDesc = MTLRenderPipelineDescriptor()
        exDesc.vertexFunction = lib.makeFunction(name: "rh3d_post_vs")
        exDesc.fragmentFunction = lib.makeFunction(name: "rh3d_bloom_extract_fs")
        exDesc.colorAttachments[0].pixelFormat = .rgba16Float
        bloomExtractPipe = try? dev.makeRenderPipelineState(descriptor: exDesc)

        let blDesc = MTLRenderPipelineDescriptor()
        blDesc.vertexFunction = lib.makeFunction(name: "rh3d_post_vs")
        blDesc.fragmentFunction = lib.makeFunction(name: "rh3d_bloom_blur_fs")
        blDesc.colorAttachments[0].pixelFormat = .rgba16Float
        bloomBlurPipe = try? dev.makeRenderPipelineState(descriptor: blDesc)

        let cpDesc = MTLRenderPipelineDescriptor()
        cpDesc.vertexFunction = lib.makeFunction(name: "rh3d_post_vs")
        cpDesc.fragmentFunction = lib.makeFunction(name: "rh3d_composite_fs")
        cpDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        compositePipe = try? dev.makeRenderPipelineState(descriptor: cpDesc)

        // Depth states
        let dNone = MTLDepthStencilDescriptor()
        dNone.depthCompareFunction = .always
        dNone.isDepthWriteEnabled = false
        depthNone = dev.makeDepthStencilState(descriptor: dNone)

        let dOp = MTLDepthStencilDescriptor()
        dOp.depthCompareFunction = .less
        dOp.isDepthWriteEnabled = true
        depthOpaque = dev.makeDepthStencilState(descriptor: dOp)

        let dRO = MTLDepthStencilDescriptor()
        dRO.depthCompareFunction = .lessEqual
        dRO.isDepthWriteEnabled = false
        depthReadOnly = dev.makeDepthStencilState(descriptor: dRO)
    }

    private func buildUnitPrimitives() {
        guard let dev = device else { return }

        // 1. Unit Box [-0.5, 0.5]^3 with exact [0, 1]^2 UVs per quad face for razor-sharp fwidth wireframe edges
        var bVerts: [Rh3dMeshVert] = []
        let faces: [(SIMD3<Float>, [SIMD3<Float>])] = [
            (SIMD3(0, 0, 1),  [SIMD3(-0.5,-0.5, 0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3(-0.5, 0.5, 0.5)]),
            (SIMD3(0, 0,-1),  [SIMD3( 0.5,-0.5,-0.5), SIMD3(-0.5,-0.5,-0.5), SIMD3(-0.5, 0.5,-0.5), SIMD3( 0.5, 0.5,-0.5)]),
            (SIMD3(1, 0, 0),  [SIMD3( 0.5,-0.5, 0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3( 0.5, 0.5, 0.5)]),
            (SIMD3(-1, 0, 0), [SIMD3(-0.5,-0.5,-0.5), SIMD3(-0.5,-0.5, 0.5), SIMD3(-0.5, 0.5, 0.5), SIMD3(-0.5, 0.5,-0.5)]),
            (SIMD3(0, 1, 0),  [SIMD3(-0.5, 0.5, 0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3(-0.5, 0.5,-0.5)]),
            (SIMD3(0,-1, 0),  [SIMD3(-0.5,-0.5,-0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3(-0.5,-0.5, 0.5)]),
        ]
        let qUV: [SIMD4<Float>] = [
            SIMD4(0, 0, 0, 0), SIMD4(1, 0, 0, 0), SIMD4(1, 1, 0, 0), SIMD4(0, 1, 0, 0),
        ]
        for (n, q) in faces {
            let n4 = SIMD4<Float>(n, 0)
            bVerts.append(contentsOf: [
                Rh3dMeshVert(pos: SIMD4(q[0], 0), normal: n4, uv: qUV[0]),
                Rh3dMeshVert(pos: SIMD4(q[1], 0), normal: n4, uv: qUV[1]),
                Rh3dMeshVert(pos: SIMD4(q[2], 0), normal: n4, uv: qUV[2]),
                Rh3dMeshVert(pos: SIMD4(q[0], 0), normal: n4, uv: qUV[0]),
                Rh3dMeshVert(pos: SIMD4(q[2], 0), normal: n4, uv: qUV[2]),
                Rh3dMeshVert(pos: SIMD4(q[3], 0), normal: n4, uv: qUV[3]),
            ])
        }
        boxVCount = bVerts.count
        boxVB = dev.makeBuffer(bytes: bVerts, length: bVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)

        // 2. Unit Cylinder (16 sides; paired u=[0,0.5] / [0.5,1] so 8 sleek architectural vertical flutes + crisp cap rings)
        var cVerts: [Rh3dMeshVert] = []
        let segs = 16
        for i in 0..<segs {
            let a0 = Float(i) / Float(segs) * .pi * 2
            let a1 = Float(i + 1) / Float(segs) * .pi * 2
            let x0 = cos(a0) * 0.5, z0 = sin(a0) * 0.5
            let x1 = cos(a1) * 0.5, z1 = sin(a1) * 0.5
            let n0 = SIMD4<Float>(cos(a0), 0, sin(a0), 0)
            let n1 = SIMD4<Float>(cos(a1), 0, sin(a1), 0)
            let p00 = SIMD4<Float>(x0, -0.5, z0, 0), p01 = SIMD4<Float>(x0, 0.5, z0, 0)
            let p10 = SIMD4<Float>(x1, -0.5, z1, 0), p11 = SIMD4<Float>(x1, 0.5, z1, 0)
            let u0: Float = (i % 2 == 0) ? 0.0 : 0.5
            let u1: Float = (i % 2 == 0) ? 0.5 : 1.0
            cVerts.append(contentsOf: [
                Rh3dMeshVert(pos: p00, normal: n0, uv: SIMD4(u0, 0, 0, 0)),
                Rh3dMeshVert(pos: p10, normal: n1, uv: SIMD4(u1, 0, 0, 0)),
                Rh3dMeshVert(pos: p11, normal: n1, uv: SIMD4(u1, 1, 0, 0)),
                Rh3dMeshVert(pos: p00, normal: n0, uv: SIMD4(u0, 0, 0, 0)),
                Rh3dMeshVert(pos: p11, normal: n1, uv: SIMD4(u1, 1, 0, 0)),
                Rh3dMeshVert(pos: p01, normal: n0, uv: SIMD4(u0, 1, 0, 0)),
                // Top cap (capFlag = 1)
                Rh3dMeshVert(pos: SIMD4(0, 0.5, 0, 1), normal: SIMD4(0, 1, 0, 0), uv: SIMD4(0.5, 0.5, 0, 0)),
                Rh3dMeshVert(pos: SIMD4(x0, 0.5, z0, 1), normal: SIMD4(0, 1, 0, 0), uv: SIMD4(x0 + 0.5, z0 + 0.5, 0, 0)),
                Rh3dMeshVert(pos: SIMD4(x1, 0.5, z1, 1), normal: SIMD4(0, 1, 0, 0), uv: SIMD4(x1 + 0.5, z1 + 0.5, 0, 0)),
                // Bottom cap (capFlag = 1)
                Rh3dMeshVert(pos: SIMD4(0, -0.5, 0, 1), normal: SIMD4(0, -1, 0, 0), uv: SIMD4(0.5, 0.5, 0, 0)),
                Rh3dMeshVert(pos: SIMD4(x1, -0.5, z1, 1), normal: SIMD4(0, -1, 0, 0), uv: SIMD4(x1 + 0.5, z1 + 0.5, 0, 0)),
                Rh3dMeshVert(pos: SIMD4(x0, -0.5, z0, 1), normal: SIMD4(0, -1, 0, 0), uv: SIMD4(x0 + 0.5, z0 + 0.5, 0, 0)),
            ])
        }
        cylVCount = cVerts.count
        cylVB = dev.makeBuffer(bytes: cVerts, length: cVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)

        // 3. Unit Octahedron Crystal with per-triangle barycentric coords in uv.zw for crisp glowing crystal edges
        var oVerts: [Rh3dMeshVert] = []
        let top = SIMD3<Float>(0, 0.5, 0), bot = SIMD3<Float>(0, -0.5, 0)
        let ring = [SIMD3<Float>(0.5, 0, 0), SIMD3<Float>(0, 0, 0.5), SIMD3<Float>(-0.5, 0, 0), SIMD3<Float>(0, 0, -0.5)]
        for i in 0..<4 {
            let a = ring[i], b = ring[(i + 1) % 4]
            let nUp = SIMD4<Float>(simd_normalize(simd_cross(b - top, a - top)), 0)
            oVerts.append(contentsOf: [
                Rh3dMeshVert(pos: SIMD4(top, 0), normal: nUp, uv: SIMD4(0, 0, 1, 0)),
                Rh3dMeshVert(pos: SIMD4(b, 0),   normal: nUp, uv: SIMD4(0, 0, 0, 1)),
                Rh3dMeshVert(pos: SIMD4(a, 0),   normal: nUp, uv: SIMD4(0, 0, 0, 0)),
            ])
            let nDn = SIMD4<Float>(simd_normalize(simd_cross(a - bot, b - bot)), 0)
            oVerts.append(contentsOf: [
                Rh3dMeshVert(pos: SIMD4(bot, 0), normal: nDn, uv: SIMD4(0, 0, 1, 0)),
                Rh3dMeshVert(pos: SIMD4(a, 0),   normal: nDn, uv: SIMD4(0, 0, 0, 1)),
                Rh3dMeshVert(pos: SIMD4(b, 0),   normal: nDn, uv: SIMD4(0, 0, 0, 0)),
            ])
        }
        octVCount = oVerts.count
        octVB = dev.makeBuffer(bytes: oVerts, length: oVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)
    }

    // MARK: - Build All 8 Real-Data OpenAgents Grid Landmarks

    private func loadSceneFromCoaxAndAtlas() {
        let jsonStr = DesktopCore.callString(65536) { oa_dc_rh3d_scene_json($0) }
        guard let data = jsonStr.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        if let durMs = obj["duration_ms"] as? Int {
            durationS = Float(durMs) / 1000.0
        }

        // Strict OpenAgents Grid Palette: monochrome white/silver + electric cyan + surgical amber (zero rainbow!)
        let accents: [SIMD4<Float>] = [
            GridTokens.cyan,      // 1. Zeta Terrain
            GridTokens.amber,     // 2. Robin 5040
            GridTokens.cyan,      // 3. 2/3 Barrier
            GridTokens.greyWhite, // 4. Theta Bridge
            GridTokens.cyan,      // 5. 175k City
            GridTokens.amber,     // 6. Behrend/Weil
            GridTokens.cyan,      // 7. 4 Kernels
            GridTokens.amberHot,  // 8. Summit Q.E.D.
        ]

        stations.removeAll()
        if let stArr = obj["stations"] as? [[String: Any]] {
            for (i, s) in stArr.enumerated() {
                let pos = SIMD3<Float>(
                    Float((s["x_mm"] as? Int) ?? 0) / 1000.0,
                    Float((s["y_mm"] as? Int) ?? 0) / 1000.0,
                    Float((s["z_mm"] as? Int) ?? 0) / 1000.0
                )
                let ev = SIMD3<Float>(
                    Float((s["eye_x_mm"] as? Int) ?? 0) / 1000.0,
                    Float((s["eye_y_mm"] as? Int) ?? 0) / 1000.0,
                    Float((s["eye_z_mm"] as? Int) ?? 0) / 1000.0
                )
                stations.append(Rh3dStationData(
                    index: i,
                    pos: pos,
                    eye: ev,
                    timeS: Float((s["time_ms"] as? Int) ?? 0) / 1000.0,
                    shortName: (s["short_name"] as? String) ?? "Station \(i+1)",
                    title: (s["title"] as? String) ?? "",
                    subtitle: (s["subtitle"] as? String) ?? "",
                    plainEnglish: (s["plain_english"] as? String) ?? "",
                    moduleName: (s["module"] as? String) ?? "Main",
                    accent: accents[i % accents.count]
                ))
            }
        }

        let zeros = ((obj["zeros_mm"] as? [Int]) ?? []).map { Float($0) / 1000.0 }
        let robin = (obj["robin"] as? [[String: Any]]) ?? []
        let theta = (obj["theta"] as? [[String: Any]]) ?? []
        let zNs = (obj["zeta_ns"] as? Int) ?? 0
        let zNt = (obj["zeta_nt"] as? Int) ?? 0
        let zHeights = ((obj["zeta_mm"] as? [Int]) ?? []).map { Float($0) / 1000.0 }

        buildWorldGeometry(zeros: zeros, robin: robin, theta: theta, zNs: zNs, zNt: zNt, zHeights: zHeights)
        uploadInstanceBuffers()
        evaluateMetaverseSpline(at: 0.0)
    }

    private func nsColor(_ c: SIMD4<Float>) -> NSColor {
        NSColor(calibratedRed: CGFloat(c.x), green: CGFloat(c.y), blue: CGFloat(c.z), alpha: 1.0)
    }

    private func addLabel(_ pos: SIMD3<Float>, _ text: String, sub: String = "", color: SIMD4<Float>, station: Int, banner: Bool = false, maxDist: Float = 28.0) {
        labelAnchors.append(Rh3dLabelAnchor(
            worldPos: pos, text: text, subtext: sub, color: nsColor(color),
            stationIndex: station, isStationBanner: banner, maxDist: maxDist
        ))
    }

    // Helper: emit a sleek dark-obsidian box edged in crisp luminous Grid wireframe
    private func addGridBox(
        center: SIMD3<Float>, size: SIMD3<Float>, rotY: Float = 0,
        face: SIMD4<Float> = GridTokens.obsidianBody,
        edge: SIMD4<Float> = GridTokens.greyLight,
        edgeIntensity: Float = 0.85,
        edgePx: Float = 1.35,
        mode: Float = 0.0,
        floorBands: Float = 0.0,
        translucent: Bool = false
    ) {
        let inst = Rh3dInstance(
            model: rh3dModelBox(center: center, size: size, rotY: rotY),
            faceColor: face,
            edgeColor: SIMD4(edge.x, edge.y, edge.z, edgeIntensity),
            params: SIMD4(mode, edgePx, floorBands, 0)
        )
        if translucent { transBoxes.append(inst) } else { opaqueBoxes.append(inst) }
    }

    // Helper: emit a dark-obsidian cylinder with crisp luminous top/bottom rings & vertical ribs
    private func addGridCyl(
        center: SIMD3<Float>, size: SIMD3<Float>,
        face: SIMD4<Float> = GridTokens.obsidianBody,
        edge: SIMD4<Float> = GridTokens.greyLight,
        edgeIntensity: Float = 0.90,
        edgePx: Float = 1.30
    ) {
        opaqueCyls.append(Rh3dInstance(
            model: rh3dModelBox(center: center, size: size),
            faceColor: face,
            edgeColor: SIMD4(edge.x, edge.y, edge.z, edgeIntensity),
            params: SIMD4(2.0, edgePx, 0, 0)
        ))
    }

    // Helper: emit a radiant wireframe-faceted crystal core (mode 3)
    private func addGridCrystal(
        center: SIMD3<Float>, size: SIMD3<Float>,
        core: SIMD4<Float>, edge: SIMD4<Float>,
        intensity: Float = 1.45, phase: Float = 0
    ) {
        opaqueOcts.append(Rh3dInstance(
            model: rh3dModelBox(center: center, size: size),
            faceColor: core,
            edgeColor: SIMD4(edge.x, edge.y, edge.z, intensity),
            params: SIMD4(3.0, 1.4, 0, phase)
        ))
    }

    private func buildWorldGeometry(zeros: [Float], robin: [[String: Any]], theta: [[String: Any]], zNs: Int, zNt: Int, zHeights: [Float]) {
        opaqueBoxes.removeAll()
        opaqueCyls.removeAll()
        opaqueOcts.removeAll()
        transBoxes.removeAll()
        transCyls.removeAll()
        labelAnchors.removeAll()
        pickables.removeAll()

        var segs: [Rh3dQuadLineSeg] = []
        func addLine(_ a: SIMD3<Float>, _ b: SIMD3<Float>, color: SIMD4<Float>, light: Float = 0.85, core: Float = 0.85, glow: Float = 4.0, pulseSpeed: Float = 0, pulsePhase: Float = 0) {
            let mode: Float = pulseSpeed > 0.001 ? 1.0 : 0.0
            segs.append(Rh3dQuadLineSeg(
                p0Width: SIMD4(a, core),
                p1Glow: SIMD4(b, glow),
                color: SIMD4(color.x, color.y, color.z, light),
                anim: SIMD4(pulseSpeed, pulsePhase, mode, 0)
            ))
        }

        // Helper: Open Grid-style Wireframe Pavilion Portal (homage to the iconic GYM wireframe frame in the-grid.jpg!)
        func addGridPortalFrame(centerX: Float, zOffset: Float, width: Float, height: Float, depth: Float, color: SIMD4<Float>, light: Float = 0.62) {
            let hw = width * 0.5, hd = depth * 0.5
            let x0 = centerX - hw, x1 = centerX + hw
            let z0 = zOffset - hd, z1 = zOffset + hd
            let y0: Float = 0.08, y1 = height
            // 4 vertical corner columns
            addLine(SIMD3(x0, y0, z0), SIMD3(x0, y1, z0), color: color, light: light, core: 0.9, glow: 3.8)
            addLine(SIMD3(x1, y0, z0), SIMD3(x1, y1, z0), color: color, light: light, core: 0.9, glow: 3.8)
            addLine(SIMD3(x0, y0, z1), SIMD3(x0, y1, z1), color: color, light: light * 0.7, core: 0.75, glow: 3.0)
            addLine(SIMD3(x1, y0, z1), SIMD3(x1, y1, z1), color: color, light: light * 0.7, core: 0.75, glow: 3.0)
            // Top beam frame
            addLine(SIMD3(x0, y1, z0), SIMD3(x1, y1, z0), color: color, light: light * 1.15, core: 1.0, glow: 4.5)
            addLine(SIMD3(x0, y1, z1), SIMD3(x1, y1, z1), color: color, light: light * 0.85, core: 0.8, glow: 3.5)
            addLine(SIMD3(x0, y1, z0), SIMD3(x0, y1, z1), color: color, light: light * 0.85, core: 0.8, glow: 3.5)
            addLine(SIMD3(x1, y1, z0), SIMD3(x1, y1, z1), color: color, light: light * 0.85, core: 0.8, glow: 3.5)
            // Base perimeter trim
            addLine(SIMD3(x0, y0, z0), SIMD3(x1, y0, z0), color: color, light: light * 0.55, core: 0.7, glow: 2.8)
            addLine(SIMD3(x0, y0, z1), SIMD3(x1, y0, z1), color: color, light: light * 0.55, core: 0.7, glow: 2.8)
        }

        // ---- 0. Station Bays, Sleek Obsidian Plinths, Proof Highway Conduits & Animated Data Pulses ----
        for (i, st) in stations.enumerated() {
            // Sleek obsidian architectural base platform edged in crisp silver-white
            addGridBox(
                center: SIMD3(st.pos.x, 0.09, st.pos.z),
                size: SIMD3(9.2, 0.18, 9.2),
                face: GridTokens.obsidianDeep,
                edge: GridTokens.greyLight,
                edgeIntensity: 0.78,
                edgePx: 1.35
            )
            // Fine glowing accent edge tracks along front & rear of bay
            addLine(SIMD3(st.pos.x - 4.6, 0.20, 4.6), SIMD3(st.pos.x + 4.6, 0.20, 4.6), color: st.accent, light: 0.95, core: 0.95, glow: 4.8)
            addLine(SIMD3(st.pos.x - 4.6, 0.20, -4.6), SIMD3(st.pos.x + 4.6, 0.20, -4.6), color: GridTokens.greyLight, light: 0.50, core: 0.75, glow: 3.2)

            // Station banner floating above landmark (staggered heights so ALL 8 banners display cleanly in Overview & close-ups)
            let bannerHeights: [Float] = [7.6, 6.2, 9.2, 6.4, 10.6, 6.8, 9.0, 11.4]
            let bannerY: Float = bannerHeights[i % bannerHeights.count]
            addLabel(SIMD3(st.pos.x, bannerY, st.pos.z), st.shortName.uppercased(), sub: st.moduleName, color: st.accent, station: i, banner: true, maxDist: 145.0)
            pickables.append(Rh3dPickable(center: SIMD3(st.pos.x, 3.0, st.pos.z), radius: 5.0, title: st.title, detail: st.plainEnglish, moduleName: st.moduleName, stationIndex: i))

            // Glowing proof conduit + animated light pulse flowing from Station i -> Station i+1
            if i + 1 < stations.count {
                let nxt = stations[i + 1]
                let p0 = SIMD3<Float>(st.pos.x + 4.6, 0.22, 0)
                let p1 = SIMD3<Float>(nxt.pos.x - 4.6, 0.22, 0)
                addLine(p0, p1, color: GridTokens.cyan, light: 0.92, core: 1.1, glow: 6.2, pulseSpeed: 0.45, pulsePhase: Float(i) * 0.14)
                addLine(p0 + SIMD3(0, 0, 0.28), p1 + SIMD3(0, 0, 0.28), color: GridTokens.greyMid, light: 0.35, core: 0.65, glow: 2.4)
                addLine(p0 + SIMD3(0, 0, -0.28), p1 + SIMD3(0, 0, -0.28), color: GridTokens.greyMid, light: 0.35, core: 0.65, glow: 2.4)

                // Graceful curved multi-segment data arc overhead linking stations
                let arcSteps = 10
                var prevArc: SIMD3<Float>? = nil
                for k in 0...arcSteps {
                    let u = Float(k) / Float(arcSteps)
                    let ax = (st.pos.x + 3.0) + ((nxt.pos.x - 3.0) - (st.pos.x + 3.0)) * u
                    let ay = 2.0 + sin(u * .pi) * 2.5
                    let az: Float = -4.2 - sin(u * .pi) * 0.7
                    let pt = SIMD3<Float>(ax, ay, az)
                    if let pa = prevArc {
                        addLine(pa, pt, color: GridTokens.greyLight, light: 0.45, core: 0.72, glow: 3.4, pulseSpeed: 0.32, pulsePhase: Float(i) * 0.22 + u * 0.18)
                    }
                    prevArc = pt
                }
            }
        }

        // ---- STATION 1 (X = -42): HOLOGRAPHIC LIDAR RIEMANN ZETA TERRAIN + CRITICAL LINE LASER WALL + 10 ZERO BEACONS ----
        let x1: Float = -42.0
        if zNs >= 2 && zNt >= 2 && zHeights.count >= zNs * zNt, let dev = device {
            let widthX: Float = 8.4
            let depthZ: Float = 8.4
            func terrainPos(_ i: Int, _ j: Int) -> SIMD3<Float> {
                let u = Float(i) / Float(zNs - 1)
                let v = Float(j) / Float(zNt - 1)
                let h = zHeights[j * zNs + i]
                return SIMD3<Float>(x1 + (u - 0.5) * widthX, 0.24 + h * 0.88, (v - 0.5) * depthZ)
            }
            var tVerts: [Rh3dTerrainVert] = []
            for j in 0..<(zNt - 1) {
                for i in 0..<(zNs - 1) {
                    let p00 = terrainPos(i, j), p10 = terrainPos(i + 1, j)
                    let p01 = terrainPos(i, j + 1), p11 = terrainPos(i + 1, j + 1)
                    let nA = simd_normalize(simd_cross(p01 - p00, p10 - p00))
                    let nB = simd_normalize(simd_cross(p01 - p10, p11 - p10))
                    func mkV(_ ii: Int, _ jj: Int, _ p: SIMD3<Float>, _ n: SIMD3<Float>) -> Rh3dTerrainVert {
                        let u = Float(ii) / Float(zNs - 1)
                        let v = Float(jj) / Float(zNt - 1)
                        let h = zHeights[jj * zNs + ii]
                        return Rh3dTerrainVert(
                            posHeight: SIMD4(p, h),
                            normalCrit: SIMD4(n, abs(u - 0.5)),
                            uvPad: SIMD4(u, v, 0, 0)
                        )
                    }
                    tVerts.append(contentsOf: [
                        mkV(i, j, p00, nA), mkV(i, j + 1, p01, nA), mkV(i + 1, j, p10, nA),
                        mkV(i + 1, j, p10, nB), mkV(i, j + 1, p01, nB), mkV(i + 1, j + 1, p11, nB),
                    ])
                }
            }
            terrainVCount = tVerts.count
            terrainVB = dev.makeBuffer(bytes: tVerts, length: tVerts.count * MemoryLayout<Rh3dTerrainVert>.stride, options: .storageModeShared)
        }
        // Sleek laser-etched amber-gold Critical Wall at Re(s) = 1/2 (mode 4)
        addGridBox(
            center: SIMD3(x1, 2.75, 0), size: SIMD3(0.06, 5.0, 8.5),
            face: SIMD4(0.08, 0.05, 0.015, 0.16),
            edge: GridTokens.amber, edgeIntensity: 1.15, edgePx: 1.5, mode: 4.0, translucent: true
        )
        addLine(SIMD3(x1, 5.26, -4.25), SIMD3(x1, 5.26, 4.25), color: GridTokens.amberHot, light: 1.35, core: 1.15, glow: 6.5)
        addLabel(SIMD3(x1, 5.65, -4.18), "CRITICAL WALL Re(s) = 1/2", sub: "ALL zeros must sit on this plane", color: GridTokens.amber, station: 0)
        addLabel(SIMD3(x1 - 3.2, 2.6, 2.6), "|ζ(σ + it)| LIDAR SURFACE", sub: "Height = real Coax zeta magnitude", color: GridTokens.cyan, station: 0)

        // 10 Real Riemann Zero Light Needles along Re(s) = 1/2
        for (zi, gamma) in zeros.enumerated() {
            let v = max(0.0, min(1.0, (gamma - 4.0) / 46.0))
            let zPos = (v - 0.5) * 8.4
            let pillarH: Float = 3.2 + (zi == 0 ? 0.95 : 0.0)
            addLine(SIMD3(x1, 0.24, zPos), SIMD3(x1, 0.24 + pillarH, zPos), color: GridTokens.amberHot, light: 1.25, core: 1.1, glow: 5.8)
            addGridCrystal(
                center: SIMD3(x1, 0.34 + pillarH, zPos), size: SIMD3(0.36, 0.50, 0.36),
                core: SIMD4(0.06, 0.24, 0.26, 1.0), edge: GridTokens.cyan, intensity: 1.55, phase: Float(zi) * 0.7
            )
            if zi == 0 || zi == 2 || zi == 7 {
                addLabel(SIMD3(x1, 0.68 + pillarH, zPos), String(format: "ZERO γ%d = %.3f", zi + 1, gamma), sub: "ζ(1/2 + iγ) = 0", color: GridTokens.cyan, station: 0)
            }
        }

        // ---- STATION 2 (X = -30): ROBIN'S CRITERION (1984) & THE 5040 WIREFRAME GATE ----
        let x2: Float = -30.0
        let ceilingY: Float = 3.40
        // Grid-style open architectural wireframe pavilion framing Robin's 5040 Gate (homage to GYM in the-grid.jpg!)
        addGridPortalFrame(centerX: x2, zOffset: -3.2, width: 8.6, height: 5.8, depth: 1.4, color: GridTokens.greyLight, light: 0.55)
        // Laser-etched horizontal Robin Ceiling plane at ratio = 1.0
        addGridBox(
            center: SIMD3(x2, ceilingY, 0), size: SIMD3(8.4, 0.05, 6.2),
            face: SIMD4(0.015, 0.055, 0.06, 0.16),
            edge: GridTokens.cyan, edgeIntensity: 1.05, edgePx: 1.4, mode: 4.0, translucent: true
        )
        addLabel(SIMD3(x2 + 3.4, ceilingY + 0.18, -2.6), "ROBIN CEILING (1.0)", sub: "RH ⇔ ALL n > 5040 stay below", color: GridTokens.cyan, station: 1)

        for (ri, r) in robin.enumerated() {
            let nVal = (r["n"] as? Int) ?? 0
            let ratio = Float((r["ratio_mm"] as? Int) ?? 900) / 1000.0
            let u = Float(ri) / Float(max(1, robin.count - 1))
            let bx = x2 + (u - 0.5) * 7.4
            let barH = max(0.55, min(5.4, (ratio - 0.68) * 10.0))
            let is5040 = (nVal == 5040)
            let exceeds = ratio >= 1.0
            let edgeCol: SIMD4<Float> = is5040 ? GridTokens.amberHot : (exceeds ? GridTokens.alertRed : GridTokens.cyan)
            let faceCol: SIMD4<Float> = is5040 ? SIMD4(0.06, 0.04, 0.014, 1.0) : GridTokens.obsidianBody
            let bw: Float = is5040 ? 0.56 : 0.40
            addGridBox(
                center: SIMD3(bx, 0.18 + barH * 0.5, 0), size: SIMD3(bw, barH, bw),
                face: faceCol, edge: edgeCol, edgeIntensity: is5040 ? 1.65 : (exceeds ? 1.05 : 0.88), edgePx: is5040 ? 1.6 : 1.35
            )
            // Luminous crisp top cap ring on each bar
            if is5040 {
                addGridCrystal(
                    center: SIMD3(bx, barH + 0.72, 0), size: SIMD3(0.62, 0.88, 0.62),
                    core: SIMD4(0.28, 0.18, 0.04, 1.0), edge: GridTokens.amberHot, intensity: 1.85, phase: 0.3
                )
                addLine(SIMD3(bx, barH + 0.2, 0), SIMD3(bx, barH + 1.3, 0), color: GridTokens.amberHot, light: 1.2, core: 1.0, glow: 5.5)
                addLabel(SIMD3(bx, barH + 1.25, 0), "n = 5040 · LAST EXCEPTION", sub: String(format: "ratio = %.4f > 1.0", ratio), color: GridTokens.amber, station: 1)
            } else if nVal == 12 {
                addLabel(SIMD3(bx, barH + 0.35, 0.6), "n = 12..2520", sub: "Early exceptions (> 1.0)", color: GridTokens.alertRed, station: 1)
            } else if nVal == 55440 {
                addLabel(SIMD3(bx, barH + 0.35, 0.6), "n = 55,440 · SAFE", sub: String(format: "ratio = %.4f < 1.0", ratio), color: GridTokens.cyan, station: 1)
            }
        }

        // ---- STATION 3 (X = -18): SMASHING THE 2/3 CLASSICAL BARRIER ----
        let x3: Float = -18.0
        let barrierY: Float = 3.8
        // Sleek laser-etched crimson-amber 2/3 barrier slab with central portal breached by Alpoge's cyan staircase
        addGridBox(
            center: SIMD3(x3, barrierY, -2.5), size: SIMD3(8.2, 0.16, 2.1),
            face: SIMD4(0.09, 0.018, 0.018, 0.22), edge: GridTokens.alertRed, edgeIntensity: 1.25, edgePx: 1.5, mode: 4.0, translucent: true
        )
        addGridBox(
            center: SIMD3(x3, barrierY, 2.5), size: SIMD3(8.2, 0.16, 2.1),
            face: SIMD4(0.09, 0.018, 0.018, 0.22), edge: GridTokens.alertRed, edgeIntensity: 1.25, edgePx: 1.5, mode: 4.0, translucent: true
        )
        // Dark obsidian stanchions supporting the 2/3 barrier
        addGridBox(center: SIMD3(x3 - 3.9, barrierY * 0.5, -2.5), size: SIMD3(0.28, barrierY, 0.28), face: GridTokens.obsidianBody, edge: GridTokens.alertRed, edgeIntensity: 0.95)
        addGridBox(center: SIMD3(x3 + 3.9, barrierY * 0.5, -2.5), size: SIMD3(0.28, barrierY, 0.28), face: GridTokens.obsidianBody, edge: GridTokens.alertRed, edgeIntensity: 0.95)
        addLine(SIMD3(x3 - 4.1, barrierY, -1.45), SIMD3(x3 + 4.1, barrierY, -1.45), color: GridTokens.alertRed, light: 1.25, core: 1.1, glow: 5.5)
        addLabel(SIMD3(x3 - 2.6, barrierY + 0.35, -2.6), "2/3 CLASSICAL BARRIER", sub: "100-year density ceiling", color: GridTokens.alertRed, station: 2)

        for (ti, th) in theta.enumerated() {
            let cls = Float((th["classical_mm"] as? Int) ?? 500) / 1000.0
            let alp = Float((th["alpoge_mm"] as? Int) ?? 700) / 1000.0
            let u = Float(ti) / Float(max(1, theta.count - 1))
            let sx = x3 + (u - 0.5) * 6.8
            let clsH = cls * 5.7
            let alpH = alp * 5.7
            // Stalled obsidian columns behind (trapped under 2/3, subtle red wireframe edges)
            addGridBox(
                center: SIMD3(sx, 0.18 + clsH * 0.5, -1.5), size: SIMD3(0.44, clsH, 0.44),
                face: GridTokens.obsidianDeep, edge: GridTokens.alertRed, edgeIntensity: 0.72, edgePx: 1.25
            )
            // Alpoge's dark-obsidian & blazing electric-cyan staircase punching cleanly past 2/3!
            let crossed = alp > 0.6667
            let sEdge = crossed ? GridTokens.cyan : GridTokens.greyLight
            addGridBox(
                center: SIMD3(sx, 0.18 + alpH * 0.5, 0.5), size: SIMD3(0.56, alpH, 0.70),
                face: crossed ? SIMD4(0.014, 0.036, 0.042, 1.0) : GridTokens.obsidianBody,
                edge: sEdge, edgeIntensity: crossed ? 1.38 : 0.80, edgePx: 1.45
            )
            if ti == 7 {
                addGridCrystal(
                    center: SIMD3(sx, alpH + 0.72, 0.5), size: SIMD3(0.54, 0.76, 0.54),
                    core: SIMD4(0.04, 0.24, 0.26, 1.0), edge: GridTokens.cyan, intensity: 1.75, phase: 1.2
                )
                addLabel(SIMD3(sx, alpH + 1.18, 0.5), "ALPÖGE √X STAIRCASE", sub: String(format: "Stage 8 hits %.2f (past 2/3!)", alp), color: GridTokens.cyan, station: 2)
            }
            if ti == 3 {
                addLabel(SIMD3(sx, clsH + 0.30, -1.5), "CLASSICAL STALLS", sub: String(format: "Trapped at %.2f < 0.67", cls), color: GridTokens.alertRed, station: 2)
            }
        }

        // ---- STATION 4 (X = -6): THETA-LIFT & FORMAL DEGREE CATHEDRAL (Pure Monochrome Silver + Cyan Pulses) ----
        let x4: Float = -6.0
        let pillars4 = [
            ("FormalDegreeBessel", "Formal degree & Bessel identity"),
            ("ThetaPairLifts", "Iterated theta-lift transfer"),
            ("Whittaker", "Whittaker period integrals"),
            ("Automorphic", "Automorphic representations"),
            ("Spectral", "Trace formula spectral side"),
            ("BesselKernels", "Archimedean Bessel kernels"),
            ("HeckeEigen", "Hecke eigenvalue bounds"),
            ("Endoscopy", "Endoscopic transfer"),
        ]
        let core4 = SIMD3<Float>(x4, 3.7, 0)
        addGridCrystal(
            center: core4, size: SIMD3(1.45, 2.15, 1.45),
            core: SIMD4(0.04, 0.14, 0.16, 1.0), edge: GridTokens.cyan, intensity: 1.75, phase: 2.0
        )
        // Concentric wireframe orbital ring around Theta Core
        for rStep in 0..<24 {
            let a0 = Float(rStep) / 24.0 * .pi * 2
            let a1 = Float(rStep + 1) / 24.0 * .pi * 2
            addLine(
                SIMD3(x4 + cos(a0) * 1.9, 3.7, sin(a0) * 1.9),
                SIMD3(x4 + cos(a1) * 1.9, 3.7, sin(a1) * 1.9),
                color: GridTokens.greyWhite, light: 0.78, core: 0.85, glow: 3.6
            )
        }
        addLabel(SIMD3(x4, 5.2, 0), "THETA-LIFT CORE", sub: "Primes ↔ Hyperbolic Geometry", color: GridTokens.cyan, station: 3)
        for (pi, (pname, psub)) in pillars4.enumerated() {
            let ang = Float(pi) / Float(pillars4.count) * .pi * 2.0 - .pi * 0.25
            let px = x4 + cos(ang) * 3.45
            let pz = sin(ang) * 3.45
            let ph: Float = 4.2 + Float(pi % 3) * 0.72
            let pEdge = (pi % 2 == 0) ? GridTokens.greyWhite : GridTokens.greyLight
            addGridCyl(center: SIMD3(px, 0.18 + ph * 0.5, pz), size: SIMD3(0.54, ph, 0.54), face: GridTokens.obsidianBody, edge: pEdge, edgeIntensity: 0.92)
            // Crisp silver capital cap
            addGridBox(center: SIMD3(px, ph + 0.28, pz), size: SIMD3(0.72, 0.16, 0.72), face: GridTokens.obsidianSlate, edge: GridTokens.cyan, edgeIntensity: 1.15)
            // Pulsing energy filament into central Theta Core
            addLine(SIMD3(px, ph + 0.25, pz), core4, color: GridTokens.cyan, light: 0.92, core: 0.85, glow: 4.8, pulseSpeed: 0.65, pulsePhase: Float(pi) * 0.13)
            if pi == 0 || pi == 1 || pi == 4 || pi == 7 {
                addLabel(SIMD3(px, ph + 0.6, pz), pname.uppercased(), sub: psub, color: GridTokens.greyWhite, station: 3)
            }
        }

        // ---- STATION 5 (X = +6): REAL 175,182-FILE OBSIDIAN-GLASS PROOF METROPOLIS (HYDRATED FROM rh-atlas.json!) ----
        let x5: Float = 6.0
        buildRealProofCity(centerX: x5, addLine: addLine)

        // ---- STATION 6 (X = +18): BEHREND SPHERE WIREFRAME LATTICE + WEIL BOUND ARCHES ----
        let x6: Float = 18.0
        // Outer precision wireframe bounding cube around the 3D Behrend lattice
        let bMin = SIMD3<Float>(x6 - 2.6, 0.6, -2.6), bMax = SIMD3<Float>(x6 + 2.6, 5.2, 2.6)
        let cubeCorners: [SIMD3<Float>] = [
            SIMD3(bMin.x, bMin.y, bMin.z), SIMD3(bMax.x, bMin.y, bMin.z), SIMD3(bMax.x, bMax.y, bMin.z), SIMD3(bMin.x, bMax.y, bMin.z),
            SIMD3(bMin.x, bMin.y, bMax.z), SIMD3(bMax.x, bMin.y, bMax.z), SIMD3(bMax.x, bMax.y, bMax.z), SIMD3(bMin.x, bMax.y, bMax.z),
        ]
        let cubeEdges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)]
        for (a, b) in cubeEdges {
            addLine(cubeCorners[a], cubeCorners[b], color: GridTokens.greyDim, light: 0.45, core: 0.65, glow: 2.6)
        }
        for ix in -2...2 {
            for iy in 0...4 {
                for iz in -2...2 {
                    let fx = Float(ix), fy = Float(iy) - 2.0, fz = Float(iz)
                    let r2 = fx * fx + fy * fy + fz * fz
                    let onBehrendShell = (r2 >= 3.8 && r2 <= 5.2)
                    let p = SIMD3<Float>(x6 + fx * 1.12, 1.05 + Float(iy) * 0.92, fz * 1.12)
                    if onBehrendShell {
                        addGridBox(
                            center: p, size: SIMD3(0.38, 0.38, 0.38),
                            face: SIMD4(0.08, 0.045, 0.012, 1.0), edge: GridTokens.amberHot, edgeIntensity: 1.50, edgePx: 1.45
                        )
                    } else if (ix + iy + iz) % 2 == 0 {
                        addGridBox(
                            center: p, size: SIMD3(0.20, 0.20, 0.20),
                            face: GridTokens.obsidianGlass, edge: GridTokens.greyDim, edgeIntensity: 0.45, edgePx: 1.0, translucent: true
                        )
                    }
                }
            }
        }
        // 3 razor-sharp luminous parabolic Weil finite-field arches overhead
        for (ai, az) in ([-2.3 as Float, 0.0, 2.3]).enumerated() {
            var prev: SIMD3<Float>? = nil
            for step in 0...16 {
                let a = Float(step) / 16.0 * .pi
                let pt = SIMD3<Float>(x6 + cos(a) * 4.1, 1.5 + sin(a) * 4.7, az)
                if let p0 = prev {
                    addLine(p0, pt, color: ai == 1 ? GridTokens.amberHot : GridTokens.greyLight, light: ai == 1 ? 1.20 : 0.72, core: ai == 1 ? 1.1 : 0.82, glow: ai == 1 ? 5.8 : 3.8, pulseSpeed: ai == 1 ? 0.40 : 0, pulsePhase: Float(step) * 0.06)
                }
                prev = pt
            }
        }
        addLabel(SIMD3(x6, 3.6, 2.5), "BEHREND SPHERE SHELL", sub: "No 3-term arithmetic progressions", color: GridTokens.amber, station: 5)
        addLabel(SIMD3(x6, 6.3, 0), "WEIL FINITE-FIELD ARCHES", sub: "Error terms bounded by √q", color: GridTokens.amberHot, station: 5)

        // ---- STATION 7 (X = +30): MULTI-KERNEL VERIFICATION FORTRESS ----
        let x7: Float = 30.0
        let core7 = SIMD3<Float>(x7, 3.25, 0)
        addGridPortalFrame(centerX: x7, zOffset: 0.0, width: 8.6, height: 6.2, depth: 8.6, color: GridTokens.greyDim, light: 0.36)
        addGridCrystal(
            center: core7, size: SIMD3(1.55, 2.35, 1.55),
            core: SIMD4(0.03, 0.22, 0.24, 1.0), edge: GridTokens.cyan, intensity: 1.85, phase: 3.1
        )
        addLabel(SIMD3(x7, 4.75, 0), "3 STANDARD AXIOMS ONLY", sub: "propext · Quot.sound · Classical.choice", color: GridTokens.cyan, station: 6)

        let kernels: [(String, String, Float, Float, SIMD4<Float>)] = [
            ("LAKE CHECK --PARANOID", "Lean v4.36.0 official kernel", -2.8, -2.8, GridTokens.cyan),
            ("LEANCHECKER", "Independent C++ kernel",  2.8, -2.8, GridTokens.greyWhite),
            ("NANODA", "Independent Rust kernel",  2.8,  2.8, GridTokens.greyWhite),
            ("CON-LECHE + CON-RON", "Independent multi-kernel audit", -2.8,  2.8, GridTokens.cyan),
        ]
        for (ki, (kname, ksub, dx, dz, kcol)) in kernels.enumerated() {
            let kh: Float = 5.4
            // Tall obsidian monolith with razor-sharp wireframe edges + vertical luminous core slit
            addGridBox(
                center: SIMD3(x7 + dx, 0.18 + kh * 0.5, dz), size: SIMD3(1.12, kh, 1.12),
                face: GridTokens.obsidianBody, edge: kcol, edgeIntensity: 1.15, edgePx: 1.45, mode: 1.0, floorBands: 9.0
            )
            addLine(SIMD3(x7 + dx, 0.5, dz), SIMD3(x7 + dx, kh - 0.1, dz), color: kcol, light: 1.15, core: 1.1, glow: 5.5)
            // Animated cyan verification beam locking into the central axiom crystal
            addLine(SIMD3(x7 + dx, kh - 0.3, dz), core7, color: GridTokens.cyan, light: 1.15, core: 0.95, glow: 5.4, pulseSpeed: 0.72, pulsePhase: Float(ki) * 0.25)
            addLabel(SIMD3(x7 + dx, kh + 0.55, dz), kname, sub: ksub, color: kcol, station: 6)
        }
        // 6 sharp amber sentinel crystals on outer front edge = exact 6 audited metaprogramming sites
        for mi in 0..<6 {
            let ang = -0.75 + Float(mi) * 0.30
            let mx = x7 + sin(ang) * 4.3
            let mz = 3.9 + cos(ang) * 0.5
            addGridCrystal(
                center: SIMD3(mx, 1.15, mz), size: SIMD3(0.34, 0.48, 0.34),
                core: SIMD4(0.18, 0.10, 0.02, 1.0), edge: GridTokens.amberHot, intensity: 1.55, phase: Float(mi) * 0.5
            )
        }
        addLabel(SIMD3(x7, 1.75, 4.4), "6 AUDITED METAPROG SITES", sub: "BuildAid.D1..D5 + CutCptCopy", color: GridTokens.amber, station: 6)

        // ---- STATION 8 (X = +42): THE SUMMIT — Main.riemannHypothesis (Q.E.D.) ----
        let x8: Float = 42.0
        // Sleek terraced obsidian ziggurat edged in razor-sharp luminous white-silver & cyan
        let tiers: [(Float, Float, Float, SIMD4<Float>, Float)] = [
            (6.8, 0.55, 0.35, GridTokens.greyLight, 0.85),
            (5.2, 0.65, 0.98, GridTokens.greyWhite, 0.98),
            (3.6, 0.85, 1.78, GridTokens.cyan,      1.15),
            (2.2, 1.15, 2.82, GridTokens.amberHot,  1.35),
        ]
        for (tw, th, ty, edgeC, eInt) in tiers {
            addGridBox(
                center: SIMD3(x8, ty, 0), size: SIMD3(tw, th, tw),
                face: GridTokens.obsidianBody, edge: edgeC, edgeIntensity: eInt, edgePx: 1.45
            )
        }
        // Central obsidian-and-gold fluted spire rising to Y = 9.4 + sky-piercing light needle
        addGridCyl(
            center: SIMD3(x8, 6.2, 0), size: SIMD3(0.76, 6.2, 0.76),
            face: GridTokens.obsidianDeep, edge: GridTokens.amberHot, edgeIntensity: 1.45, edgePx: 1.4
        )
        addLine(SIMD3(x8, 3.2, 0), SIMD3(x8, 16.5, 0), color: GridTokens.amberHot, light: 1.45, core: 1.15, glow: 8.5, pulseSpeed: 0.38, pulsePhase: 0.0)
        // Radiant Q.E.D. Double-Crystal Crown
        addGridCrystal(
            center: SIMD3(x8, 10.1, 0), size: SIMD3(1.48, 2.20, 1.48),
            core: SIMD4(0.32, 0.22, 0.05, 1.0), edge: GridTokens.amberHot, intensity: 2.0, phase: 0.0
        )
        // Double concentric luminous Grid halo rings around Summit Crown
        for hi in 0..<24 {
            let a0 = Float(hi) / 24.0 * .pi * 2
            let a1 = Float(hi + 1) / 24.0 * .pi * 2
            addLine(
                SIMD3(x8 + cos(a0) * 2.55, 8.6, sin(a0) * 2.55),
                SIMD3(x8 + cos(a1) * 2.55, 8.6, sin(a1) * 2.55),
                color: GridTokens.cyan, light: 1.25, core: 1.05, glow: 5.8
            )
            addLine(
                SIMD3(x8 + cos(a0) * 3.20, 8.6, sin(a0) * 3.20),
                SIMD3(x8 + cos(a1) * 3.20, 8.6, sin(a1) * 3.20),
                color: GridTokens.greyWhite, light: 0.72, core: 0.75, glow: 3.5
            )
        }
        addLabel(SIMD3(x8, 10.4, 0), "THEOREM RIEMANNHYPOTHESIS", sub: ":= rhOfAll  (src/Main.lean)", color: GridTokens.amberHot, station: 7)
        addLabel(SIMD3(x8 + 2.8, 3.0, 2.8), "813,182 VERIFIED THEOREMS", sub: "17.37M lines · MIT Nov 21, 2026", color: GridTokens.cyan, station: 7)

        // Upload screen-space quad-expanded vector lines buffer
        quadLineSegCount = segs.count
        if let dev = device, !segs.isEmpty {
            quadLinesVB = dev.makeBuffer(bytes: segs, length: segs.count * MemoryLayout<Rh3dQuadLineSeg>.stride, options: .storageModeShared)
        }
    }

    private func buildRealProofCity(
        centerX x5: Float,
        addLine: (SIMD3<Float>, SIMD3<Float>, SIMD4<Float>, Float, Float, Float, Float, Float) -> Void
    ) {
        let atlasURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/index/rh-atlas.json")

        // Strict Grid architectural skyline: Obsidian glass towers with crisp white/silver & cyan floor tiers + amber Root beacon
        func districtEdgeColor(_ libIdx: Int) -> (SIMD4<Float>, Float) {
            switch abs(libIdx) % 10 {
            case 0: return (GridTokens.amberHot, 1.35) // Root / Main
            case 1: return (GridTokens.cyan,     1.12) // Formalism (130k files)
            case 2: return (GridTokens.greyWhite, 1.05) // Supplements (44.5k files)
            case 5: return (GridTokens.cyanSoft, 0.95) // Bessel
            default: return (GridTokens.greyLight, 0.82)
            }
        }

        var builtFromAtlas = false
        if let data = try? Data(contentsOf: atlasURL),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let clusters = obj["clusters"] as? [[String: Any]], !clusters.isEmpty {
            builtFromAtlas = true
            let sorted = clusters.sorted { (($0["modules"] as? Int) ?? 0) > (($1["modules"] as? Int) ?? 0) }
            let count = min(42, sorted.count)
            let cols = 7
            let rows = (count + cols - 1) / cols
            for idx in 0..<count {
                let cl = sorted[idx]
                let name = (cl["name"] as? String) ?? "Cluster"
                let mods = (cl["modules"] as? Int) ?? 1
                let lines = (cl["lines"] as? Int) ?? 100
                let libIdx = (cl["lib"] as? Int) ?? 1
                let (eCol, eInt) = districtEdgeColor(libIdx)

                let gx = idx % cols
                let gz = idx / cols
                let bx = x5 + (Float(gx) - Float(cols - 1) * 0.5) * 1.15
                let bz = (Float(gz) - Float(rows - 1) * 0.5) * 1.25
                let bh = max(0.85, min(7.6, Float(log10(Double(max(10, lines)))) * 1.18 - 0.6))
                let bw = max(0.42, min(0.90, 0.36 + Float(log10(Double(max(1, mods)))) * 0.14))
                let bands = max(4.0, (bh * 2.2).rounded())

                // Mode 1 = Architectural Obsidian Glass Skyscraper with glowing horizontal floor bands & wireframe corners!
                addGridBox(
                    center: SIMD3(bx, 0.18 + bh * 0.5, bz),
                    size: SIMD3(bw, bh, bw),
                    face: GridTokens.obsidianDeep,
                    edge: eCol,
                    edgeIntensity: idx < 5 ? eInt * 1.18 : eInt * 0.85,
                    edgePx: idx < 5 ? 1.40 : 1.20,
                    mode: 1.0,
                    floorBands: bands
                )
                // Sleek vertical spire antenna + glowing rooftop parapet on top 5 districts
                if idx < 5 {
                    addGridBox(
                        center: SIMD3(bx, 0.24 + bh, bz), size: SIMD3(bw * 1.04, 0.10, bw * 1.04),
                        face: GridTokens.obsidianSlate, edge: GridTokens.greyWhite, edgeIntensity: 1.45, edgePx: 1.4
                    )
                    addLine(SIMD3(bx, 0.3 + bh, bz), SIMD3(bx, 1.35 + bh, bz), eCol, 1.15, 0.85, 4.5, 0, 0)
                    addLabel(SIMD3(bx, bh + 0.65, bz), name.uppercased(), sub: "\(mods) files · \(lines / 1000)k lines", color: eCol, station: 4)
                    pickables.append(Rh3dPickable(center: SIMD3(bx, bh * 0.5, bz), radius: 1.0, title: name, detail: "\(mods) Lean 4 files (\(lines) lines) in real rh-atlas.json", moduleName: (cl["hub"] as? String) ?? name, stationIndex: 4))
                }
            }
        }

        if !builtFromAtlas {
            let fallback: [(String, Float, Float, Float, SIMD4<Float>)] = [
                ("FORMALISM (130K FILES)", -1.6, 0.0, 7.2, GridTokens.cyan),
                ("SUPPLEMENTS (44.5K FILES)", 1.6, 0.0, 5.8, GridTokens.greyWhite),
                ("ROOT / MAIN", 0.0, -2.2, 3.6, GridTokens.amberHot),
            ]
            for (nm, dx, dz, h, c) in fallback {
                addGridBox(center: SIMD3(x5 + dx, 0.18 + h * 0.5, dz), size: SIMD3(1.5, h, 1.5), face: GridTokens.obsidianDeep, edge: c, edgeIntensity: 1.15, edgePx: 1.4, mode: 1.0, floorBands: h * 2.2)
                addLabel(SIMD3(x5 + dx, h + 0.6, dz), nm, sub: "Lean 4 Library", color: c, station: 4)
            }
        }
    }

    private func uploadInstanceBuffers() {
        guard let dev = device else { return }
        func makeBuf(_ arr: [Rh3dInstance]) -> MTLBuffer? {
            guard !arr.isEmpty else { return nil }
            return dev.makeBuffer(bytes: arr, length: arr.count * MemoryLayout<Rh3dInstance>.stride, options: .storageModeShared)
        }
        opaqueBoxIB = makeBuf(opaqueBoxes)
        opaqueCylIB = makeBuf(opaqueCyls)
        opaqueOctIB = makeBuf(opaqueOcts)
        transBoxIB = makeBuf(transBoxes)
        transCylIB = makeBuf(transCyls)
    }

    // MARK: - Views & OpenAgents Grid Chamfered HUD Setup

    private func styleGridButton(_ btn: NSButton, accent: NSColor? = nil) {
        btn.translatesAutoresizingMaskIntoConstraints = false
        btn.isBordered = false
        btn.wantsLayer = true
        btn.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .semibold)
        btn.layer?.backgroundColor = NSColor(calibratedRed: 0.02, green: 0.024, blue: 0.03, alpha: 0.86).cgColor
        btn.layer?.borderWidth = 1.0
        btn.layer?.borderColor = (accent ?? NSColor(white: 0.94, alpha: 0.30)).withAlphaComponent(accent != nil ? 0.65 : 0.28).cgColor
        btn.layer?.cornerRadius = 2.0
        let titleColor = accent ?? GridTokens.nsWhite
        btn.attributedTitle = NSAttributedString(string: "  \(btn.title)  ", attributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 10.0, weight: .semibold),
            .foregroundColor: titleColor,
            .kern: 0.6,
        ])
    }

    private func updateGridButtonTitle(_ btn: NSButton, _ text: String, accent: NSColor? = nil) {
        let col = accent ?? GridTokens.nsWhite
        btn.layer?.borderColor = col.withAlphaComponent(accent != nil ? 0.72 : 0.28).cgColor
        btn.attributedTitle = NSAttributedString(string: "  \(text)  ", attributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 10.0, weight: .semibold),
            .foregroundColor: col,
            .kern: 0.6,
        ])
    }

    private func setupViews() {
        metalHost.translatesAutoresizingMaskIntoConstraints = false
        metalHost.wantsLayer = true
        let ml = CAMetalLayer()
        ml.device = device
        ml.pixelFormat = .bgra8Unorm
        ml.framebufferOnly = true
        ml.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        metalHost.layer = ml
        metalLayer = ml
        addSubview(metalHost)

        labelsOverlay.translatesAutoresizingMaskIntoConstraints = false
        labelsOverlay.onClickStation = { [weak self] idx in
            self?.jumpToStation(idx)
        }
        addSubview(labelsOverlay)

        // Top-left chamfered Grid HUD strip
        topLeftFrame.translatesAutoresizingMaskIntoConstraints = false
        topLeftFrame.cutSize = 7.0
        topLeftFrame.showBrackets = false
        topLeftFrame.showScanlines = false
        topLeftFrame.fillAlpha = 0.80
        addSubview(topLeftFrame)

        topLeftBadge.translatesAutoresizingMaskIntoConstraints = false
        topLeftBadge.stringValue = "OPENAGENTS / THE GRID  ·  DRAG ORBIT · SCROLL ZOOM · WASD FLY · SPACE PLAY"
        topLeftBadge.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .medium)
        topLeftBadge.textColor = GridTokens.nsLight
        topLeftFrame.addSubview(topLeftBadge)

        for (btn, act, acc) in [
            (playPauseBtn, #selector(togglePlayPause), GridTokens.nsCyan as NSColor?),
            (overviewBtn,  #selector(goOverview), nil),
            (labelsBtn,    #selector(toggleLabels), nil),
            (inspectBtn,   #selector(openSourceClicked), GridTokens.nsWhite as NSColor?),
        ] {
            styleGridButton(btn, accent: acc)
            btn.target = self
            btn.action = act
            addSubview(btn)
        }

        // Bottom OpenAgents Grid Chamfered Story HUD Frame (exact hud-frame.coax L-brackets & 45° cuts!)
        storyCard.translatesAutoresizingMaskIntoConstraints = false
        storyCard.cutSize = 12.0
        storyCard.showBrackets = true
        storyCard.showScanlines = true
        storyCard.fillAlpha = 0.88
        addSubview(storyCard)

        storyKicker.translatesAutoresizingMaskIntoConstraints = false
        storyKicker.font = NSFont.monospacedSystemFont(ofSize: 9.0, weight: .medium)
        storyKicker.textColor = GridTokens.nsMid
        storyCard.addSubview(storyKicker)

        storyTitle.translatesAutoresizingMaskIntoConstraints = false
        storyTitle.font = NSFont.monospacedSystemFont(ofSize: 13.5, weight: .bold)
        storyTitle.textColor = GridTokens.nsWhite
        storyCard.addSubview(storyTitle)

        storySubtitle.translatesAutoresizingMaskIntoConstraints = false
        storySubtitle.font = NSFont.monospacedSystemFont(ofSize: 10.2, weight: .medium)
        storySubtitle.textColor = GridTokens.nsCyan
        storyCard.addSubview(storySubtitle)

        storyBody.translatesAutoresizingMaskIntoConstraints = false
        storyBody.font = NSFont.systemFont(ofSize: 11.8, weight: .regular)
        storyBody.textColor = GridTokens.nsLight
        storyBody.maximumNumberOfLines = 2
        storyCard.addSubview(storyBody)

        for (btn, act) in [(prevBtn, #selector(prevStationClicked)), (nextBtn, #selector(nextStationClicked))] {
            styleGridButton(btn, accent: nil)
            btn.target = self
            btn.action = act
            storyCard.addSubview(btn)
        }

        // Exact the-grid/index.html progress track (#f0f0f0 14% track + hsl(180,68%,53%) cyan bar)
        progressTrack.translatesAutoresizingMaskIntoConstraints = false
        progressTrack.wantsLayer = true
        progressTrack.layer?.backgroundColor = NSColor(white: 0.94, alpha: 0.14).cgColor
        storyCard.addSubview(progressTrack)

        progressFill.translatesAutoresizingMaskIntoConstraints = false
        progressFill.wantsLayer = true
        progressFill.layer?.backgroundColor = GridTokens.nsCyan.cgColor
        progressTrack.addSubview(progressFill)
        progressFillWidth = progressFill.widthAnchor.constraint(equalToConstant: 24)

        stationStack.translatesAutoresizingMaskIntoConstraints = false
        stationStack.orientation = .horizontal
        stationStack.spacing = 4
        stationStack.distribution = .fillEqually
        storyCard.addSubview(stationStack)

        stationButtons.removeAll()
        for (i, st) in stations.enumerated() {
            let b = NSButton(title: st.shortName.uppercased(), target: self, action: #selector(stationPillClicked(_:)))
            b.tag = i
            styleGridButton(b, accent: nil)
            stationStack.addArrangedSubview(b)
            stationButtons.append(b)
        }

        NSLayoutConstraint.activate([
            metalHost.topAnchor.constraint(equalTo: topAnchor),
            metalHost.leadingAnchor.constraint(equalTo: leadingAnchor),
            metalHost.trailingAnchor.constraint(equalTo: trailingAnchor),
            metalHost.bottomAnchor.constraint(equalTo: bottomAnchor),

            labelsOverlay.topAnchor.constraint(equalTo: topAnchor),
            labelsOverlay.leadingAnchor.constraint(equalTo: leadingAnchor),
            labelsOverlay.trailingAnchor.constraint(equalTo: trailingAnchor),
            labelsOverlay.bottomAnchor.constraint(equalTo: bottomAnchor),

            topLeftFrame.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            topLeftFrame.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            topLeftFrame.heightAnchor.constraint(equalToConstant: 24),

            topLeftBadge.centerYAnchor.constraint(equalTo: topLeftFrame.centerYAnchor),
            topLeftBadge.leadingAnchor.constraint(equalTo: topLeftFrame.leadingAnchor, constant: 10),
            topLeftBadge.trailingAnchor.constraint(equalTo: topLeftFrame.trailingAnchor, constant: -14),

            inspectBtn.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            inspectBtn.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            inspectBtn.heightAnchor.constraint(equalToConstant: 24),
            labelsBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            labelsBtn.trailingAnchor.constraint(equalTo: inspectBtn.leadingAnchor, constant: -6),
            labelsBtn.heightAnchor.constraint(equalToConstant: 24),
            overviewBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            overviewBtn.trailingAnchor.constraint(equalTo: labelsBtn.leadingAnchor, constant: -6),
            overviewBtn.heightAnchor.constraint(equalToConstant: 24),
            playPauseBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            playPauseBtn.trailingAnchor.constraint(equalTo: overviewBtn.leadingAnchor, constant: -6),
            playPauseBtn.heightAnchor.constraint(equalToConstant: 24),

            storyCard.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            storyCard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            storyCard.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
            storyCard.heightAnchor.constraint(equalToConstant: 136),

            storyKicker.topAnchor.constraint(equalTo: storyCard.topAnchor, constant: 8),
            storyKicker.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),

            storyTitle.topAnchor.constraint(equalTo: storyKicker.bottomAnchor, constant: 2),
            storyTitle.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),
            storyTitle.trailingAnchor.constraint(equalTo: prevBtn.leadingAnchor, constant: -10),

            nextBtn.topAnchor.constraint(equalTo: storyCard.topAnchor, constant: 10),
            nextBtn.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -16),
            nextBtn.heightAnchor.constraint(equalToConstant: 22),
            prevBtn.centerYAnchor.constraint(equalTo: nextBtn.centerYAnchor),
            prevBtn.trailingAnchor.constraint(equalTo: nextBtn.leadingAnchor, constant: -6),
            prevBtn.heightAnchor.constraint(equalToConstant: 22),

            storySubtitle.topAnchor.constraint(equalTo: storyTitle.bottomAnchor, constant: 2),
            storySubtitle.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),
            storySubtitle.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -16),

            storyBody.topAnchor.constraint(equalTo: storySubtitle.bottomAnchor, constant: 4),
            storyBody.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),
            storyBody.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -16),

            progressTrack.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),
            progressTrack.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -16),
            progressTrack.bottomAnchor.constraint(equalTo: stationStack.topAnchor, constant: -7),
            progressTrack.heightAnchor.constraint(equalToConstant: 2),

            progressFill.leadingAnchor.constraint(equalTo: progressTrack.leadingAnchor),
            progressFill.topAnchor.constraint(equalTo: progressTrack.topAnchor),
            progressFill.bottomAnchor.constraint(equalTo: progressTrack.bottomAnchor),
            progressFillWidth!,

            stationStack.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 14),
            stationStack.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -14),
            stationStack.bottomAnchor.constraint(equalTo: storyCard.bottomAnchor, constant: -8),
            stationStack.heightAnchor.constraint(equalToConstant: 21),
        ])
    }

    // MARK: - Metaverse Engine Catmull-Rom Spline Evaluation (via Coax!)

    private func evaluateMetaverseSpline(at tSec: Float) {
        var mm = [Int64](repeating: 0, count: 6)
        let tMs = Int64((tSec * 1000.0).rounded())
        let ok = DesktopCore.onCoreStack {
            mm.withUnsafeMutableBufferPointer { buf in
                oa_dc_rh3d_fly_shot(tMs, coax_v_s64(data: buf.baseAddress, count: 6))
            }
        }
        if ok != 0 {
            eye = SIMD3<Float>(Float(mm[0]) / 1000.0, Float(mm[1]) / 1000.0, Float(mm[2]) / 1000.0)
            target = SIMD3<Float>(Float(mm[3]) / 1000.0, Float(mm[4]) / 1000.0, Float(mm[5]) / 1000.0)
        }
        if eye.y < 20.0 && !stations.isEmpty {
            var best = 0
            var bestDist = Float.greatestFiniteMagnitude
            for (i, st) in stations.enumerated() {
                let d = abs(target.x - st.pos.x)
                if d < bestDist { bestDist = d; best = i }
            }
            if best != activeStation {
                activeStation = best
                selectedPick = nil
                syncStoryHUD()
            }
        }
    }

    private func syncStoryHUD() {
        if let pick = selectedPick {
            storyTitle.stringValue = pick.title.uppercased()
            storySubtitle.stringValue = "SELECTED LANDMARK · MODULE: \(pick.moduleName)"
            storyBody.stringValue = pick.detail
        } else if activeStation >= 0 && activeStation < stations.count {
            let st = stations[activeStation]
            storyTitle.stringValue = st.title.uppercased()
            storySubtitle.stringValue = st.subtitle
            storyBody.stringValue = st.plainEnglish
        }
        updateGridButtonTitle(playPauseBtn, flyPlaying ? "PAUSE" : "PLAY", accent: GridTokens.nsCyan)
        updateGridButtonTitle(labelsBtn, showLabels ? "LABELS: ON" : "LABELS: OFF", accent: nil)
        for (i, b) in stationButtons.enumerated() {
            let title = i < stations.count ? stations[i].shortName.uppercased() : "ST \(i+1)"
            let isAct = (i == activeStation)
            b.layer?.backgroundColor = isAct
                ? NSColor(calibratedRed: 0.04, green: 0.14, blue: 0.15, alpha: 0.90).cgColor
                : NSColor(calibratedRed: 0.018, green: 0.022, blue: 0.028, alpha: 0.85).cgColor
            updateGridButtonTitle(b, title, accent: isAct ? GridTokens.nsCyan : nil)
        }
        let frac = CGFloat(max(0.0, min(1.0, flyTimeS / max(1.0, durationS))))
        progressFillWidth?.constant = max(8, progressTrack.bounds.width * frac)
    }

    // MARK: - Actions & Controls

    @objc private func togglePlayPause() {
        flyPlaying.toggle()
        glideDuration = 0
        syncStoryHUD()
    }

    @objc private func goOverview() {
        flyPlaying = false
        selectedPick = nil
        startGlide(toEye: SIMD3(0, 32.0, 74.0), toTarget: SIMD3(0, 3.0, 0), dur: 0.95)
        storyTitle.stringValue = "FULL PROOF OVERVIEW — ALL 8 LANDMARKS (175,182 LEAN 4 FILES)"
        storySubtitle.stringValue = "Zeta Terrain → Robin 5040 → 2/3 Wall → Theta Bridge → 175k City → Behrend/Weil → 4 Kernels → Q.E.D."
        storyBody.stringValue = "Click any station tab below or any glowing Grid banner to swoop in closely, or click PLAY for the Metaverse Engine cinematic spline tour."
        updateGridButtonTitle(playPauseBtn, "PLAY", accent: GridTokens.nsCyan)
    }

    @objc private func toggleLabels() {
        showLabels.toggle()
        syncStoryHUD()
    }

    @objc private func openSourceClicked() {
        let mod = selectedPick?.moduleName ?? (activeStation < stations.count ? stations[activeStation].moduleName : "Main")
        onInspectModule?(mod)
    }

    @objc private func prevStationClicked() {
        let nxt = (activeStation - 1 + max(1, stations.count)) % max(1, stations.count)
        jumpToStation(nxt)
    }

    @objc private func nextStationClicked() {
        let nxt = (activeStation + 1) % max(1, stations.count)
        jumpToStation(nxt)
    }

    @objc private func stationPillClicked(_ sender: NSButton) {
        jumpToStation(sender.tag)
    }

    func jumpToStation(_ idx: Int) {
        guard idx >= 0 && idx < stations.count else { return }
        activeStation = idx
        selectedPick = nil
        flyPlaying = false
        let st = stations[idx]
        flyTimeS = st.timeS
        startGlide(toEye: st.eye, toTarget: st.pos, dur: 0.85)
        syncStoryHUD()
    }

    private func startGlide(toEye: SIMD3<Float>, toTarget: SIMD3<Float>, dur: Float) {
        glideFromEye = eye
        glideToEye = toEye
        glideFromTarget = target
        glideToTarget = toTarget
        glideElapsed = 0
        glideDuration = dur
    }

    // MARK: - Pointer & Keyboard Navigation

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        dragLast = convert(event.locationInWindow, from: nil)
        if event.modifierFlags.contains(.option) {
            isPanDragging = true
        } else {
            isOrbitDragging = true
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        dragLast = convert(event.locationInWindow, from: nil)
        isPanDragging = true
    }

    override func mouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        let dx = Float(pt.x - dragLast.x)
        let dy = Float(pt.y - dragLast.y)
        dragLast = pt
        if abs(dx) > 1.5 || abs(dy) > 1.5 {
            flyPlaying = false
            glideDuration = 0
        }
        if isPanDragging {
            panCamera(dx: dx, dy: dy)
        } else if isOrbitDragging {
            orbitCamera(dx: dx, dy: dy)
        }
    }

    override func rightMouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        let dx = Float(pt.x - dragLast.x)
        let dy = Float(pt.y - dragLast.y)
        dragLast = pt
        flyPlaying = false
        glideDuration = 0
        panCamera(dx: dx, dy: dy)
    }

    override func mouseUp(with event: NSEvent) {
        let wasDrag = !isOrbitDragging && !isPanDragging
        isOrbitDragging = false
        isPanDragging = false
        if !wasDrag && event.clickCount == 1 {
            let pt = convert(event.locationInWindow, from: nil)
            if let hit = raycastPick(at: pt) {
                selectedPick = hit
                activeStation = hit.stationIndex
                flyPlaying = false
                syncStoryHUD()
            }
        }
    }

    override func rightMouseUp(with event: NSEvent) {
        isPanDragging = false
    }

    override func scrollWheel(with event: NSEvent) {
        flyPlaying = false
        glideDuration = 0
        let diff = eye - target
        let dist = simd_length(diff)
        let scale = exp(Float(-event.scrollingDeltaY) * 0.012)
        let newDist = max(3.5, min(95.0, dist * scale))
        eye = target + simd_normalize(diff) * newDist
        syncStoryHUD()
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 49 { togglePlayPause(); return }
        if event.keyCode == 15 { goOverview(); return }
        pressedKeys.insert(event.keyCode)
        flyPlaying = false
        glideDuration = 0
    }

    override func keyUp(with event: NSEvent) {
        pressedKeys.remove(event.keyCode)
    }

    private func orbitCamera(dx: Float, dy: Float) {
        let offset = eye - target
        let r = max(2.0, simd_length(offset))
        let yaw = atan2(offset.x, offset.z) - dx * 0.0075
        var pitch = asin(max(-0.98, min(0.98, offset.y / r))) + dy * 0.0075
        pitch = max(-0.15, min(1.42, pitch))
        eye = target + SIMD3<Float>(
            r * cos(pitch) * sin(yaw),
            r * sin(pitch),
            r * cos(pitch) * cos(yaw)
        )
    }

    private func panCamera(dx: Float, dy: Float) {
        let f = simd_normalize(target - eye)
        let right = simd_normalize(simd_cross(f, SIMD3<Float>(0, 1, 0)))
        let up = simd_cross(right, f)
        let speed = max(0.015, simd_length(eye - target) * 0.0022)
        let delta = -right * (dx * speed) - up * (dy * speed)
        eye += delta
        target += delta
    }

    private func applyKeyboardFly(dt: Float) {
        guard !pressedKeys.isEmpty else { return }
        let f = simd_normalize(target - eye)
        let right = simd_normalize(simd_cross(f, SIMD3<Float>(0, 1, 0)))
        let up = SIMD3<Float>(0, 1, 0)
        var move = SIMD3<Float>.zero
        let speed: Float = 16.0 * dt
        if pressedKeys.contains(13) || pressedKeys.contains(126) { move += f * speed }
        if pressedKeys.contains(1)  || pressedKeys.contains(125) { move -= f * speed }
        if pressedKeys.contains(0)  || pressedKeys.contains(123) { move -= right * speed }
        if pressedKeys.contains(2)  || pressedKeys.contains(124) { move += right * speed }
        if pressedKeys.contains(14) { move += up * speed }
        if pressedKeys.contains(12) { move -= up * speed }
        eye += move
        target += move
    }

    private func raycastPick(at viewPt: CGPoint) -> Rh3dPickable? {
        let w = Float(max(1, bounds.width))
        let h = Float(max(1, bounds.height))
        let vp = rh3dPerspective(fovY: 0.82, aspect: w / h, nearZ: 0.2, farZ: 280.0) * rh3dLookAt(eye: eye, target: target)
        let yTop = h - Float(viewPt.y)
        var best: Rh3dPickable? = nil
        var bestDist: Float = 48.0
        for p in pickables {
            let clip = vp * SIMD4<Float>(p.center, 1.0)
            guard clip.w > 0.1 else { continue }
            let ndc = SIMD3<Float>(clip.x, clip.y, clip.z) / clip.w
            let sx = (ndc.x * 0.5 + 0.5) * w
            let sy = (0.5 - ndc.y * 0.5) * h
            let d = hypot(sx - Float(viewPt.x), sy - yTop)
            if d < bestDist { bestDist = d; best = p }
        }
        return best
    }

    // MARK: - 60 FPS Render Loop & Render Target Allocation

    private func startLoop() {
        frameTimer?.invalidate()
        lastTick = CACurrentMediaTime()
        let t = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            self?.tick()
        }
        RunLoop.main.add(t, forMode: .common)
        frameTimer = t
    }

    override func layout() {
        super.layout()
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        metalLayer?.contentsScale = scale
        metalLayer?.frame = metalHost.bounds
        metalLayer?.drawableSize = CGSize(width: max(1, metalHost.bounds.width * scale), height: max(1, metalHost.bounds.height * scale))
    }

    private func tick() {
        guard !isHiddenOrHasHiddenAncestor, window != nil else { return }
        let now = CACurrentMediaTime()
        let dt = Float(min(0.05, max(0.001, now - lastTick)))
        lastTick = now
        totalElapsedS += dt

        if glideDuration > 0 {
            glideElapsed += dt
            let raw = min(1.0, glideElapsed / glideDuration)
            let smooth = raw * raw * (3.0 - 2.0 * raw)
            eye = glideFromEye + (glideToEye - glideFromEye) * smooth
            target = glideFromTarget + (glideToTarget - glideFromTarget) * smooth
            if raw >= 1.0 { glideDuration = 0 }
        } else if flyPlaying {
            flyTimeS = (flyTimeS + dt * 0.95).truncatingRemainder(dividingBy: max(1.0, durationS))
            evaluateMetaverseSpline(at: flyTimeS)
            let frac = CGFloat(max(0.0, min(1.0, flyTimeS / max(1.0, durationS))))
            progressFillWidth?.constant = max(8, progressTrack.bounds.width * frac)
        } else {
            applyKeyboardFly(dt: dt)
        }

        renderMetalFrame()
        updateProjectedLabels()
    }

    private func ensureRenderTargets(width: Int, height: Int) -> Bool {
        guard let dev = device, width >= 2, height >= 2 else { return false }
        if rtSize == (width, height),
           hdrResolveTex != nil, msaaDepthTex != nil,
           bloomA1 != nil, bloomB1 != nil, bloomA2 != nil, bloomB2 != nil {
            return true
        }

        func makeTex(fmt: MTLPixelFormat, w: Int, h: Int, samples: Int, usage: MTLTextureUsage) -> MTLTexture? {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: fmt, width: max(1, w), height: max(1, h), mipmapped: false)
            d.textureType = samples > 1 ? .type2DMultisample : .type2D
            d.sampleCount = samples
            d.usage = usage
            d.storageMode = .private
            return dev.makeTexture(descriptor: d)
        }

        msaaColorTex = sampleCount > 1 ? makeTex(fmt: .rgba16Float, w: width, h: height, samples: sampleCount, usage: .renderTarget) : nil
        msaaDepthTex = makeTex(fmt: .depth32Float, w: width, h: height, samples: sampleCount, usage: .renderTarget)
        hdrResolveTex = makeTex(fmt: .rgba16Float, w: width, h: height, samples: 1, usage: [.renderTarget, .shaderRead])

        let hw = max(2, width / 2), hh = max(2, height / 2)
        let qw = max(2, width / 4), qh = max(2, height / 4)
        bloomA1 = makeTex(fmt: .rgba16Float, w: hw, h: hh, samples: 1, usage: [.renderTarget, .shaderRead])
        bloomB1 = makeTex(fmt: .rgba16Float, w: hw, h: hh, samples: 1, usage: [.renderTarget, .shaderRead])
        bloomA2 = makeTex(fmt: .rgba16Float, w: qw, h: qh, samples: 1, usage: [.renderTarget, .shaderRead])
        bloomB2 = makeTex(fmt: .rgba16Float, w: qw, h: qh, samples: 1, usage: [.renderTarget, .shaderRead])

        rtSize = (width, height)
        return hdrResolveTex != nil && msaaDepthTex != nil && bloomA1 != nil && bloomB1 != nil && bloomA2 != nil && bloomB2 != nil
    }

    private func buildUniforms(width: Int, height: Int, dpr: Float) -> Rh3dUniforms {
        let aspect = Float(max(1, width)) / Float(max(1, height))
        let vp = rh3dPerspective(fovY: 0.82, aspect: aspect, nearZ: 0.2, farZ: 280.0) * rh3dLookAt(eye: eye, target: target)
        let accentModes: [Float] = [0, 1, 0, 3, 0, 1, 0, 1]
        func stVec(_ idx: Int) -> SIMD4<Float> {
            let x = (idx < stations.count) ? stations[idx].pos.x : Float(-42 + idx * 12)
            let z = (idx < stations.count) ? stations[idx].pos.z : 0.0
            let act: Float = (idx == activeStation) ? 1.0 : 0.0
            return SIMD4(x, z, accentModes[idx % accentModes.count], act)
        }
        return Rh3dUniforms(
            viewProj: vp,
            invViewProj: simd_inverse(vp),
            cameraPos: SIMD4(eye, 1.0),
            viewport: SIMD4(Float(width), Float(height), dpr, totalElapsedS),
            st0: stVec(0), st1: stVec(1), st2: stVec(2), st3: stVec(3),
            st4: stVec(4), st5: stVec(5), st6: stVec(6), st7: stVec(7)
        )
    }

    private func encodeFullGridFrame(cmd: MTLCommandBuffer, destTexture: MTLTexture, width: Int, height: Int, dpr: Float) {
        guard ensureRenderTargets(width: width, height: height),
              let hdrTex = hdrResolveTex,
              let depthTex = msaaDepthTex,
              let bA1 = bloomA1, let bB1 = bloomB1,
              let bA2 = bloomA2, let bB2 = bloomB2,
              let skyP = skyPipe, let flP = floorPipe,
              let opP = opaqueMeshPipe, let trP = translucentMeshPipe,
              let exP = bloomExtractPipe, let blP = bloomBlurPipe, let cpP = compositePipe,
              let dNone = depthNone, let dOp = depthOpaque, let dRO = depthReadOnly else { return }

        var uniforms = buildUniforms(width: width, height: height, dpr: dpr)

        // ---- PASS 1: HDR 4x MSAA Scene Pass (Sky -> Infinite Analytical Grid -> Zeta LiDAR -> Obsidian Monoliths -> Glass -> Vector Lines) ----
        let sceneRP = MTLRenderPassDescriptor()
        if sampleCount > 1, let msaaCol = msaaColorTex {
            sceneRP.colorAttachments[0].texture = msaaCol
            sceneRP.colorAttachments[0].resolveTexture = hdrTex
            sceneRP.colorAttachments[0].storeAction = .multisampleResolve
        } else {
            sceneRP.colorAttachments[0].texture = hdrTex
            sceneRP.colorAttachments[0].storeAction = .store
        }
        sceneRP.colorAttachments[0].loadAction = .clear
        sceneRP.colorAttachments[0].clearColor = MTLClearColor(red: 0.002, green: 0.0025, blue: 0.003, alpha: 1.0)
        sceneRP.depthAttachment.texture = depthTex
        sceneRP.depthAttachment.loadAction = .clear
        sceneRP.depthAttachment.storeAction = .dontCare
        sceneRP.depthAttachment.clearDepth = 1.0

        if let enc = cmd.makeRenderCommandEncoder(descriptor: sceneRP) {
            // 1. Pitch-black obsidian void sky with subtle newland grey nebula & horizon mist
            enc.setRenderPipelineState(skyP)
            enc.setDepthStencilState(dNone)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 0)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)

            // 2. Infinite analytical fwidth-antialiased OpenAgents Grid Floor
            enc.setRenderPipelineState(flP)
            enc.setDepthStencilState(dOp)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 0)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 0)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)

            // 3. Station 1 Holographic LiDAR Riemann Zeta Terrain
            if let tPipe = terrainPipe, let tb = terrainVB, terrainVCount > 0 {
                enc.setRenderPipelineState(tPipe)
                enc.setDepthStencilState(dOp)
                enc.setVertexBuffer(tb, offset: 0, index: 0)
                enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
                enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
                enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: terrainVCount)
            }

            func drawBatch(_ ib: MTLBuffer?, count: Int, vb: MTLBuffer?, vCount: Int) {
                guard count > 0, let ib = ib, let vb = vb, vCount > 0 else { return }
                enc.setVertexBuffer(vb, offset: 0, index: 0)
                enc.setVertexBuffer(ib, offset: 0, index: 1)
                enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 2)
                enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 2)
                enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: vCount, instanceCount: count)
            }

            // 4. Opaque Obsidian Architecture + Procedural Crisp Wireframe Edges
            enc.setRenderPipelineState(opP)
            enc.setDepthStencilState(dOp)
            drawBatch(opaqueBoxIB, count: opaqueBoxes.count, vb: boxVB, vCount: boxVCount)
            drawBatch(opaqueCylIB, count: opaqueCyls.count, vb: cylVB, vCount: cylVCount)
            drawBatch(opaqueOctIB, count: opaqueOcts.count, vb: octVB, vCount: octVCount)

            // 5. Translucent Laser-Etched Glass Planes (Critical Wall, Robin 1.0 Ceiling, 2/3 Barrier)
            enc.setRenderPipelineState(trP)
            enc.setDepthStencilState(dRO)
            drawBatch(transBoxIB, count: transBoxes.count, vb: boxVB, vCount: boxVCount)
            drawBatch(transCylIB, count: transCyls.count, vb: cylVB, vCount: cylVCount)

            // 6. Screen-Space Expanded Glowing Vector Lines, Open Pavilions & Animated Proof Pulses
            if let qlP = qlinePipe, let qlB = quadLinesVB, quadLineSegCount > 0 {
                enc.setRenderPipelineState(qlP)
                enc.setDepthStencilState(dRO)
                enc.setVertexBuffer(qlB, offset: 0, index: 0)
                enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
                enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
                enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: quadLineSegCount * 6)
            }
            enc.endEncoding()
        }

        // Helper for 2D post passes
        func runPost(pipe: MTLRenderPipelineState, dst: MTLTexture, tex0: MTLTexture, tex1: MTLTexture? = nil, tex2: MTLTexture? = nil, postU: inout Rh3dPostUniforms) {
            let rp = MTLRenderPassDescriptor()
            rp.colorAttachments[0].texture = dst
            rp.colorAttachments[0].loadAction = .dontCare
            rp.colorAttachments[0].storeAction = .store
            if let enc = cmd.makeRenderCommandEncoder(descriptor: rp) {
                enc.setRenderPipelineState(pipe)
                enc.setFragmentTexture(tex0, index: 0)
                if let t1 = tex1 { enc.setFragmentTexture(t1, index: 1) }
                if let t2 = tex2 { enc.setFragmentTexture(t2, index: 2) }
                enc.setFragmentBytes(&postU, length: MemoryLayout<Rh3dPostUniforms>.stride, index: 0)
                enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
                enc.endEncoding()
            }
        }

        // ---- PASS 2: Soft-Knee HDR Bright-Pass Extract (Full -> Half) ----
        var pu = Rh3dPostUniforms(
            texelDir: SIMD4(1.0 / Float(width), 1.0 / Float(height), 0, 0),
            params: SIMD4(0.58, 0.35, 0.34, 0.24)
        )
        runPost(pipe: exP, dst: bA1, tex0: hdrTex, postU: &pu)

        // ---- PASS 3A: Half-Res Separable Gaussian Blur ----
        pu.texelDir = SIMD4(1.0 / Float(bA1.width), 1.0 / Float(bA1.height), 1.0, 0.0)
        runPost(pipe: blP, dst: bB1, tex0: bA1, postU: &pu)
        pu.texelDir = SIMD4(1.0 / Float(bA1.width), 1.0 / Float(bA1.height), 0.0, 1.0)
        runPost(pipe: blP, dst: bA1, tex0: bB1, postU: &pu)

        // ---- PASS 3B: Quarter-Res Wide Halo Gaussian Blur ----
        pu.texelDir = SIMD4(1.0 / Float(bA2.width), 1.0 / Float(bA2.height), 1.0, 0.0)
        runPost(pipe: blP, dst: bB2, tex0: bA1, postU: &pu)
        pu.texelDir = SIMD4(1.0 / Float(bA2.width), 1.0 / Float(bA2.height), 0.0, 1.0)
        runPost(pipe: blP, dst: bA2, tex0: bB2, postU: &pu)

        // ---- PASS 4: Composite HDR + Bloom + metaverse_tone (1 - exp(-L * 1.35)) + sRGB -> Display ----
        pu.params = SIMD4(0, 0, 0.36, 0.26)
        runPost(pipe: cpP, dst: destTexture, tex0: hdrTex, tex1: bA1, tex2: bA2, postU: &pu)
    }

    private func renderMetalFrame() {
        guard let ml = metalLayer,
              let queue = commandQueue,
              ml.drawableSize.width >= 2, ml.drawableSize.height >= 2,
              let drawable = ml.nextDrawable(),
              let cmd = queue.makeCommandBuffer() else { return }

        let dpr = Float(ml.contentsScale)
        encodeFullGridFrame(cmd: cmd, destTexture: drawable.texture, width: drawable.texture.width, height: drawable.texture.height, dpr: dpr)
        cmd.present(drawable)
        cmd.commit()
    }

    // MARK: - 3D-to-2D Projected Callout Labels

    private func computeProjectedLabels(viewSize: CGSize) -> [ProjectedLabel] {
        guard showLabels, viewSize.width > 10, viewSize.height > 10 else { return [] }
        let w = Float(viewSize.width)
        let h = Float(viewSize.height)
        let vp = rh3dPerspective(fovY: 0.82, aspect: w / h, nearZ: 0.2, farZ: 280.0) * rh3dLookAt(eye: eye, target: target)

        let titleBannerFont = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .semibold)
        let titlePartFont = NSFont.monospacedSystemFont(ofSize: 9.8, weight: .medium)
        let subFont = NSFont.monospacedSystemFont(ofSize: 8.8, weight: .regular)

        var candidates: [(dist: Float, priority: Int, label: ProjectedLabel)] = []
        for anc in labelAnchors {
            let dist = simd_length(eye - anc.worldPos)
            if dist > anc.maxDist { continue }
            let clip = vp * SIMD4<Float>(anc.worldPos, 1.0)
            guard clip.w > 0.35 else { continue }
            let ndc = SIMD3<Float>(clip.x, clip.y, clip.z) / clip.w
            if ndc.x < -0.96 || ndc.x > 0.96 || ndc.y < -0.78 || ndc.y > 0.97 { continue }

            let sx = CGFloat((ndc.x * 0.5 + 0.5) * w)
            let sy = CGFloat((0.5 - ndc.y * 0.5) * h)
            let minSY: CGFloat = anc.isStationBanner ? 18 : 50
            if sy < minSY || sy > viewSize.height - 156 { continue }

            let tw = (anc.text as NSString).size(withAttributes: [.font: anc.isStationBanner ? titleBannerFont : titlePartFont, .kern: anc.isStationBanner ? 0.7 : 0.3]).width
            let sw = anc.subtext.isEmpty ? 0 : (anc.subtext as NSString).size(withAttributes: [.font: subFont]).width
            let pw = max(tw, sw) + 20
            let ph: CGFloat = anc.subtext.isEmpty ? 22 : 35
            let stemH: CGFloat = anc.isStationBanner ? 22 : 17
            let rx = max(12, min(viewSize.width - pw - 12, sx - pw * 0.5))
            let ry = max(46, sy - stemH - ph)
            let alpha: CGFloat = anc.isStationBanner
                ? CGFloat(max(0.58, min(1.0, 1.15 - dist / 130.0)))
                : CGFloat(max(0.0, min(0.96, (anc.maxDist - dist) / 7.5)))
            guard alpha > 0.08 else { continue }

            let prio = anc.isStationBanner ? 0 : (anc.stationIndex == activeStation ? 1 : 2)
            candidates.append((dist, prio, ProjectedLabel(
                rect: CGRect(x: rx, y: ry, width: pw, height: ph),
                anchorPt: CGPoint(x: sx, y: sy),
                title: anc.text,
                subtext: anc.subtext,
                color: anc.color,
                isBanner: anc.isStationBanner,
                alpha: alpha,
                stationIndex: anc.stationIndex
            )))
        }

        candidates.sort { a, b in
            if a.priority != b.priority { return a.priority < b.priority }
            return a.dist < b.dist
        }

        var placed: [ProjectedLabel] = []
        for c in candidates {
            var lbl = c.label
            if lbl.isBanner {
                var r = lbl.rect
                var tries = 0
                while tries < 3 && placed.contains(where: { $0.rect.intersects(r.insetBy(dx: -5, dy: -4)) }) {
                    r.origin.y -= (r.height + 8)
                    tries += 1
                }
                if r.origin.y < 42 || placed.contains(where: { $0.rect.intersects(r.insetBy(dx: -5, dy: -4)) }) {
                    continue
                }
                lbl = ProjectedLabel(
                    rect: r, anchorPt: lbl.anchorPt, title: lbl.title, subtext: lbl.subtext,
                    color: lbl.color, isBanner: lbl.isBanner, alpha: lbl.alpha, stationIndex: lbl.stationIndex
                )
            } else {
                let padded = lbl.rect.insetBy(dx: -5, dy: -4)
                if placed.contains(where: { $0.rect.intersects(padded) }) { continue }
            }
            placed.append(lbl)
            if placed.count >= 16 { break }
        }
        return placed
    }

    private func updateProjectedLabels() {
        labelsOverlay.labels = computeProjectedLabels(viewSize: bounds.size)
        labelsOverlay.needsDisplay = true
    }

    // MARK: - Headless Offscreen Metal PNG Capture (Full HDR 4x MSAA + Bloom + metaverse_tone Pipeline!)

    func renderOffscreenPNG(to url: URL, width: Int = 1440, height: Int = 900, timeSeconds: Float? = nil, overview: Bool = false) -> Bool {
        guard let dev = device, let queue = commandQueue else { return false }
        if overview {
            eye = SIMD3<Float>(0, 32.0, 74.0)
            target = SIMD3<Float>(0, 3.0, 0)
            activeStation = 0
        } else if let t = timeSeconds {
            flyTimeS = t
            totalElapsedS = t
            evaluateMetaverseSpline(at: t)
        }

        let texDesc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        texDesc.usage = [.renderTarget, .shaderRead]
        texDesc.storageMode = .shared
        guard let colorTex = dev.makeTexture(descriptor: texDesc),
              let cmd = queue.makeCommandBuffer() else { return false }

        encodeFullGridFrame(cmd: cmd, destTexture: colorTex, width: width, height: height, dpr: 1.25)
        cmd.commit()
        cmd.waitUntilCompleted()

        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        colorTex.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)

        let cs = CGColorSpaceCreateDeviceRGB()
        let bi = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: cs, bitmapInfo: bi.rawValue) else { return false }

        NSGraphicsContext.saveGraphicsState()
        ctx.saveGState()
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: 1, y: -1)
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)

        let viewSize = CGSize(width: width, height: height)
        let proj = computeProjectedLabels(viewSize: viewSize)
        RhLabelsOverlayView.drawLabels(proj, in: ctx)

        // Draw authentic OpenAgents Grid Chamfered Story HUD Frame at bottom
        let cardRect = CGRect(x: 20, y: CGFloat(height) - 130, width: CGFloat(width) - 40, height: 112)
        drawGridHudFrame(in: ctx, rect: cardRect, cut: 12.0, fillAlpha: 0.90, drawBrackets: true, drawScanlines: true)

        ("OPENAGENTS / VERSE  ·  THE GRID  ·  RIEMANN HYPOTHESIS" as NSString).draw(at: CGPoint(x: cardRect.minX + 18, y: cardRect.minY + 10), withAttributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 9.5, weight: .medium),
            .foregroundColor: GridTokens.nsMid,
            .kern: 1.1,
        ])

        let titleStr = (overview ? "FULL PROOF OVERVIEW — ALL 8 LANDMARKS (175,182 LEAN 4 FILES)" : (activeStation < stations.count ? stations[activeStation].title : "")).uppercased()
        let subStr = overview ? "Zeta Terrain → Robin 5040 → 2/3 Wall → Theta Bridge → 175k City → Behrend/Weil → 4 Kernels → Summit Q.E.D." : (activeStation < stations.count ? stations[activeStation].subtitle : "")
        let bodyStr = overview ? "OpenAgents Grid renderer (4× MSAA HDR + Bloom + metaverse_tone) · Catmull-Rom spline (metaverse-engine/cam/flyover.coax) + real Coax math + rh-atlas.json." : (activeStation < stations.count ? stations[activeStation].plainEnglish : "")

        (titleStr as NSString).draw(at: CGPoint(x: cardRect.minX + 18, y: cardRect.minY + 25), withAttributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 14, weight: .bold),
            .foregroundColor: GridTokens.nsWhite,
            .kern: 0.6,
        ])
        (subStr as NSString).draw(at: CGPoint(x: cardRect.minX + 18, y: cardRect.minY + 46), withAttributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium),
            .foregroundColor: GridTokens.nsCyan,
        ])
        (bodyStr as NSString).draw(in: CGRect(x: cardRect.minX + 18, y: cardRect.minY + 64, width: cardRect.width - 36, height: 38), withAttributes: [
            .font: NSFont.systemFont(ofSize: 12, weight: .regular),
            .foregroundColor: GridTokens.nsLight,
        ])

        ctx.restoreGState()
        NSGraphicsContext.restoreGraphicsState()

        guard let cgImg = ctx.makeImage(),
              let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return false }
        CGImageDestinationAddImage(dest, cgImg, nil)
        return CGImageDestinationFinalize(dest)
    }
}
