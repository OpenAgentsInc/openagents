// Firm reason: 6 (AppKit/Metal 3D Metaverse Flythrough viewport over CAMetalLayer driven by Metaverse Engine Coax flyover spline + rh-math.coax).
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

// MARK: - Metal Vertex / Instance / Uniform Structs

private struct Rh3dMeshVert {
    var pos: SIMD3<Float>
    var normal: SIMD3<Float>
}

private struct Rh3dTerrainVert {
    var pos: SIMD3<Float>
    var normal: SIMD3<Float>
    var color: SIMD4<Float>
}

private struct Rh3dLineVert {
    var pos: SIMD3<Float>
    var color: SIMD4<Float>
}

private struct Rh3dInstance {
    var model: simd_float4x4
    var color: SIMD4<Float>
    var emissive: Float
    var highlight: Float
    var pad0: Float = 0
    var pad1: Float = 0
}

private struct Rh3dUniforms {
    var viewProj: simd_float4x4
    var cameraPos: SIMD4<Float>
    var keyLightDir: SIMD4<Float>
    var fillLightDir: SIMD4<Float>
    var fogColor: SIMD4<Float>
    var timeParams: SIMD4<Float> // x = time_s, y = fogDensity
}

// MARK: - Metal Shaders (PBR-style Hemisphere + Key + Rim + Emissive + Depth Fog)

private let rh3dMetalShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct MeshVert {
    float3 pos;
    float3 normal;
};

struct TerrainVert {
    float3 pos;
    float3 normal;
    float4 color;
};

struct LineVert {
    float3 pos;
    float4 color;
};

struct Instance {
    float4x4 model;
    float4 color;
    float emissive;
    float highlight;
    float pad0;
    float pad1;
};

struct Uniforms {
    float4x4 viewProj;
    float4 cameraPos;
    float4 keyLightDir;
    float4 fillLightDir;
    float4 fogColor;
    float4 timeParams;
};

struct FragIn {
    float4 position [[position]];
    float3 worldPos;
    float3 worldNormal;
    float4 color;
    float emissive;
    float highlight;
};

vertex FragIn rh3d_mesh_vs(
    const device MeshVert *verts [[buffer(0)]],
    const device Instance *insts [[buffer(1)]],
    constant Uniforms &u [[buffer(2)]],
    uint vid [[vertex_id]],
    uint iid [[instance_id]]
) {
    MeshVert v = verts[vid];
    Instance inst = insts[iid];
    float4 wp = inst.model * float4(v.pos, 1.0);
    float3x3 nmat = float3x3(inst.model[0].xyz, inst.model[1].xyz, inst.model[2].xyz);
    FragIn o;
    o.position = u.viewProj * wp;
    o.worldPos = wp.xyz;
    o.worldNormal = normalize(nmat * v.normal);
    o.color = inst.color;
    o.emissive = inst.emissive;
    o.highlight = inst.highlight;
    return o;
}

vertex FragIn rh3d_terrain_vs(
    const device TerrainVert *verts [[buffer(0)]],
    constant Uniforms &u [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    TerrainVert v = verts[vid];
    FragIn o;
    o.position = u.viewProj * float4(v.pos, 1.0);
    o.worldPos = v.pos;
    o.worldNormal = normalize(v.normal);
    o.color = v.color;
    o.emissive = 0.22;
    o.highlight = 0.0;
    return o;
}

fragment float4 rh3d_mesh_fs(FragIn in [[stage_in]], constant Uniforms &u [[buffer(2)]]) {
    float3 N = normalize(in.worldNormal);
    float3 V = normalize(u.cameraPos.xyz - in.worldPos);
    float3 L1 = normalize(u.keyLightDir.xyz);
    float3 L2 = normalize(u.fillLightDir.xyz);

    float ndl1 = max(dot(N, L1), 0.0);
    float ndl2 = max(dot(N, L2), 0.0);
    float hemi = clamp(N.y * 0.5 + 0.5, 0.0, 1.0);
    float3 H = normalize(L1 + V);
    float spec = pow(max(dot(N, H), 0.0), 36.0) * 0.35;
    float fresnel = pow(1.0 - max(dot(N, V), 0.0), 3.0);

    float3 ambient = mix(float3(0.08, 0.10, 0.16), float3(0.24, 0.30, 0.42), hemi);
    float3 lit = in.color.rgb * (ambient + 0.78 * ndl1 + 0.32 * ndl2 + in.emissive)
                 + float3(0.75, 0.88, 1.0) * spec
                 + in.color.rgb * fresnel * (0.42 + in.emissive * 0.65);

    if (in.highlight > 0.01) {
        float pulse = 0.75 + 0.25 * sin(u.timeParams.x * 4.0);
        lit = mix(lit, float3(1.0, 0.96, 0.72), 0.35 * in.highlight * pulse);
    }

    float dist = length(u.cameraPos.xyz - in.worldPos);
    float fog = 1.0 - exp(-pow(dist * u.timeParams.y, 1.65));
    float3 finalRGB = mix(lit, u.fogColor.rgb, clamp(fog, 0.0, 0.88));
    return float4(finalRGB, in.color.a);
}

struct LineOut {
    float4 position [[position]];
    float4 color;
    float dist;
};

vertex LineOut rh3d_line_vs(
    const device LineVert *verts [[buffer(0)]],
    constant Uniforms &u [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    LineVert v = verts[vid];
    LineOut o;
    o.position = u.viewProj * float4(v.pos, 1.0);
    o.color = v.color;
    o.dist = length(u.cameraPos.xyz - v.pos);
    return o;
}

fragment float4 rh3d_line_fs(LineOut in [[stage_in]], constant Uniforms &u [[buffer(1)]]) {
    float fade = exp(-in.dist * u.timeParams.y * 0.95);
    return float4(in.color.rgb, in.color.a * clamp(fade, 0.08, 1.0));
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

// MARK: - 3D-to-2D Projected Callout Labels Overlay (Crisp Retina Typography + Leader Stems)

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
        let titleBannerFont = NSFont.systemFont(ofSize: 11.5, weight: .bold)
        let titlePartFont = NSFont.systemFont(ofSize: 10.5, weight: .semibold)
        let subFont = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .regular)

        for lbl in labels {
            ctx.saveGState()
            ctx.setAlpha(lbl.alpha)

            // Leader stem from exact 3D projected point up to bottom center of label pill
            let pillBottom = CGPoint(x: lbl.rect.midX, y: lbl.rect.maxY)
            ctx.setStrokeColor(lbl.color.withAlphaComponent(lbl.isBanner ? 0.82 : 0.65).cgColor)
            ctx.setLineWidth(lbl.isBanner ? 1.5 : 1.1)
            ctx.beginPath()
            ctx.move(to: lbl.anchorPt)
            ctx.addLine(to: pillBottom)
            ctx.strokePath()

            // Glowing anchor dot on the 3D object
            let dotR: CGFloat = lbl.isBanner ? 3.5 : 2.6
            ctx.setFillColor(lbl.color.cgColor)
            ctx.fillEllipse(in: CGRect(x: lbl.anchorPt.x - dotR, y: lbl.anchorPt.y - dotR, width: dotR * 2, height: dotR * 2))

            // Glass pill background
            let path = CGPath(roundedRect: lbl.rect, cornerWidth: 6, cornerHeight: 6, transform: nil)
            ctx.setFillColor(NSColor(red: 0.045, green: 0.055, blue: 0.088, alpha: lbl.isBanner ? 0.88 : 0.80).cgColor)
            ctx.addPath(path)
            ctx.fillPath()

            // Accent left strip + subtle border
            ctx.setStrokeColor(lbl.color.withAlphaComponent(lbl.isBanner ? 0.88 : 0.55).cgColor)
            ctx.setLineWidth(lbl.isBanner ? 1.3 : 1.0)
            ctx.addPath(path)
            ctx.strokePath()

            let accentBar = CGRect(x: lbl.rect.minX + 1.5, y: lbl.rect.minY + 3, width: 3.0, height: max(4, lbl.rect.height - 6))
            ctx.setFillColor(lbl.color.cgColor)
            ctx.fill(accentBar)

            // Text rendering (flipped coordinates)
            let tAttrs: [NSAttributedString.Key: Any] = [
                .font: lbl.isBanner ? titleBannerFont : titlePartFont,
                .foregroundColor: NSColor(white: 0.96, alpha: 1.0),
            ]
            (lbl.title as NSString).draw(at: CGPoint(x: lbl.rect.minX + 9, y: lbl.rect.minY + 4), withAttributes: tAttrs)

            if !lbl.subtext.isEmpty {
                let sAttrs: [NSAttributedString.Key: Any] = [
                    .font: subFont,
                    .foregroundColor: lbl.color.blended(withFraction: 0.35, of: .white) ?? lbl.color,
                ]
                (lbl.subtext as NSString).draw(at: CGPoint(x: lbl.rect.minX + 9, y: lbl.rect.minY + 18.5), withAttributes: sAttrs)
            }
            ctx.restoreGState()
        }
    }
}

// MARK: - Main RhFlythroughView (Metal 3D + Metaverse Engine Coax Spline + Story HUD)

final class RhFlythroughView: NSView {
    var onInspectModule: ((String) -> Void)?

