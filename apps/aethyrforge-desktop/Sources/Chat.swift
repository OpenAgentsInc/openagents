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
    var stop: (() -> Void)?
    var nextAgent: (() -> Void)?
    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 && !event.modifierFlags.contains(.shift) { send?(); return }
        if event.keyCode == 53 { stop?(); return }
        if event.keyCode == 48 && event.modifierFlags.contains(.shift) { nextAgent?(); return }
        super.keyDown(with: event)
    }
}

@MainActor
final class ForgeChatPane: NSView, NSComboBoxDelegate {
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { false }
    let hud: ForgeHUD
    let transcript = NSTextView()
    let transcriptScroll = NSScrollView()
    let input = ForgeComposer()
    let inputScroll = NSScrollView()
    let agent = NSPopUpButton()
    let model = NSComboBox()
    let effort = NSPopUpButton()
    let folder = NSButton()
    let send = NSButton()
    let fresh = NSButton()
    let allow = NSButton()
    let deny = NSButton()
    let heading = NSTextField(labelWithString: "CODER / MESSAGES")
    let status = NSTextField(labelWithString: "Enter sends · Shift-Enter adds a line · Shift-Tab switches agent · Esc stops")
    var process: Process?
    var task: Task<Void, Never>?
    var busy = false
    var pending = false
    var session = ""
    var parent = ""
    var lastEnding = ""
    var models: [String: String] = [:]
    var catalogLoaded = false
    var catalogTask: Task<Void, Never>?
    var cwd: URL
    var home = FileManager.default.homeDirectoryForCurrentUser
    var settings: URL { home.appendingPathComponent(".openagents/aethyrforge/chat.json") }
    var sessions: URL { home.appendingPathComponent(".openagents/coder/sessions") }
    var cliOverride: URL?
    var cli: URL { cliOverride ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/openagents") }
    var stopping = false
    var onActivity: (() -> Void)?
    var selectedEngine: String { catalog(agent.indexOfSelectedItem, label: false) }
    func catalog(_ i: Int, label: Bool) -> String {
        var b = [UInt8](repeating: 0, count: 128)
        let n = fgc_catalog(Int64(i), label, &b, Int64(b.count))
        return String(decoding: b.prefix(max(0, Int(n))), as: UTF8.self)
    }
    override init(frame: NSRect) {
        hud = ForgeHUD(frame: .zero)
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
        for i in 0..<6 { agent.addItem(withTitle: catalog(i, label: true)) }
        agent.target = self; agent.action = #selector(changeAgent)
        agent.setAccessibilityLabel("Coding agent")
        model.isEditable = true; model.completes = false; model.placeholderString = "Model: provider default"; model.delegate = self
        model.setAccessibilityLabel("Model; empty uses provider default")
        effort.addItems(withTitles: ["Effort: default", "low", "medium", "high", "xhigh", "max", "ultra", "minimal", "none"])
        effort.setAccessibilityLabel("Reasoning effort")
        folder.title = "Folder…"; folder.target = self; folder.action = #selector(chooseFolder)
        send.title = "Send ↵"; send.target = self; send.action = #selector(submit)
        fresh.title = "New chat"; fresh.target = self; fresh.action = #selector(newChat)
        allow.title = "Allow once"; allow.target = self; allow.action = #selector(approve)
        deny.title = "Deny"; deny.target = self; deny.action = #selector(reject)
        for view in [agent, model, effort, folder, send, fresh, allow, deny, heading, status] { addSubview(view) }
        for control in [agent as NSControl, model, effort, folder, send, fresh, allow, deny] {
            control.controlSize = .small; control.font = .systemFont(ofSize: 11)
        }
        folder.isBordered = false; fresh.isBordered = false
        for label in [heading, status] { label.textColor = NSColor(calibratedRed: 0.4, green: 0.9, blue: 0.9, alpha: 1); label.font = .monospacedSystemFont(ofSize: 11, weight: .medium) }
        heading.lineBreakMode = .byTruncatingMiddle
        status.font = .monospacedSystemFont(ofSize: 10, weight: .medium)
        status.lineBreakMode = .byTruncatingTail
        input.setAccessibilityLabel("Message to coding agent")
        input.send = { [weak self] in self?.submit() }; input.stop = { [weak self] in self?.stopTurn() }
        input.nextAgent = { [weak self] in guard let self, !busy else { return }; agent.selectItem(at: (agent.indexOfSelectedItem + 1) % 6); changeAgent() }
        addSubview(hud)
        try? FileManager.default.createDirectory(at: cwd, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: settings), let saved = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let m = saved["models"] as? [String: String] { models = m }
            if let path = saved["cwd"] as? String, FileManager.default.fileExists(atPath: path) { cwd = URL(fileURLWithPath: path) }
            if let e = saved["engine"] as? String, let i = (0..<6).first(where: { catalog($0, label: false) == e }) { agent.selectItem(at: i) }
        }
        loadModels(); update()
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
        allow.frame = NSRect(x: t.minX + 4, y: t.maxY - 30, width: 90, height: 24)
        deny.frame = NSRect(x: t.minX + 100, y: t.maxY - 30, width: 60, height: 24)
        agent.frame = box(3); model.frame = box(4); effort.frame = box(5); folder.frame = box(6)
        inputScroll.frame = box(2); send.frame = box(7)
        status.frame = NSRect(x: 28, y: bounds.height - 29, width: bounds.width - 56, height: 12)
        for (scroll, text) in [(transcriptScroll, transcript), (inputScroll, input)] {
            text.setFrameSize(NSSize(width: scroll.contentSize.width, height: max(text.frame.height, scroll.contentSize.height)))
        }
        hud.needsDisplay = true
    }
    override func draw(_ dirtyRect: NSRect) {
        // Messages float directly over the world. Only the compact composer
        // gets a faint tint, so the game stays visible through it too.
        NSColor(calibratedWhite: 0.08, alpha: 0.22).setFill()
        NSBezierPath(roundedRect: box(0), xRadius: 6, yRadius: 6).fill()
    }
    // Only the HUD panels own hits. The center/right world stays interactive.
    override func hitTest(_ point: NSPoint) -> NSView? {
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
        for control in [agent as NSControl, model, effort, folder, fresh] { control.isEnabled = !busy }
        let configurable = ["codex", "claude", "cursor"].contains(selectedEngine)
        model.isEnabled = !busy && configurable; effort.isEnabled = !busy && configurable
        send.title = busy ? "Stop ⎋" : "Send ↵"; allow.isHidden = !pending; deny.isHidden = !pending
        heading.stringValue = pending ? "CODER / APPROVAL NEEDED" : busy ? "CODER / WORKING · \(selectedEngine)" : "CODER / MESSAGES"
        folder.title = "\(cwd.lastPathComponent) ▾"; folder.toolTip = cwd.path
        hud.busy = busy; hud.needsDisplay = true; needsDisplay = true; needsLayout = true
    }
    func loadModels() {
        model.removeAllItems()
        if selectedEngine == "claude" { model.addItems(withObjectValues: ["sonnet", "opus", "haiku"]) }
        if selectedEngine == "codex", catalogLoaded {
            for i in 0..<64 { let id = modelID(i); if !id.isEmpty { model.addItem(withObjectValue: id) } }
        }
        model.stringValue = models[selectedEngine] ?? ""
        model.toolTip = "A model ID accepted by the selected agent. Empty uses its configured default; Coder reports unsupported choices."
        updateEfforts()
        if selectedEngine == "codex", !catalogLoaded, catalogTask == nil {
            catalogTask = Task { [weak self] in
                guard let self else { return }
                defer { catalogTask = nil }
                do {
                    let data = try await ForgeCLI.run(cli, ["coder", "models", "--json"], timeout: 18, environment: environment())
                    guard !stopping else { return }
                    fgc_models_reset()
                    for line in String(decoding: data, as: UTF8.self).split(separator: "\n") {
                        let b = Array(line.utf8); _ = b.withUnsafeBufferPointer { fgc_models($0.baseAddress, Int64($0.count)) }
                    }
                    catalogLoaded = !modelID(0).isEmpty
                    if selectedEngine == "codex", catalogLoaded {
                        let draft = model.stringValue
                        model.removeAllItems()
                        for i in 0..<64 { let id = modelID(i); if !id.isEmpty { model.addItem(withObjectValue: id) } }
                        model.stringValue = draft; updateEfforts()
                    }
                    if !catalogLoaded { model.toolTip = "Codex catalog unavailable. Enter a model ID or leave empty for the configured default." }
                } catch { model.toolTip = "Model catalog unavailable: \(error.localizedDescription). Empty uses the configured default." }
            }
        }
    }
    func modelID(_ i: Int, effort: Int = -1) -> String {
        var b = [UInt8](repeating: 0, count: 128); let n = fgc_model(Int64(i), Int64(effort), &b, Int64(b.count))
        return String(decoding: b.prefix(max(0, Int(n))), as: UTF8.self)
    }
    func updateEfforts() {
        effort.removeAllItems(); effort.addItem(withTitle: "Effort: default")
        if selectedEngine == "codex", let i = (0..<64).first(where: { modelID($0) == model.stringValue && !model.stringValue.isEmpty }) {
            for j in 0..<8 { let value = modelID(i, effort: j); if !value.isEmpty { effort.addItem(withTitle: value) } }
        } else if selectedEngine == "claude" || selectedEngine == "cursor" { effort.addItems(withTitles: ["low", "medium", "high"]) }
        effort.selectItem(at: 0)
    }
    func comboBoxSelectionDidChange(_ notification: Notification) {
        if let selected = model.objectValueOfSelectedItem as? String { model.stringValue = selected }
        updateEfforts()
    }
    func controlTextDidEndEditing(_ obj: Notification) { updateEfforts() }
    func save() {
        models[selectedEngine] = model.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if let data = try? JSONSerialization.data(withJSONObject: ["engine": selectedEngine, "models": models, "cwd": cwd.path], options: [.sortedKeys]) { try? data.write(to: settings, options: .atomic) }
    }
    @objc func changeAgent() { parent = ""; session = ""; effort.selectItem(at: 0); loadModels(); save(); append("\n— \(selectedEngine): new conversation —\n", color: .systemTeal); update() }
    @objc func newChat() { guard !busy else { return }; parent = ""; session = ""; transcript.string = ""; update(); window?.makeFirstResponder(input) }
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
        let text = input.string.trimmingCharacters(in: .whitespacesAndNewlines)
        let bytes = Array(text.utf8)
        guard bytes.withUnsafeBufferPointer({ fgc_send(busy, $0.baseAddress, Int64($0.count)) }) else { status.stringValue = "Enter a message of at most 16 KiB."; return }
        save(); onActivity?(); input.string = ""; busy = true; pending = false; session = ""; lastEnding = ""
        append("\nYOU\n\(text)\n\n", color: .white)
        var args = ["coder"] + (parent.isEmpty ? ["run", text] : ["fork", parent, text])
        args += ["--engine", selectedEngine, "--cwd", cwd.path, "--json", "--ask"]
        if !model.stringValue.isEmpty { args += ["--model", model.stringValue] }
        if effort.indexOfSelectedItem > 0 { args += ["--effort", effort.titleOfSelectedItem!] }
        let p = Process(); p.executableURL = cli; p.arguments = args; p.currentDirectoryURL = cwd; p.environment = environment()
        let output = Pipe(); let errors = Pipe(); p.standardOutput = output; p.standardError = errors; p.standardInput = FileHandle.nullDevice
        process = p; status.stringValue = "Coder is starting \(selectedEngine)…"; update()
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
            busy = false; pending = false; process = nil; update()
            status.stringValue = "\(lastEnding) · \(selectedEngine) · \(session.isEmpty ? cwd.path : session)"
        }
    }
    func readField(_ i: Int) -> String {
        var b = [UInt8](repeating: 0, count: 262144); let n = fgc_copy(Int64(i), &b, Int64(b.count))
        return String(decoding: b.prefix(max(0, Int(n))), as: UTF8.self)
    }
    func receive(_ line: String) {
        let b = Array(line.utf8); let kind = b.withUnsafeBufferPointer { fgc_read($0.baseAddress, Int64($0.count)) }
        if kind == 1 { session = readField(0); return }
        let text = readField(1)
        switch kind {
        case 3: append("\(selectedEngine.uppercased())\n\(text)\n\n")
        case 4: append("› \(text)\n", color: .systemTeal); status.stringValue = text
        case 5: append("\(text)\n", color: .secondaryLabelColor)
        case 6: pending = true; append("\nAPPROVAL NEEDED\n\(text)\n", color: .systemOrange); status.stringValue = "Review the tool request, then Allow once or Deny."; update()
        case 7: lastEnding = text; append("\n[\(text)]\n", color: text.hasPrefix("finished") ? .systemTeal : .systemOrange)
        case 8: append("Edited: \(text)\n", color: .systemTeal)
        case 9: append("Model: \(text)\n", color: .systemTeal)
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
        stopping = true; catalogTask?.cancel(); stopTurn()
        if let process {
            let end = Date().addingTimeInterval(2)
            while process.isRunning && Date() < end { usleep(10_000) }
            if process.isRunning { process.terminate() }
        }
    }
}
