// Firm reason: 6 (AppKit events, CADisplayLink and CAMetalLayer effects).
import AppKit
import QuartzCore

@MainActor
final class ForgeMetalWorld: NSView, @preconcurrency NSTextInputClient {
    var host: UnsafeMutableRawPointer?
    var network: ForgeMetalNetwork?
    var audio: ForgeMetalAudio?
    var display: CADisplayLink?
    var lastTime = 0.0
    var paused = false
    var completed = false
    var resources: URL!
    var storage: URL!
    var action: ((Int32, String) -> Void)?
    var failed: ((String) -> Void)?
    var ready: (() -> Void)?
    var held: [UInt16: Int32] = [:]
    var marked = NSAttributedString(string: "")
    var tracking: NSTrackingArea?
    var captured = false
    var drawScale: Float = 1
    var preferredHz: Float = 60
    var quality = ""
    var virtualPoint = NSPoint.zero
    let tab = UUID().uuidString
    var fresh = true
    var frameCaptures: [UUID:CheckedContinuation<String?,Never>] = [:]
    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }
    override func makeBackingLayer() -> CALayer { CAMetalLayer() }
    override init(frame: NSRect) { super.init(frame: frame); wantsLayer = true }
    required init?(coder: NSCoder) { fatalError() }
    func open(resources: URL, storage: URL, page: URL, local: String, muted: Bool) throws {
        close()
        self.resources = resources; self.storage = storage
        preferredHz = Float(window?.screen?.maximumFramesPerSecond ?? 60)
        quality = (try? String(contentsOf: storage.appendingPathComponent("metal-quality.txt"), encoding: .utf8)) ?? ""
        guard let h = mf_open(resources.path, resources.appendingPathComponent("metal/libeverglade.dylib").path, false, false) else {
            throw ShellError.failed(mf_error(nil).map { String(cString: $0) } ?? "Metal host failed.")
        }
        host = h; mf_layer(h, Unmanaged.passUnretained(layer!).toOpaque())
        let origin = "\(page.scheme!)://\(page.host!):\(page.port!)"
        mf_network_config(h, origin, local)
        network = ForgeMetalNetwork(host: h)
        network?.changed = { [weak self] in self?.drain() }
        network?.frame = { [weak self] in await self?.frameJpeg() }
        audio = ForgeMetalAudio(host: h, resources: resources, muted: muted)
        let playerData = try? Data(contentsOf:storage.appendingPathComponent("metal-player.json"))
        let player = playerData.flatMap { try? JSONSerialization.jsonObject(with:$0) as? [String:Any] }
        network?.playerKey = player?["key"] as? String ?? ""
        let sound = (try? String(contentsOf:storage.appendingPathComponent("metal-sound.txt"),encoding:.utf8)) ?? ""
        let visitorFile = storage.appendingPathComponent("metal-visitor.txt")
        let visitor = (try? String(contentsOf:visitorFile,encoding:.utf8)) ?? UUID().uuidString
        try? Data(visitor.utf8).write(to:visitorFile,options:.atomic)
        let facts = "tab=metal-\(UUID().uuidString)\nfresh=1\nvisitor=metal\nnow=\(Int64(Date().timeIntervalSince1970*1000))\nnav=0\nurl=\(page.absoluteString)\nref=\nua=aethyrforge-metal\nlang=\(Locale.current.identifier)\ntz=\(TimeZone.current.identifier)\nsw=\(Int(window?.screen?.frame.width ?? bounds.width))\nsh=\(Int(window?.screen?.frame.height ?? bounds.height))\nvw=\(Int(bounds.width))\nvh=\(Int(bounds.height))\ndpr=\(window?.backingScaleFactor ?? 1)\ncoarse=0\ntouch=0\ngpu=1\nhidden=0\nfocused=1\nclip=0\nwebdriver=0\nfcp=0\ndcl=0\nplayer=\naccount=\nrealm=everglade\nslot=everglade\nvoice=everglade\nruleset=srd5\nfork=\nrules=\nstyle=painterly\nclock=cycle\n"
        let prefix = "tab=\(tab)\nfresh=\(fresh ? 1:0)\nvisitor=\(visitor)\nplayer=\(player?["id"] as? String ?? "")\naccount=\(player?["account"] as? String ?? "")\nsound=\(sound)\n"
        let lines = facts.split(separator:"\n").filter { line in !["tab=","fresh=","visitor=","player=","account="].contains(where:{ line.hasPrefix($0) }) }
        message(600, prefix + lines.joined(separator:"\n") + "\nquality=\(quality)\ncores=\(ProcessInfo.processInfo.processorCount)\nframe_hz=\(Int(preferredHz))\nminimum_hz=60\n")
        fresh = false
        for kind: Int32 in [4, 11, 12] {
            if let text = try? String(contentsOf: storage.appendingPathComponent("metal-keep-\(kind).txt"), encoding: .utf8) { message(kind, text) }
        }
        mf_focus(h, window?.isKeyWindow ?? true)
        completed = false; lastTime = 0; paused = false
        display = displayLink(target: self, selector: #selector(tick(_:)))
        let hz = preferredHz
        display?.preferredFrameRateRange = CAFrameRateRange(minimum: min(30, hz), maximum: hz, preferred: hz)
        display?.add(to: .main, forMode: .common)
        drain()
    }
    @objc func tick(_ link: CADisplayLink) {
        guard let h = host, !paused else { return }
        let now = link.targetTimestamp
        let pixels = convertToBacking(bounds).size
        var surface = MFSurface()
        guard mf_surface(h, Int64(bounds.width), Int64(bounds.height), Int64(pixels.width), Int64(pixels.height), quality, &surface) else { return }
        let before = mf_info(h, 0)
        guard mf_frame_at(h, Float(surface.width), Float(surface.height), surface.scale, Int64(now*1_000_000)) else {
            display?.isPaused = true; failed?(mf_error(h).map { String(cString: $0) } ?? "Metal frame failed."); return
        }
        if mf_info(h, 0) != before { lastTime = now; drawScale = surface.scale; audio?.drain() }
        if mf_info(h,0) != before && !frameCaptures.isEmpty {
            var count = 0
            let bytes = mf_frame_jpeg(h,&count)
            let image = bytes.map { String(decoding:UnsafeBufferPointer(start:$0,count:count),as:UTF8.self) }
            let captures = frameCaptures; frameCaptures.removeAll()
            for capture in captures.values { capture.resume(returning:image) }
        }
        if !completed && mf_info(h, 10) > 0 { completed = true; ready?() }
        drain()
    }
    func setPaused(_ value: Bool) {
        guard value != paused else { return }
        paused = value; display?.isPaused = value; lastTime = 0
        if let h = host { mf_shown(h,!value) }
        audio?.setPaused(value)
        message(value ? 601:602, "")
        if value { focus(false) } else { focus(window?.isKeyWindow ?? true) }
    }
    func message(_ kind: Int32, _ text: String) {
        guard let h = host else { return }
        Data(text.utf8).withUnsafeBytes { mf_message(h, kind, $0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
        drain()
    }
    func drain() {
        guard let h = host else { return }
        while let raw = mf_take_effect(h) {
            if let e = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any] { network?.execute(e) }
        }
        let code = mf_action(h)
        if code != 0 {
            var n = 0
            let p = mf_outbox(h, &n), text = p.map { String(decoding: UnsafeBufferPointer(start: $0, count: n), as: UTF8.self) } ?? ""
            mf_action_done(h)
            if code == 6 { quality = text; try? Data(text.utf8).write(to: storage.appendingPathComponent("metal-quality.txt"), options: .atomic) }
            else if [4, 11, 12].contains(code) { try? Data(text.utf8).write(to: storage.appendingPathComponent("metal-keep-\(code).txt"), options: .atomic) }
            else if code == 2 { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(text, forType: .string) }
            else { action?(code, text) }
        }
    }
    func focus(_ focused: Bool) {
        if !focused { held.removeAll(); setCaptured(false); marked = NSAttributedString(string: "") }
        if let h = host { mf_focus(h, focused); drain() }
    }
    func close() {
        let captures = frameCaptures; frameCaptures.removeAll()
        for capture in captures.values { capture.resume(returning:nil) }
        display?.invalidate(); display = nil
        if let h = host { mf_network_reset(h) }
        network?.close(); network = nil; audio?.close(); audio = nil
        setCaptured(false)
        if let h = host { mf_layer(h, nil); mf_close(h) }; host = nil
    }
    func frameJpeg() async -> String? {
        await withCheckedContinuation { continuation in
            let id = UUID(); frameCaptures[id] = continuation
            Task { [weak self] in
                try? await Task.sleep(for:.seconds(5))
                self?.frameCaptures.removeValue(forKey:id)?.resume(returning:nil)
            }
        }
    }
    func setCaptured(_ value: Bool) {
        guard captured != value else { return }; captured = value
        CGAssociateMouseAndMouseCursorPosition(value ? 0:1)
        if value { virtualPoint = convert(window?.mouseLocationOutsideOfEventStream ?? .zero, from: nil); NSCursor.hide() } else { NSCursor.unhide() }
    }
    override func updateTrackingAreas() {
        if let tracking { removeTrackingArea(tracking) }
        tracking = NSTrackingArea(rect: bounds, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(tracking!); super.updateTrackingAreas()
    }
    func pointer(_ event: NSEvent, phase: Int32) {
        guard let h = host else { return }
        if phase == 0 { window?.makeFirstResponder(self); audio?.start() }
        var p = convert(event.locationInWindow, from: nil)
        if captured { virtualPoint.x += event.deltaX; virtualPoint.y += event.deltaY; p = virtualPoint }
        p.x *= CGFloat(drawScale); p.y *= CGFloat(drawScale)
        _ = mf_pointer(h, Float(p.x), Float(p.y), phase, 0, 1); drain()
    }
    override func mouseDown(with e: NSEvent) { pointer(e, phase: 0) }
    override func mouseDragged(with e: NSEvent) { pointer(e, phase: 1) }
    override func mouseMoved(with e: NSEvent) { pointer(e, phase: 1) }
    override func mouseUp(with e: NSEvent) { pointer(e, phase: 2) }
    override func rightMouseDown(with e: NSEvent) { setCaptured(true); pointer(e, phase: 0) }
    override func rightMouseDragged(with e: NSEvent) { pointer(e, phase: 1) }
    override func rightMouseUp(with e: NSEvent) { pointer(e, phase: 2); setCaptured(false) }
    override func scrollWheel(with e: NSEvent) { if let h = host { mf_wheel(h, Float(e.scrollingDeltaY)); drain() } }
    func mods(_ e: NSEvent) -> Int32 { (e.modifierFlags.contains(.shift) ? 1:0) | (e.modifierFlags.contains(.control) ? 2:0) | (e.modifierFlags.contains(.option) ? 4:0) | (e.modifierFlags.contains(.command) ? 8:0) }
    func key(_ e: NSEvent) -> Int32 {
        let named: [UInt16: Int32] = [53:-1,36:-2,76:-2,51:-3,123:-4,124:-5,126:-6,125:-7,115:-8,119:-9,116:-10,121:-11,117:-12,56:-13,60:-13,59:-14,62:-14,48:-15]
        return named[e.keyCode] ?? e.characters?.unicodeScalars.first.map { Int32($0.value) } ?? 0
    }
    override func keyDown(with e: NSEvent) {
        guard let h = host else { return }; audio?.start()
        if e.modifierFlags.contains(.command), e.charactersIgnoringModifiers == "v" { if let text = NSPasteboard.general.string(forType: .string) { message(1, text) }; return }
        let k = key(e)
        if mf_info(h, 9) == 1 && k >= 0 && e.modifierFlags.intersection([.command, .control]).isEmpty { interpretKeyEvents([e]); return }
        if k != 0 { held[e.keyCode] = k; _ = mf_key(h, k, true, mods(e)); drain() }
        if k == -1 { setCaptured(false) }
    }
    override func keyUp(with e: NSEvent) { if let h = host { _ = mf_key(h, held.removeValue(forKey: e.keyCode) ?? key(e), false, mods(e)); drain() } }
    override func flagsChanged(with e: NSEvent) {
        let k = key(e); guard k == -13 || k == -14, let h = host else { return }
        let down = e.modifierFlags.contains(k == -13 ? .shift:.control)
        _ = mf_key(h, k, down, mods(e)); drain()
    }
    func insertText(_ string: Any, replacementRange: NSRange) { marked = NSAttributedString(string: ""); message(1, (string as? NSAttributedString)?.string ?? string as? String ?? "") }
    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) { marked = (string as? NSAttributedString) ?? NSAttributedString(string: string as? String ?? "") }
    func unmarkText() { marked = NSAttributedString(string: "") }
    func selectedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }
    func markedRange() -> NSRange { NSRange(location: hasMarkedText() ? 0:NSNotFound, length: marked.length) }
    func hasMarkedText() -> Bool { marked.length > 0 }
    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? { nil }
    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect { window?.convertToScreen(convert(NSRect(x: 16, y: bounds.height-40, width: 2, height: 24), to: nil)) ?? .zero }
    func characterIndex(for point: NSPoint) -> Int { 0 }
    override func doCommand(by selector: Selector) { if selector == #selector(NSResponder.deleteBackward(_:)), let h = host { _ = mf_key(h, -3, true, 0); drain() } }
}
