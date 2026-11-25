// Firm reason: 6 (AppKit/Metal 3D scene viewport over CAMetalLayer for PLE benchmark scenes).
// Reusable 3D PLE Scene visualizer (GitHub #13792, Deliverable 3):
// - PleSceneMetalView: CAMetalLayer-backed Metal renderer for ple-scene.v1.json boxes, cylinders,
//   and clash markers with directional shading, depth buffer, semantic layer colors, orbit/pan/zoom,
//   camera presets (Iso, Plan, Overhead, Under-Floor), and raycast picking.
// - PleScenePanelView: Reusable container with HUD overlays (run summary, camera presets,
//   layer visibility toggles, and selected-element inspector).
import AppKit
import Metal
import ImageIO
import UniformTypeIdentifiers
import QuartzCore
import simd

// MARK: - Camera Presets & Math

enum PleCameraPreset: String, CaseIterable {
    case iso = "Iso"
    case plan = "Plan"
    case overhead = "Overhead"
    case underFloor = "Under-Floor"
}

private struct PleCamera {
    var target: SIMD3<Float> = SIMD3<Float>(0, 0, 1.5)
    var distance: Float = 25.0
    var azimuth: Float = .pi * 0.25      // radians around Z
    var elevation: Float = 0.55          // radians above XY plane
    var fovY: Float = 45.0 * .pi / 180.0
    var nearZ: Float = 0.1
    var farZ: Float = 400.0

    var eye: SIMD3<Float> {
        let ce = cos(elevation)
        let offset = SIMD3<Float>(cos(azimuth) * ce, sin(azimuth) * ce, sin(elevation)) * distance
        return target + offset
    }

    mutating func frame(bounds: PleSceneBounds, preset: PleCameraPreset) {
        let c = bounds.center
        let ext = bounds.extent
        let radius = max(1.0, simd_length(ext) * 0.5)
        nearZ = max(0.05, radius * 0.005)
        farZ = max(250.0, radius * 24.0)
        switch preset {
        case .iso:
            target = c
            azimuth = .pi * 0.25
            elevation = 0.54
            distance = radius * 2.15
        case .plan:
            target = c
            azimuth = -.pi * 0.5
            elevation = 1.555
            distance = max(ext.x, ext.y) * 1.18
        case .overhead:
            target = SIMD3<Float>(c.x, c.y, bounds.minVec.z + ext.z * 0.72)
            azimuth = .pi * 0.18
            elevation = 0.88
            distance = radius * 1.65
        case .underFloor:
            target = SIMD3<Float>(c.x, c.y, bounds.minVec.z + min(0.6, ext.z * 0.18))
            azimuth = -.pi * 0.35
            elevation = 0.14
            distance = radius * 1.55
        }
    }

    func viewMatrix() -> simd_float4x4 {
        let e = eye
        let f = simd_normalize(target - e)
        let worldUp: SIMD3<Float> = abs(f.z) > 0.995 ? SIMD3<Float>(0, 1, 0) : SIMD3<Float>(0, 0, 1)
        let r = simd_normalize(simd_cross(f, worldUp))
        let u = simd_cross(r, f)
        let col0 = SIMD4<Float>(r.x, u.x, -f.x, 0)
        let col1 = SIMD4<Float>(r.y, u.y, -f.y, 0)
        let col2 = SIMD4<Float>(r.z, u.z, -f.z, 0)
        let col3 = SIMD4<Float>(-simd_dot(r, e), -simd_dot(u, e), simd_dot(f, e), 1)
        return simd_float4x4(col0, col1, col2, col3)
    }

    func projMatrix(aspect: Float) -> simd_float4x4 {
        let ys = 1.0 / tan(fovY * 0.5)
        let xs = ys / max(0.01, aspect)
        let zs = farZ / (nearZ - farZ)
        return simd_float4x4(
            SIMD4<Float>(xs, 0, 0, 0),
            SIMD4<Float>(0, ys, 0, 0),
            SIMD4<Float>(0, 0, zs, -1),
            SIMD4<Float>(0, 0, nearZ * zs, 0)
        )
    }

    func rightAndUp() -> (SIMD3<Float>, SIMD3<Float>) {
        let f = simd_normalize(target - eye)
        let worldUp: SIMD3<Float> = abs(f.z) > 0.995 ? SIMD3<Float>(0, 1, 0) : SIMD3<Float>(0, 0, 1)
        let r = simd_normalize(simd_cross(f, worldUp))
        let u = simd_cross(r, f)
        return (r, u)
    }
}

private func pleModelMatrix(center: SIMD3<Float>, size: SIMD3<Float>, rotZ: Float, axis: Int = 2) -> simd_float4x4 {
    let c = cos(rotZ), s = sin(rotZ)
    let m = simd_float4x4(
        SIMD4<Float>(c * size.x, s * size.x, 0, 0),
        SIMD4<Float>(-s * size.y, c * size.y, 0, 0),
        SIMD4<Float>(0, 0, size.z, 0),
        SIMD4<Float>(center.x, center.y, center.z, 1)
    )
    // The unit cylinder stands on z. A cylinder along x or y is turned first so its z lies on that
    // axis (ple_axis_out in metaverse-engine/import/ple-scene.coax): x takes (a, b, c) to (c, a, b),
    // y to (b, c, a). A box is the same box turned this way, so only cylinders pass an axis.
    switch axis {
    case 0:
        return m * simd_float4x4(SIMD4<Float>(0, 1, 0, 0), SIMD4<Float>(0, 0, 1, 0), SIMD4<Float>(1, 0, 0, 0), SIMD4<Float>(0, 0, 0, 1))
    case 1:
        return m * simd_float4x4(SIMD4<Float>(0, 0, 1, 0), SIMD4<Float>(1, 0, 0, 0), SIMD4<Float>(0, 1, 0, 0), SIMD4<Float>(0, 0, 0, 1))
    default:
        return m
    }
}

