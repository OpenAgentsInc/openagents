// Firm reason: 6 (AppKit text/IME, Metal presentation and process/file effects).
// Coder owns agents, tools, history and consent; Coax owns the HUD and JSONL.
import AppKit
import MetalKit
import Darwin

// Read available pipe bytes rather than Foundation's buffered AsyncBytes:
// an approval must reach the HUD before the child can finish its output.
func forgeLines(_ handle: FileHandle) -> AsyncThrowingStream<String, Error> {
    AsyncThrowingStream(bufferingPolicy: .bufferingOldest(1024)) { continuation in
        DispatchQueue.global(qos: .userInitiated).async {
            var pending = Data(); var bytes = [UInt8](repeating: 0, count: 8192)
            while true {
                let n = Darwin.read(handle.fileDescriptor, &bytes, bytes.count)
                if n < 0 && errno == EINTR { continue }
                if n <= 0 { break }
                pending.append(contentsOf: bytes.prefix(n))
                while let newline = pending.firstIndex(of: 10) {
                    let line = String(decoding: pending[..<newline], as: UTF8.self)
                    pending.removeSubrange(...newline)
                    if case .dropped = continuation.yield(line) { continuation.finish(throwing: ShellError.failed("Coder output exceeded the HUD queue.")); return }
                }
                if pending.count > 262144 { continuation.finish(throwing: ShellError.failed("Coder output exceeded its line limit.")); return }
            }
            if !pending.isEmpty { continuation.yield(String(decoding: pending, as: UTF8.self)) }
            continuation.finish()
        }
    }
}

