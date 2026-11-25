// Firm reason: 6 (AppKit/Metal/ModelIO 3D viewport over CAMetalLayer running Coax-emitted Metal shaders and metaverse-viewer.o).
// Unified native macOS Metal viewport for the expandable right-side Metaverse Viewer & GameBuilder pane (GitHub #14041).
import AppKit
import Metal
import ModelIO
import QuartzCore
import ImageIO
import UniformTypeIdentifiers
import IOKit.ps
import simd

/// Process-wide shared Metal device, command queue, and #authority-bounded aggregate VRAM ledger
/// across the right-side Metaverse pane, inline OA-SHOW-LIVE cards, and cached tabs (aethyr.host.v2, #14042).
final class MetaverseGpuCoordinator {
    static let shared = MetaverseGpuCoordinator()
    let device: MTLDevice?
    let queue: MTLCommandQueue?
    let budgetLimitBytes: UInt64
    private var tickCounter: UInt64 = 0

    private init() {
        let dev = MTLCreateSystemDefaultDevice()
        self.device = dev
        self.queue = dev?.makeCommandQueue()
        let ram = Int64(ProcessInfo.processInfo.physicalMemory)
        let cap = oa_mv_policy_budget_cap(ram)
        self.budgetLimitBytes = UInt64(max(64 * 1024 * 1024, cap))
    }

    func nextTick() -> UInt64 {
        tickCounter &+= 1
        return tickCounter
    }

    /// Evicts GPU buffers/textures from least-recently-used background tabs when aggregate VRAM
    /// exceeds the 75% watermark, keeping lightweight CPU state & camera pose intact.
    func enforceBudget(scenes: [String: MetaverseTabScene], activePath: String) {
        var total: UInt64 = 0
        for sc in scenes.values { total &+= sc.gpuBytes }
        let watermark = (budgetLimitBytes * 3) / 4
        guard total > watermark else { return }
        let victims = scenes.values
            .filter { $0.path != activePath && $0.gpuBytes > 0 }
            .sorted { $0.lastTouchedTick < $1.lastTouchedTick }
        for v in victims {
            total = total > v.gpuBytes ? total - v.gpuBytes : 0
            v.evictGpuResources()
            if total <= watermark { break }
        }
    }

    static func isOnBatteryPower() -> Bool {
        guard let snap = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let list = IOPSCopyPowerSourcesList(snap)?.takeRetainedValue() as? [CFTypeRef] else {
            return false
        }
        for ps in list {
            if let desc = IOPSGetPowerSourceDescription(snap, ps)?.takeUnretainedValue() as? [String: Any],
               let state = desc[kIOPSPowerSourceStateKey as String] as? String {
                if state == (kIOPSBatteryPowerValue as String) { return true }
            }
        }
        return false
    }
}

final class MetaverseTabScene {
    let path: String
    var handle: UnsafeMutableRawPointer?
    var vertexBuffer: MTLBuffer?
    var indexBuffer: MTLBuffer?
    var vertexCount: Int = 0
    var indexCount: Int = 0
    var zUp: Bool = false
    var savedRev: Int = 0
    var boundsMin = SIMD3<Float>(-1, -1, -1)
    var boundsMax = SIMD3<Float>(1, 1, 1)
    var statsText: String = ""
    var errorText: String = ""
    var treeTSV: String = ""
    var gpuBytes: UInt64 = 0
    var lastTouchedTick: UInt64 = 0

    struct JointInfo {
        let index: Int
        let name: String
        let low: Float
        let high: Float
        let stand: Float
        var current: Float
    }

    struct AnimClipInfo {
        let index: Int
        let name: String
        let duration: Float
    }

    var hasAssembly: Bool = false
    var joints: [JointInfo] = []
    var activeJoint: Int = 0
    var explodeFactor: Float = 0.0
    var animClips: [AnimClipInfo] = []
    var activeClip: Int = 0
    var animTime: Float = 0.0
    var animPlaying: Bool = false

    struct NodeDraw {
        let index: Int
        let firstIndex: Int
        let indexCount: Int
        let matIndex: Int
        var visible: Bool
        var selected: Bool
        let label: String
    }

    struct MatInfo {
        let baseColor: SIMD4<Float>
        let metallic: Float
        let roughness: Float
        var texture: MTLTexture?
    }

    var nodes: [NodeDraw] = []
    var materials: [MatInfo] = []

    // Per-tab camera & view state
    var target = SIMD3<Float>(0, 0, 0)
    var distance: Float = 5.0
    var azimuth: Float = .pi * 0.25
    var elevation: Float = 0.45
    var flyMode: Bool = false
    var flyPos = SIMD3<Float>(0, 2, 5)
    var shadingMode: Int = 0          // 0 Lit PBR, 1 Clay, 2 Normals, 3 Unlit, 4 Wire
    var overlayMask: Int = 1 | 16     // Grid + Stats by default
    var zSliceNormalized: Float = 1.0 // 1.0 = full height
    var selectedNode: Int = -1
    var lastHitWorld = SIMD3<Float>(0, 0, 0)

    init(path: String) {
        self.path = path
    }

    deinit {
        if let h = handle { oa_mv_free(h) }
    }

    func recomputeGpuBytes() {
        var b: UInt64 = 0
        if let vb = vertexBuffer { b &+= UInt64(vb.length) }
        if let ib = indexBuffer { b &+= UInt64(ib.length) }
        for m in materials {
            if let t = m.texture { b &+= UInt64(t.allocatedSize) }
        }
        gpuBytes = b
    }

    /// Evicts GPU buffers and textures while retaining Coax CPU state, camera pose, and node tree.
    func evictGpuResources() {
        vertexBuffer = nil
        indexBuffer = nil
        for i in 0..<materials.count { materials[i].texture = nil }
        gpuBytes = 0
    }

    private var vertScratch: [Float] = []

    /// Re-uploads deformed vertices (after RBA1 joint/explode posing or animation clip scrubbing) with zero per-frame allocation.
    func reuploadVerticesToGpu(device: MTLDevice) {
        guard let h = handle, vertexCount > 0 else { return }
        let need = vertexCount * 12
        if vertScratch.count < need { vertScratch = [Float](repeating: 0, count: need) }
        let written = Int(oa_mv_read_verts(h, &vertScratch, Int32(need)))
        guard written > 0 else { return }
        let byteLen = written * MemoryLayout<Float>.stride
        if let existing = vertexBuffer, existing.length >= byteLen {
            memcpy(existing.contents(), vertScratch, byteLen)
        } else {
            vertexBuffer = device.makeBuffer(bytes: vertScratch, length: byteLen, options: .storageModeShared)
        }
        var b = [Float](repeating: 0, count: 6)
        if oa_mv_get_bounds(h, &b) != 0 {
            boundsMin = SIMD3<Float>(b[0], b[1], b[2])
            boundsMax = SIMD3<Float>(b[3], b[4], b[5])
        }
        recomputeGpuBytes()
    }

    /// Restores evicted GPU buffers transparently from retained Coax state on tab switch.
    func ensureGpuBuffers(device: MTLDevice) {
        if vertexBuffer != nil && indexBuffer != nil { return }
        guard let h = handle, vertexCount > 0, indexCount > 0 else { return }
        reuploadVerticesToGpu(device: device)
        var ibuf = [UInt32](repeating: 0, count: indexCount)
        _ = oa_mv_read_indices(h, &ibuf, Int32(ibuf.count))
        indexBuffer = device.makeBuffer(bytes: ibuf, length: ibuf.count * MemoryLayout<UInt32>.stride, options: .storageModeShared)
        recomputeGpuBytes()
    }

    var boundsCenter: SIMD3<Float> { (boundsMin + boundsMax) * 0.5 }
    var boundsExtent: SIMD3<Float> { max(boundsMax - boundsMin, SIMD3<Float>(repeating: 0.05)) }
    var boundsRadius: Float { max(0.25, simd_length(boundsExtent) * 0.5) }

    var eyePosition: SIMD3<Float> {
        if flyMode { return flyPos }
        let ce = cos(elevation)
        if zUp {
            return target + SIMD3<Float>(cos(azimuth) * ce, sin(azimuth) * ce, sin(elevation)) * distance
        } else {
            return target + SIMD3<Float>(sin(azimuth) * ce, sin(elevation), cos(azimuth) * ce) * distance
        }
    }

    var lookTarget: SIMD3<Float> {
        if !flyMode { return target }
        let ce = cos(elevation)
        let fwd = zUp
            ? SIMD3<Float>(-cos(azimuth) * ce, -sin(azimuth) * ce, -sin(elevation))
            : SIMD3<Float>(-sin(azimuth) * ce, -sin(elevation), -cos(azimuth) * ce)
        return flyPos + fwd
    }

    func applyPreset(_ name: String) {
        let c = boundsCenter
        let r = boundsRadius
        target = c
        distance = r * 2.55
        switch name {
        case "front":
            azimuth = 0.0
            elevation = 0.08
        case "right":
            azimuth = .pi * 0.5
            elevation = 0.08
        case "top":
            azimuth = 0.0
            elevation = 1.54
        case "under":
            azimuth = .pi * 0.25
            elevation = -0.35
        default: // "iso", "focus"
            azimuth = .pi * 0.25
            elevation = 0.48
        }
        flyPos = eyePosition
    }

    func viewMatrix() -> simd_float4x4 {
        let e = eyePosition
        let t = lookTarget
        let f = simd_normalize(t - e)
        let upHint = zUp
            ? (abs(f.z) > 0.995 ? SIMD3<Float>(0, 1, 0) : SIMD3<Float>(0, 0, 1))
            : (abs(f.y) > 0.995 ? SIMD3<Float>(0, 0, -1) : SIMD3<Float>(0, 1, 0))
        let r = simd_normalize(simd_cross(f, upHint))
        let u = simd_cross(r, f)
        return simd_float4x4(
            SIMD4<Float>(r.x, u.x, -f.x, 0),
            SIMD4<Float>(r.y, u.y, -f.y, 0),
            SIMD4<Float>(r.z, u.z, -f.z, 0),
            SIMD4<Float>(-simd_dot(r, e), -simd_dot(u, e), simd_dot(f, e), 1)
        )
    }

    func projMatrix(aspect: Float) -> simd_float4x4 {
        let fovY: Float = 45.0 * .pi / 180.0
        let nearZ = max(0.02, boundsRadius * 0.004)
        let farZ = max(250.0, boundsRadius * 30.0)
        let baseYs = 1.0 / tan(fovY * 0.5)
        // Preserve at least 45° horizontal FOV in narrow docked portrait panes so models aren't clipped.
        let safeAspect = max(0.01, aspect)
        let ys = safeAspect < 1.0 ? baseYs * max(0.30, safeAspect) : baseYs
        let xs = ys / safeAspect
        let zs = farZ / (nearZ - farZ)
        return simd_float4x4(
            SIMD4<Float>(xs, 0, 0, 0),
            SIMD4<Float>(0, ys, 0, 0),
            SIMD4<Float>(0, 0, zs, -1),
            SIMD4<Float>(0, 0, nearZ * zs, 0)
        )
    }
}

final class MetaverseViewportView: NSView {
    private let metalLayer = CAMetalLayer()
    private var device: MTLDevice?
    private var queue: MTLCommandQueue?
    private var triPipeline: MTLRenderPipelineState?
    private var linePipeline: MTLRenderPipelineState?
    private var gbPipeline: MTLRenderPipelineState?
    private var gbCubeBuffer: MTLBuffer?
    private var gbInstanceBuffer: MTLBuffer?
    private var gbInstanceFloats: [Float] = []
    private var depthState: MTLDepthStencilState?
    private var depthTexture: MTLTexture?
    private var depthSize: CGSize = .zero
    private var defaultWhiteTex: MTLTexture?
    private var linearSampler: MTLSamplerState?

    private var gridVertexBuffer: MTLBuffer?
    private var gridVertexCount: Int = 0
    private var boundsVertexBuffer: MTLBuffer?