    private let device: MTLDevice?
    private let commandQueue: MTLCommandQueue?
    private let metalHost = NSView()
    private var metalLayer: CAMetalLayer?
    private var depthTexture: MTLTexture?
    private var depthSize: (Int, Int) = (0, 0)

    private var opaqueMeshPipe: MTLRenderPipelineState?
    private var translucentMeshPipe: MTLRenderPipelineState?
    private var terrainPipe: MTLRenderPipelineState?
    private var linePipe: MTLRenderPipelineState?
    private var depthOpaque: MTLDepthStencilState?
    private var depthTranslucent: MTLDepthStencilState?

    // Shared primitive geometry buffers
    private var boxVB: MTLBuffer?
    private var boxVCount = 0
    private var cylVB: MTLBuffer?
    private var cylVCount = 0
    private var octVB: MTLBuffer?
    private var octVCount = 0

    // Static world buffers built from Coax + rh-atlas.json
    private var terrainVB: MTLBuffer?
    private var terrainVCount = 0
    private var gridLineVB: MTLBuffer?
    private var gridLineVCount = 0

    private var opaqueBoxes: [Rh3dInstance] = []
    private var opaqueCyls: [Rh3dInstance] = []
    private var opaqueOcts: [Rh3dInstance] = []
    private var transBoxes: [Rh3dInstance] = []
    private var transCyls: [Rh3dInstance] = []

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

    // HUD views
    private let labelsOverlay = RhLabelsOverlayView()
    private let topLeftBadge = NSTextField(labelWithString: "")
    private let playPauseBtn = NSButton(title: "Pause Flythrough", target: nil, action: nil)
    private let overviewBtn = NSButton(title: "Overview (All 8)", target: nil, action: nil)
    private let labelsBtn = NSButton(title: "Labels: On", target: nil, action: nil)
    private let inspectBtn = NSButton(title: "Inspect Lean Source →", target: nil, action: nil)

    private let storyCard = NSView()
    private let storyTitle = NSTextField(labelWithString: "")
    private let storySubtitle = NSTextField(labelWithString: "")
    private let storyBody = NSTextField(wrappingLabelWithString: "")
    private let prevBtn = NSButton(title: "← Prev", target: nil, action: nil)
    private let nextBtn = NSButton(title: "Next →", target: nil, action: nil)
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
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(red: 0.032, green: 0.042, blue: 0.068, alpha: 1.0).cgColor

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

    // MARK: - Metal Setup

    private func setupMetalPipelines() {
        guard let dev = device else { return }
        guard let lib = try? dev.makeLibrary(source: rh3dMetalShaderSource, options: nil) else { return }

        let opDesc = MTLRenderPipelineDescriptor()
        opDesc.vertexFunction = lib.makeFunction(name: "rh3d_mesh_vs")
        opDesc.fragmentFunction = lib.makeFunction(name: "rh3d_mesh_fs")
        opDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        opDesc.depthAttachmentPixelFormat = .depth32Float
        opaqueMeshPipe = try? dev.makeRenderPipelineState(descriptor: opDesc)

        let trDesc = MTLRenderPipelineDescriptor()
        trDesc.vertexFunction = lib.makeFunction(name: "rh3d_mesh_vs")
        trDesc.fragmentFunction = lib.makeFunction(name: "rh3d_mesh_fs")
        trDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        trDesc.colorAttachments[0].isBlendingEnabled = true
        trDesc.colorAttachments[0].rgbBlendOperation = .add
        trDesc.colorAttachments[0].alphaBlendOperation = .add
        trDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        trDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        trDesc.colorAttachments[0].sourceAlphaBlendFactor = .one
        trDesc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        trDesc.depthAttachmentPixelFormat = .depth32Float
        translucentMeshPipe = try? dev.makeRenderPipelineState(descriptor: trDesc)

        let terDesc = MTLRenderPipelineDescriptor()
        terDesc.vertexFunction = lib.makeFunction(name: "rh3d_terrain_vs")
        terDesc.fragmentFunction = lib.makeFunction(name: "rh3d_mesh_fs")
        terDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        terDesc.depthAttachmentPixelFormat = .depth32Float
        terrainPipe = try? dev.makeRenderPipelineState(descriptor: terDesc)

        let lnDesc = MTLRenderPipelineDescriptor()
        lnDesc.vertexFunction = lib.makeFunction(name: "rh3d_line_vs")
        lnDesc.fragmentFunction = lib.makeFunction(name: "rh3d_line_fs")
        lnDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        lnDesc.colorAttachments[0].isBlendingEnabled = true
        lnDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        lnDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        lnDesc.depthAttachmentPixelFormat = .depth32Float
        linePipe = try? dev.makeRenderPipelineState(descriptor: lnDesc)

        let dOp = MTLDepthStencilDescriptor()
        dOp.depthCompareFunction = .less
        dOp.isDepthWriteEnabled = true
        depthOpaque = dev.makeDepthStencilState(descriptor: dOp)

        let dTr = MTLDepthStencilDescriptor()
        dTr.depthCompareFunction = .less
        dTr.isDepthWriteEnabled = false
        depthTranslucent = dev.makeDepthStencilState(descriptor: dTr)
    }

