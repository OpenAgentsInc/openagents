// Firm reason: 6 (AppKit/Metal 3D Metaverse Grid Flythrough viewport over CAMetalLayer driven by Metaverse Engine Coax flyover spline + rh-math.coax).
import AppKit
import AVFoundation
import CoreMedia
import CoreVideo
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
    let kicker: String
    let title: String
    let caption: String
    let detail: String
    let source: String
    let moduleName: String
    let accent: SIMD4<Float>
}

private struct Rh3dGuideContent {
    let stepTag: String
    let kicker: String
    let kickerColor: NSColor
    let title: String
    let caption: String
    let detail: String
    let source: String
    let isCompactWide: Bool
}

// Everything oa_dc_rh3d_scene_json returns besides the stations.
private struct Rh3dScene {
    var zeros: [Float] = []            // imaginary parts of the zeros of zeta, computed in Coax
    var t0: Float = 8, t1: Float = 52  // the range of t the zeta floor covers
    var heightClip: Float = 3
    var zNs = 0, zNt = 0
    var zHeights: [Float] = []         // |zeta(sigma + it)|, row j = t, column i = sigma in [0, 1]
    var psiX0: Float = 2, psiX1: Float = 60
    var psiSteps: [(x: Float, v: Float)] = []   // psi(x) right after each of its jumps
    var robin: [(n: Int, ratio: Float)] = []
    var libs: [(name: String, files: Int, lines: Int)] = []
    var checkers: [(name: String, tag: String)] = []
    var theoremNames: [String] = []
}

private struct Rh3dLabelAnchor {
    let worldPos: SIMD3<Float>
    var text: String
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
    let significance: String
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
    // One mesh line per 0.25 in sigma and per 4 in t.
    float2 meshCoord = in.uv * float2(4.0, 11.0);
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

    // ---- Mode 4: Translucent Laser-Etched Glass Plane (critical line, Robin ceiling, the strip of station 5) ----
    if (mode > 3.5) {
        float2 dEdge = min(in.uv, 1.0 - in.uv) / max(fwidth(in.uv), float2(1e-4));
        float frame = 1.0 - smoothstep(edgeW * 0.55, edgeW * 0.55 + 1.1, min(dEdge.x, dEdge.y));

        // Crisp world-space laser grid etched onto glass plane
        float2 planeCoord = (abs(N.y) > 0.5) ? in.worldPos.xz : ((abs(N.x) > 0.5) ? in.worldPos.zy : in.worldPos.xy);
        float laserGrid = analytical_grid_line(planeCoord * 1.0, fwidth(planeCoord * 1.0), 1.05);

        // Subtle animated scan sweep across the plane
        float scan = 0.0;   // a moving highlight here read as data on the charts, so the planes stay even

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

// MARK: - Friendly Explainer & Significance Guide Card (Shared by Live View & Offscreen PNGs)

private final class GridExplainerPanelView: NSView {
    var content: Rh3dGuideContent? {
        didSet { needsDisplay = true }
    }

    override var isFlipped: Bool { true }
    override func mouseDown(with event: NSEvent) {}
    override func mouseDragged(with event: NSEvent) {}
    override func scrollWheel(with event: NSEvent) {}

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext, let c = content else { return }
        ctx.setShouldAntialias(true)
        Self.drawExplainer(c, in: ctx, rect: bounds.insetBy(dx: 0.5, dy: 0.5))
    }

    private static func textHeight(_ str: String, width: CGFloat, attrs: [NSAttributedString.Key: Any]) -> CGFloat {
        guard !str.isEmpty, width > 10 else { return 0 }
        let storage = NSTextStorage(attributedString: NSAttributedString(string: str, attributes: attrs))
        let container = NSTextContainer(size: CGSize(width: width, height: 4000))
        container.lineFragmentPadding = 0
        let lm = NSLayoutManager()
        lm.addTextContainer(container)
        storage.addLayoutManager(lm)
        _ = lm.glyphRange(for: container)
        return ceil(lm.usedRect(for: container).height) + 2
    }

    private static func drawText(_ str: String, in rect: CGRect, attrs: [NSAttributedString.Key: Any]) {
        guard !str.isEmpty, rect.width > 10, rect.height > 4 else { return }
        let storage = NSTextStorage(attributedString: NSAttributedString(string: str, attributes: attrs))
        let container = NSTextContainer(size: CGSize(width: rect.width, height: max(rect.height, 4000)))
        container.lineFragmentPadding = 0
        let lm = NSLayoutManager()
        lm.addTextContainer(container)
        storage.addLayoutManager(lm)
        let glyphRange = lm.glyphRange(for: container)
        lm.drawGlyphs(forGlyphRange: glyphRange, at: rect.origin)
    }

    private static func bodyPara(_ lineSpacing: CGFloat = 2.0) -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.lineSpacing = lineSpacing
        p.lineBreakMode = .byWordWrapping
        return p
    }