    private let hudLabel = NSTextField(labelWithString: "")
    private let maskLayer = CAShapeLayer()
    private var holeRects: [NSRect] = []

    // Phase 2 (#14042): RBA1 joint/explode + glTF/anim-scene/.aimport clip scrub overlay & 0 Hz idle timer
    private let controlBar = NSStackView()
    private let animPlayBtn = NSButton(title: "Play", target: nil, action: nil)
    private let clipPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let animSlider = NSSlider(value: 0, minValue: 0, maxValue: 1, target: nil, action: nil)
    private let animTimeLabel = NSTextField(labelWithString: "0.00s")
    private let jointPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let jointSlider = NSSlider(value: 0, minValue: -3.14, maxValue: 3.14, target: nil, action: nil)
    private let explodeLabel = NSTextField(labelWithString: "Explode")
    private let explodeSlider = NSSlider(value: 0, minValue: 0, maxValue: 1, target: nil, action: nil)
    private let resetPoseBtn = NSButton(title: "Reset", target: nil, action: nil)
    private var animTimer: Timer?
    private var animLastTick: CFTimeInterval = 0
    private var idleEvictTimer: Timer?

    private(set) var activeScene: MetaverseTabScene?
    private(set) var activeGameBuilder: GameBuilderSession?
    private(set) var activeGtaWorkbench: GtaWorkbenchSession?
    private let gbHudLabel = NSTextField(labelWithString: "")
    private var gbTickTimer: Timer?
    private var gbLastTick: CFTimeInterval = 0
    private var gbRightDown = false
    private var gbHeldKeys: Set<Int32> = []
    private var sceneHeldKeys: Set<String> = []
    var onGameBuilderMutated: (() -> Void)?
    private var lastMouse: NSPoint = .zero
    private var downMouse: NSPoint = .zero
    private var dragging = false
    private var dragPanning = false
    private var optionClickPending = false
    private var wantsRenderOnNextVsync = true
    private var lastRenderWall: CFTimeInterval = 0
    private var smoothFps: Double = 0

    // GTA VI Key Lento Metal 4 Workbench bottom HUD bar
    private let gtaBar = NSStackView()
    private let gtaPlayBtn = NSButton(title: "Converge", target: nil, action: nil)
    private let gtaPresetPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let gtaCamBtn = NSButton(title: "R* Cam", target: nil, action: nil)
    private let gtaFxBtn = NSButton(title: "MetalFX", target: nil, action: nil)
    private let gtaClipBtn = NSButton(title: "Clip", target: nil, action: nil)
    private let gtaExprBtn = NSButton(title: "Expr", target: nil, action: nil)
    private let gtaSnapBtn = NSButton(title: "128-SPP", target: nil, action: nil)

    var onNodePicked: ((Int, String, SIMD3<Float>) -> Void)?
    var onFocusRequested: (() -> Void)?
    var onReleaseFocusToComposer: (() -> Void)?
    var onNeedsDisplay: (() -> Void)?
    var onFilesDropped: (([URL]) -> Void)?
    var onAttachSelectionRequested: (() -> Void)?
    var onGtaSnapshotRequested: (() -> Void)?
    // composition-v2 (#14046): 3-slot ping-pong offscreen GPU render targets composited directly in UcPlan (slot 0)
    var useCompositionV2: Bool = true
    private var extColorTextures: [MTLTexture] = []
    private var extColorRingIdx: Int = 0
    private var extColorSize: CGSize = .zero
    private let renderFlightSem = DispatchSemaphore(value: 2)
    var onExternalTextureUpdated: ((MTLTexture?) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }

    override var acceptsFirstResponder: Bool { true }

    private func setupView() {
        wantsLayer = true
        let root = CALayer()
        root.backgroundColor = NSColor(calibratedRed: 0.045, green: 0.052, blue: 0.065, alpha: 1.0).cgColor
        layer = root

        device = MetaverseGpuCoordinator.shared.device
        queue = MetaverseGpuCoordinator.shared.queue

        metalLayer.device = device
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = false
        metalLayer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        root.addSublayer(metalLayer)

        maskLayer.fillRule = .evenOdd

        hudLabel.font = .monospacedSystemFont(ofSize: 10.5, weight: .medium)
        hudLabel.textColor = NSColor(calibratedWhite: 0.80, alpha: 0.92)
        hudLabel.backgroundColor = NSColor(calibratedRed: 0.06, green: 0.07, blue: 0.09, alpha: 0.72)
        hudLabel.drawsBackground = true
        hudLabel.isBezeled = false
        hudLabel.isEditable = false
        hudLabel.wantsLayer = true
        hudLabel.layer?.cornerRadius = 5
        hudLabel.layer?.masksToBounds = true
        hudLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(hudLabel)
        NSLayoutConstraint.activate([
            hudLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            hudLabel.topAnchor.constraint(equalTo: topAnchor, constant: 10),
        ])

        setupControlBar()
        setupGtaBar()
        setupGbHudLabel()
        registerForDraggedTypes([.fileURL])
        buildPipelines()
        buildGbPipeline()
        buildDefaultResources()
        buildGridGeometry(zUp: false, radius: 5.0)
    }

    deinit {
        animTimer?.invalidate()
        gbTickTimer?.invalidate()
        idleEvictTimer?.invalidate()
    }

    private func setupGbHudLabel() {
        gbHudLabel.font = .monospacedSystemFont(ofSize: 11.5, weight: .semibold)
        gbHudLabel.textColor = NSColor(calibratedWhite: 0.96, alpha: 0.95)
        gbHudLabel.backgroundColor = NSColor(calibratedRed: 0.05, green: 0.06, blue: 0.09, alpha: 0.78)
        gbHudLabel.drawsBackground = true
        gbHudLabel.isBezeled = false
        gbHudLabel.isEditable = false
        gbHudLabel.wantsLayer = true
        gbHudLabel.layer?.cornerRadius = 6
        gbHudLabel.layer?.masksToBounds = true
        gbHudLabel.translatesAutoresizingMaskIntoConstraints = false
        gbHudLabel.isHidden = true
        addSubview(gbHudLabel)
        NSLayoutConstraint.activate([
            gbHudLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            gbHudLabel.topAnchor.constraint(equalTo: topAnchor, constant: 12),
        ])
    }

    private func buildGbPipeline() {
        guard let device else { return }
        if let path = Bundle.main.resourceURL?.appendingPathComponent("Components/gb-instances.metal").path,
           let src = try? String(contentsOfFile: path, encoding: .utf8) {
            let opts = MTLCompileOptions()
            opts.fastMathEnabled = false
            opts.languageVersion = .version2_3
            if let lib = try? device.makeLibrary(source: src, options: opts),
               let vs = lib.makeFunction(name: "gb_vs"),
               let fs = lib.makeFunction(name: "gb_fs") {
                // Reflected by coax metal gb-instances.coax: stride 28 (float3 pos@0, float3 nrm@12, float ao@24)
                let vd = MTLVertexDescriptor()
                vd.attributes[0].format = .float3; vd.attributes[0].offset = 0;  vd.attributes[0].bufferIndex = 30
                vd.attributes[1].format = .float3; vd.attributes[1].offset = 12; vd.attributes[1].bufferIndex = 30
                vd.attributes[2].format = .float;  vd.attributes[2].offset = 24; vd.attributes[2].bufferIndex = 30
                vd.layouts[30].stride = 28
                vd.layouts[30].stepFunction = .perVertex

                let desc = MTLRenderPipelineDescriptor()
                desc.vertexFunction = vs
                desc.fragmentFunction = fs
                desc.vertexDescriptor = vd
                desc.colorAttachments[0].pixelFormat = .bgra8Unorm
                desc.depthAttachmentPixelFormat = .depth32Float
                gbPipeline = try? device.makeRenderPipelineState(descriptor: desc)
            }
        }
        // Build 36-vertex unit cube padded to 7 floats (pos3, nrm3, ao=1.0) matching MESH_VERTEX_WORDS
        var cube6 = [Float](repeating: 0, count: 36 * 6)
        let nv = Int(oa_gb_get_cube(&cube6, Int32(cube6.count)))
        if nv == 36 {
            var cube7 = [Float](repeating: 1.0, count: 36 * 7)
            for i in 0..<36 {
                for k in 0..<6 { cube7[i * 7 + k] = cube6[i * 6 + k] }
                cube7[i * 7 + 6] = 1.0
            }
            gbCubeBuffer = device.makeBuffer(bytes: cube7, length: cube7.count * MemoryLayout<Float>.stride, options: .storageModeShared)
        }
        let maxInst = max(1024, Int(oa_gb_get_instance_max()))
        gbInstanceFloats = [Float](repeating: 0, count: maxInst * 20)
        gbInstanceBuffer = device.makeBuffer(length: gbInstanceFloats.count * MemoryLayout<Float>.stride, options: .storageModeShared)
    }

