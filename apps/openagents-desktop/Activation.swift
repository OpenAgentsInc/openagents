// Firm reason: 6 (AppKit single-instance AF_UNIX control socket, URL scheme routing,
// multi-display NSScreen bounds validation, quit-hold HUD, and POSIX CLI shim install).
// Implements GitHub #13889 (mirroring coax/lib/agent/desktop/activation.coax).
import AppKit
import Darwin

final class DesktopActivation {
    static let shared = DesktopActivation()

    static let quitHoldKey = "openagents.quitHoldEnabled"
    static let savedBoundsKey = "openagents.windowBounds"

    static var quitHoldEnabled: Bool {
        get { UserDefaults.standard.bool(forKey: quitHoldKey) }
        set { UserDefaults.standard.set(newValue, forKey: quitHoldKey) }
    }

    private weak var delegate: CoderDelegate?
    private var serverFD: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private var socketPath: String = ""

    private var quitArmedAt: TimeInterval = 0
    private var quitHintWindow: NSPanel?
    private var quitHintTimer: Timer?

    private init() {}

    // MARK: - Single-instance 0700 owner-verified AF_UNIX control socket

    static func defaultSocketPath() -> String {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()
        return (home as NSString).appendingPathComponent(".openagents/desktop/activate.sock")
    }

