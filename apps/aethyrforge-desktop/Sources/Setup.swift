// AppKit rendering and process/file effects for setup. Coax owns report parsing,
// model/agent status, selection, readiness and the persisted settings document.
import AppKit
import MetalKit

func setupText(_ field: Int64) -> String {
    var bytes: [UInt8] = []
    for i in 0..<8192 {
        let c = fgs_text_byte(field, Int64(i)); if c < 0 { break }; bytes.append(UInt8(c))
    }
    return String(decoding: bytes, as: UTF8.self)
}

let forgeAtlasAPI = ZeronAtlasAPI(
  next_cluster: fgu_next_cluster,
  measured: fgu_measured,
  next_cell: fgu_next_cell,
  cell: fgu_cell,
  cell_ready: fgu_cell_ready,
  cluster: fgu_cluster,
  glyph_count: fgu_glyph_count,
  font_count: fgu_font_count,
  icon_count: fgu_icon_count,
  font_size: fgu_font_size,
  font_weight: fgu_font_weight,
  icon_size: fgu_icon_size,
  glyph_code: fgu_glyph_code,
  set_font: fgu_set_font,
  icon_name: fgu_icon_name,
  set_icon: fgu_set_icon,
  set_atlas: fgu_set_atlas,
  set_dynamic: fgu_set_dynamic
)