// MARK: - GPU Vertex / Instance / Uniform Layouts

private struct PleVertex {
    var pos: SIMD3<Float>
    var normal: SIMD3<Float>
}

private struct PleInstance {
    var model: simd_float4x4
    var color: SIMD4<Float>
    var highlight: Float
    var pad0: Float = 0
    var pad1: Float = 0
    var pad2: Float = 0
}

private struct PleUniforms {
    var viewProj: simd_float4x4
    var cameraPos: SIMD4<Float>
    var keyLightDir: SIMD4<Float>
    var fillLightDir: SIMD4<Float>
    var clipPlane: SIMD4<Float> // x: maxZ, y: enabled (1 or 0), z/w: reserved
}

private struct PleGridVertex {
    var pos: SIMD3<Float>
    var color: SIMD4<Float>
}

// MARK: - Inline Metal Shaders

private let pleSceneMetalShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct PleVertex {
    float3 pos;
    float3 normal;
};

struct PleInstance {
    float4x4 model;
    float4 color;
    float highlight;
    float pad0;
    float pad1;
    float pad2;
};

struct PleUniforms {
    float4x4 viewProj;
    float4 cameraPos;
    float4 keyLightDir;
    float4 fillLightDir;
    float4 clipPlane;
};

struct MeshOut {
    float4 position [[position]];
    float3 worldPos;
    float3 worldNormal;
    float4 color;
    float highlight;
};

vertex MeshOut ple_mesh_vs(
    const device PleVertex *verts [[buffer(0)]],
    const device PleInstance *insts [[buffer(1)]],
    constant PleUniforms &u [[buffer(2)]],
    uint vid [[vertex_id]],
    uint iid [[instance_id]]
) {
    PleVertex v = verts[vid];
    PleInstance inst = insts[iid];
    float4 world = inst.model * float4(float3(v.pos), 1.0);
    // Normal matrix for orthogonal rotation + non-uniform scale: inverse-transpose of upper 3x3
    float3x3 m3 = float3x3(inst.model[0].xyz, inst.model[1].xyz, inst.model[2].xyz);
    float3 sx = float3(length(m3[0]), length(m3[1]), length(m3[2]));
    float3 nLocal = float3(v.normal) / max(sx, float3(1e-4));
    float3x3 r3 = float3x3(m3[0] / max(sx.x, 1e-4), m3[1] / max(sx.y, 1e-4), m3[2] / max(sx.z, 1e-4));
    float3 nWorld = normalize(r3 * nLocal);

    MeshOut o;
    o.position = u.viewProj * world;
    o.worldPos = world.xyz;
    o.worldNormal = nWorld;
    o.color = inst.color;
    o.highlight = inst.highlight;
    return o;
}

fragment float4 ple_mesh_fs(MeshOut in [[stage_in]], constant PleUniforms &u [[buffer(2)]]) {
    if (u.clipPlane.y > 0.5 && in.worldPos.z > u.clipPlane.x) {
        discard_fragment();
    }
    float3 N = normalize(in.worldNormal);
    float3 L1 = normalize(u.keyLightDir.xyz);
    float3 L2 = normalize(u.fillLightDir.xyz);
    float3 V = normalize(u.cameraPos.xyz - in.worldPos);

    float ndl1 = max(dot(N, L1), 0.0);
    float ndl2 = max(dot(N, L2), 0.0);
    float hemi = 0.5 + 0.5 * N.z;
    float3 ambient = mix(float3(0.18, 0.20, 0.25), float3(0.34, 0.38, 0.44), hemi);
    float3 diffuse = ndl1 * float3(0.86, 0.85, 0.82) + ndl2 * float3(0.28, 0.34, 0.42);

    float3 H = normalize(L1 + V);
    float spec = pow(max(dot(N, H), 0.0), 36.0) * 0.22;

    float3 base = in.color.rgb;
    float3 lit = base * (ambient + diffuse) + float3(spec);

    if (in.highlight > 0.5) {
        float rim = pow(1.0 - max(dot(N, V), 0.0), 2.0);
        lit = mix(lit, float3(1.0, 0.96, 0.65), 0.32 + 0.45 * rim);
    }
    return float4(lit, in.color.a);
}

struct GridVertex {
    float3 pos;
    float4 color;
};

struct GridOut {
    float4 position [[position]];
    float4 color;
};