@MainActor
final class ForgeHUD: MTKView, MTKViewDelegate {
    var pipeline: MTLRenderPipelineState?
    var queue: MTLCommandQueue?
    var busy = false
    var feed = false
    var viewport = NSSize.zero
    var drawOrigin = NSPoint.zero
    var vertexStorage: [Float] = []
    override var isOpaque: Bool { false }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    init(frame: NSRect) {
        super.init(frame: frame, device: MTLCreateSystemDefaultDevice())
        isPaused = true; enableSetNeedsDisplay = true
        colorPixelFormat = .bgra8Unorm; clearColor = MTLClearColorMake(0, 0, 0, 0)
        layer?.isOpaque = false
        guard let device, let path = Bundle.main.url(forResource: "HUD", withExtension: "metal"),
              let source = try? String(contentsOf: path, encoding: .utf8),
              let lib = try? device.makeLibrary(source: source, options: nil) else { return }
        let p = MTLRenderPipelineDescriptor()
        p.vertexFunction = lib.makeFunction(name: "hud_vs"); p.fragmentFunction = lib.makeFunction(name: "hud_fs")
        let vd = MTLVertexDescriptor()
        for i in 0..<3 { vd.attributes[i].format = .float4; vd.attributes[i].offset = i * 16; vd.attributes[i].bufferIndex = 0 }
        vd.layouts[0].stride = 48; p.vertexDescriptor = vd
        p.colorAttachments[0].pixelFormat = colorPixelFormat
        p.colorAttachments[0].isBlendingEnabled = true
        p.colorAttachments[0].sourceRGBBlendFactor = .one; p.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        p.colorAttachments[0].sourceAlphaBlendFactor = .one; p.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        pipeline = try? device.makeRenderPipelineState(descriptor: p); queue = device.makeCommandQueue(); delegate = self
    }
    required init(coder: NSCoder) { fatalError() }
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) { needsDisplay = true }
    func vertexStream(scale d: Float) -> ([Float], Int) {
        let capacity = feed ? 2_000_000 : 48000
        if vertexStorage.count < capacity { vertexStorage = [Float](repeating: 0, count: capacity) }
        let n = vertexStorage.withUnsafeMutableBufferPointer {
            feed ? fgc_feed_draw($0.baseAddress, Int64($0.count), Float(viewport.width), Float(viewport.height), d)
                 : fgc_draw($0.baseAddress, Int64($0.count), Float(viewport.width), Float(viewport.height), d, busy)
        }
        for i in 0..<Int(n) { vertexStorage[i * 12] -= Float(drawOrigin.x) * d; vertexStorage[i * 12 + 1] -= Float(drawOrigin.y) * d }
        return (vertexStorage, Int(n))
    }
    func draw(in view: MTKView) {
        guard let device, let pipeline, let queue, let pass = currentRenderPassDescriptor,
              let drawable = currentDrawable, let cmd = queue.makeCommandBuffer() else { return }
        let d = Float(drawableSize.width / max(bounds.width, 1))
        // Coax's point-space layout is window-relative. The native Metal view
        // covers only its HUD component, so WebKit can see the world around it.
        let (vertices, n) = vertexStream(scale: d)
        guard let encoder = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
        if n > 0 {
            guard let buffer = vertices.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: n * 48) }) else { encoder.endEncoding(); return }
            var size = SIMD2<Float>(Float(drawableSize.width), Float(drawableSize.height))
            encoder.setRenderPipelineState(pipeline); encoder.setVertexBuffer(buffer, offset: 0, index: 0)
            encoder.setVertexBytes(&size, length: MemoryLayout.size(ofValue: size), index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: n)
        }
        encoder.endEncoding()
        cmd.present(drawable); cmd.commit()
    }
    // Offscreen SDK acceptance uses the exact production stream and pipeline.
    // The matte belongs to this PNG only; the live view clears to transparent.
    func saveCheckFrame(_ url: URL) throws -> Int {
        guard let device, let pipeline, let queue, let cmd = queue.makeCommandBuffer() else { throw ShellError.failed("HUD Metal unavailable.") }
        let width = Int(bounds.width * 2), height = Int(bounds.height * 2)
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        desc.usage = .renderTarget; desc.storageMode = .shared
        guard let texture = device.makeTexture(descriptor: desc) else { throw ShellError.failed("HUD check texture unavailable.") }
        let pass = MTLRenderPassDescriptor(); pass.colorAttachments[0].texture = texture
        pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
        pass.colorAttachments[0].clearColor = MTLClearColorMake(0.04, 0.06, 0.07, 1)
        var (vertices, n) = vertexStream(scale: 2)
        guard n > 0, let buffer = device.makeBuffer(bytes: &vertices, length: n * 48), let encoder = cmd.makeRenderCommandEncoder(descriptor: pass) else { throw ShellError.failed("HUD feed produced no vertices.") }
        var size = SIMD2<Float>(Float(width), Float(height))
        encoder.setRenderPipelineState(pipeline); encoder.setVertexBuffer(buffer, offset: 0, index: 0)
        encoder.setVertexBytes(&size, length: MemoryLayout.size(ofValue: size), index: 1)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: n); encoder.endEncoding()
        cmd.commit(); cmd.waitUntilCompleted()
        guard cmd.status == .completed, let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bitmapFormat: [], bytesPerRow: width * 4, bitsPerPixel: 32), let data = bitmap.bitmapData else { throw ShellError.failed("HUD frame readback failed.") }
        texture.getBytes(data, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        // Metal's attachment is BGRA; the PNG bitmap accepts RGBA bytes.
        for i in stride(from: 0, to: width * height * 4, by: 4) { let blue = data[i]; data[i] = data[i + 2]; data[i + 2] = blue }
        guard let png = bitmap.representation(using: .png, properties: [:]) else { throw ShellError.failed("HUD PNG failed.") }
        try png.write(to: url); return n
    }
}

@MainActor
final class ForgeComposer: NSTextView {
    var send: (() -> Void)?
    var dismiss: (() -> Void)?
    override func keyDown(with event: NSEvent) {
        if (event.keyCode == 36 || event.keyCode == 76) && !event.modifierFlags.contains(.shift) && !hasMarkedText() { send?(); return }
        if event.keyCode == 53 && !hasMarkedText() { dismiss?(); return }
        super.keyDown(with: event)
    }
}

