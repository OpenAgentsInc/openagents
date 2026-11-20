// Firm reason: 6 (AppKit lifecycle, the window and a Unix socket over the compiled page).
// The iOS Simulator in the pane on the window's right (page/simpane.coax; docs/desktop/
// ios-simulator-spec.md; GitHub #13811). The page draws the pane and speaks the helper's wire
// (coax/lib/agent/device/sim-wire.coax) itself. This file does effects only: the socket to the
// helper (oasim serve, started through `oasim status` the way any client starts it), moving its
// lines both ways, the device menu, revealing a saved screenshot, and the view that shows the
// device's screen over the page's rect (stills in Phase 0).
import AppKit

/// The socket to the simulator helper: whole lines in, bytes out.
final class SimHostClient {
    private var fd: Int32 = -1
    private var source: DispatchSourceRead?
    private var buf = [UInt8]()
    var onLine: (([UInt8]) -> Void)?
    var onClose: (() -> Void)?
    var isOpen: Bool { fd >= 0 }

    static var home: URL {
        if let h = ProcessInfo.processInfo.environment["OA_SIM_HOME"], !h.isEmpty { return URL(fileURLWithPath: h) }
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/sim")
    }
    static var socketPath: String {
        if let s = ProcessInfo.processInfo.environment["OA_SIM_SOCKET"], !s.isEmpty { return s }
        return home.appendingPathComponent("simhost.sock").path
    }
    /// The oasim beside the workers (Contents/Resources), or OA_SIMHOST for a worktree's build.
    static var oasim: String? {
        if let s = ProcessInfo.processInfo.environment["OA_SIMHOST"], !s.isEmpty { return s }
        guard let res = Bundle.main.resourcePath else { return nil }
        let p = res + "/oasim"
        return FileManager.default.isExecutableFile(atPath: p) ? p : nil
    }

    func connect() -> Bool {
        if fd >= 0 { return true }
        let path = SimHostClient.socketPath
        let s = socket(AF_UNIX, SOCK_STREAM, 0)
        guard s >= 0 else { return false }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { Darwin.close(s); return false }
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in for (i, b) in bytes.enumerated() { raw[i] = b }; raw[bytes.count] = 0 }
        let ok = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(s, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
        guard ok == 0 else { Darwin.close(s); return false }
        var one: Int32 = 1
        setsockopt(s, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout<Int32>.size))
        fd = s
        let src = DispatchSource.makeReadSource(fileDescriptor: s, queue: .main)
        src.setEventHandler { [weak self] in self?.readable() }
        source = src
        src.resume()
        return true
    }
    private func readable() {
        var chunk = [UInt8](repeating: 0, count: 65536)
        let n = chunk.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress, 65536) }
        if n <= 0 { close(); onClose?(); return }
        buf.append(contentsOf: chunk[0..<n])
        while let nl = buf.firstIndex(of: 10) {
            let line = Array(buf[0..<nl])
            buf.removeSubrange(0...nl)
            onLine?(line)
        }
        if buf.count > 1 << 20 { buf.removeAll() }   // no line is that long: the helper went wrong
    }
    /// Writes all of bytes (the helper reads as it goes; a line is small).
    func write(_ bytes: [UInt8]) {
        guard fd >= 0, !bytes.isEmpty else { return }
        var at = 0
        while at < bytes.count {
            let n = bytes.withUnsafeBytes { Darwin.write(fd, $0.baseAddress! + at, bytes.count - at) }
            if n <= 0 { close(); onClose?(); return }
            at += n
        }
    }
    func close() {
        source?.cancel(); source = nil
        if fd >= 0 { Darwin.close(fd); fd = -1 }
        buf.removeAll()
    }
}