vertex GridOut ple_grid_vs(
    const device GridVertex *verts [[buffer(0)]],
    constant PleUniforms &u [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    GridOut o;
    o.position = u.viewProj * float4(float3(verts[vid].pos), 1.0);
    o.color = verts[vid].color;
    return o;
}

fragment float4 ple_grid_fs(GridOut in [[stage_in]]) {
    return in.color;
}
"""

// MARK: - Unit Mesh Builders

private func makeUnitBoxVertices() -> [PleVertex] {
    let faces: [(SIMD3<Float>, [SIMD3<Float>])] = [
        (SIMD3<Float>(0, 0, 1),  [SIMD3(-0.5,-0.5, 0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3(-0.5, 0.5, 0.5)]),
        (SIMD3<Float>(0, 0,-1),  [SIMD3(-0.5, 0.5,-0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3(-0.5,-0.5,-0.5)]),
        (SIMD3<Float>(1, 0, 0),  [SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5, 0.5,-0.5), SIMD3( 0.5, 0.5, 0.5), SIMD3( 0.5,-0.5, 0.5)]),
        (SIMD3<Float>(-1,0, 0),  [SIMD3(-0.5, 0.5,-0.5), SIMD3(-0.5,-0.5,-0.5), SIMD3(-0.5,-0.5, 0.5), SIMD3(-0.5, 0.5, 0.5)]),
        (SIMD3<Float>(0, 1, 0),  [SIMD3( 0.5, 0.5,-0.5), SIMD3(-0.5, 0.5,-0.5), SIMD3(-0.5, 0.5, 0.5), SIMD3( 0.5, 0.5, 0.5)]),
        (SIMD3<Float>(0,-1, 0),  [SIMD3(-0.5,-0.5,-0.5), SIMD3( 0.5,-0.5,-0.5), SIMD3( 0.5,-0.5, 0.5), SIMD3(-0.5,-0.5, 0.5)]),
    ]
    var out: [PleVertex] = []
    out.reserveCapacity(36)
    for (n, q) in faces {
        out.append(PleVertex(pos: q[0], normal: n))
        out.append(PleVertex(pos: q[1], normal: n))
        out.append(PleVertex(pos: q[2], normal: n))
        out.append(PleVertex(pos: q[0], normal: n))
        out.append(PleVertex(pos: q[2], normal: n))
        out.append(PleVertex(pos: q[3], normal: n))
    }
    return out
}

private func makeUnitCylinderVertices(segments: Int = 20) -> [PleVertex] {
    var out: [PleVertex] = []
    out.reserveCapacity(segments * 12)
    let r: Float = 0.5
    let zTop: Float = 0.5
    let zBot: Float = -0.5
    for i in 0..<segments {
        let a0 = Float(i) / Float(segments) * 2.0 * .pi
        let a1 = Float(i + 1) / Float(segments) * 2.0 * .pi
        let c0 = cos(a0), s0 = sin(a0)
        let c1 = cos(a1), s1 = sin(a1)
        let n0 = SIMD3<Float>(c0, s0, 0)
        let n1 = SIMD3<Float>(c1, s1, 0)
        let p00 = SIMD3<Float>(c0 * r, s0 * r, zBot)
        let p10 = SIMD3<Float>(c1 * r, s1 * r, zBot)
        let p11 = SIMD3<Float>(c1 * r, s1 * r, zTop)
        let p01 = SIMD3<Float>(c0 * r, s0 * r, zTop)
        // Side quad
        out.append(PleVertex(pos: p00, normal: n0))
        out.append(PleVertex(pos: p10, normal: n1))
        out.append(PleVertex(pos: p11, normal: n1))
        out.append(PleVertex(pos: p00, normal: n0))
        out.append(PleVertex(pos: p11, normal: n1))
        out.append(PleVertex(pos: p01, normal: n0))
        // Top cap
        let nTop = SIMD3<Float>(0, 0, 1)
        out.append(PleVertex(pos: SIMD3<Float>(0, 0, zTop), normal: nTop))
        out.append(PleVertex(pos: p01, normal: nTop))
        out.append(PleVertex(pos: p11, normal: nTop))
        // Bottom cap
        let nBot = SIMD3<Float>(0, 0, -1)
        out.append(PleVertex(pos: SIMD3<Float>(0, 0, zBot), normal: nBot))
        out.append(PleVertex(pos: p10, normal: nBot))
        out.append(PleVertex(pos: p00, normal: nBot))
    }
    return out
}

// MARK: - PleSceneMetalView

final class PleSceneMetalView: NSView {
    private var device: MTLDevice?
    private var commandQueue: MTLCommandQueue?
    private var opaquePipeline: MTLRenderPipelineState?
    private var translucentPipeline: MTLRenderPipelineState?
    private var gridPipeline: MTLRenderPipelineState?
    private var depthOpaqueState: MTLDepthStencilState?
    private var depthTranslucentState: MTLDepthStencilState?
    private var depthTexture: MTLTexture?
    private var depthSize: (Int, Int) = (0, 0)

    private var boxVertexBuffer: MTLBuffer?
    private var boxVertexCount: Int = 0
    private var cylVertexBuffer: MTLBuffer?
    private var cylVertexCount: Int = 0
    private var gridVertexBuffer: MTLBuffer?
    private var gridVertexCount: Int = 0

    private var camera = PleCamera()
    private struct CameraAnim {
        let startTime: CFTimeInterval
        let duration: CFTimeInterval
        let fromTarget: SIMD3<Float>
        let toTarget: SIMD3<Float>
        let fromDistance: Float
        let toDistance: Float
        let fromAzimuth: Float
        let toAzimuth: Float
        let fromElevation: Float
        let toElevation: Float
    }
    private var cameraAnim: CameraAnim?
    private var renderTimer: Timer?
    private var needsRedraw = true

    private var dragMode: DragMode = .none
    private var lastMouse: NSPoint = .zero
    private var downMouse: NSPoint = .zero
    private var didDrag = false

    private enum DragMode { case none, orbit, pan }

    private(set) var scene: PleScene?
    private var resolvedElements: [PleSceneElement] = []
    private(set) var activeDiff: PleSceneDiff? {
        didSet { needsRedraw = true }
    }
    var visibleLayers: Set<PleSemanticLayer> = Set(PleSemanticLayer.allCases) {
        didSet { needsRedraw = true }
    }
    private(set) var selectedElementId: String? {
        didSet { needsRedraw = true }
    }
    var onSelectElement: ((PleSceneElement?) -> Void)?

    override var wantsUpdateLayer: Bool { false }
    override var acceptsFirstResponder: Bool { true }

    override func makeBackingLayer() -> CALayer {
        CAMetalLayer()
    }

    private var metalLayer: CAMetalLayer? {
        layer as? CAMetalLayer
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        setupMetal()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
        setupMetal()
    }

    deinit {
        renderTimer?.invalidate()
    }

    /// Exports spherical orbit camera pose for right-pane <-> OA-SHOW-LIVE handoff (#14042).
    func exportOrbitPose() -> (target: SIMD3<Float>, distance: Float, azimuth: Float, elevation: Float) {
        (camera.target, camera.distance, camera.azimuth, camera.elevation)
    }

    /// Imports spherical orbit camera pose handed off from the right-side Metaverse pane (#14042).
    func importOrbitPose(target: SIMD3<Float>, distance: Float, azimuth: Float, elevation: Float) {
        cameraAnim = nil
        camera.target = target
        camera.distance = distance
        camera.azimuth = azimuth
        camera.elevation = elevation
        needsRedraw = true
    }

    private func setupMetal() {
        #if OA_FULL
        guard let dev = MetaverseGpuCoordinator.shared.device,
              let queue = MetaverseGpuCoordinator.shared.queue else { return }
        #else
        guard let dev = MTLCreateSystemDefaultDevice(),
              let queue = dev.makeCommandQueue() else { return }
        #endif
        self.device = dev
        self.commandQueue = queue

        if let ml = metalLayer {
            ml.device = dev
            ml.pixelFormat = .bgra8Unorm
            ml.framebufferOnly = true
            ml.isOpaque = true
            ml.contentsScale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        }

        guard let lib = try? dev.makeLibrary(source: pleSceneMetalShaderSource, options: nil),
              let meshVS = lib.makeFunction(name: "ple_mesh_vs"),
              let meshFS = lib.makeFunction(name: "ple_mesh_fs"),
              let gridVS = lib.makeFunction(name: "ple_grid_vs"),
              let gridFS = lib.makeFunction(name: "ple_grid_fs") else { return }

        let opDesc = MTLRenderPipelineDescriptor()
        opDesc.vertexFunction = meshVS
        opDesc.fragmentFunction = meshFS
        opDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        opDesc.depthAttachmentPixelFormat = .depth32Float
        opaquePipeline = try? dev.makeRenderPipelineState(descriptor: opDesc)

        let trDesc = MTLRenderPipelineDescriptor()
        trDesc.vertexFunction = meshVS
        trDesc.fragmentFunction = meshFS
        trDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        trDesc.colorAttachments[0].isBlendingEnabled = true
        trDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        trDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        trDesc.colorAttachments[0].sourceAlphaBlendFactor = .one
        trDesc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        trDesc.depthAttachmentPixelFormat = .depth32Float
        translucentPipeline = try? dev.makeRenderPipelineState(descriptor: trDesc)

        let grDesc = MTLRenderPipelineDescriptor()
        grDesc.vertexFunction = gridVS
        grDesc.fragmentFunction = gridFS
        grDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        grDesc.colorAttachments[0].isBlendingEnabled = true
        grDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        grDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        grDesc.depthAttachmentPixelFormat = .depth32Float
        gridPipeline = try? dev.makeRenderPipelineState(descriptor: grDesc)

        let dOp = MTLDepthStencilDescriptor()
        dOp.depthCompareFunction = .less
        dOp.isDepthWriteEnabled = true
        depthOpaqueState = dev.makeDepthStencilState(descriptor: dOp)

        let dTr = MTLDepthStencilDescriptor()
        dTr.depthCompareFunction = .less
        dTr.isDepthWriteEnabled = false
        depthTranslucentState = dev.makeDepthStencilState(descriptor: dTr)

        let boxVerts = makeUnitBoxVertices()
        boxVertexCount = boxVerts.count
        boxVertexBuffer = dev.makeBuffer(bytes: boxVerts, length: boxVerts.count * MemoryLayout<PleVertex>.stride, options: .storageModeShared)

        let cylVerts = makeUnitCylinderVertices(segments: 20)
        cylVertexCount = cylVerts.count
        cylVertexBuffer = dev.makeBuffer(bytes: cylVerts, length: cylVerts.count * MemoryLayout<PleVertex>.stride, options: .storageModeShared)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            updateDrawableSize()
            startDisplayTimer()
            needsRedraw = true
        } else {
            renderTimer?.invalidate()
            renderTimer = nil
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateDrawableSize()
        needsRedraw = true
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawableSize()
        needsRedraw = true
    }

    private func updateDrawableSize() {
        guard let ml = metalLayer else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        ml.contentsScale = scale
        let w = max(1, Int((bounds.width * scale).rounded()))
        let h = max(1, Int((bounds.height * scale).rounded()))
        ml.drawableSize = CGSize(width: w, height: h)
    }

    private func startDisplayTimer() {
        renderTimer?.invalidate()
        let t = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            guard let self = self, !self.isHiddenOrHasHiddenAncestor, self.window != nil else { return }
            self.stepCameraAnimation()
            self.renderFrame()
        }
        RunLoop.main.add(t, forMode: .common)
        renderTimer = t
    }

    // MARK: - Stage 4 Viewport Controls (Wireframe, Grid, Z-Clip)

    var wireframeMode: Bool = false {
        didSet { needsRedraw = true; renderFrame() }
    }
    var showGrid: Bool = true {
        didSet { needsRedraw = true; renderFrame() }
    }
    var clipZEnabled: Bool = false {
        didSet { needsRedraw = true; renderFrame() }
    }
    var clipZMax: Float = 10.0 {
        didSet { if clipZEnabled { needsRedraw = true; renderFrame() } }
    }
    var onSceneBoundsChanged: ((PleSceneBounds) -> Void)?

    // MARK: - Scene Loading & Camera Presets

    func setScene(_ newScene: PleScene, preserveView: Bool = false) {
        let prevSelectedId = preserveView ? selectedElementId : nil
        self.scene = newScene
        self.resolvedElements = newScene.allElements
        if !preserveView {
            self.cameraAnim = nil
            self.selectedElementId = nil
            self.camera.frame(bounds: newScene.bounds, preset: .iso)
            onSelectElement?(nil)
        } else if let prevId = prevSelectedId {
            if let match = self.resolvedElements.first(where: { $0.id == prevId }) {
                self.selectedElementId = prevId
                onSelectElement?(match)
            } else {
                self.selectedElementId = nil
                onSelectElement?(nil)
            }
        }
        if !preserveView {
            clipZMax = newScene.bounds.maxVec.z + 0.05
        }
        onSceneBoundsChanged?(newScene.bounds)
        rebuildGrid(bounds: newScene.bounds)
        needsRedraw = true
        renderFrame()
    }

    func setDiff(_ diff: PleSceneDiff?) {
        self.activeDiff = diff
        needsRedraw = true
        renderFrame()
    }

    /// Smoothly animates the camera target and distance to focus on a 3D world point (e.g. a clash).
    func focusOn(point: SIMD3<Float>, distance: Float = 4.5, highlightElementId: String? = nil, animated: Bool = true) {
        if let id = highlightElementId {
            self.selectedElementId = id
            if let elem = resolvedElements.first(where: { $0.id == id }) {
                onSelectElement?(elem)
            }
        }
        let clampedDist = max(1.5, min(40.0, distance))
        let targetElev: Float = max(0.35, min(0.85, camera.elevation))
        if !animated {
            cameraAnim = nil
            camera.target = point
            camera.distance = clampedDist
            camera.elevation = targetElev
            needsRedraw = true
            renderFrame()
            return
        }
        cameraAnim = CameraAnim(
            startTime: CACurrentMediaTime(),
            duration: 0.35,
            fromTarget: camera.target,
            toTarget: point,
            fromDistance: camera.distance,
            toDistance: clampedDist,
            fromAzimuth: camera.azimuth,
            toAzimuth: camera.azimuth,
            fromElevation: camera.elevation,
            toElevation: targetElev
        )
        needsRedraw = true
    }

    private func stepCameraAnimation() {
        guard let anim = cameraAnim else { return }
        let elapsed = CACurrentMediaTime() - anim.startTime
        let rawT = Float(max(0.0, min(1.0, elapsed / max(0.001, anim.duration))))
        // Cubic ease-out
        let t = 1.0 - pow(1.0 - rawT, 3.0)
        camera.target = anim.fromTarget + (anim.toTarget - anim.fromTarget) * t
        camera.distance = anim.fromDistance + (anim.toDistance - anim.fromDistance) * t
        camera.azimuth = anim.fromAzimuth + (anim.toAzimuth - anim.fromAzimuth) * t
        camera.elevation = anim.fromElevation + (anim.toElevation - anim.fromElevation) * t
        needsRedraw = true
        if rawT >= 1.0 {
            cameraAnim = nil
        }
    }

    func applyPreset(_ preset: PleCameraPreset) {
        guard let b = scene?.bounds else { return }
        cameraAnim = nil
        camera.frame(bounds: b, preset: preset)
        needsRedraw = true
    }

    func setLayerVisible(_ layer: PleSemanticLayer, visible: Bool) {
        if visible { visibleLayers.insert(layer) } else { visibleLayers.remove(layer) }
        needsRedraw = true
    }

    /// Selects an element by id and turns the camera to it (a program's request): false when the
    /// scene has none by that id.
    func selectElement(id: String) -> Bool {
        guard let elem = resolvedElements.first(where: { $0.id == id }) else { return false }
        let reach = max(elem.sizeVec.x, max(elem.sizeVec.y, elem.sizeVec.z))
        focusOn(point: elem.centerVec, distance: max(4.5, reach * 1.6), highlightElementId: id, animated: true)
        return true
    }

    private func rebuildGrid(bounds: PleSceneBounds) {
        guard let dev = device else { return }
        let minV = bounds.minVec
        let maxV = bounds.maxVec
        let pad: Float = 4.0
        let x0 = floor(minV.x - pad), x1 = ceil(maxV.x + pad)
        let y0 = floor(minV.y - pad), y1 = ceil(maxV.y + pad)
        let z = minV.z - 0.02
        let step: Float = 2.0
        var verts: [PleGridVertex] = []
        let minorColor = SIMD4<Float>(0.32, 0.36, 0.42, 0.25)
        let majorColor = SIMD4<Float>(0.44, 0.49, 0.56, 0.42)
        var x = x0
        while x <= x1 + 0.001 {
            let col = Int(x.rounded()) % 10 == 0 ? majorColor : minorColor
            verts.append(PleGridVertex(pos: SIMD3<Float>(x, y0, z), color: col))
            verts.append(PleGridVertex(pos: SIMD3<Float>(x, y1, z), color: col))
            x += step
        }
        var y = y0
        while y <= y1 + 0.001 {
            let col = Int(y.rounded()) % 10 == 0 ? majorColor : minorColor
            verts.append(PleGridVertex(pos: SIMD3<Float>(x0, y, z), color: col))
            verts.append(PleGridVertex(pos: SIMD3<Float>(x1, y, z), color: col))
            y += step
        }
        gridVertexCount = verts.count
        gridVertexBuffer = verts.isEmpty ? nil : dev.makeBuffer(bytes: verts, length: verts.count * MemoryLayout<PleGridVertex>.stride, options: .storageModeShared)
    }

    // MARK: - Rendering

    private func ensureDepthTexture(width: Int, height: Int) -> MTLTexture? {
        guard let dev = device, width > 0, height > 0 else { return nil }
        if depthSize == (width, height), let tex = depthTexture { return tex }
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
        desc.usage = .renderTarget
        desc.storageMode = .private
        depthTexture = dev.makeTexture(descriptor: desc)
        depthSize = (width, height)
        return depthTexture
    }

    private func buildInstances() -> (opBoxes: [PleInstance], opCyls: [PleInstance], trBoxes: [PleInstance], trCyls: [PleInstance]) {
        var opBoxes: [PleInstance] = []
        var opCyls: [PleInstance] = []
        var trBoxes: [PleInstance] = []
        var trCyls: [PleInstance] = []

        let diff = activeDiff

        for elem in resolvedElements {
            let sem = elem.semanticLayer
            guard visibleLayers.contains(sem) else { continue }
            if clipZEnabled && (elem.centerVec.z - elem.sizeVec.z * 0.5) > clipZMax { continue }
            var color = sem.rgba
            var translucent = sem.isTranslucent
            var highlight: Float = (elem.id == selectedElementId) ? 1.0 : 0.0

            if let diff = diff {
                if diff.addedIds.contains(elem.id) {
                    color = SIMD4<Float>(0.18, 0.90, 0.45, 1.0) // green: added in active
                    translucent = false
                    highlight = max(highlight, 0.25)
                } else if diff.movedIds.contains(elem.id) {
                    color = SIMD4<Float>(0.98, 0.62, 0.14, 1.0) // orange: moved/changed
                    translucent = false
                    highlight = max(highlight, 0.22)
                } else if sem != .clashes {
                    // Dim unchanged elements so diff stands out clearly
                    color = SIMD4<Float>(color.x * 0.48, color.y * 0.48, color.z * 0.52, sem.isTranslucent ? 0.16 : 0.36)
                    translucent = true
                }
            }

            let inst = PleInstance(
                model: pleModelMatrix(center: elem.centerVec, size: elem.sizeVec, rotZ: elem.rotZ, axis: elem.isCylinder ? elem.axisIndex : 2),
                color: color,
                highlight: highlight
            )
            if translucent {
                if elem.isCylinder { trCyls.append(inst) } else { trBoxes.append(inst) }
            } else {
                if elem.isCylinder { opCyls.append(inst) } else { opBoxes.append(inst) }
            }
        }

        // Ghost removed baseline elements in translucent crimson
        if let diff = diff {
            for elem in diff.removedElements {
                let sem = elem.semanticLayer
                guard visibleLayers.contains(sem) else { continue }
                let inst = PleInstance(
                    model: pleModelMatrix(center: elem.centerVec, size: elem.sizeVec, rotZ: elem.rotZ, axis: elem.isCylinder ? elem.axisIndex : 2),
                    color: SIMD4<Float>(0.95, 0.24, 0.30, 0.42),
                    highlight: 0.0
                )
                if elem.isCylinder { trCyls.append(inst) } else { trBoxes.append(inst) }
            }
        }

        return (opBoxes, opCyls, trBoxes, trCyls)
    }

    private func encodeScene(into enc: MTLRenderCommandEncoder, width: Int, height: Int) {
        guard let dev = device,
              let opPipe = opaquePipeline,
              let trPipe = translucentPipeline,
              let dOp = depthOpaqueState,
              let dTr = depthTranslucentState else { return }

        let aspect = Float(max(1, width)) / Float(max(1, height))
        let vp = camera.projMatrix(aspect: aspect) * camera.viewMatrix()
        var uniforms = PleUniforms(
            viewProj: vp,
            cameraPos: SIMD4<Float>(camera.eye, 1.0),
            keyLightDir: SIMD4<Float>(simd_normalize(SIMD3<Float>(0.55, 0.35, 0.75)), 0),
            fillLightDir: SIMD4<Float>(simd_normalize(SIMD3<Float>(-0.6, -0.4, 0.5)), 0),
            clipPlane: SIMD4<Float>(clipZMax, clipZEnabled ? 1.0 : 0.0, 0, 0)
        )

        // 1. Ground reference grid
        if showGrid, let gPipe = gridPipeline, let gBuf = gridVertexBuffer, gridVertexCount > 0 {
            enc.setRenderPipelineState(gPipe)
            enc.setDepthStencilState(dTr)
            enc.setVertexBuffer(gBuf, offset: 0, index: 0)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<PleUniforms>.stride, index: 1)
            enc.drawPrimitives(type: .line, vertexStart: 0, vertexCount: gridVertexCount)
        }

        let (opBoxes, opCyls, trBoxes, trCyls) = buildInstances()

        func drawBatch(_ insts: [PleInstance], verts: MTLBuffer?, vCount: Int) {
            guard !insts.isEmpty, let vb = verts, vCount > 0 else { return }
            let len = insts.count * MemoryLayout<PleInstance>.stride
            guard let ib = dev.makeBuffer(bytes: insts, length: len, options: .storageModeShared) else { return }
            enc.setVertexBuffer(vb, offset: 0, index: 0)
            enc.setVertexBuffer(ib, offset: 0, index: 1)
            enc.setVertexBytes(&uniforms, length: MemoryLayout<PleUniforms>.stride, index: 2)
            enc.setFragmentBytes(&uniforms, length: MemoryLayout<PleUniforms>.stride, index: 2)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: vCount, instanceCount: insts.count)
        }

        // 2. Opaque geometry
        enc.setTriangleFillMode(wireframeMode ? .lines : .fill)
        enc.setRenderPipelineState(opPipe)
        enc.setDepthStencilState(dOp)
        drawBatch(opBoxes, verts: boxVertexBuffer, vCount: boxVertexCount)
        drawBatch(opCyls, verts: cylVertexBuffer, vCount: cylVertexCount)

        // 3. Translucent geometry (architecture shell)
        enc.setRenderPipelineState(trPipe)
        enc.setDepthStencilState(dTr)
        drawBatch(trBoxes, verts: boxVertexBuffer, vCount: boxVertexCount)
        drawBatch(trCyls, verts: cylVertexBuffer, vCount: cylVertexCount)
    }

    func renderFrame() {
        guard let ml = metalLayer,
              let queue = commandQueue,
              ml.drawableSize.width >= 1, ml.drawableSize.height >= 1,
              let drawable = ml.nextDrawable() else { return }
        let w = drawable.texture.width
        let h = drawable.texture.height
        guard let depthTex = ensureDepthTexture(width: w, height: h),
              let cmd = queue.makeCommandBuffer() else { return }

        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = drawable.texture
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.085, green: 0.095, blue: 0.115, alpha: 1.0)
        rp.depthAttachment.texture = depthTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0

        if let enc = cmd.makeRenderCommandEncoder(descriptor: rp) {
            enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(w), height: Double(h), znear: 0, zfar: 1))
            encodeScene(into: enc, width: w, height: h)
            enc.endEncoding()
        }
        cmd.present(drawable)
        cmd.commit()
        needsRedraw = false
    }

    /// Offscreen render helper for automated verification (headless Metal rendering).
    func renderOffscreen(width: Int, height: Int) -> Data? {
        guard let dev = device, let queue = commandQueue, width > 0, height > 0 else { return nil }
        let cDesc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        cDesc.usage = [.renderTarget, .shaderRead]
        cDesc.storageMode = .shared
        guard let colorTex = dev.makeTexture(descriptor: cDesc),
              let depthTex = ensureDepthTexture(width: width, height: height),
              let cmd = queue.makeCommandBuffer() else { return nil }

        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = colorTex
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.085, green: 0.095, blue: 0.115, alpha: 1.0)
        rp.depthAttachment.texture = depthTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0

        if let enc = cmd.makeRenderCommandEncoder(descriptor: rp) {
            enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(width), height: Double(height), znear: 0, zfar: 1))
            encodeScene(into: enc, width: width, height: height)
            enc.endEncoding()
        }
        cmd.commit()
        cmd.waitUntilCompleted()
        var pixels = Data(count: width * height * 4)
        pixels.withUnsafeMutableBytes { ptr in
            if let base = ptr.baseAddress {
                colorTex.getBytes(base, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
            }
        }
        return pixels
    }

    /// Renders the current 3D viewport to PNG data at the requested resolution.
    func renderPNGData(width: Int = 1920, height: Int = 1080) -> Data? {
        guard let pixels = renderOffscreen(width: width, height: height) else { return nil }
        let bytesPerRow = width * 4
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        guard let provider = CGDataProvider(data: pixels as CFData),
              let cgImage = CGImage(
                width: width,
                height: height,
                bitsPerComponent: 8,
                bitsPerPixel: 32,
                bytesPerRow: bytesPerRow,
                space: colorSpace,
                bitmapInfo: bitmapInfo,
                provider: provider,
                decode: nil,
                shouldInterpolate: true,
                intent: .defaultIntent
              ) else { return nil }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out as CFMutableData, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(dest, cgImage, nil)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return out as Data
    }

    // MARK: - Mouse / Scroll Interaction & Raycast Picking

    override func mouseDown(with event: NSEvent) {
        cameraAnim = nil
        window?.makeFirstResponder(self)
        let pt = convert(event.locationInWindow, from: nil)
        lastMouse = pt
        downMouse = pt
        didDrag = false
        dragMode = event.modifierFlags.contains(.shift) ? .pan : .orbit
    }

    override func mouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        let dx = Float(pt.x - lastMouse.x)
        let dy = Float(pt.y - lastMouse.y)
        lastMouse = pt
        if hypot(pt.x - downMouse.x, pt.y - downMouse.y) > 3.0 { didDrag = true }
        switch dragMode {
        case .orbit:
            camera.azimuth -= dx * 0.008
            camera.elevation = max(-1.52, min(1.56, camera.elevation - dy * 0.008))
            needsRedraw = true
        case .pan:
            panBy(dx: dx, dy: dy)
        case .none:
            break
        }
    }

    override func mouseUp(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        if !didDrag && dragMode == .orbit {
            pickElement(at: pt)
        }
        dragMode = .none
    }

    override func rightMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        lastMouse = convert(event.locationInWindow, from: nil)
        dragMode = .pan
    }

    override func rightMouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        let dx = Float(pt.x - lastMouse.x)
        let dy = Float(pt.y - lastMouse.y)
        lastMouse = pt
        panBy(dx: dx, dy: dy)
    }

    override func rightMouseUp(with event: NSEvent) {
        dragMode = .none
    }

    /// A card's viewport in the transcript (#13812): the wheel scrolls the chat, Option-wheel zooms.
    var passesScroll = false

    override func scrollWheel(with event: NSEvent) {
        if passesScroll && !event.modifierFlags.contains(.option) { super.scrollWheel(with: event); return }
        cameraAnim = nil
        let delta = Float(event.scrollingDeltaY)
        let scale: Float = event.hasPreciseScrollingDeltas ? 0.004 : 0.06
        let factor = exp(-delta * scale)
        camera.distance = max(0.8, min(250.0, camera.distance * factor))
        needsRedraw = true
    }

    override func magnify(with event: NSEvent) {
        cameraAnim = nil
        let factor = Float(1.0 / max(0.2, 1.0 + event.magnification))
        camera.distance = max(0.8, min(250.0, camera.distance * factor))
        needsRedraw = true
    }

    private func panBy(dx: Float, dy: Float) {
        let (right, up) = camera.rightAndUp()
        let speed = camera.distance * 0.0018
        camera.target -= right * (dx * speed)
        camera.target -= up * (dy * speed)
        needsRedraw = true
    }

    func pickElement(at viewPoint: NSPoint) {
        guard bounds.width > 1, bounds.height > 1 else { return }
        let ndcX = Float((viewPoint.x / bounds.width) * 2.0 - 1.0)
        let ndcY = Float((viewPoint.y / bounds.height) * 2.0 - 1.0)
        let aspect = Float(bounds.width / bounds.height)
        let vp = camera.projMatrix(aspect: aspect) * camera.viewMatrix()
        let invVP = vp.inverse

        let nearClip = invVP * SIMD4<Float>(ndcX, ndcY, 0.0, 1.0)
        let farClip = invVP * SIMD4<Float>(ndcX, ndcY, 1.0, 1.0)
        guard abs(nearClip.w) > 1e-6, abs(farClip.w) > 1e-6 else { return }
        let rayOrig = SIMD3<Float>(nearClip.x, nearClip.y, nearClip.z) / nearClip.w
        let rayEnd = SIMD3<Float>(farClip.x, farClip.y, farClip.z) / farClip.w
        let rayDir = simd_normalize(rayEnd - rayOrig)

        var bestSolid: (elem: PleSceneElement, t: Float)?
        var bestArch: (elem: PleSceneElement, t: Float)?

        for elem in resolvedElements {
            let sem = elem.semanticLayer
            guard visibleLayers.contains(sem) else { continue }
            if let t = intersectElement(elem, rayOrig: rayOrig, rayDir: rayDir) {
                if sem.isTranslucent {
                    if bestArch == nil || t < bestArch!.t { bestArch = (elem, t) }
                } else {
                    if bestSolid == nil || t < bestSolid!.t { bestSolid = (elem, t) }
                }
            }
        }
        let picked = bestSolid?.elem ?? bestArch?.elem
        selectedElementId = picked?.id
        onSelectElement?(picked)
    }

    private func intersectElement(_ elem: PleSceneElement, rayOrig: SIMD3<Float>, rayDir: SIMD3<Float>) -> Float? {
        // Transform ray into element local space (translate by -center, rotate by -rotZ)
        let relO = rayOrig - elem.centerVec
        let c = cos(-elem.rotZ), s = sin(-elem.rotZ)
        let o = SIMD3<Float>(c * relO.x - s * relO.y, s * relO.x + c * relO.y, relO.z)
        let d = SIMD3<Float>(c * rayDir.x - s * rayDir.y, s * rayDir.x + c * rayDir.y, rayDir.z)
        let half = elem.sizeVec * 0.5

        // Slab AABB intersection
        var tMin: Float = 0.001
        var tMax: Float = 1e6
        for axis in 0..<3 {
            let oa = o[axis], da = d[axis], ha = half[axis]
            if abs(da) < 1e-6 {
                if oa < -ha || oa > ha { return nil }
            } else {
                var t1 = (-ha - oa) / da
                var t2 = (ha - oa) / da
                if t1 > t2 { swap(&t1, &t2) }
                tMin = max(tMin, t1)
                tMax = min(tMax, t2)
                if tMin > tMax { return nil }
            }
        }
        return tMin
    }
}