    func start(delegate: CoderDelegate) {
        self.delegate = delegate
        // Skip binding the global socket during isolated scratch test launches so tests don't collide.
        if TestLaunch.active { return }
        let path = Self.defaultSocketPath()
        let dir = (path as NSString).deletingLastPathComponent
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        chmod(dir, 0o700)
        unlink(path)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        let maxLen = MemoryLayout.size(ofValue: addr.sun_path)
        guard bytes.count < maxLen else { close(fd); return }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: maxLen) { dst in
                for i in 0..<bytes.count { dst[i] = CChar(bitPattern: bytes[i]) }
                dst[bytes.count] = 0
            }
        }

        let bindRes = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                bind(fd, sa, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bindRes == 0 else { close(fd); return }
        chmod(path, 0o600)
        guard listen(fd, 8) == 0 else { close(fd); unlink(path); return }
        _ = fcntl(fd, F_SETFL, O_NONBLOCK)

        self.serverFD = fd
        self.socketPath = path
        let src = DispatchSource.makeReadSource(fileDescriptor: fd, queue: DispatchQueue.global(qos: .userInitiated))
        src.setEventHandler { [weak self] in self?.acceptPending() }
        src.setCancelHandler { [fd] in close(fd) }
        src.resume()
        self.acceptSource = src
    }

    func stop() {
        acceptSource?.cancel()
        acceptSource = nil
        serverFD = -1
        if !socketPath.isEmpty { unlink(socketPath); socketPath = "" }
    }

    private func acceptPending() {
        guard serverFD >= 0 else { return }
        while true {
            var sa = sockaddr_un()
            var len = socklen_t(MemoryLayout<sockaddr_un>.size)
            let client = withUnsafeMutablePointer(to: &sa) { ptr in
                ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.accept(serverFD, $0, &len) }
            }
            if client < 0 { break }
            // Verify peer UID matches our own user (owner-only local control socket).
            var euid: uid_t = 0
            var egid: gid_t = 0
            guard getpeereid(client, &euid, &egid) == 0, euid == geteuid() else {
                close(client)
                continue
            }
            handleClient(fd: client)
        }
    }

    private func handleClient(fd: Int32) {
        defer { close(fd) }
        var buf = [UInt8](repeating: 0, count: 4096)
        let n = Darwin.read(fd, &buf, buf.count - 1)
        guard n > 0 else { return }
        let raw = String(decoding: buf[0..<n], as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        guard let data = raw.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = obj["op"] as? String else {
            let err = "{\"ok\":false,\"error\":\"invalid_json\"}\n"
            _ = err.withCString { Darwin.write(fd, $0, strlen($0)) }
            return
        }
        DispatchQueue.main.async { [weak self] in
            self?.dispatchCommand(op: op, obj: obj)
        }
        let ok = "{\"ok\":true}\n"
        _ = ok.withCString { Darwin.write(fd, $0, strlen($0)) }
    }

    private func dispatchCommand(op: String, obj: [String: Any]) {
        guard let delegate else { return }
        NSApp.activate(ignoringOtherApps: true)
        switch op {
        case "show":
            delegate.main?.show()
        case "open":
            if let path = obj["path"] as? String, !path.isEmpty {
                let url = URL(fileURLWithPath: path)
                var isDir: ObjCBool = false
                if FileManager.default.fileExists(atPath: path, isDirectory: &isDir), isDir.boolValue {
                    delegate.main?.show()
                    delegate.main?.openFolderURL(url)
                } else {
                    delegate.main?.show()
                    delegate.main?.openFileInPane(url)
                }
            }
        case "chat":
            if let id = obj["id"] as? String, !id.isEmpty {
                delegate.main?.show()
                delegate.main?.openChatById(id)
            }
        case "uri":
            if let raw = obj["url"] as? String, let url = URL(string: raw) {
                _ = handleURL(url)
            }
        case "new_window":
            let path = obj["path"] as? String
            delegate.openNewWindow(folderPath: path)
        default:
            break
        }
    }

    // MARK: - Custom openagents:// protocol handler

    @discardableResult
    func handleURL(_ url: URL) -> Bool {
        guard url.scheme?.lowercased() == "openagents", let delegate else { return false }
        let host = (url.host ?? "").lowercased()
        let comps = URLComponents(url: url, resolvingAgainstBaseURL: false)
        let query = Dictionary(uniqueKeysWithValues: (comps?.queryItems ?? []).compactMap { item in
            item.value.map { (item.name, $0) }
        })
        NSApp.activate(ignoringOtherApps: true)
        delegate.main?.show()
        switch host {
        case "open":
            guard let path = query["path"], !path.isEmpty else { return false }
            let target = URL(fileURLWithPath: path)
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: path, isDirectory: &isDir), isDir.boolValue {
                delegate.main?.openFolderURL(target)
            } else {
                delegate.main?.openFileInPane(target)
            }
            return true
        case "chat":
            let trimmed = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let id = !trimmed.isEmpty ? trimmed : (query["id"] ?? "")
            guard !id.isEmpty, !id.contains("/"), !id.contains("\\") else { return false }
            delegate.main?.openChatById(id)
            return true
        case "settings":
            let pageName = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/")).lowercased()
            let pages = ["general", "models", "foreman", "account", "about", "wallet", "usage"]
            let idx = pages.firstIndex(of: pageName) ?? 0
            delegate.main?.openSettings(Int32(idx))
            return true
        case "new":
            if let folder = query["folder"], !folder.isEmpty {
                delegate.main?.openFolderURL(URL(fileURLWithPath: folder))
            }
            delegate.main?.newChat()
            return true
        case "oauth":
            guard FeatureFlags.isOn(.mcp) || FeatureFlags.isOn(.account) else { return false }
            if url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/")).lowercased() == "callback" {
                NotificationCenter.default.post(name: Notification.Name("OpenAgentsOAuthCallback"), object: nil, userInfo: ["url": url])
                return true
            }
            return false
        default:
            return false
        }
    }

    // MARK: - Hold-to-Quit / Double-Press Cmd+Q protection

    func shouldAllowQuit(runningAgents: Int) -> Bool {
        if TestLaunch.active { return true }
        guard Self.quitHoldEnabled || runningAgents > 0 else {
            quitArmedAt = 0
            return true
        }
        let now = Date().timeIntervalSinceReferenceDate
        if quitArmedAt > 0 && now - quitArmedAt <= 1.5 {
            quitArmedAt = 0
            hideQuitHint()
            return true
        }
        quitArmedAt = now
        let msg = runningAgents > 0
            ? "Press ⌘Q again to quit OpenAgents (\(runningAgents) running)"
            : "Press ⌘Q again to quit OpenAgents"
        showQuitHint(msg)
        return false
    }

    private func showQuitHint(_ text: String) {
        hideQuitHint()
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 340, height: 42),
            styleMask: [.nonactivatingPanel, .borderless],
            backing: .buffered,
            defer: false
        )
        panel.level = .floating
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true

        let box = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: 340, height: 42))
        box.material = .hudWindow
        box.state = .active
        box.wantsLayer = true
        box.layer?.cornerRadius = 10
        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.textColor = .labelColor
        label.alignment = .center
        label.frame = NSRect(x: 12, y: 12, width: 316, height: 18)
        box.addSubview(label)
        panel.contentView = box

        if let parent = delegate?.main?.window {
            let pf = parent.frame
            panel.setFrameOrigin(NSPoint(x: pf.midX - 170, y: pf.midY - 21))
        } else {
            panel.center()
        }
        panel.orderFrontRegardless()
        quitHintWindow = panel
        quitHintTimer = Timer.scheduledTimer(withTimeInterval: 1.6, repeats: false) { [weak self] _ in
            self?.hideQuitHint()
        }
    }

    private func hideQuitHint() {
        quitHintTimer?.invalidate()
        quitHintTimer = nil
        quitHintWindow?.orderOut(nil)
        quitHintWindow = nil
    }

    // MARK: - Multi-display saved window bounds validation (1 px edge tolerance)

    static func saveWindowBounds(_ frame: NSRect) {
        if TestLaunch.active { return }
        let dict: [String: Double] = [
            "x": Double(frame.origin.x),
            "y": Double(frame.origin.y),
            "w": Double(frame.size.width),
            "h": Double(frame.size.height)
        ]
        UserDefaults.standard.set(dict, forKey: savedBoundsKey)
    }

    static func restoredWindowBounds(defaultSize: NSSize) -> NSRect? {
        guard let dict = UserDefaults.standard.dictionary(forKey: savedBoundsKey),
              let x = dict["x"] as? Double,
              let y = dict["y"] as? Double,
              let w = dict["w"] as? Double,
              let h = dict["h"] as? Double else { return nil }
        let saved = NSRect(x: x, y: y, width: max(420, w), height: max(320, h))
        let screens = NSScreen.screens.map(\.visibleFrame)
        return validateBounds(saved, displays: screens, fallbackSize: defaultSize)
    }

    static func validateBounds(_ saved: NSRect, displays: [NSRect], fallbackSize: NSSize) -> NSRect {
        guard let primary = displays.first else { return saved }
        for d in displays {
            let tol = d.insetBy(dx: -1, dy: -1)
            let inter = saved.intersection(tol)
            let topOK = saved.maxY > d.minY + 28 && saved.maxY <= tol.maxY
            if !inter.isNull && inter.width >= 120 && inter.height >= 80 && topOK {
                return saved
            }
        }
        let cw = min(max(420, saved.width > 0 ? saved.width : fallbackSize.width), primary.width)
        let ch = min(max(320, saved.height > 0 ? saved.height : fallbackSize.height), primary.height)
        return NSRect(
            x: primary.minX + floor((primary.width - cw) / 2),
            y: primary.minY + floor((primary.height - ch) / 2),
            width: cw,
            height: ch
        )
    }

    // MARK: - CLI PATH shim installer / uninstaller

    static func shimPath() -> String {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()
        return (home as NSString).appendingPathComponent(".openagents/bin/openagents-desktop")
    }

    static func isShimInstalled() -> Bool {
        FileManager.default.isExecutableFile(atPath: shimPath())
    }

    @discardableResult
    static func installShim() -> (ok: Bool, message: String) {
        let exe = Bundle.main.executablePath ?? "/Applications/OpenAgents.app/Contents/MacOS/OpenAgents"
        guard !exe.contains("'"), !exe.contains("\n") else {
            return (false, "Invalid application executable path.")
        }
        let dst = shimPath()
        let dir = (dst as NSString).deletingLastPathComponent
        do {
            try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            let script = "#!/bin/sh\n# Installed by OpenAgents Desktop (#13889)\nexec '\(exe)' \"$@\"\n"
            try script.write(toFile: dst, atomically: true, encoding: .utf8)
            chmod(dst, 0o755)
            return (true, "Installed CLI shim at \(dst)")
        } catch {
            return (false, "Failed to install CLI shim: \(error.localizedDescription)")
        }
    }

    @discardableResult
    static func uninstallShim() -> (ok: Bool, message: String) {
        let dst = shimPath()
        do {
            if FileManager.default.fileExists(atPath: dst) {
                try FileManager.default.removeItem(atPath: dst)
            }
            return (true, "Removed CLI shim at \(dst)")
        } catch {
            return (false, "Failed to remove CLI shim: \(error.localizedDescription)")
        }
    }
}