    static func drawExplainer(_ c: Rh3dGuideContent, in ctx: CGContext, rect: CGRect) {
        drawGridHudFrame(in: ctx, rect: rect, cut: 12.0, fillAlpha: 0.90, drawBrackets: true, drawScanlines: false)

        let padX: CGFloat = 18
        let innerW = rect.width - padX * 2
        let x0 = rect.minX + padX
        var y = rect.minY + 14

        let stepAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 10.0, weight: .semibold),
            .foregroundColor: GridTokens.nsMid,
            .kern: 1.0,
        ]
        let kickerAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 10.0, weight: .bold),
            .foregroundColor: c.kickerColor,
            .kern: 0.8,
        ]
        let titleAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: c.isCompactWide ? 19.0 : 18.0, weight: .bold),
            .foregroundColor: GridTokens.nsWhite,
            .paragraphStyle: bodyPara(1.5),
        ]
        let captionAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 14.5, weight: .medium),
            .foregroundColor: GridTokens.nsWhite,
            .paragraphStyle: bodyPara(3.0),
        ]
        let detailAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12.5, weight: .regular),
            .foregroundColor: GridTokens.nsLight,
            .paragraphStyle: bodyPara(3.0),
        ]
        let sourceAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11.0, weight: .regular),
            .foregroundColor: GridTokens.nsMid,
            .paragraphStyle: bodyPara(2.0),
        ]

        (c.stepTag as NSString).draw(at: CGPoint(x: x0, y: y), withAttributes: stepAttrs)
        if !c.kicker.isEmpty {
            let kw = (c.kicker as NSString).size(withAttributes: kickerAttrs).width
            (c.kicker as NSString).draw(at: CGPoint(x: rect.maxX - padX - kw, y: y), withAttributes: kickerAttrs)
        }
        y += 22

        let th = textHeight(c.title, width: innerW, attrs: titleAttrs)
        drawText(c.title, in: CGRect(x: x0, y: y, width: innerW, height: th), attrs: titleAttrs)
        y += th + 8

        ctx.setFillColor(c.kickerColor.withAlphaComponent(0.9).cgColor)
        ctx.fill(CGRect(x: x0, y: y, width: 34, height: 2))
        y += 12

        let ch = textHeight(c.caption, width: innerW, attrs: captionAttrs)
        drawText(c.caption, in: CGRect(x: x0, y: y, width: innerW, height: ch), attrs: captionAttrs)
        y += ch + 12

        if c.isCompactWide { return }

        if !c.detail.isEmpty {
            let dh = textHeight(c.detail, width: innerW, attrs: detailAttrs)
            if y + dh < rect.maxY - 12 {
                drawText(c.detail, in: CGRect(x: x0, y: y, width: innerW, height: dh), attrs: detailAttrs)
                y += dh + 12
            }
        }
        if !c.source.isEmpty {
            let sh = textHeight(c.source, width: innerW, attrs: sourceAttrs)
            if y + sh + 26 < rect.maxY {
                ctx.setFillColor(NSColor(white: 0.94, alpha: 0.12).cgColor)
                ctx.fill(CGRect(x: x0, y: y, width: innerW, height: 1))
                y += 9
                ("WHERE THIS COMES FROM" as NSString).draw(at: CGPoint(x: x0, y: y), withAttributes: stepAttrs)
                y += 16
                drawText(c.source, in: CGRect(x: x0, y: y, width: innerW, height: sh), attrs: sourceAttrs)
            }
        }
    }

    // The video's caption: one short statement in large type across the bottom of the frame,
    // with a progress strip of one segment per station. `size` is in the caller's drawing units.
    static let videoCaptionHeight: CGFloat = 178
    static func drawVideoCaption(step: String, kicker: String, title: String, caption: String, accent: NSColor, segments: Int, progress: CGFloat, in ctx: CGContext, size: CGSize) {
        let rect = CGRect(x: 36, y: size.height - videoCaptionHeight - 26, width: size.width - 72, height: videoCaptionHeight)
        drawGridHudFrame(in: ctx, rect: rect, cut: 14.0, fillAlpha: 0.93, drawBrackets: true, drawScanlines: false)
        let x0 = rect.minX + 28
        let innerW = rect.width - 56
        var y = rect.minY + 16

        let stepAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 13.0, weight: .bold),
            .foregroundColor: accent,
            .kern: 1.4,
        ]
        let kickerAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 13.0, weight: .semibold),
            .foregroundColor: GridTokens.nsMid,
            .kern: 1.4,
        ]
        let titleAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 27.0, weight: .bold),
            .foregroundColor: GridTokens.nsWhite,
        ]
        let captionAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 20.0, weight: .regular),
            .foregroundColor: NSColor(calibratedWhite: 0.90, alpha: 1.0),
            .paragraphStyle: bodyPara(4.0),
        ]
        (step as NSString).draw(at: CGPoint(x: x0, y: y), withAttributes: stepAttrs)
        if !kicker.isEmpty {
            let sw = (step as NSString).size(withAttributes: stepAttrs).width
            (("·  " + kicker) as NSString).draw(at: CGPoint(x: x0 + sw + 12, y: y), withAttributes: kickerAttrs)
        }
        y += 24
        (title as NSString).draw(at: CGPoint(x: x0, y: y), withAttributes: titleAttrs)
        y += 40
        let ch = textHeight(caption, width: innerW, attrs: captionAttrs)
        drawText(caption, in: CGRect(x: x0, y: y, width: innerW, height: ch), attrs: captionAttrs)

        // Progress: one segment per station, filled as the video advances.
        let n = max(1, segments)
        let gap: CGFloat = 6
        let segW = (innerW - gap * CGFloat(n - 1)) / CGFloat(n)
        let py = rect.maxY - 16
        for i in 0..<n {
            let r = CGRect(x: x0 + CGFloat(i) * (segW + gap), y: py, width: segW, height: 4)
            ctx.setFillColor(NSColor(white: 0.94, alpha: 0.16).cgColor)
            ctx.fill(r)
            let f = max(0.0, min(1.0, progress * CGFloat(n) - CGFloat(i)))
            if f > 0 {
                ctx.setFillColor(accent.cgColor)
                ctx.fill(CGRect(x: r.minX, y: r.minY, width: r.width * f, height: 4))
            }
        }
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
    // 1 in the app; the video recorder raises it so labels stay readable at 1080p.
    static var scale: CGFloat = 1.0
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
        let s = scale
        let titleBannerFont = NSFont.monospacedSystemFont(ofSize: 10.5 * s, weight: .semibold)
        let titlePartFont = NSFont.monospacedSystemFont(ofSize: 9.8 * s, weight: .medium)
        let subFont = NSFont.monospacedSystemFont(ofSize: 8.8 * s, weight: .regular)

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
            if !lbl.subtext.isEmpty {
            let ruleY = lbl.rect.minY + 17.5 * s
            ctx.setFillColor(NSColor(white: 0.94, alpha: 0.16).cgColor)
            ctx.fill(CGRect(x: lbl.rect.minX + 8, y: ruleY, width: max(12, lbl.rect.width - 16), height: 1.0))
            ctx.setFillColor(lbl.color.withAlphaComponent(0.92).cgColor)
            ctx.fill(CGRect(x: lbl.rect.minX + 8, y: ruleY, width: min(26, max(12, lbl.rect.width * 0.36)), height: 1.5))
            }

            let tAttrs: [NSAttributedString.Key: Any] = [
                .font: lbl.isBanner ? titleBannerFont : titlePartFont,
                .foregroundColor: GridTokens.nsWhite,
                .kern: lbl.isBanner ? 0.7 : 0.3,
            ]
            (lbl.title as NSString).draw(at: CGPoint(x: lbl.rect.minX + 8, y: lbl.rect.minY + 3.5 * s), withAttributes: tAttrs)

            if !lbl.subtext.isEmpty {
                let sAttrs: [NSAttributedString.Key: Any] = [
                    .font: subFont,
                    .foregroundColor: GridTokens.nsLight.withAlphaComponent(0.82),
                ]
                (lbl.subtext as NSString).draw(at: CGPoint(x: lbl.rect.minX + 8, y: lbl.rect.minY + 20.5 * s), withAttributes: sAttrs)
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
    private var lineSegs: [Rh3dQuadLineSeg] = []
    private var overviewTitle = ""
    private var overviewCaption = ""
    private var closingCaption = ""
    private var sceneFiles = 0
    private var sceneLines = 0
    private var sceneTheorems = 0

    // Station 2's animated curve: psiCurves holds psiRows rows of psiSamples values; row k is
    // the explicit formula with the first k zeros. The curve on screen is rebuilt when k changes.
    private let psiSamples = 175
    private var psiRows = 0
    private var psiCurves: [Float] = []
    private var psiShown = -1
    private var psiX0: Float = 2
    private var psiX1: Float = 60
    private var psiLabelIndex = -1
    private var dynLinesVB: MTLBuffer?
    private var dynLineSegCount = 0
    private var videoCard = 0   // 0 = a station's caption, 1 = the opening card, 2 = the closing card

    // Camera state (Y-up right-handed matching metaverse-engine/cam/flyover.coax)
    private var eye = SIMD3<Float>(-49, 8.5, 15)
    private var target = SIMD3<Float>(-42, 2.6, 0)
    private var flyPlaying = true
    private var flyTimeS: Float = 0.0
    private var showLabels = true
    private var showGuide = true
    private var isOverviewMode = false
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
    private let guideBtn = NSButton(title: "GUIDE: ON", target: nil, action: nil)
    private let labelsBtn = NSButton(title: "LABELS: ON", target: nil, action: nil)
    private let inspectBtn = NSButton(title: "SOURCE →", target: nil, action: nil)

    private let explainerPanel = GridExplainerPanelView()
    private var explainerRightConstraints: [NSLayoutConstraint] = []
    private var explainerOverviewConstraints: [NSLayoutConstraint] = []

    private let storyCard = GridHudFrameView()
    private let storyKicker = NSTextField(labelWithString: "THE RIEMANN HYPOTHESIS")
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
        let jsonStr = DesktopCore.callString(262144) { oa_dc_rh3d_scene_json($0) }
        guard let data = jsonStr.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        if let durMs = obj["duration_ms"] as? Int {
            durationS = Float(durMs) / 1000.0
        }
        overviewTitle = (obj["overview_title"] as? String) ?? ""
        overviewCaption = (obj["overview_caption"] as? String) ?? ""
        closingCaption = (obj["closing_caption"] as? String) ?? ""
        sceneFiles = (obj["files"] as? Int) ?? 0
        sceneLines = (obj["lines"] as? Int) ?? 0
        sceneTheorems = (obj["theorems"] as? Int) ?? 0

        // One accent per station: cyan for computed mathematics, amber for the historical claims,
        // white for the part nobody has explained yet.
        let accents: [SIMD4<Float>] = [
            GridTokens.cyan, GridTokens.cyan, GridTokens.amber, GridTokens.greyWhite,
            GridTokens.amber, GridTokens.cyan, GridTokens.cyan, GridTokens.amberHot,
        ]

        func mm(_ s: [String: Any], _ k: String) -> Float { Float((s[k] as? Int) ?? 0) / 1000.0 }
        stations.removeAll()
        if let stArr = obj["stations"] as? [[String: Any]] {
            for (i, s) in stArr.enumerated() {
                stations.append(Rh3dStationData(
                    index: i,
                    pos: SIMD3(mm(s, "x_mm"), mm(s, "y_mm"), mm(s, "z_mm")),
                    eye: SIMD3(mm(s, "eye_x_mm"), mm(s, "eye_y_mm"), mm(s, "eye_z_mm")),
                    timeS: mm(s, "time_ms"),
                    shortName: (s["short_name"] as? String) ?? "\(i + 1)",
                    kicker: (s["kicker"] as? String) ?? "",
                    title: (s["title"] as? String) ?? "",
                    caption: (s["caption"] as? String) ?? "",
                    detail: (s["detail"] as? String) ?? "",
                    source: (s["source"] as? String) ?? "",
                    moduleName: (s["module"] as? String) ?? "Main",
                    accent: accents[i % accents.count]
                ))
            }
        }
        guard stations.count == 8 else { return }

        var sc = Rh3dScene()
        let zerosU6 = (obj["zeros_u6"] as? [Int]) ?? []
        sc.zeros = zerosU6.map { Float(Double($0) / 1_000_000.0) }
        sc.t0 = mm(obj, "t0_mm")
        sc.t1 = mm(obj, "t1_mm")
        sc.heightClip = mm(obj, "height_clip_mm")
        sc.zNs = (obj["zeta_ns"] as? Int) ?? 0
        sc.zNt = (obj["zeta_nt"] as? Int) ?? 0
        sc.zHeights = ((obj["zeta_mm"] as? [Int]) ?? []).map { Float($0) / 1000.0 }
        sc.psiX0 = mm(obj, "psi_x0_mm")
        sc.psiX1 = mm(obj, "psi_x1_mm")
        for st in (obj["psi_steps"] as? [[Int]]) ?? [] where st.count == 2 {
            sc.psiSteps.append((Float(st[0]), Float(st[1]) / 1000.0))
        }
        for r in (obj["robin"] as? [[String: Any]]) ?? [] {
            sc.robin.append(((r["n"] as? Int) ?? 0, Float(Double((r["ratio_u6"] as? Int) ?? 0) / 1_000_000.0)))
        }
        for l in (obj["libs"] as? [[String: Any]]) ?? [] {
            sc.libs.append(((l["name"] as? String) ?? "", (l["files"] as? Int) ?? 0, (l["lines"] as? Int) ?? 0))
        }
        for c in (obj["checkers"] as? [[String: Any]]) ?? [] {
            sc.checkers.append(((c["name"] as? String) ?? "", (c["tag"] as? String) ?? ""))
        }
        sc.theoremNames = (obj["theorem_names"] as? [String]) ?? []
        psiX0 = sc.psiX0
        psiX1 = sc.psiX1

        loadPsiCurves(zerosU6: zerosU6)
        buildWorldGeometry(sc)
        uploadInstanceBuffers()
        refreshPsiCurve(zerosShown: psiRows - 1)
        evaluateMetaverseSpline(at: 0.0)
    }

    // Station 2's curves: row k is the explicit formula with the first k zeros (computed in Coax).
    private func loadPsiCurves(zerosU6: [Int]) {
        var zs = zerosU6.map { Int64($0) }
        let rows = zs.count + 1
        let samples = psiSamples
        var out = [Int64](repeating: 0, count: rows * samples)
        let got: Int64 = DesktopCore.onCoreStack {
            zs.withUnsafeMutableBufferPointer { zb in
                out.withUnsafeMutableBufferPointer { ob in
                    oa_dc_rh3d_psi_curves(
                        coax_v_s64(data: zb.baseAddress, count: Int64(zb.count)),
                        Int64(samples),
                        coax_v_s64(data: ob.baseAddress, count: Int64(ob.count))
                    )
                }
            }
        }
        psiRows = Int(got)
        psiCurves = got > 0 ? out.map { Float($0) / 1000.0 } : []
    }

    private func nsColor(_ c: SIMD4<Float>) -> NSColor {
        NSColor(calibratedRed: CGFloat(c.x), green: CGFloat(c.y), blue: CGFloat(c.z), alpha: 1.0)
    }

    private func fmtInt(_ n: Int) -> String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        f.locale = Locale(identifier: "en_US")
        return f.string(from: NSNumber(value: n)) ?? "\(n)"
    }

    private func addLabel(_ pos: SIMD3<Float>, _ text: String, sub: String = "", color: SIMD4<Float>, station: Int, banner: Bool = false, maxDist: Float = 30.0) {
        labelAnchors.append(Rh3dLabelAnchor(
            worldPos: pos, text: text, subtext: sub, color: nsColor(color),
            stationIndex: station, isStationBanner: banner, maxDist: maxDist
        ))
    }

    private func addLine(_ a: SIMD3<Float>, _ b: SIMD3<Float>, color: SIMD4<Float>, light: Float = 0.85, core: Float = 0.85, glow: Float = 4.0, pulseSpeed: Float = 0, pulsePhase: Float = 0) {
        lineSegs.append(Rh3dQuadLineSeg(
            p0Width: SIMD4(a, core),
            p1Glow: SIMD4(b, glow),
            color: SIMD4(color.x, color.y, color.z, light),
            anim: SIMD4(pulseSpeed, pulsePhase, pulseSpeed > 0.001 ? 1.0 : 0.0, 0)
        ))
    }

    // A dark box with bright edges.
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

    private func addRing(center: SIMD3<Float>, radius: Float, color: SIMD4<Float>, light: Float) {
        for k in 0..<32 {
            let a0 = Float(k) / 32.0 * .pi * 2
            let a1 = Float(k + 1) / 32.0 * .pi * 2
            addLine(center + SIMD3(cos(a0) * radius, 0, sin(a0) * radius), center + SIMD3(cos(a1) * radius, 0, sin(a1) * radius), color: color, light: light, core: 1.0, glow: 5.0)
        }
    }

    private func buildWorldGeometry(_ sc: Rh3dScene) {
        opaqueBoxes.removeAll()
        opaqueCyls.removeAll()
        opaqueOcts.removeAll()
        transBoxes.removeAll()
        transCyls.removeAll()
        labelAnchors.removeAll()
        pickables.removeAll()
        lineSegs.removeAll()

        buildStationBases()
        buildZetaLandscape(sc)
        buildPrimeStaircase(sc)
        buildRobinTest(sc)
        buildThetaChain()
        buildLastStep(sc)
        buildProofCity(sc)
        buildCheckers(sc)
        buildSummit(sc)

        quadLineSegCount = lineSegs.count
        if let dev = device, !lineSegs.isEmpty {
            quadLinesVB = dev.makeBuffer(bytes: lineSegs, length: lineSegs.count * MemoryLayout<Rh3dQuadLineSeg>.stride, options: .storageModeShared)
        }
    }

    // A platform, a banner and a light track to the next station, for each of the eight.
    private func buildStationBases() {
        let bannerHeights: [Float] = [6.4, 7.6, 6.6, 6.0, 10.2, 8.6, 6.8, 10.6]
        for (i, st) in stations.enumerated() {
            addGridBox(center: SIMD3(st.pos.x, 0.09, st.pos.z), size: SIMD3(9.2, 0.18, 9.2), face: GridTokens.obsidianDeep, edge: GridTokens.greyLight, edgeIntensity: 0.78)
            addLine(SIMD3(st.pos.x - 4.6, 0.20, 4.6), SIMD3(st.pos.x + 4.6, 0.20, 4.6), color: st.accent, light: 0.95, core: 0.95, glow: 4.8)
            addLabel(SIMD3(st.pos.x, bannerHeights[i % bannerHeights.count], st.pos.z), st.shortName.uppercased(), sub: st.kicker, color: st.accent, station: i, banner: true, maxDist: 145.0)
            pickables.append(Rh3dPickable(center: SIMD3(st.pos.x, 3.0, st.pos.z), radius: 5.0, title: st.title, detail: st.detail, significance: "", moduleName: st.moduleName, stationIndex: i))
            if i + 1 < stations.count {
                let nxt = stations[i + 1]
                addLine(SIMD3(st.pos.x + 4.6, 0.22, 0), SIMD3(nxt.pos.x - 4.6, 0.22, 0), color: GridTokens.cyan, light: 0.92, core: 1.1, glow: 6.2, pulseSpeed: 0.45, pulsePhase: Float(i) * 0.14)
            }
        }
    }

    // ---- Station 1: |zeta(s)| over the critical strip, with the zeros on Re(s) = 1/2 ----
    private func buildZetaLandscape(_ sc: Rh3dScene) {
        let cx = stations[0].pos.x
        let widthX: Float = 4.8      // sigma from 0 to 1
        let depthZ: Float = 8.6      // t from sc.t0 to sc.t1
        let hScale: Float = 0.80
        let baseY: Float = 0.24
        let ns = sc.zNs, nt = sc.zNt
        // sigma = 1 (where zeta is small) is on the camera's side, so the line of zeros is in
        // clear view and the tall side near sigma = 0 is the backdrop. Low t is nearest.
        func xOf(_ sigma: Float) -> Float { cx + (0.5 - sigma) * widthX }
        func zOf(_ t: Float) -> Float { (0.5 - (t - sc.t0) / (sc.t1 - sc.t0)) * depthZ }

        if ns >= 2 && nt >= 2 && sc.zHeights.count >= ns * nt, let dev = device {
            func vert(_ i: Int, _ j: Int, _ n: SIMD3<Float>) -> Rh3dTerrainVert {
                let u = Float(i) / Float(ns - 1)
                let v = Float(j) / Float(nt - 1)
                let h = sc.zHeights[j * ns + i]
                let p = SIMD3<Float>(xOf(u), baseY + h * hScale, (0.5 - v) * depthZ)
                return Rh3dTerrainVert(posHeight: SIMD4(p, h), normalCrit: SIMD4(n, abs(u - 0.5)), uvPad: SIMD4(u, v, 0, 0))
            }
            func up(_ a: SIMD3<Float>, _ b: SIMD3<Float>, _ c: SIMD3<Float>) -> SIMD3<Float> {
                let n = simd_normalize(simd_cross(b - a, c - a))
                return n.y < 0 ? -n : n
            }
            var tVerts: [Rh3dTerrainVert] = []
            tVerts.reserveCapacity((ns - 1) * (nt - 1) * 6)
            let zero = SIMD3<Float>(0, 1, 0)
            for j in 0..<(nt - 1) {
                for i in 0..<(ns - 1) {
                    let p00 = vert(i, j, zero).posHeight, p10 = vert(i + 1, j, zero).posHeight
                    let p01 = vert(i, j + 1, zero).posHeight, p11 = vert(i + 1, j + 1, zero).posHeight
                    let a = SIMD3(p00.x, p00.y, p00.z), b = SIMD3(p10.x, p10.y, p10.z)
                    let c = SIMD3(p01.x, p01.y, p01.z), d = SIMD3(p11.x, p11.y, p11.z)
                    let nA = up(a, c, b), nB = up(b, c, d)
                    tVerts.append(contentsOf: [
                        vert(i, j, nA), vert(i, j + 1, nA), vert(i + 1, j, nA),
                        vert(i + 1, j, nB), vert(i, j + 1, nB), vert(i + 1, j + 1, nB),
                    ])
                }
            }
            terrainVCount = tVerts.count
            terrainVB = dev.makeBuffer(bytes: tVerts, length: tVerts.count * MemoryLayout<Rh3dTerrainVert>.stride, options: .storageModeShared)
        }

        // The critical line Re(s) = 1/2 as a thin glass wall with a bright top edge.
        addGridBox(center: SIMD3(cx, 2.0, 0), size: SIMD3(0.05, 3.5, depthZ), face: SIMD4(0.08, 0.05, 0.015, 0.12), edge: GridTokens.amber, edgeIntensity: 1.1, edgePx: 1.5, mode: 4.0, translucent: true)
        addLine(SIMD3(cx, 3.76, -depthZ * 0.5), SIMD3(cx, 3.76, depthZ * 0.5), color: GridTokens.amberHot, light: 1.35, core: 1.2, glow: 6.5)
        addLabel(SIMD3(cx, 3.8, -0.6), "THE CRITICAL LINE", sub: "Real part = 1/2", color: GridTokens.amber, station: 0)

        // One needle per zero, at the heights computed in Coax.
        var shown = 0
        for gamma in sc.zeros where gamma > sc.t0 && gamma < sc.t1 {
            let z = zOf(gamma)
            let top: Float = shown == 0 ? 3.3 : 2.7
            addLine(SIMD3(cx, baseY, z), SIMD3(cx, top, z), color: GridTokens.amberHot, light: 1.25, core: 1.1, glow: 5.8)
            addGridCrystal(center: SIMD3(cx, top + 0.2, z), size: SIMD3(0.30, 0.42, 0.30), core: SIMD4(0.30, 0.18, 0.03, 1.0), edge: GridTokens.amberHot, intensity: 1.6, phase: Float(shown) * 0.7)
            if shown == 0 {
                addLabel(SIMD3(cx, top + 0.45, z), "THE FIRST ZERO", sub: String(format: "1/2 + %.4f i", gamma), color: GridTokens.amberHot, station: 0)
            }
            shown += 1
        }
        addLabel(SIMD3(xOf(0.95), baseY + 1.0, zOf(sc.t0 + (sc.t1 - sc.t0) * 0.30)), "HEIGHT = SIZE OF ZETA", sub: "Computed, not drawn", color: GridTokens.cyan, station: 0)
    }

    // ---- Station 2: psi(x) and von Mangoldt's explicit formula ----
    private func psiPoint(_ x: Float, _ v: Float, z: Float = 0) -> SIMD3<Float> {
        let cx = stations.count > 1 ? stations[1].pos.x : -30
        let u = (x - psiX0) / max(1, psiX1 - psiX0)
        let top = psiX1 * 1.04
        return SIMD3(cx - 4.0 + u * 8.0, 0.45 + max(0, min(top, v)) / top * 5.6, z)
    }

    private func buildPrimeStaircase(_ sc: Rh3dScene) {
        let cx = stations[1].pos.x
        // A dark back plate so the curves read against the sky.
        addGridBox(center: SIMD3(cx, 3.25, -0.12), size: SIMD3(8.5, 6.0, 0.06), face: SIMD4(0.004, 0.006, 0.009, 0.80), edge: GridTokens.greyMid, edgeIntensity: 0.6, edgePx: 1.2, mode: 4.0, translucent: true)
        addLine(psiPoint(sc.psiX0, 0), psiPoint(sc.psiX1, 0), color: GridTokens.greyMid, light: 0.6, core: 0.8, glow: 2.5)
        addLine(psiPoint(sc.psiX0, 0), psiPoint(sc.psiX0, sc.psiX1), color: GridTokens.greyMid, light: 0.6, core: 0.8, glow: 2.5)

        // The exact staircase: flat, then a jump of ln p at each prime power.
        var px = sc.psiX0
        var pv: Float = 0
        for st in sc.psiSteps {
            if st.x > px { addLine(psiPoint(px, pv), psiPoint(st.x, pv), color: GridTokens.greyWhite, light: 0.42, core: 3.2, glow: 1.2) }
            addLine(psiPoint(st.x, pv), psiPoint(st.x, st.v), color: GridTokens.greyWhite, light: 0.42, core: 3.2, glow: 1.2)
            addLine(psiPoint(st.x, 0), psiPoint(st.x, -1.2), color: GridTokens.greyLight, light: 0.55, core: 0.7, glow: 2.0)
            px = st.x
            pv = st.v
        }
        addLine(psiPoint(px, pv), psiPoint(sc.psiX1, pv), color: GridTokens.greyWhite, light: 0.42, core: 3.2, glow: 1.2)

        addLabel(psiPoint(23, 20.5), "THE PRIME STAIRCASE", sub: "A step up at every prime", color: GridTokens.greyWhite, station: 1)
        psiLabelIndex = labelAnchors.count
        addLabel(psiPoint(53, 54), "FORMULA WITH 100 ZEROS", sub: "One wave per zero", color: GridTokens.cyan, station: 1)
        addLabel(psiPoint(sc.psiX0, -2.5), "2", color: GridTokens.greyLight, station: 1)
        addLabel(psiPoint(sc.psiX1, -2.5), "\(Int(sc.psiX1))", color: GridTokens.greyLight, station: 1)
    }

    // How many zeros the animated curve uses at `phase` in [0, 1]: none, then one at a time,
    // then faster, then all of them.
    private func psiZerosShown(phase: Float) -> Int {
        let top = max(0, psiRows - 1)
        if phase < 0.10 { return 0 }
        if phase > 0.86 { return top }
        let w = (phase - 0.10) / 0.76
        return min(top, max(1, Int((Float(top) * w * w).rounded())))
    }

    private func refreshPsiCurve(zerosShown k: Int) {
        guard psiRows > 0, k >= 0, k < psiRows, k != psiShown, let dev = device else { return }
        psiShown = k
        let n = psiSamples
        var segs: [Rh3dQuadLineSeg] = []
        segs.reserveCapacity(n - 1)
        for i in 0..<(n - 1) {
            let xa = psiX0 + (psiX1 - psiX0) * Float(i) / Float(n - 1)
            let xb = psiX0 + (psiX1 - psiX0) * Float(i + 1) / Float(n - 1)
            segs.append(Rh3dQuadLineSeg(
                p0Width: SIMD4(psiPoint(xa, psiCurves[k * n + i], z: 0.10), 1.15),
                p1Glow: SIMD4(psiPoint(xb, psiCurves[k * n + i + 1], z: 0.10), 3.0),
                color: SIMD4(GridTokens.cyan.x * 0.3, GridTokens.cyan.y, GridTokens.cyan.z, 1.6),
                anim: SIMD4(0, 0, 0, 0)
            ))
        }
        let bytes = segs.count * MemoryLayout<Rh3dQuadLineSeg>.stride
        if dynLinesVB == nil || dynLinesVB!.length < bytes {
            dynLinesVB = dev.makeBuffer(length: bytes, options: .storageModeShared)
        }
        if let vb = dynLinesVB {
            segs.withUnsafeBytes { vb.contents().copyMemory(from: $0.baseAddress!, byteCount: bytes) }
            dynLineSegCount = segs.count
        }
        if psiLabelIndex >= 0 && psiLabelIndex < labelAnchors.count {
            labelAnchors[psiLabelIndex].text = k == 0 ? "SMOOTH TREND, NO ZEROS YET" : (k == 1 ? "FORMULA WITH 1 ZERO" : "FORMULA WITH \(k) ZEROS")
        }
    }

    // ---- Station 3: Robin's criterion ----
    private func buildRobinTest(_ sc: Rh3dScene) {
        let cx = stations[2].pos.x
        // The ratio for n = 12 is 1.44, far above the rest; the pillars start at 60 so that
        // the small gap between 5,040 (just over) and 10,080 (just under) stays visible.
        let marks = sc.robin.filter { $0.n >= 60 }
        guard !marks.isEmpty else { return }
        func heightOf(_ ratio: Float) -> Float { (ratio - 0.90) * 24.0 }
        let ceilingY = 0.18 + heightOf(1.0)
        addGridBox(center: SIMD3(cx, ceilingY, 0), size: SIMD3(8.6, 0.04, 3.0), face: SIMD4(0.015, 0.055, 0.06, 0.16), edge: GridTokens.cyan, edgeIntensity: 1.05, edgePx: 1.4, mode: 4.0, translucent: true)
        addLabel(SIMD3(cx - 4.0, ceilingY + 0.05, 1.5), "THE CEILING", sub: "Ratio = 1", color: GridTokens.cyan, station: 2)

        for (i, m) in marks.enumerated() {
            let u = Float(i) / Float(max(1, marks.count - 1))
            let bx = cx + (u - 0.5) * 7.6
            let h = max(0.3, heightOf(m.ratio))
            let over = m.ratio >= 1.0
            let is5040 = m.n == 5040
            let edge: SIMD4<Float> = is5040 ? GridTokens.amberHot : (over ? GridTokens.amber : GridTokens.cyan)
            addGridBox(center: SIMD3(bx, 0.18 + h * 0.5, 0), size: SIMD3(is5040 ? 0.52 : 0.40, h, is5040 ? 0.52 : 0.40), face: is5040 ? SIMD4(0.06, 0.04, 0.014, 1.0) : GridTokens.obsidianBody, edge: edge, edgeIntensity: is5040 ? 1.65 : 0.95, edgePx: is5040 ? 1.6 : 1.35)
            if is5040 {
                addGridCrystal(center: SIMD3(bx, 0.18 + h + 0.5, 0), size: SIMD3(0.44, 0.62, 0.44), core: SIMD4(0.30, 0.18, 0.03, 1.0), edge: GridTokens.amberHot, intensity: 1.7)
                addLabel(SIMD3(bx, 0.18 + h + 0.95, 0), "5,040: THE LAST ONE OVER", sub: String(format: "Ratio %.4f", m.ratio), color: GridTokens.amberHot, station: 2)
            } else if i == 0 {
                addLabel(SIMD3(bx, 0.18 + h + 0.1, 0), "n = \(fmtInt(m.n))", sub: String(format: "Ratio %.3f", m.ratio), color: GridTokens.amber, station: 2)
            } else if i == marks.count - 1 {
                addLabel(SIMD3(bx, 0.18 + h + 0.1, 0), "n = \(fmtInt(m.n))", sub: String(format: "Ratio %.3f, under", m.ratio), color: GridTokens.cyan, station: 2)
            }
        }
    }

    // ---- Station 4: the one idea the author's note names ----
    private func buildThetaChain() {
        let cx = stations[3].pos.x
        // Three example lengths X and their chains of sqrt(X) links. This shows how the chain
        // grows; it is not a picture of what a theta lift does.
        let rows: [(x: Int, links: Int)] = [(16, 4), (64, 8), (256, 16)]
        for (r, row) in rows.enumerated() {
            let z: Float = 2.4 - Float(r) * 2.4
            let y: Float = 1.2 + Float(r) * 1.5
            let x0 = cx - 3.9
            let spacing: Float = 0.52
            var prev: SIMD3<Float>? = nil
            for k in 0..<row.links {
                let p = SIMD3<Float>(x0 + Float(k) * spacing, y, z)
                addGridCrystal(center: p, size: SIMD3(0.26, 0.36, 0.26), core: SIMD4(0.10, 0.11, 0.12, 1.0), edge: GridTokens.greyWhite, intensity: 1.35, phase: Float(k) * 0.4)
                if let q = prev {
                    addLine(q, p, color: GridTokens.cyan, light: 1.0, core: 1.0, glow: 4.5, pulseSpeed: 0.6, pulsePhase: Float(k) * 0.09)
                }
                prev = p
            }
            let end = SIMD3<Float>(x0 + Float(row.links - 1) * spacing + 0.5, y + 0.2, z)
            addLabel(end, "X = \(row.x): \(row.links) LIFTS", sub: r == 2 ? "Chain length grows like √X" : "", color: GridTokens.greyWhite, station: 3)
        }
    }

    // ---- Station 5: no zeros right of the line, plus the mirror symmetry ----
    private func buildLastStep(_ sc: Rh3dScene) {
        let cx = stations[4].pos.x
        // The critical strip stood upright and seen from the front: real part across, t upward.
        let y0: Float = 0.5, y1: Float = 8.3, half: Float = 3.6
        func yOf(_ t: Float) -> Float { y0 + (t - sc.t0) / (sc.t1 - sc.t0) * (y1 - y0) }
        let midY = (y0 + y1) * 0.5, hgt = y1 - y0
        // Right half (real part above 1/2): what the Lean proof establishes.
        addGridBox(center: SIMD3(cx + half * 0.5, midY, 0), size: SIMD3(half, hgt, 0.05), face: SIMD4(0.015, 0.07, 0.075, 0.30), edge: GridTokens.cyan, edgeIntensity: 1.1, edgePx: 1.4, mode: 4.0, translucent: true)
        // Left half: follows from the functional equation.
        addGridBox(center: SIMD3(cx - half * 0.5, midY, 0), size: SIMD3(half, hgt, 0.05), face: SIMD4(0.05, 0.05, 0.055, 0.22), edge: GridTokens.greyLight, edgeIntensity: 0.8, edgePx: 1.3, mode: 4.0, translucent: true)
        addLine(SIMD3(cx, y0, 0.05), SIMD3(cx, y1 + 0.5, 0.05), color: GridTokens.amberHot, light: 1.4, core: 1.3, glow: 6.5)

        for (i, gamma) in sc.zeros.enumerated() where gamma > sc.t0 && gamma < sc.t1 {
            addGridCrystal(center: SIMD3(cx, yOf(gamma), 0.1), size: SIMD3(0.26, 0.36, 0.26), core: SIMD4(0.30, 0.18, 0.03, 1.0), edge: GridTokens.amberHot, intensity: 1.6, phase: Float(i) * 0.7)
        }
        // The mirror: s and 1 - conj(s) are zeros together. Arcs carry right to left.
        for k in 0..<4 {
            let y = y0 + 1.0 + Float(k) * 2.0
            var prev: SIMD3<Float>? = nil
            for j in 0...14 {
                let u = Float(j) / 14.0
                let p = SIMD3<Float>(cx + 2.3 - u * 4.6, y, 0.15 + sin(u * .pi) * 1.5)
                if let q = prev {
                    addLine(q, p, color: GridTokens.greyWhite, light: 0.7, core: 0.8, glow: 3.2, pulseSpeed: 0.5, pulsePhase: Float(k) * 0.2 + u * 0.3)
                }
                prev = p
            }
        }
        addLabel(SIMD3(cx + half * 0.62, midY + 1.4, 0.1), "NO ZEROS HERE", sub: "Proved in Lean (allChi)", color: GridTokens.cyan, station: 4)
        addLabel(SIMD3(cx - half * 0.62, midY - 0.6, 0.1), "SO NONE HERE", sub: "By mirror symmetry", color: GridTokens.greyWhite, station: 4)
        addLabel(SIMD3(cx, y1 + 0.9, 0), "ALL ZEROS ON THE LINE", color: GridTokens.amberHot, station: 4)
    }

    // ---- Station 6: the proof's folders as towers ----
    private func buildProofCity(_ sc: Rh3dScene) {
        let cx = stations[5].pos.x
        let atlasURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/index/rh-atlas.json")
        func towerHeight(lines: Int) -> Float { max(0.7, min(7.4, Float(log10(Double(max(10, lines)))) * 1.18 - 0.6)) }

        var towers: [(name: String, files: Int, lines: Int, hub: String)] = []
        if let data = try? Data(contentsOf: atlasURL),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let clusters = obj["clusters"] as? [[String: Any]], !clusters.isEmpty {
            for cl in clusters {
                let name = (cl["name"] as? String) ?? ""
                towers.append((name, (cl["modules"] as? Int) ?? 1, (cl["lines"] as? Int) ?? 100, (cl["hub"] as? String) ?? name))
            }
        } else {
            // No local index: fall back to the published per-library totals.
            towers = sc.libs.map { ($0.name, $0.files, $0.lines, "Main") }
        }
        towers.sort { $0.files > $1.files }
        let count = min(42, towers.count)
        let cols = count > 12 ? 7 : 4
        let rows = (count + cols - 1) / cols
        if towers.count > count {
            addLabel(SIMD3(cx + 3.4, 1.2, 4.2), "THE \(count) LARGEST FOLDERS", sub: "Of \(towers.count); heights on a log scale", color: GridTokens.cyan, station: 5)
        }
        for idx in 0..<count {
            let tw = towers[idx]
            let bx = cx + (Float(idx % cols) - Float(cols - 1) * 0.5) * (count > 12 ? 1.15 : 1.9)
            let bz = (Float(idx / cols) - Float(rows - 1) * 0.5) * (count > 12 ? 1.25 : 2.2)
            let bh = towerHeight(lines: tw.lines)
            let bw = max(0.42, min(0.90, 0.36 + Float(log10(Double(max(1, tw.files)))) * 0.14))
            let edge: SIMD4<Float> = idx == 0 ? GridTokens.amberHot : (idx < 6 ? GridTokens.cyan : GridTokens.greyLight)
            addGridBox(center: SIMD3(bx, 0.18 + bh * 0.5, bz), size: SIMD3(bw, bh, bw), face: GridTokens.obsidianDeep, edge: edge, edgeIntensity: idx < 6 ? 1.2 : 0.75, edgePx: idx < 6 ? 1.40 : 1.20, mode: 1.0, floorBands: max(4.0, (bh * 2.2).rounded()))
            if idx == 0 {
                let short = tw.name.split(separator: ".").last.map(String.init) ?? tw.name
                addLabel(SIMD3(bx, bh + 0.45, bz), "THE LARGEST: " + short.uppercased(), sub: "\(fmtInt(tw.files)) files · \(fmtInt(tw.lines)) lines", color: edge, station: 5)
            }
            pickables.append(Rh3dPickable(center: SIMD3(bx, bh * 0.5, bz), radius: 0.8, title: tw.name, detail: "\(fmtInt(tw.files)) Lean files and \(fmtInt(tw.lines)) lines, as counted by the local index.", significance: "", moduleName: tw.hub, stationIndex: 5))
        }
    }

    // ---- Station 7: the independent checkers ----
    private func buildCheckers(_ sc: Rh3dScene) {
        let cx = stations[6].pos.x
        let n = sc.checkers.count
        guard n > 0 else { return }
        let src = SIMD3<Float>(cx, 0.75, 2.9)
        addGridBox(center: src, size: SIMD3(1.5, 1.1, 1.1), face: GridTokens.obsidianSlate, edge: GridTokens.greyWhite, edgeIntensity: 1.3, edgePx: 1.5)
        addLabel(src + SIMD3(0, 0.7, 0), "THE PROOF", sub: "\(fmtInt(sceneLines)) lines", color: GridTokens.greyWhite, station: 6)
        for (i, ck) in sc.checkers.enumerated() {
            let u = n > 1 ? Float(i) / Float(n - 1) : 0.5
            let px = cx + (u - 0.5) * 7.4
            let h: Float = i % 2 == 0 ? 3.0 : 4.5
            let pz: Float = -1.4
            addGridCyl(center: SIMD3(px, 0.18 + h * 0.5, pz), size: SIMD3(0.9, h, 0.9), face: GridTokens.obsidianBody, edge: GridTokens.cyan, edgeIntensity: 1.05)
            addGridCrystal(center: SIMD3(px, 0.18 + h + 0.5, pz), size: SIMD3(0.42, 0.60, 0.42), core: SIMD4(0.06, 0.24, 0.26, 1.0), edge: GridTokens.cyan, intensity: 1.6, phase: Float(i) * 0.9)
            addLine(src + SIMD3(0, -0.4, -0.55), SIMD3(px, 0.25, pz + 0.45), color: GridTokens.cyan, light: 0.9, core: 0.9, glow: 4.5, pulseSpeed: 0.55, pulsePhase: Float(i) * 0.17)
            addLabel(SIMD3(px, 0.18 + h + 0.95, pz), ck.name, sub: ck.tag, color: GridTokens.cyan, station: 6)
        }
    }

    // ---- Station 8: the theorem ----
    private func buildSummit(_ sc: Rh3dScene) {
        let cx = stations[7].pos.x
        addGridBox(center: SIMD3(cx, 3.3, 0), size: SIMD3(1.7, 6.2, 1.7), face: GridTokens.obsidianDeep, edge: GridTokens.amberHot, edgeIntensity: 1.3, edgePx: 1.5, mode: 1.0, floorBands: 12)
        addGridCrystal(center: SIMD3(cx, 7.5, 0), size: SIMD3(1.1, 1.6, 1.1), core: SIMD4(0.30, 0.18, 0.03, 1.0), edge: GridTokens.amberHot, intensity: 1.9)
        addRing(center: SIMD3(cx, 6.5, 0), radius: 2.0, color: GridTokens.amberHot, light: 1.2)
        addRing(center: SIMD3(cx, 6.5, 0), radius: 2.6, color: GridTokens.greyWhite, light: 0.6)
        // One marker for each theorem of src/Main.lean.
        let n = sc.theoremNames.count
        for i in 0..<n {
            let a = Float.pi * (0.15 + 0.70 * Float(i) / Float(max(1, n - 1)))
            let p = SIMD3<Float>(cx - cos(a) * 3.4, 0.6, sin(a) * 3.4)
            addGridBox(center: SIMD3(p.x, 0.48, p.z), size: SIMD3(0.5, 0.6, 0.5), face: GridTokens.obsidianBody, edge: i < 2 ? GridTokens.amberHot : GridTokens.cyan, edgeIntensity: 1.1)
            addLine(SIMD3(p.x, 0.8, p.z), SIMD3(cx, 2.4, 0), color: i < 2 ? GridTokens.amber : GridTokens.cyan, light: 0.7, core: 0.8, glow: 3.5, pulseSpeed: 0.5, pulsePhase: Float(i) * 0.2)
        }
        addLabel(SIMD3(cx, 8.6, 0), "THE RIEMANN HYPOTHESIS", sub: "Now a theorem: riemannHypothesis", color: GridTokens.amberHot, station: 7)
        addLabel(SIMD3(cx - 3.2, 1.4, 2.4), "\(n) THEOREMS IN MAIN.LEAN", sub: "2 for zeta, 3 for every Dirichlet L-function", color: GridTokens.cyan, station: 7)
        addLabel(SIMD3(cx + 2.9, 4.2, 0.5), "\(fmtInt(sceneTheorems)) THEOREMS AND LEMMAS", sub: "In \(fmtInt(sceneFiles)) files", color: GridTokens.greyWhite, station: 7)
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
        topLeftBadge.stringValue = "DRAG TO ORBIT · SCROLL TO ZOOM · WASD TO FLY · SPACE TO PLAY"
        topLeftBadge.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .medium)
        topLeftBadge.textColor = GridTokens.nsLight
        topLeftFrame.addSubview(topLeftBadge)

        for (btn, act, acc) in [
            (playPauseBtn, #selector(togglePlayPause), GridTokens.nsCyan as NSColor?),
            (overviewBtn,  #selector(goOverview), nil),
            (guideBtn,     #selector(toggleGuide), GridTokens.nsAmber as NSColor?),
            (labelsBtn,    #selector(toggleLabels), nil),
            (inspectBtn,   #selector(openSourceClicked), GridTokens.nsWhite as NSColor?),
        ] {
            styleGridButton(btn, accent: acc)
            btn.target = self
            btn.action = act
            addSubview(btn)
        }

        // Right-side Friendly Explainer & Significance Guide Card (collapsible via GUIDE button or 'G' key)
        explainerPanel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(explainerPanel)

        // Bottom OpenAgents Grid Chamfered Navigation Bar (exact hud-frame.coax L-brackets & 45° cuts!)
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
        storyTitle.font = NSFont.monospacedSystemFont(ofSize: 13.0, weight: .bold)
        storyTitle.textColor = GridTokens.nsWhite
        storyCard.addSubview(storyTitle)

        storySubtitle.translatesAutoresizingMaskIntoConstraints = false
        storySubtitle.font = NSFont.monospacedSystemFont(ofSize: 10.2, weight: .medium)
        storySubtitle.textColor = GridTokens.nsCyan
        storyCard.addSubview(storySubtitle)

        storyBody.translatesAutoresizingMaskIntoConstraints = false
        storyBody.isHidden = true
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

        explainerRightConstraints = [
            explainerPanel.topAnchor.constraint(equalTo: topAnchor, constant: 46),
            explainerPanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            explainerPanel.widthAnchor.constraint(equalToConstant: 392),
            explainerPanel.bottomAnchor.constraint(lessThanOrEqualTo: storyCard.topAnchor, constant: -10),
            explainerPanel.heightAnchor.constraint(equalToConstant: 616),
        ]
        explainerRightConstraints[4].priority = .defaultHigh

        explainerOverviewConstraints = [
            explainerPanel.topAnchor.constraint(equalTo: topAnchor, constant: 46),
            explainerPanel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            explainerPanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            explainerPanel.heightAnchor.constraint(equalToConstant: 148),
        ]

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
            guideBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            guideBtn.trailingAnchor.constraint(equalTo: labelsBtn.leadingAnchor, constant: -6),
            guideBtn.heightAnchor.constraint(equalToConstant: 24),
            overviewBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            overviewBtn.trailingAnchor.constraint(equalTo: guideBtn.leadingAnchor, constant: -6),
            overviewBtn.heightAnchor.constraint(equalToConstant: 24),
            playPauseBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            playPauseBtn.trailingAnchor.constraint(equalTo: overviewBtn.leadingAnchor, constant: -6),
            playPauseBtn.heightAnchor.constraint(equalToConstant: 24),

            storyCard.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            storyCard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            storyCard.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
            storyCard.heightAnchor.constraint(equalToConstant: 96),

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

            progressTrack.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 16),
            progressTrack.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -16),
            progressTrack.bottomAnchor.constraint(equalTo: stationStack.topAnchor, constant: -6),
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
        NSLayoutConstraint.activate(explainerRightConstraints)
    }

    private func buildGuideContent(overview: Bool) -> Rh3dGuideContent {
        if overview {
            return Rh3dGuideContent(
                stepTag: "OVERVIEW", kicker: "8 STOPS", kickerColor: GridTokens.nsAmber,
                title: overviewTitle, caption: overviewCaption, detail: "", source: "", isCompactWide: true
            )
        }
        if let pick = selectedPick, pick.radius < 4.0 {
            return Rh3dGuideContent(
                stepTag: "SELECTED", kicker: "STOP \(pick.stationIndex + 1) OF \(max(1, stations.count))", kickerColor: GridTokens.nsCyan,
                title: pick.title, caption: pick.detail, detail: "", source: "Lean module: \(pick.moduleName)", isCompactWide: false
            )
        }
        if activeStation >= 0 && activeStation < stations.count {
            let st = stations[activeStation]
            return Rh3dGuideContent(
                stepTag: "STOP \(activeStation + 1) OF \(stations.count)", kicker: st.kicker, kickerColor: nsColor(st.accent),
                title: st.title, caption: st.caption, detail: st.detail, source: st.source, isCompactWide: false
            )
        }
        return Rh3dGuideContent(stepTag: "", kicker: "", kickerColor: GridTokens.nsCyan, title: overviewTitle, caption: "", detail: "", source: "", isCompactWide: false)
    }

    private func applyGuideLayout() {
        explainerPanel.isHidden = !showGuide
        if isOverviewMode {
            NSLayoutConstraint.deactivate(explainerRightConstraints)
            NSLayoutConstraint.activate(explainerOverviewConstraints)
        } else {
            NSLayoutConstraint.deactivate(explainerOverviewConstraints)
            NSLayoutConstraint.activate(explainerRightConstraints)
        }
        explainerPanel.content = buildGuideContent(overview: isOverviewMode)
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
        if isOverviewMode {
            storyTitle.stringValue = overviewTitle
            storySubtitle.stringValue = "Click a stop to fly to it, or press PLAY for the tour"
        } else if let pick = selectedPick {
            storyTitle.stringValue = pick.title
            storySubtitle.stringValue = "Lean module: \(pick.moduleName)"
        } else if activeStation >= 0 && activeStation < stations.count {
            let st = stations[activeStation]
            storyTitle.stringValue = st.title
            storySubtitle.stringValue = "STOP \(activeStation + 1) OF \(stations.count)  ·  \(st.kicker)"
        }
        updateGridButtonTitle(playPauseBtn, flyPlaying ? "PAUSE" : "PLAY", accent: GridTokens.nsCyan)
        updateGridButtonTitle(guideBtn, showGuide ? "GUIDE: ON" : "GUIDE: OFF", accent: showGuide ? GridTokens.nsAmber : nil)
        updateGridButtonTitle(labelsBtn, showLabels ? "LABELS: ON" : "LABELS: OFF", accent: nil)
        for (i, b) in stationButtons.enumerated() {
            let title = i < stations.count ? stations[i].shortName.uppercased() : "ST \(i+1)"
            let isAct = (!isOverviewMode && i == activeStation)
            b.layer?.backgroundColor = isAct
                ? NSColor(calibratedRed: 0.04, green: 0.14, blue: 0.15, alpha: 0.90).cgColor
                : NSColor(calibratedRed: 0.018, green: 0.022, blue: 0.028, alpha: 0.85).cgColor
            updateGridButtonTitle(b, title, accent: isAct ? GridTokens.nsCyan : nil)
        }
        let frac = CGFloat(max(0.0, min(1.0, flyTimeS / max(1.0, durationS))))
        progressFillWidth?.constant = max(8, progressTrack.bounds.width * frac)
        applyGuideLayout()
    }

    // MARK: - Actions & Controls

    @objc private func togglePlayPause() {
        flyPlaying.toggle()
        if flyPlaying { isOverviewMode = false }
        glideDuration = 0
        syncStoryHUD()
    }

    @objc private func goOverview() {
        flyPlaying = false
        selectedPick = nil
        isOverviewMode = true
        startGlide(toEye: SIMD3(0, 32.0, 74.0), toTarget: SIMD3(0, 3.0, 0), dur: 0.95)
        syncStoryHUD()
    }

    @objc private func toggleGuide() {
        showGuide.toggle()
        syncStoryHUD()
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
        isOverviewMode = false
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
        if event.keyCode == 5  { toggleGuide(); return } // G -> toggle explainer guide
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

        // Station 2 adds the zeros one at a time, on a 14-second loop; elsewhere it shows them all.
        if activeStation == 1 && !isOverviewMode {
            refreshPsiCurve(zerosShown: psiZerosShown(phase: (totalElapsedS / 14.0).truncatingRemainder(dividingBy: 1.0)))
        } else {
            refreshPsiCurve(zerosShown: max(0, psiRows - 1))
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
        let accentModes: [Float] = [0, 0, 1, 3, 1, 0, 0, 1]
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
                if let dyn = dynLinesVB, dynLineSegCount > 0 {
                    enc.setVertexBuffer(dyn, offset: 0, index: 0)
                    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: dynLineSegCount * 6)
                }
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

    // `video` lays the labels out for the recorded walkthrough: larger type, no station banners
    // while the camera is at a station, and nothing under the caption.
    private func computeProjectedLabels(viewSize: CGSize, video: Bool = false) -> [ProjectedLabel] {
        guard showLabels, viewSize.width > 10, viewSize.height > 10 else { return [] }
        let w = Float(viewSize.width)
        let h = Float(viewSize.height)
        let vp = rh3dPerspective(fovY: 0.82, aspect: w / h, nearZ: 0.2, farZ: 280.0) * rh3dLookAt(eye: eye, target: target)
        let s = RhLabelsOverlayView.scale

        let titleBannerFont = NSFont.monospacedSystemFont(ofSize: 10.5 * s, weight: .semibold)
        let titlePartFont = NSFont.monospacedSystemFont(ofSize: 9.8 * s, weight: .medium)
        let subFont = NSFont.monospacedSystemFont(ofSize: 8.8 * s, weight: .regular)

        let rightLimit: CGFloat = (!video && showGuide && !isOverviewMode) ? (viewSize.width - 420) : (viewSize.width - 12)
        let topLimit: CGFloat = video ? 16 : ((showGuide && isOverviewMode) ? 204 : 46)
        let bottomLimit: CGFloat = video ? (viewSize.height - GridExplainerPanelView.videoCaptionHeight - 40) : (viewSize.height - 118)

        var candidates: [(dist: Float, priority: Int, label: ProjectedLabel)] = []
        for anc in labelAnchors {
            if video && anc.isStationBanner && !isOverviewMode { continue }
            if video && !anc.isStationBanner && anc.stationIndex != activeStation { continue }
            let dist = simd_length(eye - anc.worldPos)
            if dist > anc.maxDist { continue }
            let clip = vp * SIMD4<Float>(anc.worldPos, 1.0)
            guard clip.w > 0.35 else { continue }
            let ndc = SIMD3<Float>(clip.x, clip.y, clip.z) / clip.w
            if ndc.x < -0.98 || ndc.x > 0.98 || ndc.y < -0.90 || ndc.y > 0.98 { continue }

            let sx = CGFloat((ndc.x * 0.5 + 0.5) * w)
            let sy = CGFloat((0.5 - ndc.y * 0.5) * h)
            if sx > rightLimit - 14 { continue }
            let minSY: CGFloat = anc.isStationBanner ? max(18, topLimit - 24) : max(50, topLimit)
            if sy < minSY || sy > bottomLimit { continue }

            let tw = (anc.text as NSString).size(withAttributes: [.font: anc.isStationBanner ? titleBannerFont : titlePartFont, .kern: anc.isStationBanner ? 0.7 : 0.3]).width
            let sw = anc.subtext.isEmpty ? 0 : (anc.subtext as NSString).size(withAttributes: [.font: subFont]).width
            let pw = max(tw, sw) + 20 * s
            let ph: CGFloat = (anc.subtext.isEmpty ? 22 : 35) * s
            let stemH: CGFloat = (anc.isStationBanner ? 22 : 17) * s
            let rx = max(12, min(rightLimit - pw, sx - pw * 0.5))
            let ry = max(topLimit, sy - stemH - ph)
            let alpha: CGFloat = anc.isStationBanner
                ? CGFloat(max(0.58, min(1.0, 1.15 - dist / 130.0)))
                : CGFloat(max(0.0, min(0.96, (anc.maxDist - dist) / 7.5)))
            guard alpha > 0.08 else { continue }

            let prio = anc.isStationBanner ? 0 : (anc.stationIndex == activeStation ? 1 : 2)
            candidates.append((dist, prio, ProjectedLabel(
                rect: CGRect(x: rx, y: ry, width: pw, height: ph),
                anchorPt: CGPoint(x: sx, y: sy),
                title: anc.text, subtext: anc.subtext, color: anc.color,
                isBanner: anc.isStationBanner, alpha: alpha, stationIndex: anc.stationIndex
            )))
        }

        candidates.sort { a, b in
            if a.priority != b.priority { return a.priority < b.priority }
            return a.dist < b.dist
        }

        // A label that would cover another moves up a row or two before it gives up.
        var placed: [ProjectedLabel] = []
        for c in candidates {
            var r = c.label.rect
            var tries = 0
            func hits(_ r: CGRect) -> Bool { placed.contains { $0.rect.intersects(r.insetBy(dx: -5, dy: -4)) } }
            while tries < 3 && hits(r) {
                r.origin.y -= (r.height + 8)
                tries += 1
            }
            if r.origin.y < topLimit - 30 || hits(r) { continue }
            placed.append(ProjectedLabel(
                rect: r, anchorPt: c.label.anchorPt, title: c.label.title, subtext: c.label.subtext,
                color: c.label.color, isBanner: c.label.isBanner, alpha: c.label.alpha, stationIndex: c.label.stationIndex
            ))
            if placed.count >= 16 { break }
        }
        return placed
    }

    private func updateProjectedLabels() {
        labelsOverlay.labels = computeProjectedLabels(viewSize: bounds.size)
        labelsOverlay.needsDisplay = true
    }

    // MARK: - Overlays shared by still frames and the video

    // The app's own HUD (labels, guide card, title bar) drawn into a still frame.
    private func drawAppOverlay(in ctx: CGContext, size: CGSize) {
        RhLabelsOverlayView.drawLabels(computeProjectedLabels(viewSize: size), in: ctx)
        let guideRect = isOverviewMode
            ? CGRect(x: 20, y: 20, width: size.width - 40, height: 158)
            : CGRect(x: size.width - 412, y: 24, width: 392, height: 616)
        GridExplainerPanelView.drawExplainer(buildGuideContent(overview: isOverviewMode), in: ctx, rect: guideRect)

        let cardRect = CGRect(x: 20, y: size.height - 96, width: size.width - 40, height: 78)
        drawGridHudFrame(in: ctx, rect: cardRect, cut: 12.0, fillAlpha: 0.90, drawBrackets: true, drawScanlines: false)
        let st = activeStation < stations.count ? stations[activeStation] : nil
        let kicker = isOverviewMode ? "OVERVIEW" : "STOP \(activeStation + 1) OF \(stations.count)  ·  \(st?.kicker ?? "")"
        (kicker as NSString).draw(at: CGPoint(x: cardRect.minX + 18, y: cardRect.minY + 10), withAttributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 9.2, weight: .medium), .foregroundColor: GridTokens.nsMid, .kern: 1.1,
        ])
        ((isOverviewMode ? overviewTitle : (st?.title ?? "")) as NSString).draw(at: CGPoint(x: cardRect.minX + 18, y: cardRect.minY + 28), withAttributes: [
            .font: NSFont.systemFont(ofSize: 17.0, weight: .bold), .foregroundColor: GridTokens.nsWhite,
        ])
    }

    // The video's overlay: scene labels in large type and one caption across the bottom.
    private func drawVideoOverlay(in ctx: CGContext, size: CGSize, cardAlpha: CGFloat, fade: CGFloat, progress: CGFloat) {
        RhLabelsOverlayView.scale = 1.55
        RhLabelsOverlayView.drawLabels(computeProjectedLabels(viewSize: size, video: true), in: ctx)
        RhLabelsOverlayView.scale = 1.0

        if cardAlpha > 0.01 {
            var step = "", kicker = "", title = overviewTitle, caption = overviewCaption
            var accent = GridTokens.nsAmber
            if videoCard == 2 {
                step = "SOURCES"
                caption = closingCaption
            } else if videoCard == 1 || activeStation >= stations.count {
                step = "THE RIEMANN HYPOTHESIS"
                kicker = "OPEN 1859 TO 2026"
            } else {
                let st = stations[activeStation]
                step = "\(activeStation + 1) OF \(stations.count)"
                kicker = st.kicker
                title = st.title
                caption = st.caption
                accent = nsColor(st.accent)
            }
            ctx.saveGState()
            ctx.setAlpha(cardAlpha)
            GridExplainerPanelView.drawVideoCaption(step: step, kicker: kicker, title: title, caption: caption, accent: accent, segments: stations.count, progress: progress, in: ctx, size: size)
            ctx.restoreGState()
        }
        if fade > 0.005 {
            ctx.setFillColor(NSColor(calibratedWhite: 0.0, alpha: fade).cgColor)
            ctx.fill(CGRect(origin: .zero, size: size))
        }
    }

    // MARK: - The recorded walkthrough's timeline

    private let videoIntro: Float = 6.0
    private let videoSlot: Float = 11.0
    private let videoGlide: Float = 2.2
    private let videoOutro: Float = 8.0
    var videoDuration: Float { videoIntro + Float(stations.count) * videoSlot + videoOutro }

    // The camera pose for station `idx`, drifting a few degrees over orbitU in [0, 1]. It looks
    // a little below the landmark so the landmark sits above the caption.
    private func videoStationPose(_ idx: Int, orbitU: Float) -> (eye: SIMD3<Float>, target: SIMD3<Float>) {
        let st = stations[idx]
        let off = st.eye - st.pos
        // How close the camera comes (1 = the app's own station view) and how far below the
        // landmark it aims, chosen per station so each one fills the frame above the caption.
        let zoom: [Float] = [0.70, 0.92, 0.80, 0.78, 0.95, 1.25, 0.88, 1.02]
        let aimDown: [Float] = [2.6, 1.4, 2.3, 2.8, 2.0, 1.5, 2.7, 2.3]
        let z = zoom[idx % zoom.count]
        let rXZ = hypot(off.x, off.z) * z
        let yaw = atan2(off.x, off.z) + (orbitU * orbitU * (3.0 - 2.0 * orbitU) - 0.5) * 0.30
        let e = st.pos + SIMD3<Float>(rXZ * sin(yaw), off.y * z + sin(orbitU * .pi) * 0.4, rXZ * cos(yaw))
        return (e, st.pos - SIMD3<Float>(0, aimDown[idx % aimDown.count], 0))
    }

    // Sets the camera, the active station and the animated curve for time `t` of the video.
    // Returns the caption's opacity, the fade to black, and progress through the stations.
    private func applyVideoFrame(_ t: Float) -> (cardAlpha: CGFloat, fade: CGFloat, progress: CGFloat) {
        func ss(_ a: Float, _ b: Float, _ x: Float) -> Float {
            let u = max(0.0, min(1.0, (x - a) / max(1e-5, b - a)))
            return u * u * (3.0 - 2.0 * u)
        }
        let n = stations.count
        let total = videoDuration
        let outroStart = videoIntro + Float(n) * videoSlot
        totalElapsedS = t
        flyTimeS = t
        selectedPick = nil

        let ovStart = SIMD3<Float>(-7.0, 33.5, 76.0), ovEnd = SIMD3<Float>(4.0, 31.5, 72.0)
        let ovFinal = SIMD3<Float>(0.0, 32.0, 74.0), ovTarget = SIMD3<Float>(0.0, -4.0, 0.0)
        var card: Float = 1.0
        var zerosShown = max(0, psiRows - 1)
        let fade = t < 0.7 ? 1.0 - ss(0.0, 0.7, t) : (t > total - 0.9 ? ss(total - 0.9, total, t) : 0.0)

        if t < videoIntro {
            eye = ovStart + (ovEnd - ovStart) * ss(0, videoIntro, t)
            target = ovTarget
            isOverviewMode = true
            activeStation = 0
            videoCard = 1
            card = ss(0.3, 1.0, t) * (1.0 - ss(videoIntro - 0.4, videoIntro, t))
        } else if t < outroStart {
            let local = t - videoIntro
            let idx = min(n - 1, Int(local / videoSlot))
            let stT = local - Float(idx) * videoSlot
            activeStation = idx
            isOverviewMode = false
            videoCard = 0
            if stT < videoGlide {
                let u = stT / videoGlide
                let s = u * u * (3.0 - 2.0 * u)
                let from: (eye: SIMD3<Float>, target: SIMD3<Float>) = idx == 0 ? (ovEnd, ovTarget) : videoStationPose(idx - 1, orbitU: 1.0)
                let to = videoStationPose(idx, orbitU: 0.0)
                eye = from.eye + (to.eye - from.eye) * s + SIMD3<Float>(0, sin(u * .pi) * (idx == 0 ? 2.0 : 1.3), 0)
                target = from.target + (to.target - from.target) * s
                card = ss(videoGlide * 0.45, videoGlide, stT)
            } else {
                let pose = videoStationPose(idx, orbitU: min(1.0, (stT - videoGlide) / (videoSlot - videoGlide)))
                eye = pose.eye
                target = pose.target
                card = 1.0 - ss(videoSlot - 0.35, videoSlot, stT)
            }
            if idx == 1 {
                zerosShown = psiZerosShown(phase: (stT - videoGlide) / (videoSlot - videoGlide - 1.2))
            }
        } else {
            let ot = t - outroStart
            let pull: Float = 3.0
            if ot < pull {
                let u = ot / pull
                let s = u * u * (3.0 - 2.0 * u)
                let from = videoStationPose(n - 1, orbitU: 1.0)
                eye = from.eye + (ovFinal - from.eye) * s + SIMD3<Float>(0, sin(u * .pi) * 3.0, 0)
                target = from.target + (ovTarget - from.target) * s
                isOverviewMode = u > 0.5
                activeStation = n - 1
                card = isOverviewMode ? ss(0.55, 0.95, u) : 0.0
            } else {
                let u = (ot - pull) / max(0.1, videoOutro - pull)
                eye = ovFinal + SIMD3<Float>(u * 2.5, u * 0.8, u * 1.5)
                target = ovTarget
                isOverviewMode = true
                activeStation = n - 1
            }
            videoCard = 2
        }
        refreshPsiCurve(zerosShown: zerosShown)
        let progress = max(0.0, min(1.0, (t - videoIntro) / (outroStart - videoIntro)))
        return (CGFloat(card), CGFloat(fade), CGFloat(progress))
    }

    // MARK: - Headless still frames (the full HDR, 4x MSAA and bloom pipeline)

    private func renderFrame(width: Int, height: Int, dpr: Float, overlayScale: CGFloat, overlay: (CGContext, CGSize) -> Void) -> CGImage? {
        guard let dev = device, let queue = commandQueue else { return nil }
        let texDesc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        texDesc.usage = [.renderTarget, .shaderRead]
        texDesc.storageMode = .shared
        guard let colorTex = dev.makeTexture(descriptor: texDesc), let cmd = queue.makeCommandBuffer() else { return nil }
        encodeFullGridFrame(cmd: cmd, destTexture: colorTex, width: width, height: height, dpr: dpr)
        cmd.commit()
        cmd.waitUntilCompleted()

        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        colorTex.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        let cs = CGColorSpaceCreateDeviceRGB()
        let bi = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: cs, bitmapInfo: bi.rawValue) else { return nil }
        NSGraphicsContext.saveGraphicsState()
        ctx.saveGState()
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: overlayScale, y: -overlayScale)
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        overlay(ctx, CGSize(width: CGFloat(width) / overlayScale, height: CGFloat(height) / overlayScale))
        ctx.restoreGState()
        NSGraphicsContext.restoreGraphicsState()
        return ctx.makeImage()
    }

    private func writePNG(_ img: CGImage?, to url: URL) -> Bool {
        guard let img = img, let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return false }
        CGImageDestinationAddImage(dest, img, nil)
        return CGImageDestinationFinalize(dest)
    }

    // A still of the app view: at `timeSeconds` of the flyover, or the overview.
    func renderOffscreenPNG(to url: URL, width: Int = 1440, height: Int = 900, timeSeconds: Float? = nil, overview: Bool = false) -> Bool {
        isOverviewMode = overview
        selectedPick = nil
        if overview {
            eye = SIMD3<Float>(0, 32.0, 74.0)
            target = SIMD3<Float>(0, 3.0, 0)
            activeStation = 0
        } else if let t = timeSeconds {
            flyTimeS = t
            totalElapsedS = t
            evaluateMetaverseSpline(at: t)
        }
        refreshPsiCurve(zerosShown: max(0, psiRows - 1))
        return writePNG(renderFrame(width: width, height: height, dpr: 1.25, overlayScale: 1.0) { ctx, size in
            self.drawAppOverlay(in: ctx, size: size)
        }, to: url)
    }

    // A still of the recorded walkthrough at `seconds` into it, exactly as the video shows it.
    func renderVideoStill(to url: URL, atSeconds seconds: Float, width: Int = 1920, height: Int = 1080) -> Bool {
        guard stations.count == 8 else { return false }
        let state = applyVideoFrame(seconds)
        let uiScale = CGFloat(width) / 1280.0
        return writePNG(renderFrame(width: width, height: height, dpr: Float(uiScale), overlayScale: uiScale) { ctx, size in
            self.drawVideoOverlay(in: ctx, size: size, cardAlpha: state.cardAlpha, fade: state.fade, progress: state.progress)
        }, to: url)
    }

    // MARK: - Walkthrough video recorder (Metal frames into AVAssetWriter H.264)

    func recordWalkthroughMP4(to url: URL, width: Int = 1920, height: Int = 1080, fps: Int = 30) -> Bool {
        guard let dev = device, let queue = commandQueue, stations.count == 8 else { return false }

        try? FileManager.default.removeItem(at: url)
        guard let writer = try? AVAssetWriter(outputURL: url, fileType: .mp4) else {
            fputs("rh3d record: failed to create AVAssetWriter\n", stderr)
            return false
        }
        writer.shouldOptimizeForNetworkUse = true
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: 10_000_000,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoMaxKeyFrameIntervalKey: fps * 2,
            ] as [String: Any],
        ]
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput.expectsMediaDataInRealTime = false
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: videoInput, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA),
            kCVPixelBufferWidthKey as String: width,
            kCVPixelBufferHeightKey as String: height,
            kCVPixelBufferCGImageCompatibilityKey as String: true,
            kCVPixelBufferCGBitmapContextCompatibilityKey as String: true,
        ])
        guard writer.canAdd(videoInput) else { return false }
        writer.add(videoInput)
        guard writer.startWriting() else {
            fputs("rh3d record: startWriting failed: \(writer.error?.localizedDescription ?? "unknown")\n", stderr)
            return false
        }
        writer.startSession(atSourceTime: .zero)

        let texDesc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        texDesc.usage = [.renderTarget, .shaderRead]
        texDesc.storageMode = .shared
        guard let colorTex = dev.makeTexture(descriptor: texDesc) else { return false }

        let totalDur = videoDuration
        let totalFrames = Int((totalDur * Float(fps)).rounded())
        let uiScale = CGFloat(width) / 1280.0
        let logicalSize = CGSize(width: CGFloat(width) / uiScale, height: CGFloat(height) / uiScale)
        let cs = CGColorSpaceCreateDeviceRGB()
        let bi = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        let prevShowLabels = showLabels
        showLabels = true

        let tStart = CACurrentMediaTime()
        var appended = 0
        for frameIdx in 0..<totalFrames {
            let state = applyVideoFrame(Float(frameIdx) / Float(fps))
            autoreleasepool {
                guard let cmd = queue.makeCommandBuffer() else { return }
                encodeFullGridFrame(cmd: cmd, destTexture: colorTex, width: width, height: height, dpr: Float(uiScale))
                cmd.commit()
                cmd.waitUntilCompleted()

                while !videoInput.isReadyForMoreMediaData { Thread.sleep(forTimeInterval: 0.002) }
                guard let pool = adaptor.pixelBufferPool else { return }
                var pbOpt: CVPixelBuffer?
                guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &pbOpt) == kCVReturnSuccess, let pb = pbOpt else { return }
                CVPixelBufferLockBaseAddress(pb, [])
                if let baseAddr = CVPixelBufferGetBaseAddress(pb) {
                    let bpr = CVPixelBufferGetBytesPerRow(pb)
                    colorTex.getBytes(baseAddr, bytesPerRow: bpr, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
                    if let ctx = CGContext(data: baseAddr, width: width, height: height, bitsPerComponent: 8, bytesPerRow: bpr, space: cs, bitmapInfo: bi.rawValue) {
                        NSGraphicsContext.saveGraphicsState()
                        ctx.saveGState()
                        ctx.translateBy(x: 0, y: CGFloat(height))
                        ctx.scaleBy(x: uiScale, y: -uiScale)
                        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
                        drawVideoOverlay(in: ctx, size: logicalSize, cardAlpha: state.cardAlpha, fade: state.fade, progress: state.progress)
                        ctx.restoreGState()
                        NSGraphicsContext.restoreGraphicsState()
                    }
                }
                CVPixelBufferUnlockBaseAddress(pb, [])
                if adaptor.append(pb, withPresentationTime: CMTime(value: CMTimeValue(frameIdx), timescale: CMTimeScale(fps))) { appended += 1 }
            }
            if frameIdx > 0 && frameIdx % (fps * 10) == 0 {
                print(String(format: "rh3d record: %d/%d frames (%.1fs elapsed)", frameIdx, totalFrames, CACurrentMediaTime() - tStart))
                fflush(stdout)
            }
        }
        showLabels = prevShowLabels

        videoInput.markAsFinished()
        let sem = DispatchSemaphore(value: 0)
        writer.finishWriting { sem.signal() }
        sem.wait()

        if writer.status == .completed && appended == totalFrames {
            print(String(format: "rh3d record: wrote %@ (%dx%d @ %dfps, %.1fs in %.1fs)", url.path, width, height, fps, totalDur, CACurrentMediaTime() - tStart))
            return true
        }
        fputs("rh3d record: \(appended)/\(totalFrames) frames, writer error: \(writer.error?.localizedDescription ?? "none")\n", stderr)
        return false
    }
}