@MainActor
final class ForgeChatPane: NSView {
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { false }
    let hud: ForgeHUD
    let transcript = NSTextView()
    let transcriptScroll = NSScrollView()
    let messageHUD: ForgeHUD
    let input = ForgeComposer()
    let inputScroll = NSScrollView()
    let router = NSTextField(labelWithString: "Automatic routing · Elephant")
    let folder = NSButton()
    let send = NSButton()
    let fresh = NSButton()
    let allow = NSButton()
    let deny = NSButton()
    let heading = NSTextField(labelWithString: "CODER / MESSAGES")
    let status = NSTextField(labelWithString: "Enter sends · Shift-Enter adds a line · Esc closes")
    var process: Process?
    var task: Task<Void, Never>?
    var clockTimer: Timer?
    var renderedPresentation: Int64 = -1
    var busy = false
    var pending = false
    var composerOpen = false
    var presentation: Int64 {
        fgc_chat_mode(composerOpen, busy, clockMilliseconds)
    }
    var clockMilliseconds: Int64 { Int64(ProcessInfo.processInfo.systemUptime * 1000) }
    var messageText: String {
        var bytes = [UInt8](repeating: 0, count: 32768)
        let n = fgc_feed_copy(&bytes, Int64(bytes.count))
        return String(decoding: bytes.prefix(max(0, Int(n))), as: UTF8.self)
    }
    var session = ""
    var parent = ""
    var lastEnding = ""
    var selectedEngine = "auto"
    var liveChunks = 0
    var streamedWithoutComposer = false
    var engineOverride: String? // native test fixture only; the product always routes
    var cwd: URL
    var home = FileManager.default.homeDirectoryForCurrentUser
    var settings: URL { home.appendingPathComponent(".openagents/aethyrforge/chat.json") }
    var sessions: URL { home.appendingPathComponent(".openagents/coder/sessions") }
    var cliOverride: URL?
    var sceneContextURL: URL?
    var sceneSessionURL: URL?
    var cli: URL { cliOverride ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/openagents") }
    var stopping = false
    var onActivity: (() -> Void)?
    var onClose: (() -> Void)?
    override init(frame: NSRect) {
        hud = ForgeHUD(frame: .zero)
        messageHUD = ForgeHUD(frame: .zero); messageHUD.feed = true
        cwd = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/aethyrforge/workspace")
        super.init(frame: frame)
        fgc_reset()
        appearance = NSAppearance(named: .darkAqua)
        for view in [transcript, input] { view.isRichText = false; view.font = .monospacedSystemFont(ofSize: 13, weight: .regular); view.textColor = NSColor(white: 0.9, alpha: 1); view.backgroundColor = .clear; view.drawsBackground = false; view.textContainerInset = NSSize(width: 4, height: 6); view.isAutomaticQuoteSubstitutionEnabled = false; view.isAutomaticDashSubstitutionEnabled = false }
        transcript.isEditable = false; transcript.isSelectable = true
        input.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        input.textContainerInset = NSSize(width: 4, height: 3)
        for (scroll, text) in [(transcriptScroll, transcript), (inputScroll, input)] {
            scroll.documentView = text; scroll.hasVerticalScroller = true; scroll.drawsBackground = false
            text.isVerticallyResizable = true; text.isHorizontallyResizable = false
            text.autoresizingMask = [.width]; text.textContainer?.widthTracksTextView = true
            addSubview(scroll)
        }
        addSubview(messageHUD)
        router.setAccessibilityLabel("Automatic task routing")
        router.lineBreakMode = .byTruncatingTail
        folder.title = "Folder…"; folder.target = self; folder.action = #selector(chooseFolder)
        send.title = "Send ↵"; send.target = self; send.action = #selector(submit)
        fresh.title = "New chat"; fresh.target = self; fresh.action = #selector(newChat)
        allow.title = "Allow once"; allow.target = self; allow.action = #selector(approve)
        deny.title = "Deny"; deny.target = self; deny.action = #selector(reject)
        for view in [router, folder, send, fresh, allow, deny, heading, status] { addSubview(view) }
        for control in [folder, send, fresh, allow, deny] {
            control.controlSize = .small; control.font = .systemFont(ofSize: 11)
        }
        folder.isBordered = false; fresh.isBordered = false
        for label in [router, heading, status] { label.textColor = NSColor(calibratedRed: 0.4, green: 0.9, blue: 0.9, alpha: 1); label.font = .monospacedSystemFont(ofSize: 11, weight: .medium) }
        heading.lineBreakMode = .byTruncatingMiddle
        status.font = .monospacedSystemFont(ofSize: 10, weight: .medium)
        status.lineBreakMode = .byTruncatingTail
        input.setAccessibilityLabel("Message to coding agent")
        input.send = { [weak self] in guard let self, !busy else { return }; submit() }
        input.dismiss = { [weak self] in self?.closeComposer() }
        addSubview(hud)
        try? FileManager.default.createDirectory(at: cwd, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: settings), let saved = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let path = saved["cwd"] as? String, FileManager.default.fileExists(atPath: path) { cwd = URL(fileURLWithPath: path) }
        }
        // One shared decision-door config for Coder and every desktop client.
        let config = home.appendingPathComponent(".openagents/coder/router.json")
        if !FileManager.default.fileExists(atPath: config.path) {
            try? FileManager.default.createDirectory(at: config.deletingLastPathComponent(), withIntermediateDirectories: true)
            var bytes = [UInt8](repeating: 0, count: 512)
            let n = fgc_router_default(&bytes, Int64(bytes.count))
            if n > 0 { try? Data(bytes.prefix(Int(n))).write(to: config, options: .withoutOverwriting) }
        }
        update()
    }
    required init?(coder: NSCoder) { fatalError() }
    func box(_ kind: Int) -> NSRect {
        let v = (0..<4).map { CGFloat(fgc_box(Int64(kind), Float(bounds.width), Float(bounds.height), Int64($0))) }
        return NSRect(x: v[0], y: v[1], width: v[2], height: v[3])
    }
    override func layout() {
        super.layout()
        hud.frame = box(0).insetBy(dx: -4, dy: -4)
        hud.viewport = bounds.size; hud.drawOrigin = hud.frame.origin
        let t = box(1)
        heading.frame = NSRect(x: t.minX + 8, y: t.minY + 4, width: t.width - 104, height: 16)
        fresh.frame = NSRect(x: t.maxX - 88, y: t.minY + 1, width: 80, height: 22)
        transcriptScroll.frame = NSRect(x: t.minX + 4, y: t.minY + 28, width: t.width - 8, height: t.height - (pending ? 64 : 32))
        messageHUD.frame = t.insetBy(dx: -4, dy: -4)
        messageHUD.viewport = bounds.size; messageHUD.drawOrigin = messageHUD.frame.origin
        allow.frame = NSRect(x: t.minX + 4, y: t.maxY - 30, width: 90, height: 24)
        deny.frame = NSRect(x: t.minX + 100, y: t.maxY - 30, width: 60, height: 24)
        router.frame = box(3); folder.frame = box(6)
        inputScroll.frame = box(2); send.frame = box(7)
        status.frame = NSRect(x: 28, y: bounds.height - 29, width: bounds.width - 56, height: 12)
        for (scroll, text) in [(transcriptScroll, transcript), (inputScroll, input)] {
            text.setFrameSize(NSSize(width: scroll.contentSize.width, height: max(text.frame.height, scroll.contentSize.height)))
        }
        hud.needsDisplay = true; messageHUD.needsDisplay = true
    }
    override func draw(_ dirtyRect: NSRect) {
        // Messages float directly over the world. Only the compact composer
        // gets a faint tint, so the game stays visible through it too.
        guard presentation == 2 else { return }
        NSColor(calibratedWhite: 0.08, alpha: 0.22).setFill()
        NSBezierPath(roundedRect: box(0), xRadius: 6, yRadius: 6).fill()
    }
    // Only the HUD panels own hits. The center/right world stays interactive.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard presentation == 2 else { return nil }
        let p = convert(point, from: superview)
        let messages = box(1).contains(p) && (!transcript.string.isEmpty || pending)
        guard box(0).contains(p) || messages || fresh.frame.contains(p) else { return nil }
        return super.hitTest(point)
    }
    func append(_ text: String, color: NSColor = NSColor(white: 0.9, alpha: 1)) {
        let nearBottom = transcript.visibleRect.maxY >= transcript.bounds.maxY - 32
        transcript.textStorage?.append(NSAttributedString(string: text, attributes: [.foregroundColor: color, .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)]))
        // The durable CLI log retains the complete history; keep the view bounded.
        if transcript.string.utf16.count > 200_000 { transcript.textStorage?.deleteCharacters(in: NSRange(location: 0, length: 50_000)) }
        if nearBottom { transcript.scrollToEndOfDocument(nil) }
    }
    func update() {
        let mode = presentation
        renderedPresentation = mode
        for view in [hud as NSView, inputScroll, router, folder, send, fresh, heading, status, transcriptScroll] { view.isHidden = mode != 2 }
        messageHUD.isHidden = mode != 1
        // A platform clock only. Coax owns whether the feed is alive and when
        // its lifetime ends; no deadline or delayed-hide state lives in Swift.
        if mode == 1 && clockTimer == nil {
            clockTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if self.presentation != self.renderedPresentation { self.update() }
                }
            }
        } else if mode != 1 { clockTimer?.invalidate(); clockTimer = nil }
        for control in [folder, fresh] { control.isEnabled = !busy }
        send.title = busy ? "Stop" : "Send ↵"; allow.isHidden = mode != 2 || !pending; deny.isHidden = mode != 2 || !pending
        heading.stringValue = pending ? "CODER / APPROVAL NEEDED" : busy ? "CODER / WORKING · \(selectedEngine)" : "CODER / MESSAGES"
        folder.title = "\(cwd.lastPathComponent) ▾"; folder.toolTip = cwd.path
        hud.busy = busy; hud.needsDisplay = true; messageHUD.needsDisplay = true; needsDisplay = true; needsLayout = true
    }
    func openComposer() {
        composerOpen = true; onActivity?(); update(); window?.makeFirstResponder(input)
    }
    func closeComposer() {
        composerOpen = false; update(); onClose?()
    }
    func appendMessage(_ text: String) {
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { fgc_feed_event(1, $0.baseAddress, Int64($0.count), clockMilliseconds) }
        update(); layoutSubtreeIfNeeded()
    }
    func clearMessages() {
        fgc_feed_event(0, nil, 0, clockMilliseconds)
    }
    func finishMessages() {
        fgc_feed_event(2, nil, 0, clockMilliseconds)
        update()
    }
    func save() {
        if let data = try? JSONSerialization.data(withJSONObject: ["routing": "auto", "cwd": cwd.path], options: [.sortedKeys]) { try? data.write(to: settings, options: .atomic) }
    }
    @objc func newChat() { guard !busy else { return }; parent = ""; session = ""; transcript.string = ""; clearMessages(); update(); window?.makeFirstResponder(input) }
    @objc func chooseFolder() {
        let panel = NSOpenPanel(); panel.canChooseDirectories = true; panel.canChooseFiles = false; panel.allowsMultipleSelection = false; panel.directoryURL = cwd
        if panel.runModal() == .OK, let url = panel.url { cwd = url; parent = ""; session = ""; save(); append("\nWorking folder: \(url.path)\n", color: .systemTeal); update() }
    }
    func environment() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let resources = Bundle.main.resourceURL!
        // Launch Services supplies a short PATH; agent CLIs keep their own logins.
        let realHome = FileManager.default.homeDirectoryForCurrentUser
        let nodeRoot = realHome.appendingPathComponent(".nvm/versions/node")
        let nodes = ((try? FileManager.default.contentsOfDirectory(atPath: nodeRoot.path)) ?? []).sorted { $0.compare($1, options: .numeric) == .orderedDescending }.map { nodeRoot.appendingPathComponent($0 + "/bin").path }
        env["PATH"] = ([realHome.appendingPathComponent(".openagents/bin").path, realHome.appendingPathComponent(".local/bin").path, "/opt/homebrew/bin", "/usr/local/bin", env["PATH"] ?? "/usr/bin:/bin"] + nodes).joined(separator: ":")
        env["OPENAGENTS_CODER_BIN"] = resources.appendingPathComponent("coder").path
        env["CODER_MICRO_WORKER"] = resources.appendingPathComponent("mcworker").path
        env.removeValue(forKey: "CODER_ASK_FILE"); env["CODER_REALM_TOOLS"] = "0"
        if let sceneSessionURL {
            env["AETHYRFORGE_SCENE_SESSION"] = sceneSessionURL.path
            env["AETHYRFORGE_SCENE_TOOL"] = resources.appendingPathComponent("aethyrforge-scene").path
        }
        env["BROWSER"] = "/usr/bin/true"; env["NO_OPEN_BROWSER"] = "1"
        if home != FileManager.default.homeDirectoryForCurrentUser { env["CODER_HOME"] = home.path }
        return env
    }
    @objc func submit() {
        if busy { stopTurn(); return }
        startTurn(input.string, voice: false)
    }
    func startTurn(_ draft: String, voice: Bool) {
        guard !stopping else { return }
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        let bytes = Array(text.utf8)
        guard bytes.withUnsafeBufferPointer({ fgc_send(busy, $0.baseAddress, Int64($0.count)) }) else { status.stringValue = "Enter a message of at most 16 KiB."; return }
        save(); onActivity?(); if !voice { input.string = "" }
        clearMessages(); busy = true; pending = false; session = ""; lastEnding = ""
        liveChunks = 0; streamedWithoutComposer = false
        closeComposer()
        append("\n\(voice ? "VOICE" : "YOU")\n\(text)\n\n", color: .white)
        if voice { appendMessage("You: \(text)\n\n") }
        var args = ["coder"] + (parent.isEmpty ? ["run", text] : ["fork", parent, text])
        args += ["--cwd", cwd.path]
        if let sceneContextURL { args += ["--context", sceneContextURL.path] }
        for i in 0..<8 {
            var bytes = [UInt8](repeating: 0, count: 128)
            let n = fgc_launch_option(Int64(i), &bytes, Int64(bytes.count))
            if n <= 0 { break }
            let option = String(decoding: bytes.prefix(Int(n)), as: UTF8.self)
            args.append(option == "auto" ? (engineOverride ?? option) : option)
        }
        let p = Process(); p.executableURL = cli; p.arguments = args; p.currentDirectoryURL = cwd; p.environment = environment()
        let output = Pipe(); let errors = Pipe(); p.standardOutput = output; p.standardError = errors; p.standardInput = FileHandle.nullDevice
        selectedEngine = engineOverride ?? "auto"
        router.stringValue = "Automatic routing · deciding…"
        process = p; status.stringValue = "Coder is routing this task…"; update()
        task = Task { [weak self] in
            guard let self else { return }
            do {
                try p.run()
                let stderr = Task { var text = ""; for try await line in forgeLines(errors.fileHandleForReading) { if text.utf8.count < 8192 { text += line + "\n" } }; return text }
                for try await line in forgeLines(output.fileHandleForReading) { if line.utf8.count <= 262144 { receive(line) } }
                p.waitUntilExit()
                let problem = (try? await stderr.value) ?? ""
                if lastEnding.isEmpty { append("\nCoder exited (\(p.terminationStatus)). \(problem.prefix(8192))\n", color: .systemOrange); lastEnding = "failed" }
            } catch {
                if p.isRunning {
                    stopTurn()
                    let end = Date().addingTimeInterval(2)
                    while p.isRunning && Date() < end { try? await Task.sleep(for: .milliseconds(50)) }
                    if p.isRunning { p.terminate() }
                }
                append("\nCoder: \(error.localizedDescription)\n", color: .systemOrange); lastEnding = "failed"
            }
            if !session.isEmpty { parent = session }
            busy = false; pending = false; process = nil; finishMessages()
            status.stringValue = "\(lastEnding) · \(selectedEngine) · \(session.isEmpty ? cwd.path : session)"
        }
    }
    func readField(_ i: Int) -> String {
        var b = [UInt8](repeating: 0, count: 262144); let n = fgc_copy(Int64(i), &b, Int64(b.count))
        return String(decoding: b.prefix(max(0, Int(n))), as: UTF8.self)
    }
    func receive(_ line: String) {
        let b = Array(line.utf8); let kind = b.withUnsafeBufferPointer { fgc_read($0.baseAddress, Int64($0.count)) }
        if kind == 1 { session = readField(0); let engine = readField(1); if !engine.isEmpty { selectedEngine = engine }; update(); return }
        let text = readField(1)
        switch kind {
        case 3: append("\(selectedEngine.uppercased())\n\(text)\n\n"); appendMessage(text + "\n\n")
        case 4: append("› \(text)\n", color: .systemTeal); status.stringValue = text
        case 5: append("\(text)\n", color: .secondaryLabelColor)
        case 6: pending = true; append("\nAPPROVAL NEEDED\n\(text)\n", color: .systemOrange); appendMessage(text + "\nPress Enter to review this tool request.\n\n"); status.stringValue = "Review the tool request, then Allow once or Deny."; update()
        case 7: lastEnding = text; append("\n[\(text)]\n", color: text.hasPrefix("finished") ? .systemTeal : .systemOrange)
        case 8: append("Edited: \(text)\n", color: .systemTeal)
        case 9: append("Model: \(text)\n", color: .systemTeal); router.stringValue = text; router.toolTip = text
        case 10: append("Router: \(text)\n", color: .systemTeal); router.stringValue = text; router.toolTip = text
        case 11, 12:
            if kind == 11 { append("\(selectedEngine.uppercased())\n") }
            append(text); appendMessage(text); liveChunks += 1
            if busy && !composerOpen && hud.isHidden && inputScroll.isHidden && !messageHUD.isHidden { streamedWithoutComposer = true }
        case 13: append("\n\n"); appendMessage("\n\n")
        default: break
        }
    }
    @objc func approve() { answer(true) }
    @objc func reject() { answer(false) }
    func answer(_ yes: Bool) {
        let b = Array(session.utf8)
        guard b.withUnsafeBufferPointer({ fgc_answer(busy, pending, $0.baseAddress, Int64($0.count)) }) else { return }
        do { try Data((yes ? "y" : "n").utf8).write(to: sessions.appendingPathComponent(session + ".answer"), options: .atomic); pending = false; append(yes ? "Allowed once.\n" : "Denied.\n", color: .systemTeal); update() }
        catch { status.stringValue = "Cannot answer Coder: \(error.localizedDescription)" }
    }
    func stopTurn() {
        guard busy else { return }
        pending = false; update(); status.stringValue = "Stopping Coder…"
        // The CLI's existing stop file is polled even while approval is pending.
        let b = Array(session.utf8)
        if b.withUnsafeBufferPointer({ fgc_answer(busy, true, $0.baseAddress, Int64($0.count)) }) {
            try? Data("stop".utf8).write(to: sessions.appendingPathComponent(session + ".stop"), options: .atomic)
        } else { process?.terminate() }
    }
    func shutdown() {
        clockTimer?.invalidate(); clockTimer = nil
        stopping = true; stopTurn()
        if let process {
            let end = Date().addingTimeInterval(2)
            while process.isRunning && Date() < end { usleep(10_000) }
            if process.isRunning { process.terminate() }
        }
    }
}