    private func buildUnitPrimitives() {
        guard let dev = device else { return }

        // 1. Unit Box [-0.5, 0.5]^3
        var bVerts: [Rh3dMeshVert] = []
        let faces: [(SIMD3<Float>, [SIMD3<Float>])] = [
            (SIMD3(0, 0, 1),  [SIMD3(-0.5,-0.5, 0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3(-0.5, 0.5, 0.5)]),
            (SIMD3(0, 0,-1),  [SIMD3( 0.5,-0.5,-0.5), SIMD3(-0.5,-0.5,-0.5), SIMD3(-0.5, 0.5,-0.5), SIMD3( 0.5, 0.5,-0.5)]),
            (SIMD3(1, 0, 0),  [SIMD3( 0.5,-0.5, 0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3( 0.5, 0.5, 0.5)]),
            (SIMD3(-1, 0, 0), [SIMD3(-0.5,-0.5,-0.5), SIMD3(-0.5,-0.5, 0.5), SIMD3(-0.5, 0.5, 0.5), SIMD3(-0.5, 0.5,-0.5)]),
            (SIMD3(0, 1, 0),  [SIMD3(-0.5, 0.5, 0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3(-0.5, 0.5,-0.5)]),
            (SIMD3(0,-1, 0),  [SIMD3(-0.5,-0.5,-0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3(-0.5,-0.5, 0.5)]),
        ]
        for (n, q) in faces {
            bVerts.append(contentsOf: [
                Rh3dMeshVert(pos: q[0], normal: n), Rh3dMeshVert(pos: q[1], normal: n), Rh3dMeshVert(pos: q[2], normal: n),
                Rh3dMeshVert(pos: q[0], normal: n), Rh3dMeshVert(pos: q[2], normal: n), Rh3dMeshVert(pos: q[3], normal: n),
            ])
        }
        boxVCount = bVerts.count
        boxVB = dev.makeBuffer(bytes: bVerts, length: bVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)

        // 2. Unit Cylinder along Y (height 1, radius 0.5, 16 sides)
        var cVerts: [Rh3dMeshVert] = []
        let segs = 16
        for i in 0..<segs {
            let a0 = Float(i) / Float(segs) * .pi * 2
            let a1 = Float(i + 1) / Float(segs) * .pi * 2
            let x0 = cos(a0) * 0.5, z0 = sin(a0) * 0.5
            let x1 = cos(a1) * 0.5, z1 = sin(a1) * 0.5
            let n0 = SIMD3<Float>(cos(a0), 0, sin(a0))
            let n1 = SIMD3<Float>(cos(a1), 0, sin(a1))
            let p00 = SIMD3<Float>(x0, -0.5, z0), p01 = SIMD3<Float>(x0, 0.5, z0)
            let p10 = SIMD3<Float>(x1, -0.5, z1), p11 = SIMD3<Float>(x1, 0.5, z1)
            cVerts.append(contentsOf: [
                Rh3dMeshVert(pos: p00, normal: n0), Rh3dMeshVert(pos: p10, normal: n1), Rh3dMeshVert(pos: p11, normal: n1),
                Rh3dMeshVert(pos: p00, normal: n0), Rh3dMeshVert(pos: p11, normal: n1), Rh3dMeshVert(pos: p01, normal: n0),
                // Top & bottom caps
                Rh3dMeshVert(pos: SIMD3(0, 0.5, 0), normal: SIMD3(0, 1, 0)),
                Rh3dMeshVert(pos: p01, normal: SIMD3(0, 1, 0)),
                Rh3dMeshVert(pos: p11, normal: SIMD3(0, 1, 0)),
                Rh3dMeshVert(pos: SIMD3(0, -0.5, 0), normal: SIMD3(0, -1, 0)),
                Rh3dMeshVert(pos: p10, normal: SIMD3(0, -1, 0)),
                Rh3dMeshVert(pos: p00, normal: SIMD3(0, -1, 0)),
            ])
        }
        cylVCount = cVerts.count
        cylVB = dev.makeBuffer(bytes: cVerts, length: cVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)

        // 3. Unit Octahedron Crystal (glowing beacons & floating cores)
        var oVerts: [Rh3dMeshVert] = []
        let top = SIMD3<Float>(0, 0.5, 0), bot = SIMD3<Float>(0, -0.5, 0)
        let ring = [SIMD3<Float>(0.5, 0, 0), SIMD3<Float>(0, 0, 0.5), SIMD3<Float>(-0.5, 0, 0), SIMD3<Float>(0, 0, -0.5)]
        for i in 0..<4 {
            let a = ring[i], b = ring[(i + 1) % 4]
            let nUp = simd_normalize(simd_cross(b - top, a - top))
            oVerts.append(contentsOf: [
                Rh3dMeshVert(pos: top, normal: nUp), Rh3dMeshVert(pos: b, normal: nUp), Rh3dMeshVert(pos: a, normal: nUp),
            ])
            let nDn = simd_normalize(simd_cross(a - bot, b - bot))
            oVerts.append(contentsOf: [
                Rh3dMeshVert(pos: bot, normal: nDn), Rh3dMeshVert(pos: a, normal: nDn), Rh3dMeshVert(pos: b, normal: nDn),
            ])
        }
        octVCount = oVerts.count
        octVB = dev.makeBuffer(bytes: oVerts, length: oVerts.count * MemoryLayout<Rh3dMeshVert>.stride, options: .storageModeShared)
    }

    // MARK: - Build All 8 Real-Data 3D Landmarks from Coax & rh-atlas.json

    private func loadSceneFromCoaxAndAtlas() {
        let jsonStr = DesktopCore.callString(65536) { oa_dc_rh3d_scene_json($0) }
        guard let data = jsonStr.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        if let durMs = obj["duration_ms"] as? Int {
            durationS = Float(durMs) / 1000.0
        }

        let accents: [SIMD4<Float>] = [
            SIMD4(0.18, 0.88, 0.72, 1.0), // 1. Zeta Terrain (Emerald/Cyan)
            SIMD4(0.98, 0.74, 0.22, 1.0), // 2. Robin 5040 (Gold)
            SIMD4(0.20, 0.84, 0.98, 1.0), // 3. 2/3 Barrier (Electric Cyan)
            SIMD4(0.58, 0.46, 0.98, 1.0), // 4. Theta Bridge (Violet/Sapphire)
            SIMD4(0.24, 0.90, 0.56, 1.0), // 5. 175k City (Emerald)
            SIMD4(0.98, 0.64, 0.24, 1.0), // 6. Behrend/Weil (Warm Amber)
            SIMD4(0.32, 0.82, 0.96, 1.0), // 7. 4 Kernels (Ice Cyan)
            SIMD4(1.00, 0.84, 0.30, 1.0), // 8. Summit Q.E.D. (Radiant Gold)
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

    private func buildWorldGeometry(zeros: [Float], robin: [[String: Any]], theta: [[String: Any]], zNs: Int, zNt: Int, zHeights: [Float]) {
        opaqueBoxes.removeAll()
        opaqueCyls.removeAll()
        opaqueOcts.removeAll()
        transBoxes.removeAll()
        transCyls.removeAll()
        labelAnchors.removeAll()
        pickables.removeAll()

        // ---- 0. Floor Horizon Grid + Highway Spine ----
        var lines: [Rh3dLineVert] = []
        let minorCol = SIMD4<Float>(0.22, 0.30, 0.45, 0.24)
        let majorCol = SIMD4<Float>(0.34, 0.48, 0.72, 0.42)
        for ix in stride(from: -64, through: 64, by: 2) {
            let x = Float(ix)
            let c = (ix % 12 == 0) ? majorCol : minorCol
            lines.append(Rh3dLineVert(pos: SIMD3(x, 0.0, -34), color: c))
            lines.append(Rh3dLineVert(pos: SIMD3(x, 0.0, 34), color: c))
        }
        for iz in stride(from: -34, through: 34, by: 2) {
            let z = Float(iz)
            let c = (iz == 0) ? SIMD4<Float>(0.95, 0.78, 0.28, 0.55) : ((iz % 10 == 0) ? majorCol : minorCol)
            lines.append(Rh3dLineVert(pos: SIMD3(-64, 0.0, z), color: c))
            lines.append(Rh3dLineVert(pos: SIMD3(64, 0.0, z), color: c))
        }
        gridLineVCount = lines.count
        if let dev = device, !lines.isEmpty {
            gridLineVB = dev.makeBuffer(bytes: lines, length: lines.count * MemoryLayout<Rh3dLineVert>.stride, options: .storageModeShared)
        }

        // Glowing golden proof highway spine + station plinths + inter-station arching energy conduits
        for (i, st) in stations.enumerated() {
            let basePos = SIMD3<Float>(st.pos.x, 0.08, st.pos.z)
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: basePos, size: SIMD3(9.2, 0.16, 9.2)), color: SIMD4(0.08, 0.11, 0.18, 1.0), emissive: 0.12, highlight: 0))
            // Glowing accent rim rails around plinth
            let rimCol = st.accent
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(st.pos.x, 0.17, st.pos.z + 4.6), size: SIMD3(9.4, 0.08, 0.10)), color: rimCol, emissive: 0.72, highlight: 0))
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(st.pos.x, 0.17, st.pos.z - 4.6), size: SIMD3(9.4, 0.08, 0.10)), color: rimCol, emissive: 0.72, highlight: 0))

            // Station banner floating high above landmark (staggered heights so all 8 banners display cleanly in Overview)
            let bannerY: Float = (i == 7) ? 11.4 : ((i == 4) ? 9.6 : ((i % 2 == 0) ? 8.8 : 6.8))
            addLabel(SIMD3(st.pos.x, bannerY, st.pos.z), st.shortName.uppercased(), sub: st.moduleName, color: st.accent, station: i, banner: true, maxDist: 140.0)
            pickables.append(Rh3dPickable(center: SIMD3(st.pos.x, 3.0, st.pos.z), radius: 5.0, title: st.title, detail: st.plainEnglish, moduleName: st.moduleName, stationIndex: i))

            // Glowing arched conduit to next station
            if i + 1 < stations.count {
                let nxt = stations[i + 1]
                let p0 = SIMD3<Float>(st.pos.x + 4.5, 0.28, 0)
                let p1 = SIMD3<Float>(nxt.pos.x - 4.5, 0.28, 0)
                opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: p0, to: p1, radius: 0.09), color: SIMD4(0.98, 0.80, 0.30, 1.0), emissive: 0.85, highlight: 0))
                // Elevated luminous arc showing proof flow from i -> i+1
                let midX = (st.pos.x + nxt.pos.x) * 0.5
                let a0 = SIMD3<Float>(st.pos.x + 2.5, 2.2, -3.5)
                let am = SIMD3<Float>(midX, 4.4, -4.2)
                let a1 = SIMD3<Float>(nxt.pos.x - 2.5, 2.2, -3.5)
                opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: a0, to: am, radius: 0.055), color: st.accent, emissive: 0.90, highlight: 0))
                opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: am, to: a1, radius: 0.055), color: nxt.accent, emissive: 0.90, highlight: 0))
            }
        }

        // ---- STATION 1 (X = -42): REAL 3D RIEMANN ZETA TERRAIN + CRITICAL LINE WALL + 10 ZERO BEACONS ----
        let x1: Float = -42.0
        if zNs >= 2 && zNt >= 2 && zHeights.count >= zNs * zNt, let dev = device {
            let widthX: Float = 8.4   // sigma in [0.12, 0.88] mapped to X
            let depthZ: Float = 8.4   // t in [4, 50] mapped to Z
            func terrainPos(_ i: Int, _ j: Int) -> SIMD3<Float> {
                let u = Float(i) / Float(zNs - 1)
                let v = Float(j) / Float(zNt - 1)
                let h = zHeights[j * zNs + i]
                return SIMD3<Float>(x1 + (u - 0.5) * widthX, 0.22 + h * 0.88, (v - 0.5) * depthZ)
            }
            func terrainColor(_ i: Int, _ j: Int) -> SIMD4<Float> {
                let u = Float(i) / Float(zNs - 1)
                let h = zHeights[j * zNs + i]
                let onCrit = abs(u - 0.5) < 0.055
                if h < 0.35 && onCrit {
                    return SIMD4(1.0, 0.88, 0.28, 1.0) // Glowing gold right inside a critical-line zero!
                }
                let tNorm = min(1.0, h / 3.2)
                return SIMD4(
                    0.10 + 0.58 * tNorm,
                    0.78 - 0.42 * tNorm,
                    0.82 - 0.18 * tNorm + (onCrit ? 0.15 : 0.0),
                    1.0
                )
            }
            var tVerts: [Rh3dTerrainVert] = []
            for j in 0..<(zNt - 1) {
                for i in 0..<(zNs - 1) {
                    let p00 = terrainPos(i, j), p10 = terrainPos(i + 1, j)
                    let p01 = terrainPos(i, j + 1), p11 = terrainPos(i + 1, j + 1)
                    let c00 = terrainColor(i, j), c10 = terrainColor(i + 1, j)
                    let c01 = terrainColor(i, j + 1), c11 = terrainColor(i + 1, j + 1)
                    let nA = simd_normalize(simd_cross(p01 - p00, p10 - p00))
                    let nB = simd_normalize(simd_cross(p01 - p10, p11 - p10))
                    tVerts.append(contentsOf: [
                        Rh3dTerrainVert(pos: p00, normal: nA, color: c00),
                        Rh3dTerrainVert(pos: p01, normal: nA, color: c01),
                        Rh3dTerrainVert(pos: p10, normal: nA, color: c10),
                        Rh3dTerrainVert(pos: p10, normal: nB, color: c10),
                        Rh3dTerrainVert(pos: p01, normal: nB, color: c01),
                        Rh3dTerrainVert(pos: p11, normal: nB, color: c11),
                    ])
                }
            }
            terrainVCount = tVerts.count
            terrainVB = dev.makeBuffer(bytes: tVerts, length: tVerts.count * MemoryLayout<Rh3dTerrainVert>.stride, options: .storageModeShared)
        }
        // Translucent Golden Critical Line Glass Wall at Re(s) = 1/2 (X = x1)
        transBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x1, 2.7, 0), size: SIMD3(0.08, 5.0, 8.5)), color: SIMD4(1.0, 0.84, 0.26, 0.24), emissive: 0.65, highlight: 0))
        opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x1, 5.2, 0), size: SIMD3(0.14, 0.08, 8.6)), color: SIMD4(1.0, 0.86, 0.30, 1.0), emissive: 0.95, highlight: 0))
        addLabel(SIMD3(x1, 5.3, -3.9), "Critical Wall Re(s) = 1/2", sub: "ALL zeros must sit on this plane", color: SIMD4(1.0, 0.86, 0.30, 1.0), station: 0)
        addLabel(SIMD3(x1 - 3.2, 2.6, 2.6), "|ζ(σ + it)| Terrain", sub: "Height = zeta magnitude", color: SIMD4(0.18, 0.88, 0.72, 1.0), station: 0)

        // 10 Real Riemann Zero Beacons along the Critical Line
        for (zi, gamma) in zeros.enumerated() {
            let v = max(0.0, min(1.0, (gamma - 4.0) / 46.0))
            let zPos = (v - 0.5) * 8.4
            let pillarH: Float = 3.2 + (zi == 0 ? 0.9 : 0.0)
            opaqueCyls.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x1, 0.2 + pillarH * 0.5, zPos), size: SIMD3(0.16, pillarH, 0.16)), color: SIMD4(1.0, 0.86, 0.28, 1.0), emissive: 0.90, highlight: 0))
            opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x1, 0.3 + pillarH, zPos), size: SIMD3(0.42, 0.56, 0.42)), color: SIMD4(0.32, 0.96, 0.98, 1.0), emissive: 0.95, highlight: 0))
            if zi == 0 || zi == 2 || zi == 7 {
                addLabel(SIMD3(x1, 0.65 + pillarH, zPos), String(format: "Zero γ%d = %.3f", zi + 1, gamma), sub: "ζ(1/2 + iγ) = 0", color: SIMD4(0.32, 0.96, 0.98, 1.0), station: 0)
            }
        }

        // ---- STATION 2 (X = -30): ROBIN'S CRITERION (1984) & THE 5040 GATE ----
        let x2: Float = -30.0
        let ceilingY: Float = 3.40
        // Translucent Emerald Ceiling Plane at Ratio = 1.0
        transBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x2, ceilingY, 0), size: SIMD3(8.4, 0.08, 6.2)), color: SIMD4(0.18, 0.90, 0.56, 0.26), emissive: 0.55, highlight: 0))
        addLabel(SIMD3(x2 + 3.4, ceilingY + 0.15, -2.6), "Robin Ceiling (1.0)", sub: "RH ⇔ ALL n > 5040 stay below", color: SIMD4(0.18, 0.90, 0.56, 1.0), station: 1)

        for (ri, r) in robin.enumerated() {
            let nVal = (r["n"] as? Int) ?? 0
            let ratio = Float((r["ratio_mm"] as? Int) ?? 900) / 1000.0
            let u = Float(ri) / Float(max(1, robin.count - 1))
            let bx = x2 + (u - 0.5) * 7.4
            let barH = max(0.5, min(5.4, (ratio - 0.68) * 10.0))
            let is5040 = (nVal == 5040)
            let exceeds = ratio >= 1.0
            let col: SIMD4<Float> = is5040
                ? SIMD4(1.0, 0.84, 0.20, 1.0)
                : (exceeds ? SIMD4(0.96, 0.42, 0.28, 1.0) : SIMD4(0.18, 0.86, 0.56, 1.0))
            let bw: Float = is5040 ? 0.58 : 0.40
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(bx, 0.2 + barH * 0.5, 0), size: SIMD3(bw, barH, bw)), color: col, emissive: is5040 ? 0.85 : 0.35, highlight: 0))
            if is5040 {
                opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(bx, barH + 0.72, 0), size: SIMD3(0.68, 0.92, 0.68)), color: SIMD4(1.0, 0.88, 0.28, 1.0), emissive: 0.95, highlight: 0))
                addLabel(SIMD3(bx, barH + 1.25, 0), "n = 5040 (LAST EXCEPTION)", sub: String(format: "ratio = %.4f > 1.0", ratio), color: SIMD4(1.0, 0.86, 0.26, 1.0), station: 1)
            } else if nVal == 12 {
                addLabel(SIMD3(bx, barH + 0.35, 0.6), "n = 12..2520", sub: "Early exceptions (> 1.0)", color: SIMD4(0.96, 0.48, 0.32, 1.0), station: 1)
            } else if nVal == 55440 {
                addLabel(SIMD3(bx, barH + 0.35, 0.6), "n = 55,440 (Safe)", sub: String(format: "ratio = %.4f < 1.0", ratio), color: SIMD4(0.22, 0.90, 0.60, 1.0), station: 1)
            }
        }

        // ---- STATION 3 (X = -18): SMASHING THE 2/3 BARRIER ----
        let x3: Float = -18.0
        let barrierY: Float = 3.8 // height representing 2/3 = 0.6667
        // Massive Crimson 2/3 Barrier Wall with a breached gap in the middle for Alpoge's cyan staircase
        transBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x3, barrierY, -2.5), size: SIMD3(8.2, 0.32, 2.2)), color: SIMD4(0.96, 0.22, 0.28, 0.42), emissive: 0.65, highlight: 0))
        transBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x3, barrierY, 2.5), size: SIMD3(8.2, 0.32, 2.2)), color: SIMD4(0.96, 0.22, 0.28, 0.42), emissive: 0.65, highlight: 0))
        opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x3 - 3.9, barrierY * 0.5, -2.5), size: SIMD3(0.30, barrierY, 0.30)), color: SIMD4(0.88, 0.22, 0.28, 1.0), emissive: 0.45, highlight: 0))
        opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x3 + 3.9, barrierY * 0.5, -2.5), size: SIMD3(0.30, barrierY, 0.30)), color: SIMD4(0.88, 0.22, 0.28, 1.0), emissive: 0.45, highlight: 0))
        addLabel(SIMD3(x3 - 2.6, barrierY + 0.35, -2.6), "Old 2/3 Classical Wall", sub: "100-year density ceiling", color: SIMD4(0.98, 0.32, 0.36, 1.0), station: 2)

        for (ti, th) in theta.enumerated() {
            let cls = Float((th["classical_mm"] as? Int) ?? 500) / 1000.0
            let alp = Float((th["alpoge_mm"] as? Int) ?? 700) / 1000.0
            let u = Float(ti) / Float(max(1, theta.count - 1))
            let sx = x3 + (u - 0.5) * 6.8
            let clsH = cls * 5.7
            let alpH = alp * 5.7
            // Red stalled classical columns behind (Z = -1.6)
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(sx, 0.2 + clsH * 0.5, -1.5), size: SIMD3(0.46, clsH, 0.46)), color: SIMD4(0.76, 0.24, 0.30, 1.0), emissive: 0.28, highlight: 0))
            // Blazing cyan Alpoge sqrt(X) staircase punching right up & over 2/3 (Z = +0.5)
            let crossed = alp > 0.6667
            let stepCol: SIMD4<Float> = crossed ? SIMD4(0.16, 0.92, 0.98, 1.0) : SIMD4(0.28, 0.68, 0.96, 1.0)
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(sx, 0.2 + alpH * 0.5, 0.5), size: SIMD3(0.58, alpH, 0.72)), color: stepCol, emissive: crossed ? 0.65 : 0.35, highlight: 0))
            if ti == 7 {
                opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(sx, alpH + 0.7, 0.5), size: SIMD3(0.58, 0.78, 0.58)), color: SIMD4(0.24, 0.98, 0.88, 1.0), emissive: 0.95, highlight: 0))
                addLabel(SIMD3(sx, alpH + 1.15, 0.5), "Alpöge √X Staircase", sub: String(format: "Stage 8 hits %.2f (past 2/3!)", alp), color: SIMD4(0.20, 0.94, 0.98, 1.0), station: 2)
            }
            if ti == 3 {
                addLabel(SIMD3(sx, clsH + 0.3, -1.5), "Classical Stalls", sub: String(format: "Trapped at %.2f < 0.67", cls), color: SIMD4(0.92, 0.40, 0.42, 1.0), station: 2)
            }
        }

        // ---- STATION 4 (X = -6): THETA-LIFT & FORMAL DEGREE CATHEDRAL ----
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
        let core4 = SIMD3<Float>(x4, 3.6, 0)
        opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: core4, size: SIMD3(1.5, 2.2, 1.5)), color: SIMD4(0.68, 0.46, 1.0, 1.0), emissive: 0.95, highlight: 0))
        addLabel(SIMD3(x4, 5.1, 0), "Theta-Lift Core", sub: "Primes ↔ Hyperbolic Geometry", color: SIMD4(0.76, 0.58, 1.0, 1.0), station: 3)
        for (pi, (pname, psub)) in pillars4.enumerated() {
            let ang = Float(pi) / Float(pillars4.count) * .pi * 2.0 - .pi * 0.25
            let px = x4 + cos(ang) * 3.4
            let pz = sin(ang) * 3.4
            let ph: Float = 4.2 + Float(pi % 3) * 0.75
            let col = (pi % 2 == 0) ? SIMD4<Float>(0.42, 0.52, 0.98, 1.0) : SIMD4<Float>(0.64, 0.40, 0.96, 1.0)
            opaqueCyls.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(px, 0.2 + ph * 0.5, pz), size: SIMD3(0.56, ph, 0.56)), color: col, emissive: 0.42, highlight: 0))
            // Golden capital ring + energy beam into central core
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(px, ph + 0.3, pz), size: SIMD3(0.74, 0.18, 0.74)), color: SIMD4(0.98, 0.82, 0.34, 1.0), emissive: 0.75, highlight: 0))
            opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: SIMD3(px, ph + 0.25, pz), to: core4, radius: 0.04), color: SIMD4(0.45, 0.82, 1.0, 1.0), emissive: 0.90, highlight: 0))
            if pi == 0 || pi == 1 || pi == 4 || pi == 7 {
                addLabel(SIMD3(px, ph + 0.6, pz), pname, sub: psub, color: col, station: 3)
            }
        }

        // ---- STATION 5 (X = +6): REAL 175,182-FILE PROOF CITY (HYDRATED FROM rh-atlas.json!) ----
        let x5: Float = 6.0
        buildRealProofCity(centerX: x5)

        // ---- STATION 6 (X = +18): BEHREND SETS CRYSTAL LATTICE + WEIL BOUND ARCHES ----
        let x6: Float = 18.0
        for ix in -2...2 {
            for iy in 0...4 {
                for iz in -2...2 {
                    let fx = Float(ix), fy = Float(iy) - 2.0, fz = Float(iz)
                    let r2 = fx * fx + fy * fy + fz * fz
                    let onBehrendShell = (r2 >= 3.8 && r2 <= 5.2) // Spherical shell free of 3-term APs!
                    let p = SIMD3<Float>(x6 + fx * 1.15, 1.0 + Float(iy) * 0.95, fz * 1.15)
                    if onBehrendShell {
                        opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: p, size: SIMD3(0.42, 0.42, 0.42)), color: SIMD4(1.0, 0.72, 0.20, 1.0), emissive: 0.85, highlight: 0))
                    } else if (ix + iy + iz) % 2 == 0 {
                        transBoxes.append(Rh3dInstance(model: rh3dModelBox(center: p, size: SIMD3(0.24, 0.24, 0.24)), color: SIMD4(0.32, 0.38, 0.52, 0.26), emissive: 0.15, highlight: 0))
                    }
                }
            }
        }
        // 3 sweeping golden Weil Bound arches overhead
        for az in [-2.2 as Float, 0.0, 2.2] {
            var prev: SIMD3<Float>? = nil
            for step in 0...10 {
                let a = Float(step) / 10.0 * .pi
                let pt = SIMD3<Float>(x6 + cos(a) * 4.0, 1.6 + sin(a) * 4.6, az)
                if let p0 = prev {
                    opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: p0, to: pt, radius: 0.07), color: SIMD4(0.98, 0.78, 0.24, 1.0), emissive: 0.82, highlight: 0))
                }
                prev = pt
            }
        }
        addLabel(SIMD3(x6, 3.6, 2.5), "Behrend Sphere Shell", sub: "No 3-term arithmetic progressions", color: SIMD4(1.0, 0.72, 0.22, 1.0), station: 5)
        addLabel(SIMD3(x6, 6.2, 0), "Weil Finite-Field Arches", sub: "Error terms bounded by √q", color: SIMD4(1.0, 0.84, 0.34, 1.0), station: 5)

        // ---- STATION 7 (X = +30): MULTI-KERNEL VERIFICATION FORTRESS ----
        let x7: Float = 30.0
        let core7 = SIMD3<Float>(x7, 3.2, 0)
        opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: core7, size: SIMD3(1.6, 2.4, 1.6)), color: SIMD4(0.18, 0.94, 0.62, 1.0), emissive: 0.95, highlight: 0))
        addLabel(SIMD3(x7, 4.7, 0), "3 Standard Axioms Only", sub: "propext · Quot.sound · Classical.choice", color: SIMD4(0.20, 0.96, 0.66, 1.0), station: 6)

        let kernels: [(String, String, Float, Float, SIMD4<Float>)] = [
            ("lake check --paranoid", "Lean v4.36.0 official kernel", -2.8, -2.8, SIMD4(0.24, 0.86, 0.98, 1.0)),
            ("leanchecker", "Independent C++ kernel",  2.8, -2.8, SIMD4(0.32, 0.92, 0.68, 1.0)),
            ("nanoda", "Independent Rust kernel",  2.8,  2.8, SIMD4(0.68, 0.56, 1.00, 1.0)),
            ("con-leche + con-ron", "Independent multi-kernel audit", -2.8,  2.8, SIMD4(0.98, 0.76, 0.28, 1.0)),
        ]
        for (kname, ksub, dx, dz, kcol) in kernels {
            let kh: Float = 5.4
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x7 + dx, 0.2 + kh * 0.5, dz), size: SIMD3(1.1, kh, 1.1)), color: kcol, emissive: 0.48, highlight: 0))
            opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: SIMD3(x7 + dx, kh - 0.3, dz), to: core7, radius: 0.055), color: SIMD4(0.22, 0.96, 0.66, 1.0), emissive: 0.95, highlight: 0))
            addLabel(SIMD3(x7 + dx, kh + 0.55, dz), kname, sub: ksub, color: kcol, station: 6)
        }
        // 6 small amber beacons on outer side = exact 6 audited metaprogramming files
        for mi in 0..<6 {
            let ang = -0.75 + Float(mi) * 0.30
            let mx = x7 + sin(ang) * 4.3
            let mz = 3.9 + cos(ang) * 0.5
            opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(mx, 1.15, mz), size: SIMD3(0.34, 0.48, 0.34)), color: SIMD4(0.98, 0.68, 0.20, 1.0), emissive: 0.90, highlight: 0))
        }
        addLabel(SIMD3(x7, 1.75, 4.4), "6 Audited Metaprog Sites", sub: "BuildAid.D1..D5 + CutCptCopy", color: SIMD4(0.98, 0.68, 0.20, 1.0), station: 6)

        // ---- STATION 8 (X = +42): THE SUMMIT — Main.riemannHypothesis (Q.E.D.) ----
        let x8: Float = 42.0
        // Stepped Golden-Emerald Cathedral Pyramid + Towering Spire
        let tiers: [(Float, Float, Float, SIMD4<Float>)] = [
            (6.8, 0.6, 0.3, SIMD4(0.14, 0.24, 0.34, 1.0)),
            (5.2, 0.7, 0.95, SIMD4(0.16, 0.44, 0.48, 1.0)),
            (3.6, 0.9, 1.75, SIMD4(0.18, 0.68, 0.56, 1.0)),
            (2.2, 1.2, 2.8, SIMD4(0.24, 0.86, 0.60, 1.0)),
        ]
        for (tw, th, ty, tc) in tiers {
            opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x8, ty, 0), size: SIMD3(tw, th, tw)), color: tc, emissive: 0.38, highlight: 0))
        }
        // Central radiant golden spire rising to Y = 10.6
        opaqueCyls.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x8, 6.2, 0), size: SIMD3(0.85, 6.4, 0.85)), color: SIMD4(1.0, 0.84, 0.28, 1.0), emissive: 0.88, highlight: 0))
        opaqueOcts.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x8, 10.1, 0), size: SIMD3(1.5, 2.2, 1.5)), color: SIMD4(1.0, 0.92, 0.46, 1.0), emissive: 1.0, highlight: 0))
        // Halo ring of 12 luminous segments around crown
        for hi in 0..<12 {
            let a0 = Float(hi) / 12.0 * .pi * 2
            let a1 = Float(hi + 1) / 12.0 * .pi * 2
            let p0 = SIMD3<Float>(x8 + cos(a0) * 2.5, 8.6, sin(a0) * 2.5)
            let p1 = SIMD3<Float>(x8 + cos(a1) * 2.5, 8.6, sin(a1) * 2.5)
            opaqueCyls.append(Rh3dInstance(model: rh3dModelBeam(from: p0, to: p1, radius: 0.075), color: SIMD4(0.30, 0.98, 0.72, 1.0), emissive: 0.95, highlight: 0))
        }
        addLabel(SIMD3(x8, 10.2, 0), "theorem riemannHypothesis", sub: ":= rhOfAll  (src/Main.lean)", color: SIMD4(1.0, 0.90, 0.36, 1.0), station: 7)
        addLabel(SIMD3(x8 + 2.8, 3.0, 2.8), "813,182 Verified Theorems", sub: "17.37M lines · MIT Nov 21, 2026", color: SIMD4(0.24, 0.96, 0.66, 1.0), station: 7)
    }

    private func buildRealProofCity(centerX x5: Float) {
        let atlasURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/index/rh-atlas.json")
        let libColors: [SIMD4<Float>] = [
            SIMD4(1.00, 0.86, 0.28, 1.0), // 0: Root/Main (Gold)
            SIMD4(0.18, 0.88, 0.56, 1.0), // 1: Formalism (Emerald)
            SIMD4(0.28, 0.68, 0.98, 1.0), // 2: Supplements (Sapphire)
            SIMD4(0.68, 0.48, 0.98, 1.0), // 3: Kernels (Violet)
            SIMD4(0.98, 0.64, 0.24, 1.0), // 4: Bridge (Amber)
            SIMD4(0.20, 0.88, 0.92, 1.0), // 5: Bessel (Cyan)
            SIMD4(0.92, 0.46, 0.72, 1.0), // 6: Theta (Magenta)
            SIMD4(0.96, 0.76, 0.26, 1.0), // 7: Behrend (Warm Gold)
            SIMD4(0.46, 0.88, 0.42, 1.0), // 8: Weil (Lime)
            SIMD4(0.72, 0.78, 0.88, 1.0), // 9: Other (Silver)
        ]

        var builtFromAtlas = false
        if let data = try? Data(contentsOf: atlasURL),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let clusters = obj["clusters"] as? [[String: Any]], !clusters.isEmpty {
            builtFromAtlas = true
            // Sort top 42 real clusters by module count and arrange as a glowing 3D skyline
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
                let col = libColors[abs(libIdx) % libColors.count]

                let gx = idx % cols
                let gz = idx / cols
                let bx = x5 + (Float(gx) - Float(cols - 1) * 0.5) * 1.15
                let bz = (Float(gz) - Float(rows - 1) * 0.5) * 1.25
                let bh = max(0.8, min(7.6, Float(log10(Double(max(10, lines)))) * 1.18 - 0.6))
                let bw = max(0.42, min(0.92, 0.36 + Float(log10(Double(max(1, mods)))) * 0.14))

                opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(bx, 0.2 + bh * 0.5, bz), size: SIMD3(bw, bh, bw)), color: col, emissive: idx < 4 ? 0.58 : 0.30, highlight: 0))
                // Glowing skyscraper crown beacon for top clusters
                if idx < 5 {
                    opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(bx, 0.25 + bh, bz), size: SIMD3(bw * 1.06, 0.12, bw * 1.06)), color: SIMD4(1.0, 0.95, 0.72, 1.0), emissive: 0.92, highlight: 0))
                    addLabel(SIMD3(bx, bh + 0.55, bz), name, sub: "\(mods) files · \(lines / 1000)k lines", color: col, station: 4)
                    pickables.append(Rh3dPickable(center: SIMD3(bx, bh * 0.5, bz), radius: 1.0, title: name, detail: "\(mods) Lean 4 files (\(lines) lines) in real rh-atlas.json", moduleName: (cl["hub"] as? String) ?? name, stationIndex: 4))
                }
            }
        }

        if !builtFromAtlas {
            // Fallback representative skyline if rh-atlas.json hasn't been built yet
            let fallback: [(String, Float, Float, Float, SIMD4<Float>)] = [
                ("Formalism (130k files)", -1.6, 0.0, 7.2, libColors[1]),
                ("Supplements (44.5k files)", 1.6, 0.0, 5.8, libColors[2]),
                ("Root / Main", 0.0, -2.2, 3.6, libColors[0]),
            ]
            for (nm, dx, dz, h, c) in fallback {
                opaqueBoxes.append(Rh3dInstance(model: rh3dModelBox(center: SIMD3(x5 + dx, 0.2 + h * 0.5, dz), size: SIMD3(1.6, h, 1.6)), color: c, emissive: 0.45, highlight: 0))
                addLabel(SIMD3(x5 + dx, h + 0.6, dz), nm, sub: "Lean 4 Library", color: c, station: 4)
            }
        }
    }

    // MARK: - Views & HUD Setup

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

        // Top-left title & controls hint
        topLeftBadge.translatesAutoresizingMaskIntoConstraints = false
        topLeftBadge.stringValue = "METAVERSE ENGINE 3D · Drag Orbit · Scroll Zoom · WASD Fly · Click Any Landmark · Space Play/Pause"
        topLeftBadge.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        topLeftBadge.textColor = NSColor(white: 0.86, alpha: 1.0)
        topLeftBadge.wantsLayer = true
        topLeftBadge.layer?.cornerRadius = 6
        topLeftBadge.layer?.backgroundColor = NSColor(red: 0.04, green: 0.05, blue: 0.08, alpha: 0.78).cgColor
        topLeftBadge.layer?.borderWidth = 1
        topLeftBadge.layer?.borderColor = NSColor(white: 1.0, alpha: 0.14).cgColor
        addSubview(topLeftBadge)

        for (btn, act) in [
            (playPauseBtn, #selector(togglePlayPause)),
            (overviewBtn, #selector(goOverview)),
            (labelsBtn, #selector(toggleLabels)),
            (inspectBtn, #selector(openSourceClicked)),
        ] {
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.bezelStyle = .rounded
            btn.controlSize = .small
            btn.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
            btn.target = self
            btn.action = act
            addSubview(btn)
        }
        playPauseBtn.contentTintColor = NSColor(calibratedRed: 0.24, green: 0.92, blue: 0.62, alpha: 1.0)
        inspectBtn.contentTintColor = NSColor(calibratedRed: 0.34, green: 0.82, blue: 0.98, alpha: 1.0)

        // Bottom Cinematic Story Bar
        storyCard.translatesAutoresizingMaskIntoConstraints = false
        storyCard.wantsLayer = true
        storyCard.layer?.cornerRadius = 10
        storyCard.layer?.backgroundColor = NSColor(red: 0.045, green: 0.055, blue: 0.088, alpha: 0.90).cgColor
        storyCard.layer?.borderWidth = 1
        storyCard.layer?.borderColor = NSColor(white: 1.0, alpha: 0.16).cgColor
        addSubview(storyCard)

        storyTitle.translatesAutoresizingMaskIntoConstraints = false
        storyTitle.font = NSFont.systemFont(ofSize: 14, weight: .bold)
        storyTitle.textColor = NSColor(white: 0.98, alpha: 1.0)
        storyCard.addSubview(storyTitle)

        storySubtitle.translatesAutoresizingMaskIntoConstraints = false
        storySubtitle.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        storySubtitle.textColor = NSColor(calibratedRed: 0.36, green: 0.90, blue: 0.78, alpha: 1.0)
        storyCard.addSubview(storySubtitle)

        storyBody.translatesAutoresizingMaskIntoConstraints = false
        storyBody.font = NSFont.systemFont(ofSize: 12, weight: .regular)
        storyBody.textColor = NSColor(white: 0.86, alpha: 1.0)
        storyBody.maximumNumberOfLines = 2
        storyCard.addSubview(storyBody)

        for (btn, act) in [(prevBtn, #selector(prevStationClicked)), (nextBtn, #selector(nextStationClicked))] {
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.bezelStyle = .rounded
            btn.controlSize = .small
            btn.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
            btn.target = self
            btn.action = act
            storyCard.addSubview(btn)
        }

        progressTrack.translatesAutoresizingMaskIntoConstraints = false
        progressTrack.wantsLayer = true
        progressTrack.layer?.cornerRadius = 2
        progressTrack.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.12).cgColor
        storyCard.addSubview(progressTrack)

        progressFill.translatesAutoresizingMaskIntoConstraints = false
        progressFill.wantsLayer = true
        progressFill.layer?.cornerRadius = 2
        progressFill.layer?.backgroundColor = NSColor(calibratedRed: 0.24, green: 0.90, blue: 0.62, alpha: 0.95).cgColor
        progressTrack.addSubview(progressFill)
        progressFillWidth = progressFill.widthAnchor.constraint(equalToConstant: 20)

        stationStack.translatesAutoresizingMaskIntoConstraints = false
        stationStack.orientation = .horizontal
        stationStack.spacing = 4
        stationStack.distribution = .fillEqually
        storyCard.addSubview(stationStack)

        stationButtons.removeAll()
        for (i, st) in stations.enumerated() {
            let b = NSButton(title: st.shortName, target: self, action: #selector(stationPillClicked(_:)))
            b.tag = i
            b.bezelStyle = .rounded
            b.controlSize = .mini
            b.font = NSFont.systemFont(ofSize: 10, weight: .medium)
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

            topLeftBadge.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            topLeftBadge.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),

            inspectBtn.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            inspectBtn.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            labelsBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            labelsBtn.trailingAnchor.constraint(equalTo: inspectBtn.leadingAnchor, constant: -6),
            overviewBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            overviewBtn.trailingAnchor.constraint(equalTo: labelsBtn.leadingAnchor, constant: -6),
            playPauseBtn.centerYAnchor.constraint(equalTo: inspectBtn.centerYAnchor),
            playPauseBtn.trailingAnchor.constraint(equalTo: overviewBtn.leadingAnchor, constant: -6),

            storyCard.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            storyCard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            storyCard.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
            storyCard.heightAnchor.constraint(equalToConstant: 124),

            storyTitle.topAnchor.constraint(equalTo: storyCard.topAnchor, constant: 10),
            storyTitle.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 14),
            storyTitle.trailingAnchor.constraint(equalTo: prevBtn.leadingAnchor, constant: -10),

            nextBtn.topAnchor.constraint(equalTo: storyCard.topAnchor, constant: 8),
            nextBtn.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -12),
            prevBtn.centerYAnchor.constraint(equalTo: nextBtn.centerYAnchor),
            prevBtn.trailingAnchor.constraint(equalTo: nextBtn.leadingAnchor, constant: -6),

            storySubtitle.topAnchor.constraint(equalTo: storyTitle.bottomAnchor, constant: 2),
            storySubtitle.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 14),
            storySubtitle.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -14),

            storyBody.topAnchor.constraint(equalTo: storySubtitle.bottomAnchor, constant: 4),
            storyBody.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 14),
            storyBody.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -14),

            progressTrack.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 14),
            progressTrack.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -14),
            progressTrack.bottomAnchor.constraint(equalTo: stationStack.topAnchor, constant: -6),
            progressTrack.heightAnchor.constraint(equalToConstant: 3),

            progressFill.leadingAnchor.constraint(equalTo: progressTrack.leadingAnchor),
            progressFill.topAnchor.constraint(equalTo: progressTrack.topAnchor),
            progressFill.bottomAnchor.constraint(equalTo: progressTrack.bottomAnchor),
            progressFillWidth!,

            stationStack.leadingAnchor.constraint(equalTo: storyCard.leadingAnchor, constant: 12),
            stationStack.trailingAnchor.constraint(equalTo: storyCard.trailingAnchor, constant: -12),
            stationStack.bottomAnchor.constraint(equalTo: storyCard.bottomAnchor, constant: -8),
            stationStack.heightAnchor.constraint(equalToConstant: 22),
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
        // Nearest station by X coordinate when below panoramic height
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
            storyTitle.stringValue = pick.title
            storySubtitle.stringValue = "Selected 3D Landmark · Module: \(pick.moduleName)"
            storyBody.stringValue = pick.detail
        } else if activeStation >= 0 && activeStation < stations.count {
            let st = stations[activeStation]
            storyTitle.stringValue = st.title
            storySubtitle.stringValue = st.subtitle
            storyBody.stringValue = st.plainEnglish
        }
        playPauseBtn.title = flyPlaying ? "Pause Flythrough" : "Resume Flythrough"
        labelsBtn.title = showLabels ? "Labels: On" : "Labels: Off"
        for (i, b) in stationButtons.enumerated() {
            b.contentTintColor = (i == activeStation) ? NSColor(calibratedRed: 0.24, green: 0.94, blue: 0.64, alpha: 1.0) : nil
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
        storyTitle.stringValue = "FULL PROOF OVERVIEW — All 8 Landmarks (175,182 Lean 4 Files)"
        storySubtitle.stringValue = "Left-to-Right Journey: Zeta Terrain → Robin 5040 → 2/3 Wall → Theta Bridge → 175k City → Behrend/Weil → 4 Kernels → Q.E.D."
        storyBody.stringValue = "Click any station pill below or any 3D glowing banner to swoop in closely, or click Resume Flythrough for the Metaverse Engine cinematic spline tour."
        playPauseBtn.title = "Resume Flythrough"
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

    // MARK: - Pointer & Keyboard Navigation (Orbit / Pan / Zoom / WASD Free-Fly)

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        dragLast = convert(event.locationInWindow, from: nil)
        if event.modifierFlags.contains(.option) {
            isPanDragging = true
        } else {
            isOrbitDragging = true
        }
        // Single quick click picking check happens on mouseUp if minimal drag
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
            // Check if click hit a 3D pickable
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
        if event.keyCode == 49 { // Space -> play/pause
            togglePlayPause()
            return
        }
        if event.keyCode == 15 { // R -> overview
            goOverview()
            return
        }
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
        var yaw = atan2(offset.x, offset.z) - dx * 0.0075
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
        if pressedKeys.contains(13) || pressedKeys.contains(126) { move += f * speed }       // W / Up
        if pressedKeys.contains(1)  || pressedKeys.contains(125) { move -= f * speed }       // S / Down
        if pressedKeys.contains(0)  || pressedKeys.contains(123) { move -= right * speed }   // A / Left
        if pressedKeys.contains(2)  || pressedKeys.contains(124) { move += right * speed }   // D / Right
        if pressedKeys.contains(14) { move += up * speed }                                   // E up
        if pressedKeys.contains(12) { move -= up * speed }                                   // Q down
        eye += move
        target += move
    }

    private func raycastPick(at viewPt: CGPoint) -> Rh3dPickable? {
        let w = Float(max(1, bounds.width))
        let h = Float(max(1, bounds.height))
        let vp = rh3dPerspective(fovY: 0.82, aspect: w / h, nearZ: 0.2, farZ: 240.0) * rh3dLookAt(eye: eye, target: target)
        let yTop = h - Float(viewPt.y) // NSView default bottom-left -> top-left screen coords
        var best: Rh3dPickable? = nil
        var bestDist: Float = 48.0
        for p in pickables {
            let clip = vp * SIMD4<Float>(p.center, 1.0)
            guard clip.w > 0.1 else { continue }
            let ndc = SIMD3<Float>(clip.x, clip.y, clip.z) / clip.w
            let sx = (ndc.x * 0.5 + 0.5) * w
            let sy = (0.5 - ndc.y * 0.5) * h
            let d = hypot(sx - Float(viewPt.x), sy - yTop)
            if d < bestDist {
                bestDist = d
                best = p
            }
        }
        return best
    }

    // MARK: - 60 FPS Render Loop & Label Projection

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

    private func ensureDepthTexture(width: Int, height: Int) -> MTLTexture? {
        guard let dev = device, width > 0, height > 0 else { return nil }
        if depthSize == (width, height), let tex = depthTexture { return tex }
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
        d.usage = .renderTarget
        d.storageMode = .private
        depthTexture = dev.makeTexture(descriptor: d)
        depthSize = (width, height)
        return depthTexture
    }

    private func encodeWorld(into enc: MTLRenderCommandEncoder, width: Int, height: Int) {
        guard let dev = device,
              let opPipe = opaqueMeshPipe,
              let trPipe = translucentMeshPipe,
              let dOp = depthOpaque,
              let dTr = depthTranslucent else { return }

        let aspect = Float(max(1, width)) / Float(max(1, height))
        let vp = rh3dPerspective(fovY: 0.82, aspect: aspect, nearZ: 0.2, farZ: 240.0) * rh3dLookAt(eye: eye, target: target)
        var uniforms = Rh3dUniforms(
            viewProj: vp,
            cameraPos: SIMD4<Float>(eye, 1.0),
            keyLightDir: SIMD4<Float>(simd_normalize(SIMD3<Float>(0.45, 0.82, 0.55)), 0),
            fillLightDir: SIMD4<Float>(simd_normalize(SIMD3<Float>(-0.65, 0.35, -0.45)), 0),
            fogColor: SIMD4<Float>(0.032, 0.042, 0.068, 1.0),
            timeParams: SIMD4<Float>(totalElapsedS, 0.0085, 0, 0)
        )

        // 1. Floor grid lines
        if let lPipe = linePipe, let lb = gridLineVB, gridLineVCount > 0 {
            enc.setRenderPipelineState(lPipe)
            enc.setDepthStencilState(dTr)
            enc.setVertexBuffer(lb, offset: 0, index: 0)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
            enc.drawPrimitives(type: .line, vertexStart: 0, vertexCount: gridLineVCount)
        }

        // 2. Real 3D Zeta terrain heightfield mesh
        if let tPipe = terrainPipe, let tb = terrainVB, terrainVCount > 0 {
            enc.setRenderPipelineState(tPipe)
            enc.setDepthStencilState(dOp)
            enc.setVertexBuffer(tb, offset: 0, index: 0)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 1)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 2)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: terrainVCount)
        }

        func drawInstanced(_ insts: [Rh3dInstance], vb: MTLBuffer?, vCount: Int) {
            guard !insts.isEmpty, let vb, vCount > 0 else { return }
            guard let ib = dev.makeBuffer(bytes: insts, length: insts.count * MemoryLayout<Rh3dInstance>.stride, options: .storageModeShared) else { return }
            enc.setVertexBuffer(vb, offset: 0, index: 0)
            enc.setVertexBuffer(ib, offset: 0, index: 1)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 2)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<Rh3dUniforms>.stride, index: 2)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: vCount, instanceCount: insts.count)
        }

        // 3. Opaque 3D architecture (plinths, towers, pillars, crystals, conduits)
        enc.setRenderPipelineState(opPipe)
        enc.setDepthStencilState(dOp)
        drawInstanced(opaqueBoxes, vb: boxVB, vCount: boxVCount)
        drawInstanced(opaqueCyls, vb: cylVB, vCount: cylVCount)
        drawInstanced(opaqueOcts, vb: octVB, vCount: octVCount)

        // 4. Translucent glass walls (Critical Wall Re(s)=1/2, Robin 1.0 ceiling, 2/3 crimson wall)
        enc.setRenderPipelineState(trPipe)
        enc.setDepthStencilState(dTr)
        drawInstanced(transBoxes, vb: boxVB, vCount: boxVCount)
        drawInstanced(transCyls, vb: cylVB, vCount: cylVCount)
    }

    private func renderMetalFrame() {
        guard let ml = metalLayer,
              let queue = commandQueue,
              ml.drawableSize.width >= 2, ml.drawableSize.height >= 2,
              let drawable = ml.nextDrawable() else { return }
        let w = drawable.texture.width
        let h = drawable.texture.height
        guard let depthTex = ensureDepthTexture(width: w, height: h),
              let cmd = queue.makeCommandBuffer() else { return }

        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = drawable.texture
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.032, green: 0.042, blue: 0.068, alpha: 1.0)
        rp.depthAttachment.texture = depthTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0

        if let enc = cmd.makeRenderCommandEncoder(descriptor: rp) {
            encodeWorld(into: enc, width: w, height: h)
            enc.endEncoding()
        }
        cmd.present(drawable)
        cmd.commit()
    }

    private func computeProjectedLabels(viewSize: CGSize) -> [ProjectedLabel] {
        guard showLabels, viewSize.width > 10, viewSize.height > 10 else { return [] }
        let w = Float(viewSize.width)
        let h = Float(viewSize.height)
        let vp = rh3dPerspective(fovY: 0.82, aspect: w / h, nearZ: 0.2, farZ: 240.0) * rh3dLookAt(eye: eye, target: target)

        let titleBannerFont = NSFont.systemFont(ofSize: 11.5, weight: .bold)
        let titlePartFont = NSFont.systemFont(ofSize: 10.5, weight: .semibold)
        let subFont = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .regular)

        var candidates: [(dist: Float, priority: Int, label: ProjectedLabel)] = []
        for anc in labelAnchors {
            let dist = simd_length(eye - anc.worldPos)
            if dist > anc.maxDist { continue }
            let clip = vp * SIMD4<Float>(anc.worldPos, 1.0)
            guard clip.w > 0.35 else { continue }
            let ndc = SIMD3<Float>(clip.x, clip.y, clip.z) / clip.w
            if ndc.x < -0.96 || ndc.x > 0.96 || ndc.y < -0.78 || ndc.y > 0.92 { continue }

            let sx = CGFloat((ndc.x * 0.5 + 0.5) * w)
            let sy = CGFloat((0.5 - ndc.y * 0.5) * h)
            // Keep clear of top toolbar (48pt) and bottom story bar (144pt)
            if sy < 52 || sy > viewSize.height - 148 { continue }

            let tw = (anc.text as NSString).size(withAttributes: [.font: anc.isStationBanner ? titleBannerFont : titlePartFont]).width
            let sw = anc.subtext.isEmpty ? 0 : (anc.subtext as NSString).size(withAttributes: [.font: subFont]).width
            let pw = max(tw, sw) + 18
            let ph: CGFloat = anc.subtext.isEmpty ? 22 : 34
            let stemH: CGFloat = anc.isStationBanner ? 22 : 18
            let rx = max(12, min(viewSize.width - pw - 12, sx - pw * 0.5))
            let ry = max(48, sy - stemH - ph)
            let alpha: CGFloat = anc.isStationBanner
                ? CGFloat(max(0.55, min(1.0, 1.15 - dist / 120.0)))
                : CGFloat(max(0.0, min(0.98, (anc.maxDist - dist) / 7.5)))
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

        // Priority first (station banners > active station parts > neighbor parts), then nearest camera
        candidates.sort { a, b in
            if a.priority != b.priority { return a.priority < b.priority }
            return a.dist < b.dist
        }

        // Greedy screen-space overlap avoidance so labels stay crystal clear
        var placed: [ProjectedLabel] = []
        for c in candidates {
            let padded = c.label.rect.insetBy(dx: -4, dy: -3)
            if placed.contains(where: { $0.rect.intersects(padded) }) { continue }
            placed.append(c.label)
            if placed.count >= 16 { break }
        }
        return placed
    }

    private func updateProjectedLabels() {
        labelsOverlay.labels = computeProjectedLabels(viewSize: bounds.size)
        labelsOverlay.needsDisplay = true
    }

    // MARK: - Headless Offscreen Metal PNG Capture (for visual verification without opening any window)

    func renderOffscreenPNG(to url: URL, width: Int = 1440, height: Int = 900, timeSeconds: Float? = nil, overview: Bool = false) -> Bool {
        guard let dev = device, let queue = commandQueue else { return false }
        if overview {
            eye = SIMD3<Float>(0, 32.0, 74.0)
            target = SIMD3<Float>(0, 3.0, 0)
            activeStation = 0
        } else if let t = timeSeconds {
            flyTimeS = t
            evaluateMetaverseSpline(at: t)
        }

        let texDesc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        texDesc.usage = [.renderTarget, .shaderRead]
        texDesc.storageMode = .shared
        guard let colorTex = dev.makeTexture(descriptor: texDesc),
              let depthTex = ensureDepthTexture(width: width, height: height),
              let cmd = queue.makeCommandBuffer() else { return false }

        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = colorTex
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.032, green: 0.042, blue: 0.068, alpha: 1.0)
        rp.depthAttachment.texture = depthTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0

        if let enc = cmd.makeRenderCommandEncoder(descriptor: rp) {
            encodeWorld(into: enc, width: width, height: height)
            enc.endEncoding()
        }
        cmd.commit()
        cmd.waitUntilCompleted()

        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        colorTex.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)

        let cs = CGColorSpaceCreateDeviceRGB()
        let bi = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: cs, bitmapInfo: bi.rawValue) else { return false }

        // Draw crisp 3D-projected labels + bottom story card onto the offscreen image (flipped coords)
        NSGraphicsContext.saveGraphicsState()
        ctx.saveGState()
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: 1, y: -1)
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)

        let viewSize = CGSize(width: width, height: height)
        let proj = computeProjectedLabels(viewSize: viewSize)
        RhLabelsOverlayView.drawLabels(proj, in: ctx)

        // Render bottom story card banner in offscreen shot too
        let cardRect = CGRect(x: 20, y: CGFloat(height) - 122, width: CGFloat(width) - 40, height: 104)
        let cpath = CGPath(roundedRect: cardRect, cornerWidth: 10, cornerHeight: 10, transform: nil)
        ctx.setFillColor(NSColor(red: 0.045, green: 0.055, blue: 0.088, alpha: 0.92).cgColor)
        ctx.addPath(cpath); ctx.fillPath()
        ctx.setStrokeColor(NSColor(white: 1.0, alpha: 0.18).cgColor)
        ctx.setLineWidth(1.2); ctx.addPath(cpath); ctx.strokePath()

        let titleStr = overview ? "FULL PROOF OVERVIEW — All 8 Landmarks (175,182 Lean 4 Files)" : (activeStation < stations.count ? stations[activeStation].title : "")
        let subStr = overview ? "Left-to-Right: Zeta Terrain → Robin 5040 → 2/3 Wall → Theta Bridge → 175k City → Behrend/Weil → 4 Kernels → Summit Q.E.D." : (activeStation < stations.count ? stations[activeStation].subtitle : "")
        let bodyStr = overview ? "Powered by OpenAgents Metaverse Engine Catmull-Rom flyover spline (metaverse-engine/cam/flyover.coax) + real Coax math (rh-math.coax) + rh-atlas.json." : (activeStation < stations.count ? stations[activeStation].plainEnglish : "")

        (titleStr as NSString).draw(at: CGPoint(x: cardRect.minX + 16, y: cardRect.minY + 12), withAttributes: [
            .font: NSFont.systemFont(ofSize: 15, weight: .bold), .foregroundColor: NSColor.white,
        ])
        (subStr as NSString).draw(at: CGPoint(x: cardRect.minX + 16, y: cardRect.minY + 34), withAttributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 11, weight: .medium), .foregroundColor: NSColor(calibratedRed: 0.36, green: 0.90, blue: 0.78, alpha: 1.0),
        ])
        (bodyStr as NSString).draw(in: CGRect(x: cardRect.minX + 16, y: cardRect.minY + 54, width: cardRect.width - 32, height: 42), withAttributes: [
            .font: NSFont.systemFont(ofSize: 12.5, weight: .regular), .foregroundColor: NSColor(white: 0.88, alpha: 1.0),
        ])

        ctx.restoreGState()
        NSGraphicsContext.restoreGraphicsState()

        guard let cgImg = ctx.makeImage(),
              let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return false }
        CGImageDestinationAddImage(dest, cgImg, nil)
        return CGImageDestinationFinalize(dest)
    }
}
