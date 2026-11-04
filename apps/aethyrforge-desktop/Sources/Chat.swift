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
    var viewport = NSSize.zero
    var drawOrigin = NSPoint.zero
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
    func draw(in view: MTKView) {
        guard let device, let pipeline, let queue, let pass = currentRenderPassDescriptor,
              let drawable = currentDrawable, let cmd = queue.makeCommandBuffer() else { return }
        var vertices = [Float](repeating: 0, count: 48000)
        let d = Float(drawableSize.width / max(bounds.width, 1))
        let n = vertices.withUnsafeMutableBufferPointer { fgc_draw($0.baseAddress, Int64($0.count), Float(viewport.width), Float(viewport.height), d, busy) }
        // Coax's point-space layout is window-relative. The native Metal view
        // covers only the composer, so WebKit can see the rest of its surface.
        for i in 0..<Int(n) { vertices[i * 12] -= Float(drawOrigin.x) * d; vertices[i * 12 + 1] -= Float(drawOrigin.y) * d }
        guard let buffer = device.makeBuffer(bytes: &vertices, length: Int(n) * 48), let encoder = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
        var size = SIMD2<Float>(Float(drawableSize.width), Float(drawableSize.height))
        encoder.setRenderPipelineState(pipeline); encoder.setVertexBuffer(buffer, offset: 0, index: 0)
        encoder.setVertexBytes(&size, length: MemoryLayout.size(ofValue: size), index: 1)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n)); encoder.endEncoding()
        cmd.present(drawable); cmd.commit()
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
    let messages = NSTextView()
    let messagesScroll = NSScrollView()
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
    var messageHideTask: Task<Void, Never>?
    var messagesFinishedAt: TimeInterval?
    var busy = false
    var pending = false
    var composerOpen = false
    var presentation: Int64 {
        let age = messagesFinishedAt.map { Int64((ProcessInfo.processInfo.systemUptime - $0) * 1000) } ?? -1
        return fgc_chat_mode(composerOpen, busy, !messages.string.isEmpty, age)
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
    var cli: URL { cliOverride ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/openagents") }
    var stopping = false
    var onActivity: (() -> Void)?
    var onClose: (() -> Void)?
    override init(frame: NSRect) {
        hud = ForgeHUD(frame: .zero)
        cwd = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/aethyrforge/workspace")
        super.init(frame: frame)
        fgc_reset()
        appearance = NSAppearance(named: .darkAqua)
        for view in [transcript, messages, input] { view.isRichText = false; view.font = .monospacedSystemFont(ofSize: 13, weight: .regular); view.textColor = NSColor(white: 0.9, alpha: 1); view.backgroundColor = .clear; view.drawsBackground = false; view.textContainerInset = NSSize(width: 4, height: 6); view.isAutomaticQuoteSubstitutionEnabled = false; view.isAutomaticDashSubstitutionEnabled = false }
        transcript.isEditable = false; transcript.isSelectable = true
        messages.isEditable = false; messages.isSelectable = false
        input.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        input.textContainerInset = NSSize(width: 4, height: 3)
        for (scroll, text) in [(transcriptScroll, transcript), (messagesScroll, messages), (inputScroll, input)] {
            scroll.documentView = text; scroll.hasVerticalScroller = true; scroll.drawsBackground = false
            text.isVerticallyResizable = true; text.isHorizontallyResizable = false
            text.autoresizingMask = [.width]; text.textContainer?.widthTracksTextView = true
            addSubview(scroll)
        }
        messagesScroll.hasVerticalScroller = false
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
        messagesScroll.frame = NSRect(x: t.minX + 4, y: t.minY, width: t.width - 8, height: t.height)
        allow.frame = NSRect(x: t.minX + 4, y: t.maxY - 30, width: 90, height: 24)
        deny.frame = NSRect(x: t.minX + 100, y: t.maxY - 30, width: 60, height: 24)
        router.frame = box(3); folder.frame = box(6)
        inputScroll.frame = box(2); send.frame = box(7)
        status.frame = NSRect(x: 28, y: bounds.height - 29, width: bounds.width - 56, height: 12)
        for (scroll, text) in [(transcriptScroll, transcript), (messagesScroll, messages), (inputScroll, input)] {
            text.setFrameSize(NSSize(width: scroll.contentSize.width, height: max(text.frame.height, scroll.contentSize.height)))
        }
        hud.needsDisplay = true
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
        for view in [hud as NSView, inputScroll, router, folder, send, fresh, heading, status, transcriptScroll] { view.isHidden = mode != 2 }
        messagesScroll.isHidden = mode != 1
        for control in [folder, fresh] { control.isEnabled = !busy }
        send.title = busy ? "Stop" : "Send ↵"; allow.isHidden = mode != 2 || !pending; deny.isHidden = mode != 2 || !pending
        heading.stringValue = pending ? "CODER / APPROVAL NEEDED" : busy ? "CODER / WORKING · \(selectedEngine)" : "CODER / MESSAGES"
        folder.title = "\(cwd.lastPathComponent) ▾"; folder.toolTip = cwd.path
        hud.busy = busy; hud.needsDisplay = true; needsDisplay = true; needsLayout = true
    }
    func openComposer() {
        composerOpen = true; onActivity?(); update(); window?.makeFirstResponder(input)
    }
    func closeComposer() {
        composerOpen = false; update(); onClose?()
    }
    func appendMessage(_ text: String) {
        messages.textStorage?.append(NSAttributedString(string: text, attributes: [.foregroundColor: NSColor(white: 0.95, alpha: 1), .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)]))
        if messages.string.utf16.count > 200_000 { messages.textStorage?.deleteCharacters(in: NSRange(location: 0, length: 50_000)) }
        update(); layoutSubtreeIfNeeded(); messages.scrollToEndOfDocument(nil)
    }
    func clearMessages() {
        messageHideTask?.cancel(); messageHideTask = nil
        messagesFinishedAt = nil; messages.string = ""
    }
    func finishMessages() {
        messageHideTask?.cancel()
        messagesFinishedAt = ProcessInfo.processInfo.systemUptime
        update()
        messageHideTask = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(fc_chat_hold_ms())) }
            catch { return }
            guard let self, !Task.isCancelled else { return }
            messagesFinishedAt = nil; update()
        }
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
        env["CODER_ASK_FILE"] = "1"; env["CODER_REALM_TOOLS"] = "0"
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
        args += ["--engine", engineOverride ?? "auto", "--cwd", cwd.path, "--stream-json", "--ask"]
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
            if busy && !composerOpen && hud.isHidden && inputScroll.isHidden && !messagesScroll.isHidden { streamedWithoutComposer = true }
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
        messageHideTask?.cancel(); messageHideTask = nil; messagesFinishedAt = nil
        stopping = true; stopTurn()
        if let process {
            let end = Date().addingTimeInterval(2)
            while process.isRunning && Date() < end { usleep(10_000) }
            if process.isRunning { process.terminate() }
        }
    }
}