/// The device's screen over the page: a layer whose contents are the newest still (Phase 0) or,
/// later, the device's IOSurface.
final class SimulatorPaneView: NSView {
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.contentsGravity = .resizeAspect
        layer?.masksToBounds = true
        layer?.cornerRadius = 18
        layer?.backgroundColor = NSColor.black.cgColor
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }
    func show(_ image: CGImage) { layer?.contents = image }
    // a press goes to the page beneath until input lands (Phase 1)
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension CoderWindow {
    /// View > iOS Simulator (Option-Command-S) and the palette's row.
    func toggleSimulator() {
        cd_sim_show(page, cd_sim_open(page) != 0 ? 0 : 1)
        act(cd_action(page))
        wake()
    }

    /// cd_action 46: what the pane asked.
    func simAsked() {
        switch cd_sim_op(page) {
        case 1: showSimMenu()
        case 2:
            var out = [CChar](repeating: 0, count: 4096)
            if cd_sim_saved(page, &out, Int32(out.count)) > 0 { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: String(cString: out))]) }
        case 3: simTimer?.invalidate(); simTimer = nil; simView?.isHidden = true
        case 4: startSimulator()
        default: break
        }
    }

    /// The pane opened: the folders, the socket (the helper started if none answers), the clock.
    func startSimulator() {
        let stills = SimHostClient.home.appendingPathComponent("stills")
        let shots = folder.appendingPathComponent(".openagents/shots")
        try? FileManager.default.createDirectory(at: stills, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        cd_sim_setup(page, stills.path, shots.path)
        connectSimulator()
        if simTimer == nil {
            let t = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in self?.simClock() }
            RunLoop.main.add(t, forMode: .common)
            simTimer = t
        }
    }
    private func connectSimulator() {
        if simClient.isOpen { return }
        simClient.onLine = { [weak self] line in
            guard let self else { return }
            line.withUnsafeBufferPointer { p in
                p.baseAddress!.withMemoryRebound(to: CChar.self, capacity: max(1, line.count)) { _ = cd_sim_line(self.page, $0, Int32(line.count)) }
            }
            self.pumpSimulator()
            self.act(cd_action(self.page))
        }
        simClient.onClose = { [weak self] in
            guard let self else { return }
            cd_sim_connected(self.page, 0, "The simulator helper stopped. OpenAgents starts it again.")
            self.simRetryAt = Date().addingTimeInterval(2)
            self.wake()
        }
        if simClient.connect() { cd_sim_connected(page, 1, ""); pumpSimulator(); return }
        // nothing answers: `oasim status` starts the helper (detached, as any client does), then we connect
        guard let oasim = SimHostClient.oasim else {
            cd_sim_connected(page, 0, "This build of OpenAgents has no oasim, so the simulator can't start.")
            return
        }
        guard !simStarting else { return }
        simStarting = true
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let p = Process()
            p.executableURL = URL(fileURLWithPath: oasim)
            p.arguments = ["status"]
            p.standardOutput = FileHandle.nullDevice; p.standardError = FileHandle.nullDevice
            try? p.run(); p.waitUntilExit()
            DispatchQueue.main.async {
                guard let self else { return }
                self.simStarting = false
                if self.simClient.connect() { cd_sim_connected(self.page, 1, ""); self.pumpSimulator() }
                else { cd_sim_connected(self.page, 0, "The simulator helper could not be started."); self.simRetryAt = Date().addingTimeInterval(5) }
                self.wake()
            }
        }
    }
    /// What the page queued for the helper goes out.
    func pumpSimulator() {
        guard simClient.isOpen else { return }
        var out = [UInt8](repeating: 0, count: 16384)
        let n = out.withUnsafeMutableBufferPointer { p in p.baseAddress!.withMemoryRebound(to: CChar.self, capacity: p.count) { cd_sim_out(page, $0, Int32(p.count)) } }
        if n > 0 { simClient.write(Array(out[0..<Int(n)])) }
    }
    /// Four times a second while the pane is open: the next still, a reconnect when the helper went.
    private func simClock() {
        guard cd_sim_open(page) != 0 else { return }
        if !simClient.isOpen {
            if let at = simRetryAt, Date() >= at { simRetryAt = nil; connectSimulator() }
            return
        }
        cd_sim_tick(page)
        pumpSimulator()
    }

    /// The device menu under the pointer: a row a device; the shown one ticked.
    private func showSimMenu() {
        var out = [CChar](repeating: 0, count: 8192)
        guard cd_sim_menu(page, &out, Int32(out.count)) > 0 else { return }
        let menu = NSMenu()
        for (i, row) in String(cString: out).split(separator: "\n").enumerated() {
            let cells = row.split(separator: "\t", omittingEmptySubsequences: false)
            let title = cells.count > 1 && !cells[1].isEmpty ? "\(cells[0]) (\(cells[1]))" : String(cells.first ?? "")
            let item = NSMenuItem(title: title, action: #selector(CoderWindow.simPicked(_:)), keyEquivalent: "")
            item.target = self; item.tag = i
            if cells.count > 2 && cells[2] == "1" { item.state = .on }
            menu.addItem(item)
        }
        let at = canvas.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        menu.popUp(positioning: nil, at: at, in: canvas)
    }
    @objc func simPicked(_ sender: NSMenuItem) {
        _ = cd_sim_pick(page, Int32(sender.tag))
        simGen = -1
        pumpSimulator(); wake()
    }

    /// Keeps the screen's view over the page's rect, hidden under an overlay or when none shows,
    /// and shows each new still.
    func syncSimulatorFrame() {
        pumpSimulator()   // what a press queued (Stop above all) goes out this frame, not on the next clock
        var rect = [Float](repeating: 0, count: 5)
        guard cd_sim_rect(page, &rect) != 0, rect[2] > 1, rect[3] > 1, rect[4] == 0 else {
            if let v = simView, !v.isHidden { v.isHidden = true }
            return
        }
        let view: SimulatorPaneView
        if let v = simView { view = v } else { view = SimulatorPaneView(frame: .zero); canvas.addSubview(view); simView = view }
        let h = canvas.bounds.height
        let w = CGFloat(rect[2]).rounded(), ht = CGFloat(rect[3]).rounded()
        let target = NSRect(x: CGFloat(rect[0]).rounded(), y: max(0, (h - CGFloat(rect[1]) - ht).rounded()), width: max(1, w), height: max(1, ht))
        if view.frame != target { view.frame = target; view.layer?.cornerRadius = min(w, ht) * 0.11 }
        var path = [CChar](repeating: 0, count: 4096)
        let gen = Int(cd_sim_still(page, &path, Int32(path.count)))
        if gen != simGen, path[0] != 0,
           let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: String(cString: path)) as CFURL, nil),
           let img = CGImageSourceCreateImageAtIndex(src, 0, nil) {
            simGen = gen
            view.show(img)
        }
        if view.isHidden { view.isHidden = false }
    }
}