@MainActor
final class ForgeSetupPane: MTKView, MTKViewDelegate {
    // Value holders and transparent accessibility/input controls only.
    // Every visible primitive, label and hit rectangle comes from chrome.coax.
    let cliStatus = NSTextField(labelWithString: "")
    let voiceStatus = NSTextField(labelWithString: "")
    let modelPath = NSTextField(labelWithString: "")
    let editModel = NSTextField(labelWithString: "")
    let microphoneStatus = NSTextField(labelWithString: "")
    let preparationStatus = NSTextField(labelWithString: "")
    let agentStatus = NSTextField(labelWithString: "")
    let routerStatus = NSTextField(labelWithString: "")
    let microphone = NSButton(title: "Allow microphone", target: nil, action: nil)
    let verify = NSButton(title: "Verify voice model", target: nil, action: nil)
    let refresh = NSButton(title: "Refresh", target: nil, action: nil)
    let enter = NSButton(title: "Open Everglade", target: nil, action: nil)
    let folder = NSButton(title: "Open folder", target: nil, action: nil)
    var enterAction: (() -> Void)?
    var refreshAction: (() -> Void)?
    var verifyAction: (() -> Void)?
    var folderAction: (() -> Void)?
    var microphoneAction: (() -> Void)?
    var sceneReady = false, checking = true, verifying = false
    var statusMode = false, worldPaused = false
    var errorMessage: String?
    var renderer: ZeronMetalRenderer!
    var atlas: ZeronAtlas?
    var drawCount = 0
    var gpuSeconds = 0.0
    var lastVertexCount = 0
    override var isOpaque: Bool { false }
    override var acceptsFirstResponder: Bool { true }
    init(frame: NSRect) {
        let device = MTLCreateSystemDefaultDevice()!
        super.init(frame: frame, device: device)
        colorPixelFormat = .bgra8Unorm
        clearColor = MTLClearColorMake(0,0,0,0)
        isPaused = true; enableSetNeedsDisplay = true; layer?.isOpaque = false
        let path = Bundle.main.url(forResource: "Zeron", withExtension: "metal")!
        let source = try! String(contentsOf: path, encoding: .utf8)
        renderer = try! ZeronMetalRenderer(device: device, library: device.makeLibrary(source: source, options: nil))
        delegate = self
        for (i,b) in [enter,refresh,verify,folder,microphone].enumerated() {
            b.isTransparent = true; b.tag = i+1; b.target = self; b.action = #selector(pressed(_:)); addSubview(b)
        }
        setAccessibilityLabel("Aethyrforge setup")
        update()
    }
    required init(coder: NSCoder) { fatalError("init(coder:) is unavailable") }
    override func layout() {
        super.layout()
        for b in [enter,refresh,verify,folder,microphone] {
            b.frame = NSRect(x: CGFloat(fgu_box(Int64(b.tag),Float(bounds.width),Float(bounds.height),0)),
                y: bounds.height-CGFloat(fgu_box(Int64(b.tag),Float(bounds.width),Float(bounds.height),1))-CGFloat(fgu_box(Int64(b.tag),Float(bounds.width),Float(bounds.height),3)),
                width: CGFloat(fgu_box(Int64(b.tag),Float(bounds.width),Float(bounds.height),2)),
                height: CGFloat(fgu_box(Int64(b.tag),Float(bounds.width),Float(bounds.height),3)))
        }
    }
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) { needsDisplay = true }
    func draw(in view: MTKView) {
        guard !isHidden, let pass = currentRenderPassDescriptor, let drawable = currentDrawable else { return }
        render(pass, size: drawableSize, dpr: window?.backingScaleFactor ?? 2) { $0.present(drawable) }
    }
    func render(_ pass: MTLRenderPassDescriptor, size: CGSize, dpr: CGFloat, done: ((MTLCommandBuffer)->Void)? = nil) {
        guard let device, let state = fgu_state() else { return }
        if atlas == nil || atlas!.dpr != dpr { atlas = zeronAtlas(device, state: state, dpr: dpr, api: forgeAtlasAPI) }
        let bytes = Array((errorMessage ?? "").utf8)
        bytes.withUnsafeBufferPointer { fgu_update(sceneReady,checking,verifying,statusMode,worldPaused,$0.baseAddress,Int64($0.count)) }
        let buffer = device.makeBuffer(length: 32768*28*4, options: .storageModeShared)!
        var n: Int64 = 0
        for _ in 0..<3 {
            n = fgu_draw(buffer.contents().assumingMemoryBound(to: Float.self),32768*28,Float(size.width),Float(size.height),Float(dpr))
            atlas!.serve(state)
        }
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].clearColor = MTLClearColorMake(0,0,0,0)
        guard let cmd = renderer.queue.makeCommandBuffer(), let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
        renderer.encode(enc, vertices: buffer, count: Int(n), size: size, atlas: atlas!)
        enc.endEncoding(); done?(cmd)
        cmd.addCompletedHandler { [weak self] command in
            let seconds = max(0,command.gpuEndTime-command.gpuStartTime)
            DispatchQueue.main.async { self?.gpuSeconds += seconds }
        }
        cmd.commit(); drawCount += 1; lastVertexCount = Int(n)
    }
    func update() {
        cliStatus.stringValue = errorMessage ?? setupText(2)
        voiceStatus.stringValue = setupText(4); modelPath.stringValue = setupText(6)
        editModel.stringValue = setupText(5); microphoneStatus.stringValue = setupText(14)
        preparationStatus.stringValue = setupText(16); agentStatus.stringValue = setupText(7)
        microphone.title = setupText(15); microphone.isEnabled = fgs_status(3) != 0
        refresh.isEnabled = !checking && !verifying
        verify.isEnabled = !checking && !verifying && fgs_status(1)>0
        enter.title = statusMode ? "Return to Everglade" : "Open Everglade"
        enter.isEnabled = statusMode || (errorMessage == nil && !checking && !verifying && fgs_can_enter(sceneReady))
        needsLayout = true; needsDisplay = true
    }
    @objc private func pressed(_ button: NSButton) {
        guard button.isEnabled else { return }
        switch button.tag {
        case 1: enterAction?()
        case 2: refreshAction?()
        case 3: verifyAction?()
        case 4: folderAction?()
        case 5: microphoneAction?()
        default: break
        }
    }
    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 || event.keyCode == 76 || event.keyCode == 53 { if enter.isEnabled { enterAction?() }; return }
        super.keyDown(with: event)
    }
}
struct ForgeCLI {
    static func run(_ executable: URL, _ arguments: [String], timeout: TimeInterval = 15, environment: [String: String]? = nil) async throws -> Data {
        try await Task.detached(priority: .userInitiated) {
            let process = Process(); process.executableURL = executable; process.arguments = arguments
            var env = environment ?? ProcessInfo.processInfo.environment
            let shared = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin").path
            env["PATH"] = shared + ":" + (env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin")
            process.environment = env
            let pipe = Pipe(); process.standardOutput = pipe; process.standardError = FileHandle.nullDevice
            try process.run()
            let limit = DispatchWorkItem { if process.isRunning { process.terminate() } }
            let killLimit = DispatchWorkItem { if process.isRunning { kill(process.processIdentifier, SIGKILL) } }
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: limit)
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout + 2, execute: killLimit)
            defer { limit.cancel(); killLimit.cancel() }
            var data = Data()
            while let part = try pipe.fileHandleForReading.read(upToCount: 8192), !part.isEmpty {
                data.append(part)
                if data.count > 65536 { process.terminate(); throw ShellError.failed("The local CLI report exceeded its size limit.") }
            }
            process.waitUntilExit()
            guard !data.isEmpty else { throw ShellError.failed("The local CLI did not return a status report.") }
            return data
        }.value
    }
}