    private func setupControlBar() {
        controlBar.orientation = .horizontal
        controlBar.spacing = 8
        controlBar.alignment = .centerY
        controlBar.edgeInsets = NSEdgeInsets(top: 5, left: 10, bottom: 5, right: 10)
        controlBar.wantsLayer = true
        controlBar.layer?.backgroundColor = NSColor(calibratedRed: 0.07, green: 0.08, blue: 0.11, alpha: 0.88).cgColor
        controlBar.layer?.cornerRadius = 7
        controlBar.layer?.borderWidth = 1
        controlBar.layer?.borderColor = NSColor(calibratedWhite: 1.0, alpha: 0.12).cgColor
        controlBar.translatesAutoresizingMaskIntoConstraints = false
        controlBar.isHidden = true

        animPlayBtn.bezelStyle = .rounded
        animPlayBtn.controlSize = .small
        animPlayBtn.target = self
        animPlayBtn.action = #selector(toggleAnimPlay)

        clipPopup.controlSize = .small
        clipPopup.target = self
        clipPopup.action = #selector(clipChanged)

        animSlider.controlSize = .small
        animSlider.target = self
        animSlider.action = #selector(animScrubbed)
        animSlider.widthAnchor.constraint(greaterThanOrEqualToConstant: 120).isActive = true

        animTimeLabel.font = .monospacedDigitSystemFont(ofSize: 10, weight: .regular)
        animTimeLabel.textColor = NSColor(calibratedWhite: 0.85, alpha: 0.95)

        jointPopup.controlSize = .small
        jointPopup.target = self
        jointPopup.action = #selector(jointChanged)

        jointSlider.controlSize = .small
        jointSlider.target = self
        jointSlider.action = #selector(jointScrubbed)
        jointSlider.widthAnchor.constraint(greaterThanOrEqualToConstant: 96).isActive = true

        explodeLabel.font = .systemFont(ofSize: 10, weight: .medium)
        explodeLabel.textColor = NSColor(calibratedWhite: 0.78, alpha: 0.95)
        explodeSlider.controlSize = .small
        explodeSlider.target = self
        explodeSlider.action = #selector(explodeScrubbed)
        explodeSlider.widthAnchor.constraint(equalToConstant: 78).isActive = true

        resetPoseBtn.bezelStyle = .rounded
        resetPoseBtn.controlSize = .small
        resetPoseBtn.target = self
        resetPoseBtn.action = #selector(resetPoseClicked)

        for v in [animPlayBtn, clipPopup, animSlider, animTimeLabel, jointPopup, jointSlider, explodeLabel, explodeSlider, resetPoseBtn] as [NSView] {
            controlBar.addArrangedSubview(v)
        }
        addSubview(controlBar)
        NSLayoutConstraint.activate([
            controlBar.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 10),
            controlBar.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -10),
            controlBar.centerXAnchor.constraint(equalTo: centerXAnchor),
            controlBar.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -10),
        ])
    }

    private func setupGtaBar() {
        gtaBar.orientation = .horizontal
        gtaBar.spacing = 6
        gtaBar.alignment = .centerY
        gtaBar.edgeInsets = NSEdgeInsets(top: 5, left: 8, bottom: 5, right: 8)
        gtaBar.wantsLayer = true
        gtaBar.layer?.backgroundColor = NSColor(calibratedRed: 0.06, green: 0.07, blue: 0.10, alpha: 0.88).cgColor
        gtaBar.layer?.cornerRadius = 7
        gtaBar.layer?.borderWidth = 1
        gtaBar.layer?.borderColor = NSColor(calibratedWhite: 1.0, alpha: 0.14).cgColor
        gtaBar.translatesAutoresizingMaskIntoConstraints = false
        gtaBar.isHidden = true

        for b in [gtaPlayBtn, gtaCamBtn, gtaFxBtn, gtaClipBtn, gtaExprBtn, gtaSnapBtn] {
            b.bezelStyle = .rounded
            b.controlSize = .small
            b.target = self
        }
        gtaPlayBtn.action = #selector(gtaTogglePaused)
        gtaCamBtn.action = #selector(gtaToggleCam)
        gtaFxBtn.action = #selector(gtaToggleFx)
        gtaClipBtn.action = #selector(gtaCycleClip)
        gtaExprBtn.action = #selector(gtaCycleExpr)
        gtaSnapBtn.action = #selector(gtaSnapClicked)

        gtaPresetPopup.controlSize = .small
        gtaPresetPopup.target = self
        gtaPresetPopup.action = #selector(gtaPresetChanged)
        gtaPresetPopup.removeAllItems()
        for p in GtaWorkbenchSession.presets { gtaPresetPopup.addItem(withTitle: p.label) }

        for v in [gtaPlayBtn, gtaPresetPopup, gtaCamBtn, gtaFxBtn, gtaClipBtn, gtaExprBtn, gtaSnapBtn] as [NSView] {
            gtaBar.addArrangedSubview(v)
        }
        addSubview(gtaBar)
        NSLayoutConstraint.activate([
            gtaBar.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 8),
            gtaBar.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            gtaBar.centerXAnchor.constraint(equalTo: centerXAnchor),
            gtaBar.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -10),
        ])
    }

    func syncGtaBar() {
        guard let gta = activeGtaWorkbench else { gtaBar.isHidden = true; return }
        gtaBar.isHidden = false
        gtaPlayBtn.title = gta.state.paused ? "Live" : "Converge"
        gtaCamBtn.title = gta.state.rockstarCam ? "R* Cam ●" : "R* Cam"
        gtaFxBtn.title = gta.state.denoiserOn ? "SVGF 2x ●" : "Raw PT"
        let exprName = (gta.state.exprIndex >= 0 && Int(gta.state.exprIndex) < GtaWorkbenchSession.exprNames.count)
            ? GtaWorkbenchSession.exprNames[Int(gta.state.exprIndex)] : "Auto"
        gtaExprBtn.title = "Expr: \(exprName)"
        if let idx = GtaWorkbenchSession.presets.firstIndex(where: { $0.id == gta.state.presetName }) {
            gtaPresetPopup.selectItem(at: idx)
        }
    }

    @objc private func gtaTogglePaused() { activeGtaWorkbench?.togglePaused(); syncGtaBar(); onNeedsDisplay?() }
    @objc private func gtaToggleCam() { activeGtaWorkbench?.toggleRockstarCam(); syncGtaBar(); onNeedsDisplay?() }
    @objc private func gtaToggleFx() { activeGtaWorkbench?.toggleDenoiser(); syncGtaBar(); onNeedsDisplay?() }
    @objc private func gtaCycleClip() { activeGtaWorkbench?.cycleClip(); syncGtaBar(); onNeedsDisplay?() }
    @objc private func gtaCycleExpr() { activeGtaWorkbench?.cycleExpr(); syncGtaBar(); onNeedsDisplay?() }
    @objc private func gtaSnapClicked() { onGtaSnapshotRequested?() }
    @objc private func gtaPresetChanged() {
        let idx = gtaPresetPopup.indexOfSelectedItem
        guard idx >= 0 && idx < GtaWorkbenchSession.presets.count else { return }
        activeGtaWorkbench?.applyPreset(GtaWorkbenchSession.presets[idx].id)
        syncGtaBar()
        onNeedsDisplay?()
    }

    private func syncControlBar() {
        guard let sc = activeScene else {
            controlBar.isHidden = true
            stopAnimTimer()
            return
        }
        let showAnim = !sc.animClips.isEmpty
        let showAsm = sc.hasAssembly && !sc.joints.isEmpty
        controlBar.isHidden = !(showAnim || showAsm)

        animPlayBtn.isHidden = !showAnim
        clipPopup.isHidden = !showAnim
        animSlider.isHidden = !showAnim
        animTimeLabel.isHidden = !showAnim
        if showAnim {
            clipPopup.removeAllItems()
            for cl in sc.animClips { clipPopup.addItem(withTitle: cl.name) }
            let ci = min(max(0, sc.activeClip), sc.animClips.count - 1)
            clipPopup.selectItem(at: ci)
            let dur = max(0.01, sc.animClips[ci].duration)
            animSlider.minValue = 0
            animSlider.maxValue = Double(dur)
            animSlider.doubleValue = Double(sc.animTime)
            animPlayBtn.title = sc.animPlaying ? "Pause" : "Play"
            animTimeLabel.stringValue = String(format: "%.2f / %.2fs", sc.animTime, dur)
        }

        jointPopup.isHidden = !showAsm
        jointSlider.isHidden = !showAsm
        explodeLabel.isHidden = !showAsm
        explodeSlider.isHidden = !showAsm
        resetPoseBtn.isHidden = !showAsm
        if showAsm {
            jointPopup.removeAllItems()
            for j in sc.joints { jointPopup.addItem(withTitle: j.name) }
            let ji = min(max(0, sc.activeJoint), sc.joints.count - 1)
            jointPopup.selectItem(at: ji)
            let j = sc.joints[ji]
            jointSlider.minValue = Double(j.low)
            jointSlider.maxValue = Double(j.high)
            jointSlider.doubleValue = Double(j.current)
            explodeSlider.doubleValue = Double(sc.explodeFactor)
        }
        updateAnimTimer()
    }

    @objc private func toggleAnimPlay() {
        guard let sc = activeScene, !sc.animClips.isEmpty else { return }
        sc.animPlaying.toggle()
        animPlayBtn.title = sc.animPlaying ? "Pause" : "Play"
        updateAnimTimer()
    }

    @objc private func clipChanged() {
        guard let sc = activeScene, !sc.animClips.isEmpty else { return }
        sc.activeClip = max(0, clipPopup.indexOfSelectedItem)
        sc.animTime = 0
        applyAnimTime(0)
        syncControlBar()
    }

    @objc private func animScrubbed() {
        applyAnimTime(Float(animSlider.doubleValue))
    }

    private func applyAnimTime(_ t: Float, deferRender: Bool = false) {
        guard let sc = activeScene, let h = sc.handle, let device, !sc.animClips.isEmpty else { return }
        let ci = min(max(0, sc.activeClip), sc.animClips.count - 1)
        let dur = max(0.01, sc.animClips[ci].duration)
        sc.animTime = t.truncatingRemainder(dividingBy: dur)
        if sc.animTime < 0 { sc.animTime += dur }
        if oa_mv_apply_anim_time(h, Int32(ci), sc.animTime) != 0 {
            sc.reuploadVerticesToGpu(device: device)
            animSlider.doubleValue = Double(sc.animTime)
            animTimeLabel.stringValue = String(format: "%.2f / %.2fs", sc.animTime, dur)
            if !deferRender { requestRender() }
        }
    }

    @objc private func jointChanged() {
        guard let sc = activeScene, sc.hasAssembly, !sc.joints.isEmpty else { return }
        sc.activeJoint = min(max(0, jointPopup.indexOfSelectedItem), sc.joints.count - 1)
        let j = sc.joints[sc.activeJoint]
        jointSlider.minValue = Double(j.low)
        jointSlider.maxValue = Double(j.high)
        jointSlider.doubleValue = Double(j.current)
    }

    @objc private func jointScrubbed() {
        guard let sc = activeScene, sc.hasAssembly, let h = sc.handle, let device,
              sc.activeJoint >= 0 && sc.activeJoint < sc.joints.count else { return }
        let angle = Float(jointSlider.doubleValue)
        if oa_mv_apply_joint(h, Int32(sc.activeJoint), angle) != 0 {
            sc.joints[sc.activeJoint].current = angle
            sc.reuploadVerticesToGpu(device: device)
            renderNow()
        }
    }

    @objc private func explodeScrubbed() {
        guard let sc = activeScene, sc.hasAssembly, let h = sc.handle, let device else { return }
        let e = Float(explodeSlider.doubleValue)
        if oa_mv_apply_explode(h, e) != 0 {
            sc.explodeFactor = e
            sc.reuploadVerticesToGpu(device: device)
            renderNow()
        }
    }

    @objc private func resetPoseClicked() {
        guard let sc = activeScene, sc.hasAssembly, let h = sc.handle, let device else { return }
        if oa_mv_apply_reset_pose(h) != 0 {
            for i in 0..<sc.joints.count { sc.joints[i].current = sc.joints[i].stand }
            sc.explodeFactor = 0
            sc.reuploadVerticesToGpu(device: device)
            syncControlBar()
            renderNow()
        }
    }

    func updateAnimTimer() {
        stopAnimTimer()
        if (activeScene?.animPlaying ?? false) && !(activeScene?.animClips.isEmpty ?? true) {
            onNeedsDisplay?()
        }
    }

    private func stopAnimTimer() {
        animTimer?.invalidate()
        animTimer = nil
    }

    /// Pauses animation tick immediately at 0 Hz when hidden, and releases transient depth target after 10 s idle.
    func setPaneVisible(_ visible: Bool) {
        if visible {
            idleEvictTimer?.invalidate()
            idleEvictTimer = nil
            onNeedsDisplay?()
        } else {
            stopAnimTimer()
            stopGbTimer()
            sceneHeldKeys.removeAll()
            gbHeldKeys.removeAll()
            idleEvictTimer?.invalidate()
            idleEvictTimer = Timer.scheduledTimer(withTimeInterval: 10.0, repeats: false) { [weak self] _ in
                guard let self, self.isHidden else { return }
                self.depthTexture = nil
                self.depthSize = .zero
            }
        }
    }

    private func buildPipelines() {
        guard let device else { return }
        let metalPath = Bundle.main.resourceURL?.appendingPathComponent("Components/metaverse-model.metal").path
        guard let path = metalPath, let src = try? String(contentsOfFile: path, encoding: .utf8) else {
            fputs("MetaverseViewportView: missing Components/metaverse-model.metal\n", stderr)
            return
        }
        let opts = MTLCompileOptions()
        opts.fastMathEnabled = false
        opts.languageVersion = .version2_3
        guard let lib = try? device.makeLibrary(source: src, options: opts),
              let vs = lib.makeFunction(name: "mv_vs"),
              let fs = lib.makeFunction(name: "mv_fs") else {
            fputs("MetaverseViewportView: failed to compile Coax metaverse-model.metal\n", stderr)
            return
        }

        // Exact layout reflected by coax metal model-view-shader.coax:
        // vertex-buffer 30, stride 48: attr0 float3@0, attr1 float3@12, attr2 float2@24, attr3 float4@32
        let vd = MTLVertexDescriptor()
        vd.attributes[0].format = .float3; vd.attributes[0].offset = 0;  vd.attributes[0].bufferIndex = 30
        vd.attributes[1].format = .float3; vd.attributes[1].offset = 12; vd.attributes[1].bufferIndex = 30
        vd.attributes[2].format = .float2; vd.attributes[2].offset = 24; vd.attributes[2].bufferIndex = 30
        vd.attributes[3].format = .float4; vd.attributes[3].offset = 32; vd.attributes[3].bufferIndex = 30
        vd.layouts[30].stride = 48
        vd.layouts[30].stepFunction = .perVertex

        let desc = MTLRenderPipelineDescriptor()
        desc.vertexFunction = vs
        desc.fragmentFunction = fs
        desc.vertexDescriptor = vd
        desc.colorAttachments[0].pixelFormat = .bgra8Unorm
        desc.colorAttachments[0].isBlendingEnabled = true
        desc.colorAttachments[0].rgbBlendOperation = .add
        desc.colorAttachments[0].alphaBlendOperation = .add
        desc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        desc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        desc.colorAttachments[0].sourceAlphaBlendFactor = .one
        desc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        desc.depthAttachmentPixelFormat = .depth32Float

        triPipeline = try? device.makeRenderPipelineState(descriptor: desc)
        linePipeline = triPipeline

        let ds = MTLDepthStencilDescriptor()
        ds.depthCompareFunction = .lessEqual
        ds.isDepthWriteEnabled = true
        depthState = device.makeDepthStencilState(descriptor: ds)
    }

    private func buildDefaultResources() {
        guard let device else { return }
        let td = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: 1, height: 1, mipmapped: false)
        td.usage = .shaderRead
        if let tex = device.makeTexture(descriptor: td) {
            var white: [UInt8] = [255, 255, 255, 255]
            tex.replace(region: MTLRegionMake2D(0, 0, 1, 1), mipmapLevel: 0, withBytes: &white, bytesPerRow: 4)
            defaultWhiteTex = tex
        }
        let sd = MTLSamplerDescriptor()
        sd.minFilter = .linear
        sd.magFilter = .linear
        sd.sAddressMode = .repeat
        sd.tAddressMode = .repeat
        linearSampler = device.makeSamplerState(descriptor: sd)
    }

    private func decodeTexture(_ data: Data) -> MTLTexture? {
        guard let device,
              let src = CGImageSourceCreateWithData(data as CFData, nil),
              let cg = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
        let w = min(cg.width, 2048), h = min(cg.height, 2048)
        guard w > 0, h > 0 else { return nil }
        var rgba = [UInt8](repeating: 255, count: w * h * 4)
        let cs = CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(data: &rgba, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            return nil
        }
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
        let td = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: w, height: h, mipmapped: false)
        td.usage = .shaderRead
        guard let tex = device.makeTexture(descriptor: td) else { return nil }
        tex.replace(region: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0, withBytes: rgba, bytesPerRow: w * 4)
        return tex
    }

    private func buildGridGeometry(zUp: Bool, radius: Float) {
        guard let device else { return }
        let step = max(0.5, pow(10.0, floor(log10(max(0.5, radius * 0.4)))))
        let halfLines = 12
        let extent = step * Float(halfLines)
        var verts: [Float] = []
        verts.reserveCapacity((halfLines * 2 + 1) * 4 * 12)
        func push(_ x: Float, _ y: Float, _ z: Float, _ r: Float, _ g: Float, _ b: Float, _ a: Float) {
            verts.append(contentsOf: [x, y, z, 0, 1, 0, 0, 0, r, g, b, a])
        }
        for i in -halfLines...halfLines {
            let p = Float(i) * step
            let major = (i == 0)
            let c: Float = major ? 0.38 : 0.20
            let a: Float = major ? 0.65 : 0.35
            if zUp {
                push(-extent, p, 0, major ? 0.72 : c, c, c, a)
                push( extent, p, 0, major ? 0.72 : c, c, c, a)
                push(p, -extent, 0, c, major ? 0.72 : c, c, a)
                push(p,  extent, 0, c, major ? 0.72 : c, c, a)
            } else {
                push(-extent, 0, p, major ? 0.72 : c, c, c, a)
                push( extent, 0, p, major ? 0.72 : c, c, c, a)
                push(p, 0, -extent, c, c, major ? 0.78 : c, a)
                push(p, 0,  extent, c, c, major ? 0.78 : c, a)
            }
        }
        gridVertexCount = verts.count / 12
        gridVertexBuffer = device.makeBuffer(bytes: verts, length: verts.count * MemoryLayout<Float>.stride, options: .storageModeShared)
    }

    private func buildBoundsGeometry(minV: SIMD3<Float>, maxV: SIMD3<Float>) {
        guard let device else { return }
        let x0 = minV.x, y0 = minV.y, z0 = minV.z
        let x1 = maxV.x, y1 = maxV.y, z1 = maxV.z
        let corners: [SIMD3<Float>] = [
            SIMD3(x0,y0,z0), SIMD3(x1,y0,z0), SIMD3(x1,y1,z0), SIMD3(x0,y1,z0),
            SIMD3(x0,y0,z1), SIMD3(x1,y0,z1), SIMD3(x1,y1,z1), SIMD3(x0,y1,z1),
        ]
        let edges: [(Int,Int)] = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)]
        var verts: [Float] = []
        verts.reserveCapacity(24 * 12)
        for (a, b) in edges {
            let pa = corners[a], pb = corners[b]
            verts.append(contentsOf: [pa.x, pa.y, pa.z, 0, 1, 0, 0, 0, 0.22, 0.83, 0.75, 0.85])
            verts.append(contentsOf: [pb.x, pb.y, pb.z, 0, 1, 0, 0, 0, 0.22, 0.83, 0.75, 0.85])
        }
        boundsVertexBuffer = device.makeBuffer(bytes: verts, length: verts.count * MemoryLayout<Float>.stride, options: .storageModeShared)
    }

    /// Loads any 3D file using pure-Coax `metaverse-viewer.o` (with ModelIO fallback for USD/USDZ).
    func loadScene(from url: URL, preserveCameraFrom prev: MetaverseTabScene? = nil) -> MetaverseTabScene {
        let scene = MetaverseTabScene(path: url.path)
        let ext = url.pathExtension.lowercased()

        if ext == "usdz" || ext == "usd" || ext == "usda" || ext == "usdc" {
            loadViaModelIO(url: url, into: scene)
        } else {
            loadViaCoaxCore(url: url, into: scene)
        }

        if let prev, prev.path == scene.path {
            scene.target = prev.target
            scene.distance = prev.distance
            scene.azimuth = prev.azimuth
            scene.elevation = prev.elevation
            scene.flyMode = prev.flyMode
            scene.flyPos = prev.flyPos
            scene.shadingMode = prev.shadingMode
            scene.overlayMask = prev.overlayMask
            scene.zSliceNormalized = prev.zSliceNormalized
        } else {
            scene.applyPreset("iso")
        }
        return scene
    }

    private func loadViaCoaxCore(url: URL, into scene: MetaverseTabScene) {
        guard let device else { scene.errorText = "Metal GPU device unavailable."; return }
        guard let raw = try? Data(contentsOf: url) else {
            scene.errorText = "Could not read \(url.lastPathComponent)."
            return
        }
        // For text .gltf, resolve a safe sibling .bin buffer if referenced.
        var extraBin = Data()
        if url.pathExtension.lowercased() == "gltf",
           let obj = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
           let bufs = obj["buffers"] as? [[String: Any]],
           let uri = bufs.first?["uri"] as? String {
            let safe = DesktopCore.withCoaxString(uri) { oa_dc_mv_gltf_uri_safe($0) } != 0
            if safe {
                let sib = url.deletingLastPathComponent().appendingPathComponent(uri).standardizedFileURL
                if sib.path.hasPrefix(url.deletingLastPathComponent().standardizedFileURL.path),
                   let bd = try? Data(contentsOf: sib) {
                    extraBin = bd
                }
            }
        }
        guard let h = oa_mv_alloc() else { scene.errorText = "Out of memory allocating 3D viewer state."; return }
        scene.handle = h
        let ok: Int32 = raw.withUnsafeBytes { rptr in
            extraBin.withUnsafeBytes { eptr in
                oa_mv_load_buf(
                    h, url.path,
                    rptr.bindMemory(to: UInt8.self).baseAddress, Int32(raw.count),
                    eptr.bindMemory(to: UInt8.self).baseAddress, Int32(extraBin.count)
                )
            }
        }
        if ok == 0 {
            var errBuf = [CChar](repeating: 0, count: 256)
            _ = oa_mv_get_err(h, &errBuf, Int32(errBuf.count))
            scene.errorText = String(cString: errBuf)
            return
        }

        let nv = Int(oa_mv_get_vert_count(h))
        let ni = Int(oa_mv_get_index_count(h))
        let nn = Int(oa_mv_get_node_count(h))
        let nm = Int(oa_mv_get_mat_count(h))
        scene.vertexCount = nv
        scene.indexCount = ni
        scene.zUp = oa_mv_is_z_up(h) != 0

        var vbuf = [Float](repeating: 0, count: nv * 12)
        _ = oa_mv_read_verts(h, &vbuf, Int32(vbuf.count))
        scene.vertexBuffer = device.makeBuffer(bytes: vbuf, length: vbuf.count * MemoryLayout<Float>.stride, options: .storageModeShared)

        var ibuf = [UInt32](repeating: 0, count: ni)
        _ = oa_mv_read_indices(h, &ibuf, Int32(ibuf.count))
        scene.indexBuffer = device.makeBuffer(bytes: ibuf, length: ibuf.count * MemoryLayout<UInt32>.stride, options: .storageModeShared)

        var b = [Float](repeating: 0, count: 6)
        if oa_mv_get_bounds(h, &b) != 0 {
            scene.boundsMin = SIMD3<Float>(b[0], b[1], b[2])
            scene.boundsMax = SIMD3<Float>(b[3], b[4], b[5])
        }

        var statsBuf = [CChar](repeating: 0, count: 128)
        _ = oa_mv_get_stats(h, &statsBuf, Int32(statsBuf.count))
        scene.statsText = String(cString: statsBuf)

        var tsvBuf = [CChar](repeating: 0, count: 16384)
        _ = oa_mv_get_tree_tsv(h, &tsvBuf, Int32(tsvBuf.count))
        scene.treeTSV = String(cString: tsvBuf)

        scene.materials = (0..<nm).map { mi in
            var mr = [Float](repeating: 0, count: 6)
            var tx = [Int64](repeating: -1, count: 2)
            _ = oa_mv_get_mat_info(h, Int32(mi), &mr, &tx)
            var mtlTex: MTLTexture?
            if tx[0] >= 0 && tx[1] > 0 && Int(tx[0] + tx[1]) <= raw.count {
                let slice = raw.subdata(in: Int(tx[0])..<Int(tx[0] + tx[1]))
                mtlTex = decodeTexture(slice)
            }
            return MetaverseTabScene.MatInfo(
                baseColor: SIMD4<Float>(mr[0], mr[1], mr[2], mr[3]),
                metallic: mr[4], roughness: mr[5], texture: mtlTex
            )
        }

        scene.nodes = (0..<nn).map { i in
            var meta = [Int64](repeating: 0, count: 5)
            var lbl = [CChar](repeating: 0, count: 64)
            _ = oa_mv_get_node_info(h, Int32(i), &meta, &lbl, Int32(lbl.count))
            return MetaverseTabScene.NodeDraw(
                index: i, firstIndex: Int(meta[0]), indexCount: Int(meta[1]),
                matIndex: Int(meta[2]), visible: meta[3] != 0, selected: meta[4] != 0,
                label: String(cString: lbl)
            )
        }

        // Phase 2 (#14042): realm saved_rev, RBA1 joints, and animation clip metadata
        scene.savedRev = Int(oa_mv_get_saved_rev(h))
        scene.hasAssembly = oa_mv_is_assembly(h) != 0
        if scene.hasAssembly {
            let jc = Int(oa_mv_get_joint_count(h))
            scene.joints = (0..<jc).map { ji in
                var rng = [Float](repeating: 0, count: 4)
                var nm = [CChar](repeating: 0, count: 128)
                _ = oa_mv_get_joint_info(h, Int32(ji), &rng, &nm, Int32(nm.count))
                return MetaverseTabScene.JointInfo(
                    index: ji, name: String(cString: nm),
                    low: rng[0], high: rng[1], stand: rng[2], current: rng[3]
                )
            }
        }
        let ac = Int(oa_mv_get_anim_count(h))
        scene.animClips = (0..<ac).map { ci in
            var dur: Float = 1.0
            var nm = [CChar](repeating: 0, count: 64)
            _ = oa_mv_get_anim_info(h, Int32(ci), &dur, &nm, Int32(nm.count))
            return MetaverseTabScene.AnimClipInfo(index: ci, name: String(cString: nm), duration: dur)
        }
        scene.lastTouchedTick = MetaverseGpuCoordinator.shared.nextTick()
        scene.recomputeGpuBytes()
    }

    private func loadViaModelIO(url: URL, into scene: MetaverseTabScene) {
        guard let device else { scene.errorText = "Metal GPU device unavailable."; return }
        let asset = MDLAsset(url: url)
        asset.loadTextures()
        guard let meshes = asset.childObjects(of: MDLMesh.self) as? [MDLMesh], !meshes.isEmpty else {
            scene.errorText = "ModelIO found no meshes in \(url.lastPathComponent)."
            return
        }
        var verts: [Float] = []
        var indices: [UInt32] = []
        var minV = SIMD3<Float>(repeating: .greatestFiniteMagnitude)
        var maxV = SIMD3<Float>(repeating: -.greatestFiniteMagnitude)
        var tsv = ""
        scene.materials = [MetaverseTabScene.MatInfo(baseColor: SIMD4<Float>(0.84, 0.86, 0.90, 1), metallic: 0.1, roughness: 0.45, texture: nil)]

        for (mi, mesh) in meshes.enumerated() {
            if mesh.vertexAttributeData(forAttributeNamed: MDLVertexAttributeNormal) == nil {
                mesh.addNormals(withAttributeNamed: MDLVertexAttributeNormal, creaseThreshold: 0.5)
            }
            guard let posAttr = mesh.vertexAttributeData(forAttributeNamed: MDLVertexAttributePosition) else { continue }
            let nrmAttr = mesh.vertexAttributeData(forAttributeNamed: MDLVertexAttributeNormal)
            let uvAttr = mesh.vertexAttributeData(forAttributeNamed: MDLVertexAttributeTextureCoordinate)
            let baseVert = UInt32(verts.count / 12)
            let vcount = min(mesh.vertexCount, 100_000)
            for vi in 0..<vcount {
                let pPtr = posAttr.dataStart.advanced(by: vi * posAttr.stride).assumingMemoryBound(to: Float.self)
                let p = SIMD3<Float>(pPtr[0], pPtr[1], pPtr[2])
                minV = min(minV, p); maxV = max(maxV, p)
                var n = SIMD3<Float>(0, 1, 0)
                if let nrmAttr {
                    let nPtr = nrmAttr.dataStart.advanced(by: vi * nrmAttr.stride).assumingMemoryBound(to: Float.self)
                    n = SIMD3<Float>(nPtr[0], nPtr[1], nPtr[2])
                }
                var uv = SIMD2<Float>(0, 0)
                if let uvAttr {
                    let uPtr = uvAttr.dataStart.advanced(by: vi * uvAttr.stride).assumingMemoryBound(to: Float.self)
                    uv = SIMD2<Float>(uPtr[0], uPtr[1])
                }
                verts.append(contentsOf: [p.x, p.y, p.z, n.x, n.y, n.z, uv.x, uv.y, 1, 1, 1, 1])
            }
            let firstIdx = indices.count
            if let subs = mesh.submeshes as? [MDLSubmesh] {
                for sub in subs where sub.geometryType == .triangles {
                    let map = sub.indexBuffer.map()
                    let count = sub.indexCount
                    if sub.indexType == .uInt16 {
                        let ptr = map.bytes.assumingMemoryBound(to: UInt16.self)
                        for k in 0..<count { indices.append(baseVert + UInt32(ptr[k])) }
                    } else if sub.indexType == .uInt32 {
                        let ptr = map.bytes.assumingMemoryBound(to: UInt32.self)
                        for k in 0..<count { indices.append(baseVert + ptr[k]) }
                    }
                }
            }
            let ic = indices.count - firstIdx
            let name = mesh.name.isEmpty ? "mesh_\(mi)" : mesh.name
            scene.nodes.append(.init(index: mi, firstIndex: firstIdx, indexCount: ic, matIndex: 0, visible: true, selected: false, label: name))
            tsv += "0\t1\t0\t\(name)\t\(name)\n"
        }
        guard !indices.isEmpty else { scene.errorText = "USD asset has no triangles."; return }
        scene.vertexCount = verts.count / 12
        scene.indexCount = indices.count
        scene.boundsMin = minV
        scene.boundsMax = maxV
        scene.treeTSV = tsv
        scene.statsText = "\(indices.count / 3) tris · \(scene.nodes.count) nodes · USD"
        scene.vertexBuffer = device.makeBuffer(bytes: verts, length: verts.count * MemoryLayout<Float>.stride, options: .storageModeShared)
        scene.indexBuffer = device.makeBuffer(bytes: indices, length: indices.count * MemoryLayout<UInt32>.stride, options: .storageModeShared)
        scene.lastTouchedTick = MetaverseGpuCoordinator.shared.nextTick()
        scene.recomputeGpuBytes()
    }

    func setActiveScene(_ scene: MetaverseTabScene?) {
        activeGameBuilder = nil
        activeGtaWorkbench = nil
        gtaBar.isHidden = true
        gbHudLabel.isHidden = true
        stopGbTimer()
        sceneHeldKeys.removeAll()
        activeScene = scene
        if let scene {
            if let device { scene.ensureGpuBuffers(device: device) }
            scene.lastTouchedTick = MetaverseGpuCoordinator.shared.nextTick()
            buildGridGeometry(zUp: scene.zUp, radius: scene.boundsRadius)
            buildBoundsGeometry(minV: scene.boundsMin, maxV: scene.boundsMax)
            hudLabel.stringValue = " \(scene.statsText) "
            hudLabel.isHidden = (scene.overlayMask & 16) == 0 || scene.statsText.isEmpty
        } else {
            hudLabel.isHidden = true
        }
        syncControlBar()
        requestRender()
    }

    func setActiveGameBuilder(_ session: GameBuilderSession?) {
        activeScene = nil
        activeGtaWorkbench = nil
        gtaBar.isHidden = true
        stopAnimTimer()
        controlBar.isHidden = true
        sceneHeldKeys.removeAll()
        activeGameBuilder = session
        if let session, let h = session.handle {
            if bounds.width > 1 && bounds.height > 1 {
                oa_gb_set_size(h, Float(bounds.width), Float(bounds.height))
            }
            let snap = session.snapshotUi()
            hudLabel.stringValue = " GameBuilder · \(snap.actorCount) actors · rev \(snap.savedRev) "
            hudLabel.isHidden = false
            updateGbTimer()
        } else {
            hudLabel.isHidden = true
            gbHudLabel.isHidden = true
            stopGbTimer()
        }
        requestRender()
    }

    func setActiveGtaWorkbench(_ session: GtaWorkbenchSession?) {
        activeScene = nil
        activeGameBuilder = nil
        stopAnimTimer()
        stopGbTimer()
        controlBar.isHidden = true
        gbHudLabel.isHidden = true
        sceneHeldKeys.removeAll()
        activeGtaWorkbench = session
        if let session {
            session.onFrameTextureReady = { [weak self, weak session] tex in
                guard let self, self.activeGtaWorkbench === session else { return }
                self.metalLayer.isHidden = true
                self.layer?.backgroundColor = NSColor.clear.cgColor
                self.layer?.mask = nil
                self.onExternalTextureUpdated?(tex)
                if let s = session { self.hudLabel.stringValue = s.hudSummary }
            }
            session.onReady = { [weak self, weak session] in
                guard let self, self.activeGtaWorkbench === session else { return }
                self.syncGtaBar()
                if let s = session { self.hudLabel.stringValue = s.hudSummary }
                self.onNeedsDisplay?()
            }
            session.onStateChanged = { [weak self, weak session] in
                guard let self, self.activeGtaWorkbench === session else { return }
                self.syncGtaBar()
                if let s = session { self.hudLabel.stringValue = s.hudSummary }
            }
            if bounds.width > 1 && bounds.height > 1 {
                session.requestResize(size: bounds.size)
            }
            hudLabel.stringValue = session.hudSummary
            hudLabel.isHidden = false
            syncGtaBar()
            if let existing = session.latestTexture {
                onExternalTextureUpdated?(existing)
            }
            onNeedsDisplay?()
        } else {
            gtaBar.isHidden = true
            hudLabel.isHidden = true
        }
    }

    /// True whenever the viewport needs the window's native CADisplayLink (60-120 Hz ProMotion).
    var wantsContinuousFrame: Bool {
        guard !isHidden, window != nil, !(window?.isMiniaturized ?? false) else { return false }
        if let gta = activeGtaWorkbench, !gta.loading, gta.errorText.isEmpty { return true }
        if wantsRenderOnNextVsync { return true }
        if !sceneHeldKeys.isEmpty { return true }
        if let sc = activeScene, sc.animPlaying, !sc.animClips.isEmpty { return true }
        if let gb = activeGameBuilder {
            if gb.snapshotUi().playing || gbRightDown || !gbHeldKeys.isEmpty { return true }
        }
        return false
    }

    func requestRender() {
        wantsRenderOnNextVsync = true
        onNeedsDisplay?()
    }

    /// Runs locked to CoderWindow's native ProMotion CADisplayLink (120 Hz) for zero-jitter rendering & WASD navigation.
    func stepDisplayLink(dt rawDt: Float) {
        guard !isHidden, bounds.width > 1, bounds.height > 1 else { return }
        let dt = min(0.05, max(0.001, rawDt))

        if let gta = activeGtaWorkbench {
            if !gta.loading && gta.errorText.isEmpty {
                if !sceneHeldKeys.isEmpty {
                    let boost: Float = NSEvent.modifierFlags.contains(.shift) ? 2.2 : 1.0
                    let fwd: Float = (sceneHeldKeys.contains("w") ? 1 : 0) - (sceneHeldKeys.contains("s") ? 1 : 0)
                    let right: Float = (sceneHeldKeys.contains("d") ? 1 : 0) - (sceneHeldKeys.contains("a") ? 1 : 0)
                    let up: Float = (sceneHeldKeys.contains("e") ? 1 : 0) - (sceneHeldKeys.contains("q") ? 1 : 0)
                    if fwd != 0 || right != 0 || up != 0 {
                        gta.moveWasd(fwd: fwd * boost, right: right * boost, up: up * boost, dt: dt)
                    }
                }
                gta.stepAsync(dt: dt)
            }
            hudLabel.stringValue = gta.hudSummary
            return
        }

        if let sc = activeScene {
            var dirty = wantsRenderOnNextVsync
            if !sceneHeldKeys.isEmpty {
                let boost: Float = NSEvent.modifierFlags.contains(.shift) ? 2.4 : 1.0
                let fwdIn: Float = (sceneHeldKeys.contains("w") ? 1 : 0) - (sceneHeldKeys.contains("s") ? 1 : 0)
                let rightIn: Float = (sceneHeldKeys.contains("d") ? 1 : 0) - (sceneHeldKeys.contains("a") ? 1 : 0)
                let upIn: Float = (sceneHeldKeys.contains("e") ? 1 : 0) - (sceneHeldKeys.contains("q") ? 1 : 0)
                if fwdIn != 0 || rightIn != 0 || upIn != 0 {
                    let e = sc.eyePosition
                    let f = simd_normalize(sc.lookTarget - e)
                    let upHint = sc.zUp ? SIMD3<Float>(0, 0, 1) : SIMD3<Float>(0, 1, 0)
                    let r = simd_normalize(simd_cross(f, upHint))
                    let spd = max(0.35, sc.boundsRadius * 1.35) * boost * dt
                    let delta = f * (fwdIn * spd) + r * (rightIn * spd) + upHint * (upIn * spd)
                    if sc.flyMode {
                        sc.flyPos += delta
                    } else {
                        sc.target += delta
                    }
                    dirty = true
                }
            }
            if sc.animPlaying && !sc.animClips.isEmpty {
                applyAnimTime(sc.animTime + dt, deferRender: true)
                dirty = true
            }
            if dirty {
                wantsRenderOnNextVsync = false
                renderNow()
            }
            return
        }

        if let gb = activeGameBuilder, let h = gb.handle {
            var dirty = wantsRenderOnNextVsync
            let snap = gb.snapshotUi()
            if snap.playing || gbRightDown || !gbHeldKeys.isEmpty {
                let moved = oa_gb_step_tick(h, dt) != 0
                if oa_gb_is_world_drawn(h) != 0 {
                    let scale = Float(window?.backingScaleFactor ?? 2.0)
                    _ = oa_gb_step_world(h, Float(max(1, bounds.width)), Float(max(1, bounds.height)), scale, dt)
                }
                GameBuilderAudioPlayer.shared.playEvents(from: h, muted: gb.muted)
                if moved || oa_gb_consume_dirty(h) != 0 { dirty = true }
            }
            if dirty {
                wantsRenderOnNextVsync = false
                renderNow()
            }
        }
    }

    func updateGbTimer() {
        stopGbTimer()
        if activeGameBuilder != nil { onNeedsDisplay?() }
    }

    private func stopGbTimer() {
        gbTickTimer?.invalidate()
        gbTickTimer = nil
    }

    func setHoles(_ rects: [NSRect]) {
        holeRects = rects
        let path = CGMutablePath()
        path.addRect(bounds)
        for r in rects { path.addRect(r) }
        maskLayer.frame = bounds
        maskLayer.path = path
        layer?.mask = rects.isEmpty ? nil : maskLayer
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let local = convert(point, from: superview)
        for h in holeRects where h.contains(local) { return nil }
        return super.hitTest(point)
    }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metalLayer.frame = bounds
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        metalLayer.contentsScale = scale
        let pw = max(1, Int((bounds.width * scale).rounded()))
        let ph = max(1, Int((bounds.height * scale).rounded()))
        if Int(metalLayer.drawableSize.width) != pw || Int(metalLayer.drawableSize.height) != ph {
            metalLayer.drawableSize = CGSize(width: pw, height: ph)
        }
        CATransaction.commit()
        if let gb = activeGameBuilder, let h = gb.handle, bounds.width > 1, bounds.height > 1 {
            oa_gb_set_size(h, Float(bounds.width), Float(bounds.height))
        }
        if let gta = activeGtaWorkbench, bounds.width > 1, bounds.height > 1 {
            gta.requestResize(size: bounds.size)
        }
        if !holeRects.isEmpty { setHoles(holeRects) }
        requestRender()
    }

    private func ensureDepthTexture(width: Int, height: Int) {
        guard let device else { return }
        if Int(depthSize.width) == width && Int(depthSize.height) == height && depthTexture != nil { return }
        let td = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
        td.storageMode = .private
        td.usage = .renderTarget
        depthTexture = device.makeTexture(descriptor: td)
        depthSize = CGSize(width: width, height: height)
    }

    private func encodeScene(into enc: MTLRenderCommandEncoder, scene: MetaverseTabScene, aspect: Float) {
        guard let triPipeline, let depthState, let whiteTex = defaultWhiteTex, let linearSampler else { return }
        let viewM = scene.viewMatrix()
        let projM = scene.projMatrix(aspect: aspect)
        let mvp = projM * viewM
        let model = matrix_identity_float4x4
        let eye = scene.eyePosition
        var lengths: [UInt32] = [48]

        func fillUniforms(baseColor: SIMD4<Float>, metallic: Float, roughness: Float, hasTex: Bool, selected: Bool, modeOverride: Int? = nil) -> [Float] {
            var u = [Float](repeating: 0, count: 48)
            withUnsafeBytes(of: mvp) { ptr in
                let f = ptr.bindMemory(to: Float.self)
                for k in 0..<16 { u[k] = f[k] }
            }
            withUnsafeBytes(of: model) { ptr in
                let f = ptr.bindMemory(to: Float.self)
                for k in 0..<16 { u[16 + k] = f[k] }
            }
            u[32] = eye.x; u[33] = eye.y; u[34] = eye.z
            u[35] = Float(modeOverride ?? scene.shadingMode)
            u[36] = 0.45; u[37] = scene.zUp ? 0.55 : 0.82; u[38] = scene.zUp ? 0.70 : 0.35
            let sliceEnabled = (scene.overlayMask & 4) != 0
            u[39] = sliceEnabled ? (scene.boundsMin.z + scene.boundsExtent.z * scene.zSliceNormalized) : 100000.0
            u[40] = baseColor.x; u[41] = baseColor.y; u[42] = baseColor.z; u[43] = baseColor.w
            u[44] = metallic; u[45] = roughness
            u[46] = hasTex ? 1.0 : 0.0
            u[47] = selected ? 1.0 : 0.0
            return u
        }

        enc.setRenderPipelineState(triPipeline)
        enc.setDepthStencilState(depthState)
        enc.setFragmentSamplerState(linearSampler, index: 0)
        enc.setVertexBytes(&lengths, length: 4, index: 29)
        enc.setFragmentBytes(&lengths, length: 4, index: 29)

        // Optional studio ground grid
        if (scene.overlayMask & 1) != 0, let gridBuf = gridVertexBuffer, gridVertexCount > 0 {
            var gu = fillUniforms(baseColor: SIMD4<Float>(1, 1, 1, 1), metallic: 0, roughness: 1, hasTex: false, selected: false, modeOverride: 4)
            enc.setTriangleFillMode(.fill)
            enc.setVertexBuffer(gridBuf, offset: 0, index: 30)
            enc.setVertexBytes(&gu, length: gu.count * 4, index: 0)
            enc.setFragmentBytes(&gu, length: gu.count * 4, index: 0)
            enc.setFragmentTexture(whiteTex, index: 0)
            enc.drawPrimitives(type: .line, vertexStart: 0, vertexCount: gridVertexCount)
        }

        // Optional bounding box wireframe
        if (scene.overlayMask & 2) != 0, let bBuf = boundsVertexBuffer {
            var bu = fillUniforms(baseColor: SIMD4<Float>(1, 1, 1, 1), metallic: 0, roughness: 1, hasTex: false, selected: false, modeOverride: 4)
            enc.setTriangleFillMode(.fill)
            enc.setVertexBuffer(bBuf, offset: 0, index: 30)
            enc.setVertexBytes(&bu, length: bu.count * 4, index: 0)
            enc.setFragmentBytes(&bu, length: bu.count * 4, index: 0)
            enc.setFragmentTexture(whiteTex, index: 0)
            enc.drawPrimitives(type: .line, vertexStart: 0, vertexCount: 24)
        }

        // Scene mesh nodes
        guard let vb = scene.vertexBuffer, let ib = scene.indexBuffer, scene.indexCount > 0 else { return }
        enc.setVertexBuffer(vb, offset: 0, index: 30)
        enc.setTriangleFillMode(scene.shadingMode == 4 ? .lines : .fill)

        for nd in scene.nodes where nd.visible && nd.indexCount >= 3 {
            let mat = nd.matIndex >= 0 && nd.matIndex < scene.materials.count
                ? scene.materials[nd.matIndex]
                : MetaverseTabScene.MatInfo(baseColor: SIMD4<Float>(0.85, 0.87, 0.90, 1), metallic: 0.05, roughness: 0.55, texture: nil)
            let tex = mat.texture ?? whiteTex
            var u = fillUniforms(
                baseColor: mat.baseColor, metallic: mat.metallic, roughness: mat.roughness,
                hasTex: mat.texture != nil, selected: nd.selected || nd.index == scene.selectedNode
            )
            enc.setVertexBytes(&u, length: u.count * 4, index: 0)
            enc.setFragmentBytes(&u, length: u.count * 4, index: 0)
            enc.setFragmentTexture(tex, index: 0)
            enc.drawIndexedPrimitives(
                type: .triangle,
                indexCount: nd.indexCount,
                indexType: .uint32,
                indexBuffer: ib,
                indexBufferOffset: nd.firstIndex * MemoryLayout<UInt32>.stride
            )
        }
    }

    /// Encodes GameBuilder's Coax instance stream (unit cube + .gbm meshes + selection/claims/changes/playtest lines)
    /// using our triple-target Coax Metal shader (Components/gb-instances.metal).
    private func encodeGameBuilder(into enc: MTLRenderCommandEncoder, session: GameBuilderSession) {
        guard let h = session.handle else { return }
        var cam = [Float](repeating: 0, count: 20)
        guard oa_gb_get_camera(h, &cam) != 0 else { return }
        let instCount = Int(oa_gb_get_draw_instances(h, &gbInstanceFloats, Int32(gbInstanceFloats.count)))
        guard instCount > 0 else { return }
        encodeGameBuilderCore(into: enc, session: session, cam: cam, instFloats: gbInstanceFloats, instCount: instCount, updateHud: true)
    }

    private func encodeGameBuilderCore(into enc: MTLRenderCommandEncoder, session: GameBuilderSession, cam: [Float], instFloats: [Float], instCount: Int, updateHud: Bool) {
        guard let gbPipeline, let depthState, let cubeBuf = gbCubeBuffer, let instBuf = gbInstanceBuffer, let h = session.handle else { return }

        // Group instances by mesh_id (0 = unit cube, >0 = hydrated .gbm mesh) for batched instanced drawing
        var orderedFloats: [Float] = []
        orderedFloats.reserveCapacity(instCount * 20)
        var batches: [(meshId: Int32, start: Int, count: Int)] = []
        var byMesh: [Int32: [Int]] = [:]
        for i in 0..<instCount {
            let mid = Int32(instFloats[i * 20 + 19].rounded())
            byMesh[mid, default: []].append(i)
        }
        for mid in byMesh.keys.sorted() {
            let idxs = byMesh[mid]!
            let start = orderedFloats.count / 20
            for idx in idxs {
                let base = idx * 20
                for k in 0..<20 { orderedFloats.append(instFloats[base + k]) }
            }
            batches.append((mid, start, idxs.count))
        }
        let byteLen = min(instBuf.length, orderedFloats.count * MemoryLayout<Float>.stride)
        memcpy(instBuf.contents(), orderedFloats, byteLen)

        var sky = [Float](repeating: 0, count: 12)
        let hasSky = oa_gb_get_sky(h, &sky) != 0
        var u = [Float](repeating: 0, count: 32)
        for k in 0..<20 { u[k] = cam[k] }
        u[20] = hasSky ? sky[0] : 0.08
        u[21] = hasSky ? sky[1] : 0.11
        u[22] = hasSky ? sky[2] : 0.18
        var lengths: [UInt32] = [32, UInt32(orderedFloats.count)]

        enc.setRenderPipelineState(gbPipeline)
        enc.setDepthStencilState(depthState)
        enc.setTriangleFillMode(.fill)
        enc.setVertexBuffer(instBuf, offset: 0, index: 1)
        enc.setVertexBytes(&lengths, length: 8, index: 29)
        enc.setFragmentBytes(&lengths, length: 8, index: 29)

        for b in batches {
            u[23] = Float(b.start)
            enc.setVertexBytes(&u, length: u.count * 4, index: 0)
            enc.setFragmentBytes(&u, length: u.count * 4, index: 0)
            if b.meshId > 0, let gbmBuf = session.gbmVertexBuffer, let rng = session.gbmMeshRanges[b.meshId] {
                enc.setVertexBuffer(gbmBuf, offset: 0, index: 30)
                enc.drawPrimitives(type: .triangle, vertexStart: rng.first, vertexCount: rng.count, instanceCount: b.count)
            } else {
                enc.setVertexBuffer(cubeBuf, offset: 0, index: 30)
                enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 36, instanceCount: b.count)
            }
        }

        guard updateHud else { return }
        // Update Play HUD badge when playing
        var hud = [Float](repeating: 0, count: 80)
        if oa_gb_get_play_hud(h, &hud, Int32(hud.count)) != 0 && hud[0] > 0.5 {
            let hp = Int(hud[2].rounded())
            let maxHp = max(1, Int(hud[3].rounded()))
            let mine = Int(hud[4].rounded())
            let rival = Int(hud[5].rounded())
            let secs = Int(max(0, hud[6]).rounded())
            gbHudLabel.stringValue = "  PLAY  ·  Score \(mine)–\(rival)  ·  HP \(hp)/\(maxHp)  ·  \(secs)s  ·  Esc to Build  "
            gbHudLabel.isHidden = false
        } else {
            gbHudLabel.isHidden = true
        }
    }

    /// Renders GameBuilder's staged pictures.coax camera + instances into an offscreen BGRA8 buffer
    /// for before/after JPEG encoding via oa_gb_submit_pic (#14044).
    func renderGameBuilderStagedPictureBGRA(session: GameBuilderSession, width: Int = 320, height: Int = 192) -> [UInt8]? {
        guard let device, let queue, let h = session.handle else { return nil }
        var cam = [Float](repeating: 0, count: 20)
        guard oa_gb_get_pic_camera(h, &cam) != 0 else { return nil }
        let instCount = Int(oa_gb_get_pic_draw_instances(h, &gbInstanceFloats, Int32(gbInstanceFloats.count)))
        guard instCount > 0 else { return nil }

        let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        cd.usage = [.renderTarget, .shaderRead]
        cd.storageMode = .shared
        let dd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
        dd.usage = .renderTarget
        dd.storageMode = .private
        guard let colorTex = device.makeTexture(descriptor: cd),
              let dTex = device.makeTexture(descriptor: dd),
              let cb = queue.makeCommandBuffer() else { return nil }
        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = colorTex
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.055, green: 0.062, blue: 0.078, alpha: 1.0)
        rp.depthAttachment.texture = dTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0
        if let enc = cb.makeRenderCommandEncoder(descriptor: rp) {
            enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(width), height: Double(height), znear: 0, zfar: 1))
            encodeGameBuilderCore(into: enc, session: session, cam: cam, instFloats: gbInstanceFloats, instCount: instCount, updateHud: false)
            enc.endEncoding()
        }
        cb.commit()
        cb.waitUntilCompleted()

        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        colorTex.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        return bytes
    }

    private func nextExtColorTexture(width: Int, height: Int) -> MTLTexture? {
        guard let device, width >= 1, height >= 1, width <= 4096, height <= 4096 else { return nil }
        let targetSize = CGSize(width: width, height: height)
        if extColorTextures.count != 3 || extColorSize != targetSize {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
            d.usage = [.renderTarget, .shaderRead]
            d.storageMode = .private
            var ring: [MTLTexture] = []
            for _ in 0..<3 {
                guard let t = device.makeTexture(descriptor: d) else { return nil }
                ring.append(t)
            }
            extColorTextures = ring
            extColorSize = targetSize
            extColorRingIdx = 0
        }
        let tex = extColorTextures[extColorRingIdx % extColorTextures.count]
        extColorRingIdx = (extColorRingIdx + 1) % extColorTextures.count
        return tex
    }

    func renderNow() {
        guard !isHidden, bounds.width > 1, bounds.height > 1 else { return }
        if activeGtaWorkbench != nil {
            onNeedsDisplay?()
            return
        }
        guard let queue, (activeScene != nil || activeGameBuilder != nil) else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        let reqW = max(1, Int((bounds.width * scale).rounded()))
        let reqH = max(1, Int((bounds.height * scale).rounded()))

        // composition-v2 (#14046): render asynchronously into 3-slot ping-pong private GPU textures for UcPlan slot 0.
        // Never stalls CPU; publishes completed texture via addCompletedHandler so ui-host.m composites at 120 Hz tear-free.
        if useCompositionV2, reqW <= 4096, reqH <= 4096, let extTex = nextExtColorTexture(width: reqW, height: reqH) {
            guard renderFlightSem.wait(timeout: .now()) == .success else {
                wantsRenderOnNextVsync = true
                return
            }
            ensureDepthTexture(width: reqW, height: reqH)
            guard let depthTexture, let cb = queue.makeCommandBuffer() else {
                renderFlightSem.signal()
                return
            }
            let rp = MTLRenderPassDescriptor()
            rp.colorAttachments[0].texture = extTex
            rp.colorAttachments[0].loadAction = .clear
            rp.colorAttachments[0].storeAction = .store
            rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.055, green: 0.062, blue: 0.078, alpha: 1.0)
            rp.depthAttachment.texture = depthTexture
            rp.depthAttachment.loadAction = .clear
            rp.depthAttachment.storeAction = .dontCare
            rp.depthAttachment.clearDepth = 1.0
            if let enc = cb.makeRenderCommandEncoder(descriptor: rp) {
                enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(reqW), height: Double(reqH), znear: 0, zfar: 1))
                if let gb = activeGameBuilder {
                    encodeGameBuilder(into: enc, session: gb)
                } else if let scene = activeScene {
                    encodeScene(into: enc, scene: scene, aspect: Float(reqW) / max(1, Float(reqH)))
                }
                enc.endEncoding()
            }
            let sem = renderFlightSem
            cb.addCompletedHandler { [weak self] _ in
                sem.signal()
                DispatchQueue.main.async { [weak self] in
                    guard let self, !self.isHidden, self.activeGtaWorkbench == nil else { return }
                    let now = CACurrentMediaTime()
                    if self.lastRenderWall > 0 {
                        let fdt = now - self.lastRenderWall
                        if fdt > 0.001 && fdt < 0.5 {
                            self.smoothFps = self.smoothFps > 0 ? (self.smoothFps * 0.85 + (1.0 / fdt) * 0.15) : (1.0 / fdt)
                        }
                    }
                    self.lastRenderWall = now
                    self.metalLayer.isHidden = true
                    self.layer?.backgroundColor = NSColor.clear.cgColor
                    self.layer?.mask = nil
                    self.onExternalTextureUpdated?(extTex)
                    self.updateHudLabel()
                }
            }
            cb.commit()
            return
        }

        // Fallback when window > 4096px or composition-v2 disabled: present directly on CAMetalLayer
        metalLayer.isHidden = false
        onExternalTextureUpdated?(nil)
        guard let drawable = metalLayer.nextDrawable() else { return }
        let w = drawable.texture.width, h = drawable.texture.height
        ensureDepthTexture(width: w, height: h)
        guard let depthTexture, let cb = queue.makeCommandBuffer() else { return }

        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = drawable.texture
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.055, green: 0.062, blue: 0.078, alpha: 1.0)
        rp.depthAttachment.texture = depthTexture
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0

        if let enc = cb.makeRenderCommandEncoder(descriptor: rp) {
            enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(w), height: Double(h), znear: 0, zfar: 1))
            if let gb = activeGameBuilder {
                encodeGameBuilder(into: enc, session: gb)
            } else if let scene = activeScene {
                encodeScene(into: enc, scene: scene, aspect: Float(w) / max(1, Float(h)))
            }
            enc.endEncoding()
        }
        cb.present(drawable)
        cb.commit()
        updateHudLabel()
    }

    private func updateHudLabel() {
        if let gta = activeGtaWorkbench {
            hudLabel.stringValue = gta.hudSummary
            hudLabel.isHidden = false
        } else if let scene = activeScene {
            let fpsStr = smoothFps >= 5.0 ? String(format: " · %.0f FPS", min(120.0, smoothFps)) : ""
            hudLabel.stringValue = " \(scene.statsText)\(fpsStr) "
            hudLabel.isHidden = (scene.overlayMask & 16) == 0 || scene.statsText.isEmpty
        } else if let gb = activeGameBuilder {
            let snap = gb.snapshotUi()
            let suffix = gb.farmLoaded ? " · Farm" : (gb.gbmLoaded ? " · GBM" : "")
            let fpsStr = smoothFps >= 5.0 ? String(format: " · %.0f FPS", min(120.0, smoothFps)) : ""
            hudLabel.stringValue = " \(gb.gameId.isEmpty ? "GameBuilder" : gb.gameId) · \(snap.actorCount) actors · rev \(snap.savedRev)\(suffix)\(fpsStr) "
            hudLabel.isHidden = false
        }
    }

    /// Renders an offscreen PNG snapshot of the active 3D scene, GameBuilder, or GTA VI Metal 4 workbench for Snapshot-to-Chat.
    func capturePNG(width: Int = 1280, height: Int = 800) -> Data? {
        if let gta = activeGtaWorkbench {
            return gta.capturePngData(spp: 64)
        }
        guard let device, let queue, (activeScene != nil || activeGameBuilder != nil) else { return nil }
        let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        cd.usage = [.renderTarget, .shaderRead]
        cd.storageMode = .shared
        let dd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
        dd.usage = .renderTarget
        dd.storageMode = .private
        guard let colorTex = device.makeTexture(descriptor: cd),
              let dTex = device.makeTexture(descriptor: dd),
              let cb = queue.makeCommandBuffer() else { return nil }
        let rp = MTLRenderPassDescriptor()
        rp.colorAttachments[0].texture = colorTex
        rp.colorAttachments[0].loadAction = .clear
        rp.colorAttachments[0].storeAction = .store
        rp.colorAttachments[0].clearColor = MTLClearColor(red: 0.055, green: 0.062, blue: 0.078, alpha: 1.0)
        rp.depthAttachment.texture = dTex
        rp.depthAttachment.loadAction = .clear
        rp.depthAttachment.storeAction = .dontCare
        rp.depthAttachment.clearDepth = 1.0
        if let enc = cb.makeRenderCommandEncoder(descriptor: rp) {
            enc.setViewport(MTLViewport(originX: 0, originY: 0, width: Double(width), height: Double(height), znear: 0, zfar: 1))
            if let gb = activeGameBuilder {
                encodeGameBuilder(into: enc, session: gb)
            } else if let scene = activeScene {
                encodeScene(into: enc, scene: scene, aspect: Float(width) / Float(height))
            }
            enc.endEncoding()
        }
        cb.commit()
        cb.waitUntilCompleted()

        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        colorTex.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        let cs = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue
        guard let prov = CGDataProvider(data: Data(bytes) as CFData),
              let cg = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: cs, bitmapInfo: CGBitmapInfo(rawValue: bitmapInfo), provider: prov, decode: nil, shouldInterpolate: false, intent: .defaultIntent) else {
            return nil
        }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, cg, nil)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return out as Data
    }

    // MARK: - Pointer & Keyboard Forwarding (3D Viewer + Embedded GameBuilder Core)

    private func gbPoint(from event: NSEvent) -> (Float, Float) {
        let p = convert(event.locationInWindow, from: nil)
        return (Float(p.x), Float(bounds.height - p.y))
    }

    private func gbMods(from flags: NSEvent.ModifierFlags) -> Int32 {
        var m: Int32 = 0
        if flags.contains(.shift)   { m |= 1 }
        if flags.contains(.option)  { m |= 2 }
        if flags.contains(.command) { m |= 4 }
        if flags.contains(.control) { m |= 8 }
        return m
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        onFocusRequested?()
        optionClickPending = event.modifierFlags.contains(.option)
        if activeGtaWorkbench != nil {
            let p = convert(event.locationInWindow, from: nil)
            downMouse = p
            lastMouse = p
            dragging = true
            dragPanning = event.modifierFlags.contains(.shift) || event.modifierFlags.contains(.option)
            return
        }
        if let gb = activeGameBuilder, let h = gb.handle {
            oa_gb_set_claims_clock(h, Int64(Date().timeIntervalSince1970))
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_down(h, 0, x, y, gbMods(from: event.modifierFlags))
            if gb.snapshotUi().playing { oa_gb_set_play_fire(h, 1) }
            requestRender()
            return
        }
        let p = convert(event.locationInWindow, from: nil)
        downMouse = p
        lastMouse = p
        dragging = true
        dragPanning = event.modifierFlags.contains(.shift) || event.modifierFlags.contains(.option)
    }

    override func rightMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        onFocusRequested?()
        if activeGtaWorkbench != nil {
            let p = convert(event.locationInWindow, from: nil)
            downMouse = p
            lastMouse = p
            dragging = true
            dragPanning = true
            return
        }
        if let gb = activeGameBuilder, let h = gb.handle {
            gbRightDown = true
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_down(h, 1, x, y, gbMods(from: event.modifierFlags))
            updateGbTimer()
            requestRender()
            return
        }
        let p = convert(event.locationInWindow, from: nil)
        downMouse = p
        lastMouse = p
        dragging = true
        dragPanning = true
    }

    override func otherMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        onFocusRequested?()
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_down(h, 2, x, y, gbMods(from: event.modifierFlags))
            requestRender()
        }
    }

    override func mouseDragged(with event: NSEvent) {
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            if gb.snapshotUi().playing {
                oa_gb_set_play_look(h, Float(event.deltaX) * 0.004, Float(event.deltaY) * 0.004)
            } else {
                oa_gb_mouse_move(h, x, y)
            }
            requestRender()
            return
        }
        handleDrag(event)
    }

    override func rightMouseDragged(with event: NSEvent) {
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_move(h, x, y)
            requestRender()
            return
        }
        handleDrag(event)
    }

    override func otherMouseDragged(with event: NSEvent) {
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_move(h, x, y)
            requestRender()
        }
    }

    private func handleDrag(_ event: NSEvent) {
        guard dragging else { return }
        let p = convert(event.locationInWindow, from: nil)
        let dx = Float(p.x - lastMouse.x)
        let dy = Float(p.y - lastMouse.y)
        lastMouse = p

        if let gta = activeGtaWorkbench {
            if dragPanning {
                gta.pan(dx: dx, dy: dy)
            } else {
                gta.orbit(dx: dx, dy: dy)
            }
            onNeedsDisplay?()
            return
        }
        guard let scene = activeScene else { return }
        if dragPanning && !scene.flyMode {
            let e = scene.eyePosition
            let f = simd_normalize(scene.target - e)
            let upHint = scene.zUp ? SIMD3<Float>(0, 0, 1) : SIMD3<Float>(0, 1, 0)
            let r = simd_normalize(simd_cross(f, upHint))
            let u = simd_cross(r, f)
            let scale = scene.distance * 0.0018
            scene.target -= r * (dx * scale)
            scene.target -= u * (dy * scale)
        } else {
            scene.azimuth -= dx * 0.008
            scene.elevation = max(-1.52, min(1.52, scene.elevation - dy * 0.008))
        }
        requestRender()
    }

    override func mouseUp(with event: NSEvent) {
        let wasOption = optionClickPending
        optionClickPending = false
        if activeGtaWorkbench != nil {
            dragging = false; dragPanning = false
            return
        }
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_up(h, 0, x, y)
            if gb.snapshotUi().playing { oa_gb_set_play_fire(h, 0) }
            gb.flushSaveIfDue()
            onGameBuilderMutated?()
            if wasOption && !gb.snapshotUi().playing {
                onAttachSelectionRequested?()
            }
            requestRender()
            return
        }
        defer { dragging = false; dragPanning = false }
        let p = convert(event.locationInWindow, from: nil)
        if hypot(p.x - downMouse.x, p.y - downMouse.y) < 4.0 {
            performPick(at: p)
            if wasOption { onAttachSelectionRequested?() }
        }
    }

    override func rightMouseUp(with event: NSEvent) {
        if let gb = activeGameBuilder, let h = gb.handle {
            gbRightDown = false
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_up(h, 1, x, y)
            updateGbTimer()
            requestRender()
            return
        }
        dragging = false; dragPanning = false
    }

    override func otherMouseUp(with event: NSEvent) {
        if let gb = activeGameBuilder, let h = gb.handle {
            let (x, y) = gbPoint(from: event)
            oa_gb_mouse_up(h, 2, x, y)
            requestRender()
        }
    }

    override func scrollWheel(with event: NSEvent) {
        if let gta = activeGtaWorkbench {
            gta.zoom(delta: Float(event.scrollingDeltaY))
            onNeedsDisplay?()
            return
        }
        if let gb = activeGameBuilder, let h = gb.handle {
            oa_gb_scroll_wheel(h, Float(event.scrollingDeltaY) * 0.15)
            requestRender()
            return
        }
        guard let scene = activeScene else { return }
        let factor: Float = exp(Float(-event.scrollingDeltaY) * 0.012)
        if scene.flyMode {
            let dir = simd_normalize(scene.lookTarget - scene.flyPos)
            scene.flyPos += dir * Float(event.scrollingDeltaY) * scene.boundsRadius * 0.02
        } else {
            scene.distance = max(scene.boundsRadius * 0.15, min(scene.boundsRadius * 20.0, scene.distance * factor))
        }
        requestRender()
    }

    private func mapGbKey(_ event: NSEvent) -> Int32? {
        if event.keyCode == 51 { return 8 }   // Backspace
        if event.keyCode == 117 { return 127 } // Forward delete
        if event.keyCode == 49 { return 32 }  // Space (jump in Play)
        guard let ch = event.charactersIgnoringModifiers?.lowercased().unicodeScalars.first else { return nil }
        let v = Int32(ch.value)
        if v >= 32 && v <= 126 { return v }
        return nil
    }

    private func navKeyName(_ event: NSEvent) -> String? {
        switch event.keyCode {
        case 126: return "w" // Up arrow
        case 125: return "s" // Down arrow
        case 123: return "a" // Left arrow
        case 124: return "d" // Right arrow
        default: break
        }
        guard let ch = event.charactersIgnoringModifiers?.lowercased(), ch.count == 1 else { return nil }
        if ["w", "a", "s", "d", "q", "e"].contains(ch) { return ch }
        return nil
    }

    override func keyDown(with event: NSEvent) {
        // Never intercept Cmd-chords (menu shortcuts like Cmd+Shift+M, Cmd+Opt+G, Cmd+Q)
        if event.modifierFlags.contains(.command) { super.keyDown(with: event); return }

        if let gta = activeGtaWorkbench {
            if event.keyCode == 53 { // Esc returns focus to chat composer
                sceneHeldKeys.removeAll()
                onReleaseFocusToComposer?()
                return
            }
            let ch = event.charactersIgnoringModifiers?.lowercased() ?? ""
            if let nav = navKeyName(event) {
                sceneHeldKeys.insert(nav)
                onNeedsDisplay?()
                return
            }
            switch ch {
            case " ": gta.togglePaused(); syncGtaBar(); onNeedsDisplay?(); return
            case "1": gta.applyPreset("stg1"); syncGtaBar(); onNeedsDisplay?(); return
            case "2": gta.applyPreset("stg2"); syncGtaBar(); onNeedsDisplay?(); return
            case "3": gta.applyPreset("stg3"); syncGtaBar(); onNeedsDisplay?(); return
            case "4": gta.applyPreset("stg4"); syncGtaBar(); onNeedsDisplay?(); return
            case "5": gta.applyPreset("stg5"); syncGtaBar(); onNeedsDisplay?(); return
            case "6": gta.applyPreset("two_shot"); syncGtaBar(); onNeedsDisplay?(); return
            case "7": gta.applyPreset("jason_cu"); syncGtaBar(); onNeedsDisplay?(); return
            case "0": gta.applyPreset("showcase"); syncGtaBar(); onNeedsDisplay?(); return
            case "p": gta.applyPreset("portrait_live"); syncGtaBar(); onNeedsDisplay?(); return
            case "c": gta.toggleRockstarCam(); syncGtaBar(); onNeedsDisplay?(); return
            case "m": gta.cycleClip(); syncGtaBar(); onNeedsDisplay?(); return
            case "x": gta.cycleExpr(); syncGtaBar(); onNeedsDisplay?(); return
            case "f": gta.toggleDenoiser(); syncGtaBar(); onNeedsDisplay?(); return
            case "k": onGtaSnapshotRequested?(); return
            default: break
            }
            return
        }

        if let gb = activeGameBuilder, let h = gb.handle {
            if event.keyCode == 53 { // Esc -> stop Play or unfocus viewport
                if gb.snapshotUi().playing {
                    _ = gb.runVerb("stop", argsJson: "")
                    gb.muted = true
                    onGameBuilderMutated?()
                    updateGbTimer()
                    requestRender()
                    return
                }
                gbHeldKeys.removeAll()
                onReleaseFocusToComposer?()
                return
            }
            if let k = mapGbKey(event) {
                if k == Int32(Character("v").asciiValue!) && gb.snapshotUi().playing {
                    oa_gb_toggle_play_view(h)
                } else {
                    gbHeldKeys.insert(k)
                    oa_gb_send_key(h, k, 1, gbMods(from: event.modifierFlags))
                }
                gb.flushSaveIfDue()
                onGameBuilderMutated?()
                updateGbTimer()
                requestRender()
                return
            }
            return
        }

        if let scene = activeScene {
            if event.keyCode == 53 { // Esc returns keyboard focus to chat composer
                sceneHeldKeys.removeAll()
                onReleaseFocusToComposer?()
                return
            }
            if let nav = navKeyName(event) {
                sceneHeldKeys.insert(nav)
                onNeedsDisplay?()
                return
            }
            let ch = event.charactersIgnoringModifiers?.lowercased() ?? ""
            if ch == " " && !scene.animClips.isEmpty {
                toggleAnimPlay()
                return
            }
            if ch == "r" || ch == "f" {
                scene.applyPreset("iso")
                requestRender()
                return
            }
            return
        }
        super.keyDown(with: event)
    }

    override func keyUp(with event: NSEvent) {
        if let nav = navKeyName(event) {
            sceneHeldKeys.remove(nav)
        }
        if let gb = activeGameBuilder, let h = gb.handle {
            if let k = mapGbKey(event) {
                gbHeldKeys.remove(k)
                oa_gb_send_key(h, k, 0, gbMods(from: event.modifierFlags))
                updateGbTimer()
                requestRender()
                return
            }
        }
        if activeGtaWorkbench != nil || activeScene != nil { return }
        super.keyUp(with: event)
    }

    override func magnify(with event: NSEvent) {
        if let gta = activeGtaWorkbench {
            gta.zoom(delta: Float(event.magnification * 180.0))
            onNeedsDisplay?()
            return
        }
        guard let scene = activeScene else { return }
        scene.distance = max(scene.boundsRadius * 0.15, min(scene.boundsRadius * 20.0, scene.distance / max(0.2, Float(1.0 + event.magnification))))
        requestRender()
    }

    private func performPick(at p: NSPoint) {
        guard let scene = activeScene, let h = scene.handle, bounds.width > 1, bounds.height > 1 else { return }
        let nx = Float(p.x / bounds.width) * 2.0 - 1.0
        let ny = Float(p.y / bounds.height) * 2.0 - 1.0
        let aspect = Float(bounds.width / bounds.height)
        let invVP = simd_inverse(scene.projMatrix(aspect: aspect) * scene.viewMatrix())
        let nearW = invVP * SIMD4<Float>(nx, ny, 0.0, 1.0)
        let farW  = invVP * SIMD4<Float>(nx, ny, 1.0, 1.0)
        guard abs(nearW.w) > 1e-6, abs(farW.w) > 1e-6 else { return }
        let o = SIMD3<Float>(nearW.x, nearW.y, nearW.z) / nearW.w
        let f = SIMD3<Float>(farW.x, farW.y, farW.z) / farW.w
        let d = simd_normalize(f - o)
        var hit = [Float](repeating: 0, count: 3)
        let idx = Int(oa_mv_ray_pick(h, o.x, o.y, o.z, d.x, d.y, d.z, &hit))
        scene.selectedNode = idx
        oa_mv_select_node(h, Int32(idx))
        for i in 0..<scene.nodes.count { scene.nodes[i].selected = (i == idx) }
        if idx >= 0 && idx < scene.nodes.count {
            scene.lastHitWorld = SIMD3<Float>(hit[0], hit[1], hit[2])
            onNodePicked?(idx, scene.nodes[idx].label, scene.lastHitWorld)
        } else {
            onNodePicked?(-1, "", .zero)
        }
        renderNow()
    }

    // MARK: - Drag and Drop 3D Files

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        return .copy
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        guard let urls = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL], !urls.isEmpty else {
            return false
        }
        onFilesDropped?(urls)
        return true
    }
}
