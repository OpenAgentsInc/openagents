// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Dialogs: a chat's budget and goal, the Claude key and the OpenRouter models, drawn by the page
// (page/overlay.coax, GitHub #13746). The field with the keyboard is the page's own editor
// (page/editor.coax, GitHub #13742): a secure one shows dots and never copies.
import AppKit
import CryptoKit
import UniformTypeIdentifiers

/// A dialog the page draws: its words, its fields and its buttons in the foot's order. `confirm` is
/// the button Return presses (a destructive dialog names Cancel), `cancel` the one Escape stands for.
struct CoderDialog {
    enum Kind: Int32 { case primary = 0, secondary = 1, danger = 2 }
    enum Size: Int32 { case sm = 0, md = 1, lg = 2 }
    struct Field { var label = ""; var value = ""; var hint = ""; var secure = false; var lines: Int32 = 1 }
    struct Button { var title: String; var kind: Kind = .secondary; var aside = false }
    var size: Size = .sm
    var title: String
    var body = ""
    var fields: [Field] = []
    var buttons: [Button] = []
    var confirm = -1
    var cancel = -1
    var focus = 0
    var selectAll = false
    /// One button, OK: a notice (an NSAlert sheet's default button).
    static func notice(_ title: String, _ body: String = "") -> CoderDialog {
        CoderDialog(title: title, body: body, buttons: [Button(title: "OK", kind: .primary)], confirm: 0, cancel: 0)
    }
}

/// What the overlay keeps per window: the answers' handlers, and the clock and the keys while one is up.
final class CoderOverlay: NSObject {
    weak var owner: CoderWindow?
    let page: UnsafeMutableRawPointer
    private var fields = 0
    private var dialogDone: ((Int, [String]) -> Void)?
    private var menuDone: ((Int) -> Void)?
    private var keys: Any?
    private var clock: Timer?
    private var paletteWatched = false

    init(owner: CoderWindow) {
        self.owner = owner; page = owner.page
        super.init()
        cd_on_overlay(page, { raw in guard let raw else { return }; Unmanaged<CoderOverlay>.fromOpaque(raw).takeUnretainedValue().collect() },
                      Unmanaged.passUnretained(self).toOpaque())
    }

    func present(_ d: CoderDialog, done: @escaping (Int, [String]) -> Void) {
        if let earlier = dialogDone { dialogDone = nil; earlier(-1, []) }
        cd_dialog_begin(page, d.size.rawValue, d.title, d.body)
        for f in d.fields { _ = cd_dialog_field(page, f.label, f.value, f.hint, f.secure ? 1 : 0, f.lines) }
        for b in d.buttons { _ = cd_dialog_button(page, b.title, b.kind.rawValue, b.aside ? 1 : 0) }
        fields = min(d.fields.count, 4)
        dialogDone = done; menuDone = nil
        cd_dialog_show(page, Int32(d.confirm), Int32(d.cancel), Int32(d.focus))
        if d.selectAll { cd_dialog_select_all(page) }
        // the page's view takes the keys: its field with the keyboard, or nothing to type into
        if let owner { owner.window.makeFirstResponder(owner.canvas) }
        begin()
    }
    struct MenuItem { var label = ""; var kind: Int32 = 0; var checked = false; var disabled = false; var danger = false }
    /// A menu at the pointer, in the canvas's device pixels; done gets the item chosen (-1 none).
    func presentMenu(_ items: [MenuItem], x: Float, y: Float, done: @escaping (Int) -> Void) {
        cd_menu_begin(page)
        for i in items { _ = cd_menu_item(page, i.label, i.kind, i.checked ? 1 : 0, i.disabled ? 1 : 0, i.danger ? 1 : 0) }
        menuDone = done
        cd_menu_show(page, x, y)
        begin()
    }
    /// The command palette (page/palette.coax) opened: its field takes the keyboard while it is up,
    /// and the composer takes it back when it closes.
    func watchPalette() {
        paletteWatched = true
        if let owner { owner.window.makeFirstResponder(owner.canvas) }
        begin()
    }
    /// The palette is up with no dialog over it.
    var paletteUp: Bool { cd_palette_open(page) != 0 && dialogDone == nil }
    var isOpen: Bool { cd_overlay_open(page) != 0 }
    var menuOpen: Bool { menuDone != nil }
    var dialogOpen: Bool { isOpen && !menuOpen }

    private func begin() {
        owner?.wake()
        if keys == nil { keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in self?.key(event) == true ? nil : event } }
        if clock == nil {
            let t = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.place() }
            RunLoop.main.add(t, forMode: .common); clock = t
        }
    }
    /// Escape for whoever else listens for it (settings): true when a dialog or a menu took it.
    func escape() -> Bool {
        guard isOpen else { return false }
        _ = cd_overlay_key(page, -1, 0); place(); collect()
        return true
    }
    /// The keys while a dialog or a menu is up: Escape, Return, the arrows and Tab are the page's;
    /// a dialog's typing is its field's; the menu bar's shortcuts wait, but for editing ones.
    private func key(_ event: NSEvent) -> Bool {
        guard isOpen, event.window === owner?.window else { return false }
        let flags = event.modifierFlags
        var mods: Int32 = 0
        if flags.contains(.shift) { mods |= 1 }; if flags.contains(.control) { mods |= 2 }; if flags.contains(.option) { mods |= 4 }; if flags.contains(.command) { mods |= 8 }
        // ⌃Tab and ⌃⇧Tab are the window's (the next and previous chat)
        if event.keyCode == 48 && flags.contains(.control) { return false }
        if paletteUp {
            // the menu bar's shortcuts close it and go on (⌘K only closes); Backspace is the page's (editor.coax)
            if flags.contains(.command) && ![36, 76, 53, 126, 125, 48].contains(event.keyCode) {
                let ch = event.charactersIgnoringModifiers?.lowercased() ?? ""
                if ["c", "v", "x", "a", "z"].contains(ch) { return false }
                _ = cd_palette_toggle(page, 0); place(); collect()
                return ch == "k"
            }
        }
        var code: Int32 = 0
        switch event.keyCode {
        case 53: code = -1
        case 36, 76: code = -2
        case 126: code = -6
        case 125: code = -7
        case 48: code = -13
        default: break
        }
        if code != 0 || menuOpen {
            let took = cd_overlay_key(page, code, mods) != 0
            if took { place(); collect() }
            return took || menuOpen
        }
        let editing = owner?.window.firstResponder === owner?.canvas && cd_wants_text(page) != 0
        if flags.contains(.command) { return !(editing && ["c", "v", "x", "a", "z"].contains(event.charactersIgnoringModifiers?.lowercased() ?? "")) }
        return !editing
    }
    /// While one is up: once it is gone, the clock and the keys stop. The palette closed: the
    /// composer takes the keyboard back (a dialog it opened takes it instead).
    func place() {
        if paletteWatched && cd_palette_open(page) == 0 { paletteWatched = false; if dialogDone == nil { owner?.refocus() } }
        if !isOpen { finish() }
    }
    /// Nothing is up any more: the clock and the keys stop.
    private func finish() {
        clock?.invalidate(); clock = nil
        if let k = keys { NSEvent.removeMonitor(k); keys = nil }
    }
    /// After the pointer or a key: an answer the page gave goes to its handler, once.
    func collect() {
        let a = Int(cd_dialog_answer(page))
        if a != -2, let done = dialogDone {
            dialogDone = nil
            var values: [String] = []
            var out = [CChar](repeating: 0, count: 4097)
            for i in 0..<fields { values.append(cd_dialog_value(page, Int32(i), &out, Int32(out.count)) >= 0 ? String(cString: out) : "") }
            DispatchQueue.main.async { done(a, values) }
        }
        let m = Int(cd_menu_answer(page))
        if m != -2, let done = menuDone {
            menuDone = nil
            DispatchQueue.main.async { done(m) }
        }
        owner?.wake()
    }
}

private var coderOverlays: [ObjectIdentifier: CoderOverlay] = [:]

#if OA_FULL
/// SHA-256 verified Executor Models & Compiled Weight-Module store (~/.openagents/modules)
/// backed by Coax #authority admission (psionic/lib/executor-desktop.coax via oa_dc_exec_scan; #13924).
enum ExecutorModelStore {
    struct Snapshot {
        let selectable: Bool
        let stateCode: Int
        let verifiedModules: Int
        let tamperedModules: Int
        let status: String
        let lineage: String
        let badge: String
        let details: String
    }

    static var lastError: String?
    static var isInstalling: Bool = false

    static func scan(home: String = "") -> Snapshot {
        let ramMB = Int64(ProcessInfo.processInfo.physicalMemory / (1024 * 1024))
        var stBuf = [UInt8](repeating: 0, count: 160)
        var linBuf = [UInt8](repeating: 0, count: 192)
        var bdgBuf = [UInt8](repeating: 0, count: 64)
        var detBuf = [UInt8](repeating: 0, count: 2048)
        var counts = [Int64](repeating: 0, count: 4)
        let rc: Int64 = DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(home) { hStr in
                DesktopCore.withByteBuf(&stBuf) { stV in
                    DesktopCore.withByteBuf(&linBuf) { linV in
                        DesktopCore.withByteBuf(&bdgBuf) { bdgV in
                            DesktopCore.withByteBuf(&detBuf) { detV in
                                DesktopCore.withS64Buf(&counts) { cntV in
                                    oa_dc_exec_scan(hStr, ramMB, stV, linV, bdgV, detV, cntV)
                                }
                            }
                        }
                    }
                }
            }
        }
        func cstr(_ b: [UInt8]) -> String {
            let n = b.firstIndex(of: 0) ?? b.count
            return String(decoding: b[0..<n], as: UTF8.self)
        }
        return Snapshot(
            selectable: rc == 1 && counts[3] == 1,
            stateCode: Int(counts[0]),
            verifiedModules: Int(counts[1]),
            tamperedModules: Int(counts[2]),
            status: cstr(stBuf),
            lineage: cstr(linBuf),
            badge: cstr(bdgBuf),
            details: cstr(detBuf)
        )
    }

    static var isSelectable: Bool { scan().selectable }

    static func findOpenAgentsCLI() -> String? {
        let fm = FileManager.default
        let homeBin = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/openagents").path
        var candidates: [String] = []
        if let res = Bundle.main.resourcePath { candidates.append(res + "/openagents") }
        candidates.append(homeBin)
        candidates.append("/usr/local/bin/openagents")
        candidates.append("/opt/homebrew/bin/openagents")
        for c in candidates where fm.isExecutableFile(atPath: c) { return c }
        return nil
    }

    static func installBuiltins(completion: @escaping (String?) -> Void) {
        guard !isInstalling else { return }
        guard let cli = findOpenAgentsCLI() else {
            lastError = "openagents CLI not found in app bundle or ~/.openagents/bin/openagents."
            completion(lastError)
            return
        }
        isInstalling = true
        lastError = nil
        DispatchQueue.global(qos: .userInitiated).async {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: cli)
            p.arguments = ["module", "install-builtins"]
            let pipe = Pipe()
            p.standardOutput = pipe
            p.standardError = pipe
            var errMsg: String?
            do {
                try p.run()
                p.waitUntilExit()
                if p.terminationStatus != 0 {
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    let out = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    errMsg = out.isEmpty ? "openagents module install-builtins exited \(p.terminationStatus)." : out
                }
            } catch {
                errMsg = "Could not launch openagents CLI: \(error.localizedDescription)"
            }
            DispatchQueue.main.async {
                isInstalling = false
                lastError = errMsg
                completion(errMsg)
            }
        }
    }
}
#endif

/// Resident native Psionic Metal inference server for Inkling 2 Mini on 127.0.0.1:<port>.
/// Defaults to 18794 and automatically picks the next free localhost port (18794..<18858) when
/// another process occupies 18794, unless OA_INKLING_PORT was explicitly set.
final class LocalModelServer {
    enum State: Equatable {
        case idle
        case starting
        case ready(Int)
        case failed(String)
    }

    static let shared = LocalModelServer()
    static let stateChangedNotification = Notification.Name("OpenAgentsLocalModelServerStateChanged")
    static let host = "127.0.0.1"
    static let defaultPort = 18794
    static let portScanCount = 64
    static let contextTokens = 32768

    static var explicitPort: Int? {
        if let raw = ProcessInfo.processInfo.environment["OA_INKLING_PORT"], let p = Int(raw), p > 0, p < 65536 {
            return p
        }
        return nil
    }
    static var port: Int { shared.currentPort }
    static var baseURL: String { "http://\(host):\(port)" }
    static var logURL: URL { LocalModelStore.logsDir.appendingPathComponent("inkling-serve.log") }
    static var runDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/run", isDirectory: true)
    }
    static var pidURL: URL { runDir.appendingPathComponent("inkling-serve.pid") }
    static var portURL: URL { runDir.appendingPathComponent("inkling-serve.port") }

    private let stateLock = NSLock()
    private var _state: State = .idle
    private var _currentPort: Int = explicitPort ?? defaultPort
    private var process: Process?
    private var bootInFlight = false
    private var waiters: [(String?) -> Void] = []
    private let queue = DispatchQueue(label: "openagents.local-model-server")

    var state: State {
        stateLock.lock()
        defer { stateLock.unlock() }
        return _state
    }
    var currentPort: Int {
        stateLock.lock()
        defer { stateLock.unlock() }
        return _currentPort
    }
    var isReady: Bool {
        if case .ready = state { return true }
        return false
    }

    private func setState(_ next: State, port: Int? = nil) {
        var changed = false
        stateLock.lock()
        if let port, port > 0, port < 65536 { _currentPort = port }
        if _state != next {
            _state = next
            changed = true
        }
        stateLock.unlock()
        if changed {
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: Self.stateChangedNotification, object: self)
            }
        }
    }

    static func findPsionicServer() -> (bin: String, metal: String)? {
        let fm = FileManager.default
        let homeBinDir = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin").path
        let dirs = [Bundle.main.resourcePath ?? "", homeBinDir]
        for dir in dirs where !dir.isEmpty {
            let bin = dir + "/psionic-inkling-serve"
            let metal = dir + "/inkling-kernels.metal"
            if fm.isExecutableFile(atPath: bin) && fm.fileExists(atPath: metal) {
                return (bin, metal)
            }
        }
        return nil
    }

    private enum PortProbe: Equatable { case psionicHealthy, psionicOtherVariant, occupied(String), free }

    private static func readRecordedPort() -> Int? {
        guard let raw = try? String(contentsOf: portURL, encoding: .utf8) else { return nil }
        guard let p = Int(raw.trimmingCharacters(in: .whitespacesAndNewlines)), p > 0, p < 65536 else { return nil }
        return p
    }

    private static func readRecordedPID() -> pid_t? {
        guard let raw = try? String(contentsOf: pidURL, encoding: .utf8) else { return nil }
        guard let p = Int32(raw.trimmingCharacters(in: .whitespacesAndNewlines)), p > 1 else { return nil }
        return p
    }

    private static func isLocalPortFree(_ port: Int) -> Bool {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        if fd < 0 { return false }
        defer { close(fd) }
        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = in_port_t(UInt16(port).bigEndian)
        addr.sin_addr = in_addr(s_addr: inet_addr(host))
        let res = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        return res == 0
    }

    private static func listenerDescription(port: Int) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
        p.arguments = ["-nP", "-iTCP:\(port)", "-sTCP:LISTEN", "-Fp", "-Fc"]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        guard (try? p.run()) != nil else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let lines = String(decoding: data, as: UTF8.self).split(separator: "\n")
        var pid = "", cmd = ""
        for line in lines {
            if line.hasPrefix("p"), pid.isEmpty { pid = String(line.dropFirst()) }
            else if line.hasPrefix("c"), cmd.isEmpty { cmd = String(line.dropFirst()) }
        }
        if !pid.isEmpty {
            return cmd.isEmpty ? "PID \(pid)" : "\(cmd) (PID \(pid))"
        }
        return nil
    }

    private func probePortHealth(port: Int) -> PortProbe {
        guard let url = URL(string: "http://\(Self.host):\(port)/health") else { return .free }
        var req = URLRequest(url: url)
        req.timeoutInterval = 1.0
        let sem = DispatchSemaphore(value: 0)
        var result: PortProbe = .free
        let wantModel = LocalModelStore.activeHealthModelName
        let wantLora = LocalModelStore.activeHealthLoraName
        URLSession.shared.dataTask(with: req) { data, resp, _ in
            if let http = resp as? HTTPURLResponse {
                if http.statusCode == 200,
                   let data, let body = String(data: data, encoding: .utf8),
                   body.contains("psionic-metal") {
                    if let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                       let model = obj["model"] as? String {
                        let loraFile = (obj["lora_file"] as? String) ?? ""
                        if model == wantModel && loraFile == wantLora {
                            result = .psionicHealthy
                        } else {
                            result = .psionicOtherVariant
                        }
                    } else if wantLora.isEmpty && wantModel == LocalModelCatalogEntry.inkling2Mini.fileName {
                        // Legacy server without model/lora_file fields in /health matches default base only.
                        result = .psionicHealthy
                    } else {
                        result = .psionicOtherVariant
                    }
                } else {
                    let who = Self.listenerDescription(port: port) ?? "another HTTP server"
                    result = .occupied(who)
                }
            }
            sem.signal()
        }.resume()
        _ = sem.wait(timeout: .now() + 1.5)
        if case .free = result {
            if !Self.isLocalPortFree(port) {
                return .occupied(Self.listenerDescription(port: port) ?? "another process")
            }
            if let who = Self.listenerDescription(port: port) {
                return .occupied(who)
            }
        }
        return result
    }

    private func findExistingHealthyPort() -> Int? {
        var candidates: [Int] = []
        if let p = Self.explicitPort {
            candidates = [p]
        } else {
            let cur = currentPort
            candidates.append(cur)
            if let recorded = Self.readRecordedPort(), !candidates.contains(recorded) {
                candidates.append(recorded)
            }
            if !candidates.contains(Self.defaultPort) {
                candidates.append(Self.defaultPort)
            }
        }
        for p in candidates {
            switch probePortHealth(port: p) {
            case .psionicHealthy:
                return p
            case .psionicOtherVariant:
                // Free unified memory by cleanly terminating our previous detached server if recorded.
                if let recPort = Self.readRecordedPort(), recPort == p {
                    terminateOwnedAndRecordedSync()
                }
            case .occupied, .free:
                break
            }
        }
        return nil
    }

    private func pickCandidatePorts() -> (ports: [Int], explicitError: String?) {
        if let p = Self.explicitPort {
            switch probePortHealth(port: p) {
            case .psionicHealthy, .free:
                return ([p], nil)
            case .psionicOtherVariant:
                terminateOwnedAndRecordedSync()
                if Self.isLocalPortFree(p) { return ([p], nil) }
                return ([], "Configured port \(p) (OA_INKLING_PORT) is running a different local model variant.")
            case .occupied(let who):
                return ([], "Configured port \(p) (OA_INKLING_PORT) is already in use by \(who).")
            }
        }
        var freePorts: [Int] = []
        for offset in 0..<Self.portScanCount {
            let p = Self.defaultPort + offset
            if Self.isLocalPortFree(p) && Self.listenerDescription(port: p) == nil {
                freePorts.append(p)
                if freePorts.count >= 4 { break }
            }
        }
        if freePorts.isEmpty {
            return ([], "No free localhost port found between \(Self.defaultPort) and \(Self.defaultPort + Self.portScanCount - 1) for Inkling 2 Mini.")
        }
        return (freePorts, nil)
    }

    private static func readLogTail() -> String {
        guard let text = try? String(contentsOf: logURL, encoding: .utf8) else { return "" }
        let lines = text.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
        return lines.suffix(3).joined(separator: " | ")
    }

    /// Eagerly starts Inkling 2 Mini in the background once weights are verified so the composer
    /// unlocks automatically before the user submits a message.
    func prewarmIfReady() {
        guard LocalModelStore.isSelectable, Self.findPsionicServer() != nil else { return }
        queue.async {
            if self.bootInFlight { return }
            switch self.state {
            case .failed:
                return
            case .ready(let p):
                if self.probePortHealth(port: p) == .psionicHealthy {
                    return
                }
            case .idle, .starting:
                break
            }
            self.startBootLocked()
        }
    }

    /// Clears any previous server error and retries starting Inkling 2 Mini.
    func retry(completion: ((String?) -> Void)? = nil) {
        queue.async {
            if let completion { self.waiters.append(completion) }
            if self.bootInFlight { return }
            self.startBootLocked()
        }
    }

    func ensureRunning(onLoading: (() -> Void)? = nil, completion: @escaping (String?) -> Void) {
        queue.async {
            if case .ready(let p) = self.state {
                if case .psionicHealthy = self.probePortHealth(port: p) {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }
            }
            if let onLoading {
                DispatchQueue.main.async { onLoading() }
            }
            self.waiters.append(completion)
            if self.bootInFlight { return }
            self.startBootLocked()
        }
    }

    private func finishBootLocked(port: Int?, error: String?) {
        self.bootInFlight = false
        if let port, error == nil {
            self.setState(.ready(port), port: port)
        } else {
            self.setState(.failed(error ?? "Inkling 2 Mini could not start."))
        }
        let callbacks = self.waiters
        self.waiters = []
        if !callbacks.isEmpty {
            DispatchQueue.main.async {
                for cb in callbacks { cb(error) }
            }
        }
    }

    private func startBootLocked() {
        self.bootInFlight = true
        self.setState(.starting)

        if let healthyPort = self.findExistingHealthyPort() {
            self.finishBootLocked(port: healthyPort, error: nil)
            return
        }
        guard LocalModelStore.hardwareSupported else {
            self.finishBootLocked(port: nil, error: "\(LocalModelStore.entry.name) requires Apple Silicon and >= \(LocalModelStore.entry.minRamGB) GB unified memory (\(LocalModelStore.physicalMemoryGB) GB detected).")
            return
        }
        guard LocalModelStore.isInstalled else {
            self.finishBootLocked(port: nil, error: "\(LocalModelStore.entry.name) weights are not verified on disk (\(LocalModelStore.statusSummary())).")
            return
        }
        guard let server = Self.findPsionicServer() else {
            self.finishBootLocked(port: nil, error: "psionic-inkling-serve not found in app bundle or ~/.openagents/bin.")
            return
        }
        let picked = self.pickCandidatePorts()
        if let explicitError = picked.explicitError {
            self.finishBootLocked(port: nil, error: explicitError)
            return
        }
        let ctxTokens = LocalModelStore.recommendedContextTokens
        let modelPath = LocalModelStore.activeModelURL.path
        let loraPath = LocalModelStore.activeLoraURL?.path
        var lastError = "Could not start Psionic Metal server."
        for targetPort in picked.ports {
            self.terminateOwnedSync()
            try? FileManager.default.createDirectory(at: LocalModelStore.logsDir, withIntermediateDirectories: true)
            try? FileManager.default.createDirectory(at: Self.runDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            FileManager.default.createFile(atPath: Self.logURL.path, contents: nil)
            let logHandle = try? FileHandle(forWritingTo: Self.logURL)
            try? logHandle?.truncate(atOffset: 0)
            let p = Process()
            p.executableURL = URL(fileURLWithPath: server.bin)
            var args = [
                "--model", modelPath,
                "--metal", server.metal,
                "--host", Self.host,
                "--port", String(targetPort),
                "--ctx", String(ctxTokens),
                "--idle-exit-s", "300",
            ]
            if let loraPath, !loraPath.isEmpty {
                args.append(contentsOf: ["--lora", loraPath])
            }
            p.arguments = args
            p.standardOutput = logHandle ?? FileHandle.nullDevice
            p.standardError = logHandle ?? FileHandle.nullDevice
            p.terminationHandler = { [weak self] proc in
                guard let self else { return }
                self.queue.async {
                    guard self.process === proc else { return }
                    self.process = nil
                    try? FileManager.default.removeItem(at: Self.pidURL)
                    try? FileManager.default.removeItem(at: Self.portURL)
                    if !self.bootInFlight {
                        if proc.terminationStatus == 0 {
                            self.setState(.idle)
                        } else {
                            let tail = Self.readLogTail()
                            let msg = tail.isEmpty
                                ? "Inkling 2 Mini server stopped unexpectedly (exit \(proc.terminationStatus))."
                                : "Inkling 2 Mini server stopped: \(tail)"
                            self.setState(.failed(msg))
                        }
                    }
                }
            }
            do {
                try p.run()
                self.process = p
                try? "\(p.processIdentifier)\n".write(to: Self.pidURL, atomically: true, encoding: .utf8)
                try? "\(targetPort)\n".write(to: Self.portURL, atomically: true, encoding: .utf8)
            } catch {
                try? logHandle?.close()
                lastError = "Could not start Psionic Metal server: \(error.localizedDescription)"
                break
            }
            var bindCollision = false
            for _ in 0..<360 {
                if case .psionicHealthy = self.probePortHealth(port: targetPort) {
                    self.finishBootLocked(port: targetPort, error: nil)
                    return
                }
                if self.process?.isRunning != true {
                    let status = self.process?.terminationStatus ?? -1
                    let tail = Self.readLogTail()
                    let lower = tail.lowercased()
                    if lower.contains("address already in use") || lower.contains("bind") {
                        bindCollision = true
                        lastError = tail
                        break
                    }
                    lastError = tail.isEmpty
                        ? "Psionic Metal server exited (\(status)) before becoming ready (see \(Self.logURL.path))."
                        : "Psionic Metal server exited (\(status)): \(tail)"
                    break
                }
                Thread.sleep(forTimeInterval: 0.25)
            }
            if bindCollision { continue }
            if self.process?.isRunning == true {
                let tail = Self.readLogTail()
                let suffix = tail.isEmpty ? "See \(Self.logURL.path)." : tail
                lastError = "Timed out waiting for Psionic Metal server on 127.0.0.1:\(targetPort). \(suffix)"
                self.terminateOwnedSync()
            }
            break
        }
        self.finishBootLocked(port: nil, error: lastError)
    }

    func stop() {
        queue.async {
            self.terminateOwnedAndRecordedSync()
            if !self.bootInFlight {
                self.setState(.idle)
            }
        }
    }

    /// Stops any active/detached server for the old variant and prewarms the newly selected variant if ready.
    func restartForVariantChange() {
        queue.async {
            self.terminateOwnedAndRecordedSync()
            if !self.bootInFlight {
                self.setState(.idle)
            }
            if LocalModelStore.isSelectable && Self.findPsionicServer() != nil && !self.bootInFlight {
                self.startBootLocked()
            }
        }
    }

    /// Called on app quit: when daemon runs outlive quit, leave our detached server alive (it exits via --idle-exit-s 300);
    /// otherwise send clean SIGTERM so unified memory (~20 GB) is released immediately.
    func stopIfIdle(runsOutlive: Bool) {
        if runsOutlive { return }
        queue.sync {
            self.terminateOwnedSync()
        }
    }

    private func terminateOwnedAndRecordedSync() {
        let hadOwned = self.process != nil
        self.terminateOwnedSync()
        if !hadOwned, let pid = Self.readRecordedPID(), let port = Self.readRecordedPort() {
            if let who = Self.listenerDescription(port: port), who.contains("psionic") {
                _ = Darwin.kill(pid, SIGTERM)
                let deadline = Date().addingTimeInterval(2.0)
                while !Self.isLocalPortFree(port) && Date() < deadline {
                    Thread.sleep(forTimeInterval: 0.05)
                }
            }
            try? FileManager.default.removeItem(at: Self.pidURL)
            try? FileManager.default.removeItem(at: Self.portURL)
        }
    }

    private func terminateOwnedSync() {
        guard let p = self.process else { return }
        self.process = nil
        if p.isRunning {
            p.terminate()
            let deadline = Date().addingTimeInterval(2.0)
            while p.isRunning && Date() < deadline {
                Thread.sleep(forTimeInterval: 0.05)
            }
        }
        try? FileManager.default.removeItem(at: Self.pidURL)
        try? FileManager.default.removeItem(at: Self.portURL)
    }
}

extension CoderWindow {
    /// The window's dialogs and menus (made the first time one is asked for).
    var overlay: CoderOverlay {
        if let kept = coderOverlays[ObjectIdentifier(self)] { return kept }
        let made = CoderOverlay(owner: self); coderOverlays[ObjectIdentifier(self)] = made
        return made
    }
    /// Shows a dialog over the page. done gets the button pressed (-1 for Escape or a click outside)
    /// and the fields' text; with none, the keyboard goes back where it was as it closes.
    func present(_ dialog: CoderDialog, done: ((Int, [String]) -> Void)? = nil) {
        overlay.present(dialog, done: done ?? { [weak self] _, _ in self?.refocus() })
    }

    #if OA_FULL
    /// Asks for a new max budget (USD) for the shown chat thread.
    func editBudget() {
        let currentCost = Double(cd_cost_usd_micros(page)) / 1_000_000.0
        let currentBudget = Double(cd_budget_usd_micros(page)) / 1_000_000.0
        var d = CoderDialog(title: "Chat Max Budget", body: String(format: "Set the maximum spend (USD) for this chat thread. Current spend: $%.2f.", currentCost))
        d.fields = [.init(value: String(format: "%.2f", currentBudget), hint: "50.00")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1, let text = values.first else { return }
            let cleaned = text
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .replacingOccurrences(of: "$", with: "")
                .replacingOccurrences(of: ",", with: "")
            guard let dollars = Double(cleaned), dollars.isFinite, dollars > 0, dollars <= 1_000_000 else { return }
            let micros = Int64((dollars * 1_000_000.0).rounded())
            guard micros > 0 else { return }
            _ = cd_set_budget_usd_micros(self.page, micros)
            self.wake()
        }
    }
    #endif
    /// Opens the thread goal editor (/goal or /goal edit) to create, edit, pause/resume, or clear a thread goal.
    func editGoal() {
        let status = cd_goal_status(page)
        let elapsed = cd_goal_time(page)
        var buf = [CChar](repeating: 0, count: 4097)
        let n = cd_goal_objective(page, &buf, Int32(buf.count))
        let current = n > 0 ? String(cString: buf) : ""
        var d = CoderDialog(size: .md, title: status == 0 ? "Set Thread Goal" : "Edit Thread Goal")
        if status == 0 {
            d.body = "Describe a long-running objective. Coder will work autonomously across turns until it is complete, paused, or hits the chat budget."
        } else {
            let label: String
            switch status {
            case 1: label = "Active"
            case 2: label = "Paused"
            case 3: label = "Complete"
            case 4: label = "Blocked"
            case 5: label = "Budget limit"
            default: label = "None"
            }
            let timeStr = elapsed < 60 ? "\(elapsed)s" : "\(elapsed / 60)m"
            d.body = "Status: \(label) · Elapsed: \(timeStr). Update the objective below or change the goal state."
        }
        // a box of five lines: Return makes a new line, Command-Return saves
        d.fields = [.init(value: current, lines: 5)]
        // the foot: Clear and Pause or Resume at its start, Cancel and the way on at its end
        var toggle = -1, clear = -1
        if status != 0 {
            d.buttons = [.init(title: "Clear", aside: true), .init(title: status == 1 ? "Pause" : "Resume", aside: true)]
            clear = 0; toggle = 1
        }
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: status == 0 ? "Start Goal" : "Save Goal", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            if answer == save {
                let text = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !text.isEmpty { _ = cd_goal_set(self.page, text) }
            } else if answer == toggle {
                _ = cd_goal_set_status(self.page, status == 1 ? 2 : 1)
            } else if answer == clear {
                _ = cd_goal_clear(self.page)
            }
            self.wake()
        }
    }
    #if OA_FULL
    /// Settings > Models > Gemini API key: sets, replaces or removes the user's Gemini API key.
    func chooseGeminiKey() {
        var about = "Use your own Gemini API key (from Google AI Studio) to run Microcoder on Gemini models. Coder keeps it in a file only you can read."
        if let kept = CoderGemini.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + "." }
        else { about += "\n\nNo key is set now." }
        var d = CoderDialog(size: .md, title: "Gemini API Key", body: about)
        d.fields = [.init(hint: "AIza…", secure: true)]
        let hadKey = CoderGemini.saved != nil
        if hadKey { d.buttons.append(.init(title: "Remove Key", kind: .danger, aside: true)) }
        let remove = hadKey ? 0 : -9
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            let key: String
            switch answer {
            case save:
                key = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if key.isEmpty { self.refocus(); return }
            case remove: key = ""
            default: self.refocus(); return
            }
            if !CoderGemini.save(key) {
                self.present(.notice("The key could not be saved.", "Coder could not write " + CoderGemini.file.path + "."))
                return
            }
            self.syncCloudAuth()
            self.refocus()
        }
    }
    #endif
    /// Onboarding / Settings direct actions (CP_ASK_LOCAL_DOWNLOAD=160, CP_ASK_LOCAL_CANCEL=161).
    func startInklingDownload() {
        if LocalModelStore.isSelectable {
            LocalModelServer.shared.retry { [weak self] _ in
                self?.syncLocalModels()
            }
            syncLocalModels()
            return
        }
        if LocalModelStore.entry.isCustom || (LocalModelStore.entry.requiresHFToken && LocalModelStore.effectiveHFToken == nil) {
            manageLocalModels()
            return
        }
        LocalModelStore.startDownload { [weak self] in
            self?.syncLocalModels()
        }
        syncLocalModels()
    }
    func cancelInklingDownload() {
        LocalModelStore.cancelDownload { [weak self] in
            self?.syncLocalModels()
        }
        syncLocalModels()
    }

    /// True when the foreground chat or any background chat is actively running a reply on Inkling 2 Mini or Hybrid Executor.
    private func isInklingReplyRunning() -> Bool {
        if cd_running(page) != 0 && (cd_model_inkling(page) != 0 || cd_model_hybrid(page) != 0) { return true }
        for r in runs.values where r.page != page {
            if cd_running(r.page) != 0 && (cd_model_inkling(r.page) != 0 || cd_model_hybrid(r.page) != 0) { return true }
        }
        return false
    }

    private func revealLocalModelInFinder() {
        let fm = FileManager.default
        if fm.fileExists(atPath: LocalModelStore.weightsURL.path) {
            NSWorkspace.shared.activateFileViewerSelecting([LocalModelStore.weightsURL])
        } else if fm.fileExists(atPath: LocalModelStore.partialURL.path) {
            NSWorkspace.shared.activateFileViewerSelecting([LocalModelStore.partialURL])
        } else {
            try? fm.createDirectory(at: LocalModelStore.rootDir, withIntermediateDirectories: true)
            NSWorkspace.shared.activateFileViewerSelecting([LocalModelStore.rootDir])
        }
    }

    private func openLocalModelHFRepo() {
        let raw = LocalModelStore.entry.hfRepoURL ?? "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF"
        if let url = URL(string: raw) {
            NSWorkspace.shared.open(url)
        }
    }

    /// Secure dialog to store/remove a Hugging Face read token (~/.openagents/hf-token, 0600) for gated checkpoints.
    func chooseHFToken() {
        let repo = LocalModelStore.entry.hfRepoURL ?? "https://huggingface.co/orcarouter/Inkling-2-Mini-abliterated-GGUF"
        var about = "Some checkpoints (such as OrcaRouter Abliterated) are gated on Hugging Face. First click Open HF Repo to accept the repo terms, then paste a Hugging Face read token (hf_…). OpenAgents stores it locally (0600) and sends it only via curl stdin."
        if let tok = LocalModelStore.effectiveHFToken, let src = LocalModelStore.hfTokenSource {
            about += "\n\nCurrent token: \(LocalModelStore.hfTokenTail(tok)) (\(src))."
        } else {
            about += "\n\nNo Hugging Face token configured yet."
        }
        var d = CoderDialog(size: .md, title: "Hugging Face Token", body: about)
        d.fields = [.init(hint: "hf_…", secure: true)]
        let hadSaved = LocalModelStore.savedHFToken != nil
        var removeIdx = -9
        if hadSaved {
            removeIdx = d.buttons.count
            d.buttons.append(.init(title: "Remove", kind: .danger, aside: true))
        }
        let openRepoIdx = d.buttons.count
        d.buttons.append(.init(title: "Open HF Repo", aside: !hadSaved))
        let cancelIdx = d.buttons.count
        d.buttons.append(.init(title: "Cancel"))
        let saveIdx = d.buttons.count
        d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = saveIdx
        d.cancel = cancelIdx
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == openRepoIdx {
                self.openLocalModelHFRepo()
                self.chooseHFToken()
                return
            }
            if answer == removeIdx {
                _ = LocalModelStore.saveHFToken("")
                LocalModelStore.lastError = nil
                self.syncLocalModels()
                self.manageLocalModels()
                return
            }
            if answer == saveIdx {
                let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if raw.isEmpty {
                    self.manageLocalModels()
                    return
                }
                if !LocalModelStore.saveHFToken(raw) {
                    self.present(.notice("Invalid Hugging Face token", "Token must contain printable non-whitespace ASCII characters without quotes.")) { [weak self] _, _ in
                        self?.chooseHFToken()
                    }
                    return
                }
                LocalModelStore.lastError = nil
                self.syncLocalModels()
                self.manageLocalModels()
                return
            }
            self.manageLocalModels()
        }
    }

    /// Native Finder picker to import any local Inkling 2 Mini `.gguf` (useful for gated HF downloads or BYO quants).
    func importLocalGGUFDialog() {
        guard !LocalModelStore.isDownloading, !LocalModelStore.isVerifying else {
            manageLocalModels()
            return
        }
        if isInklingReplyRunning() {
            present(.notice("Inkling 2 Mini is in use", "Stop the active reply before importing a local checkpoint.")) { [weak self] _, _ in
                self?.manageLocalModels()
            }
            return
        }
        guard !choosingFolder, window.attachedSheet == nil else {
            manageLocalModels()
            return
        }
        choosingFolder = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            guard self.window.attachedSheet == nil else { self.choosingFolder = false; self.manageLocalModels(); return }
            guard self.canPresentOpenPanel() else { self.importLocalGGUFFallback(); return }
            let panel = NSOpenPanel()
            panel.canChooseFiles = true
            panel.canChooseDirectories = false
            panel.allowsMultipleSelection = false
            if let ggufType = UTType(filenameExtension: "gguf") {
                panel.allowedContentTypes = [ggufType]
            }
            panel.directoryURL = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
                ?? FileManager.default.homeDirectoryForCurrentUser
            panel.prompt = "Import"
            panel.message = "Choose a local Inkling 2 Mini .gguf file to import into \(LocalModelStore.entry.name)."
            panel.beginSheetModal(for: self.window) { [weak self] answer in
                guard let self else { return }
                self.choosingFolder = false
                if answer == .OK, let url = panel.url {
                    LocalModelServer.shared.stop()
                    LocalModelStore.importLocalGGUF(from: url) { [weak self] in
                        self?.syncLocalModels()
                    }
                }
                self.syncLocalModels()
                self.manageLocalModels()
            }
        }
    }

    private func importLocalGGUFFallback() {
        var d = CoderDialog(
            size: .md,
            title: "Import Local .gguf",
            body: "Enter the full path to a local Inkling 2 Mini .gguf file to import into \(LocalModelStore.entry.name)."
        )
        d.fields = [.init(value: "", hint: "~/Downloads/Inkling-2-Mini.gguf")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Import", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            self.choosingFolder = false
            if answer == 1 {
                let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !raw.isEmpty {
                    let expanded = (raw as NSString).expandingTildeInPath
                    let url = URL(fileURLWithPath: expanded)
                    LocalModelServer.shared.stop()
                    LocalModelStore.importLocalGGUF(from: url) { [weak self] in
                        self?.syncLocalModels()
                    }
                }
            }
            self.syncLocalModels()
            self.manageLocalModels()
        }
    }

    /// Native popup menu listing all Inkling 2 Mini variants (Official, AtomicChat Uncensored LoRA/quants,
    /// OrcaRouter Abliterated, Custom BYO) plus quick actions (Import, HF Token, Repo, Finder, Verify, Delete).
    private func presentLocalModelVariantMenu() {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let activeId = LocalModelStore.activeVariantId

        func addHeader(_ title: String) {
            let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }
        func addVariant(_ v: LocalModelCatalogEntry) {
            let item = NSMenuItem(
                title: LocalModelStore.variantMenuLabel(for: v),
                action: #selector(CoderWindow.localVariantPicked(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = v.id
            item.state = (v.id == activeId) ? .on : .off
            item.isEnabled = true
            menu.addItem(item)
        }

        addHeader("Official")
        addVariant(.inkling2Mini)
        menu.addItem(.separator())
        addHeader("Uncensored — AtomicChat")
        addVariant(.inkling2MiniUncensoredLora)
        addVariant(.inkling2MiniUncensoredQ4K)
        addVariant(.inkling2MiniUncensoredIQ3S)
        addVariant(.inkling2MiniUncensoredIQ2M)
        menu.addItem(.separator())
        addHeader("Abliterated — OrcaRouter (HF Gated)")
        addVariant(.inkling2MiniAbliteratedMix16G)
        addVariant(.inkling2MiniAbliteratedMix12G)
        menu.addItem(.separator())
        addHeader("Bring Your Own")
        addVariant(.inkling2MiniCustom)
        menu.addItem(.separator())

        func addTool(_ title: String, tag: Int, enabled: Bool = true) {
            let item = NSMenuItem(title: title, action: #selector(CoderWindow.localModelToolPicked(_:)), keyEquivalent: "")
            item.target = self
            item.tag = tag
            item.isEnabled = enabled
            menu.addItem(item)
        }
        addTool("Import Local .gguf…", tag: 1)
        addTool("Set Hugging Face Token…", tag: 2)
        if LocalModelStore.entry.hfRepoURL != nil {
            addTool("Open Hugging Face Repo Page", tag: 3)
        }
        addTool("Reveal in Finder", tag: 4)
        if LocalModelStore.weightsBytes > 0 {
            addTool("Verify SHA-256", tag: 5)
        }
        if LocalModelStore.diskUsageBytes > 0 {
            addTool("Delete Cached Variant…", tag: 6)
        }

        let at = canvas.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        let picked = menu.popUp(positioning: nil, at: at, in: canvas)
        if !picked {
            manageLocalModels()
        }
    }

    @objc func localVariantPicked(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else {
            manageLocalModels()
            return
        }
        if id == LocalModelStore.activeVariantId {
            manageLocalModels()
            return
        }
        if isInklingReplyRunning() {
            present(.notice(
                "Inkling 2 Mini is in use",
                "Stop the active Inkling 2 Mini reply before switching local model variants."
            )) { [weak self] _, _ in
                self?.manageLocalModels()
            }
            return
        }
        if LocalModelStore.setActiveVariant(id) {
            LocalModelServer.shared.restartForVariantChange()
        }
        syncLocalModels()
        manageLocalModels()
    }

    @objc func localModelToolPicked(_ sender: NSMenuItem) {
        switch sender.tag {
        case 1: importLocalGGUFDialog()
        case 2: chooseHFToken()
        case 3:
            openLocalModelHFRepo()
            manageLocalModels()
        case 4:
            revealLocalModelInFinder()
            manageLocalModels()
        case 5:
            LocalModelStore.verifyInstalledNow { [weak self] in self?.syncLocalModels() }
            syncLocalModels()
            manageLocalModels()
        case 6:
            confirmDeleteLocalModel()
        default:
            manageLocalModels()
        }
    }

    private func confirmDeleteLocalModel() {
        if isInklingReplyRunning() {
            present(.notice(
                "Inkling 2 Mini is in use",
                "Stop the active Inkling 2 Mini reply before deleting its cached weights."
            )) { [weak self] _, _ in
                self?.manageLocalModels()
            }
            return
        }
        let reclaim = LocalModelStore.formatBytes(LocalModelStore.diskUsageBytes)
        var d = CoderDialog(
            size: .sm,
            title: "Delete \(LocalModelStore.entry.shortLabel)?",
            body: "This removes \(reclaim) from \(LocalModelStore.inklingDir.path) and stops the local Psionic Metal server. Other downloaded variants are kept."
        )
        d.buttons = [
            .init(title: "Delete", kind: .danger, aside: true),
            .init(title: "Cancel", kind: .primary),
        ]
        d.confirm = 1
        d.cancel = 1
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 0 {
                if self.isInklingReplyRunning() {
                    self.present(.notice("Inkling 2 Mini is in use", "Stop the active reply before deleting its cached weights."))
                    return
                }
                LocalModelStore.removeInkling { LocalModelServer.shared.stop() }
                self.syncLocalModels()
            }
            self.manageLocalModels()
        }
    }

    /// Settings > Models > Local Models & composer model picker (#13826, #14028):
    /// multi-variant catalog (Official IQ4_NL, AtomicChat Uncensored LoRA & AD quants, OrcaRouter Abliterated,
    /// Custom .gguf import), RAM & disk compatibility, resumable download with Pause/Resume/Cancel,
    /// HF token config, live progress/speed/ETA, streaming SHA-256 verification, Reveal in Finder, and Delete.
    /// Keeps <= 4 buttons per dialog (APP_DLG_BUTTONS == 4).
    func manageLocalModels() {
        let st = LocalModelStore.state
        let cur = LocalModelStore.entry
        localModelsDialogGen += 1
        let gen = localModelsDialogGen
        localModelsDialogState = st
        let titleSuffix = cur.id == LocalModelCatalogEntry.inkling2Mini.id ? "Inkling 2 Mini" : cur.shortLabel
        var d = CoderDialog(size: .lg, title: "Local Models — \(titleSuffix)", body: LocalModelStore.dialogBody())
        var switchIdx = -9
        var deleteIdx = -9
        var revealIdx = -9
        var verifyIdx = -9
        var downloadIdx = -9
        var downloadBaseIdx = -9
        var importIdx = -9
        var tokenIdx = -9
        var pauseIdx = -9
        var cancelDlIdx = -9
        var closeIdx = -9

        switch st {
        case .installed:
            deleteIdx = d.buttons.count; d.buttons.append(.init(title: "Delete…", kind: .danger, aside: true))
            switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…"))
            verifyIdx = d.buttons.count; d.buttons.append(.init(title: "Verify SHA-256"))
            closeIdx = d.buttons.count; d.buttons.append(.init(title: "Done", kind: .primary))
            d.confirm = closeIdx; d.cancel = closeIdx
        case .downloading:
            cancelDlIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel Download", kind: .danger, aside: true))
            pauseIdx = d.buttons.count; d.buttons.append(.init(title: "Pause"))
            closeIdx = d.buttons.count; d.buttons.append(.init(title: "Close", kind: .primary))
            d.confirm = closeIdx; d.cancel = closeIdx
        case .verifying:
            revealIdx = d.buttons.count; d.buttons.append(.init(title: "Reveal in Finder", aside: true))
            closeIdx = d.buttons.count; d.buttons.append(.init(title: "Close", kind: .primary))
            d.confirm = closeIdx; d.cancel = closeIdx
        case .paused:
            cancelDlIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel Download", kind: .danger, aside: true))
            switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…"))
            closeIdx = d.buttons.count; d.buttons.append(.init(title: "Close"))
            downloadIdx = d.buttons.count; d.buttons.append(.init(title: "Resume", kind: .primary))
            d.confirm = downloadIdx; d.cancel = closeIdx
        case .notInstalled:
            if cur.isCustom {
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…", aside: true))
                closeIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
                importIdx = d.buttons.count; d.buttons.append(.init(title: "Import Local .gguf…", kind: .primary))
                d.confirm = importIdx; d.cancel = closeIdx
            } else if cur.isLoraOverlay && LocalModelStore.isFileInstalled && LocalModelStore.needsBaseModel {
                deleteIdx = d.buttons.count; d.buttons.append(.init(title: "Delete…", kind: .danger, aside: true))
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…"))
                closeIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
                downloadBaseIdx = d.buttons.count; d.buttons.append(.init(title: "Download Base (17.6 GB)", kind: .primary))
                d.confirm = downloadBaseIdx; d.cancel = closeIdx
            } else if LocalModelStore.hasUnverifiedWeights {
                deleteIdx = d.buttons.count; d.buttons.append(.init(title: "Delete…", kind: .danger, aside: true))
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…"))
                closeIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
                verifyIdx = d.buttons.count; d.buttons.append(.init(title: "Verify SHA-256", kind: .primary))
                d.confirm = verifyIdx; d.cancel = closeIdx
            } else if cur.requiresHFToken && LocalModelStore.effectiveHFToken == nil {
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…", aside: true))
                importIdx = d.buttons.count; d.buttons.append(.init(title: "Import .gguf…"))
                closeIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
                tokenIdx = d.buttons.count; d.buttons.append(.init(title: "HF Token…", kind: .primary))
                d.confirm = tokenIdx; d.cancel = closeIdx
            } else if LocalModelStore.hasEnoughDiskForDownload {
                let szLabel = LocalModelStore.formatBytes(cur.expectedBytes)
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…", aside: true))
                if cur.requiresHFToken {
                    importIdx = d.buttons.count; d.buttons.append(.init(title: "Import .gguf…"))
                    tokenIdx = d.buttons.count; d.buttons.append(.init(title: "HF Token…"))
                } else {
                    importIdx = d.buttons.count; d.buttons.append(.init(title: "Import .gguf…"))
                    closeIdx = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
                }
                downloadIdx = d.buttons.count; d.buttons.append(.init(title: "Download (\(szLabel))", kind: .primary))
                d.confirm = downloadIdx; d.cancel = closeIdx >= 0 ? closeIdx : -1
            } else {
                switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…", aside: true))
                revealIdx = d.buttons.count; d.buttons.append(.init(title: "Open Folder"))
                closeIdx = d.buttons.count; d.buttons.append(.init(title: "Close", kind: .primary))
                d.confirm = closeIdx; d.cancel = closeIdx
            }
        case .unsupportedHardware:
            switchIdx = d.buttons.count; d.buttons.append(.init(title: "Switch Variant…", aside: true))
            closeIdx = d.buttons.count; d.buttons.append(.init(title: "Close", kind: .primary))
            d.confirm = closeIdx; d.cancel = closeIdx
        }

        present(d) { [weak self] answer, _ in
            guard let self, self.localModelsDialogGen == gen else { return }
            self.localModelsDialogState = nil
            if answer == switchIdx {
                self.presentLocalModelVariantMenu()
            } else if answer == downloadIdx {
                LocalModelStore.startDownload { [weak self] in self?.syncLocalModels() }
                self.syncLocalModels()
                self.manageLocalModels()
            } else if answer == downloadBaseIdx {
                if LocalModelStore.setActiveVariant(LocalModelCatalogEntry.inkling2Mini.id) {
                    LocalModelStore.startDownload { [weak self] in self?.syncLocalModels() }
                }
                self.syncLocalModels()
                self.manageLocalModels()
            } else if answer == importIdx {
                self.importLocalGGUFDialog()
            } else if answer == tokenIdx {
                self.chooseHFToken()
            } else if answer == pauseIdx {
                LocalModelStore.pauseDownload { [weak self] in self?.syncLocalModels() }
                self.syncLocalModels()
                self.manageLocalModels()
            } else if answer == cancelDlIdx {
                LocalModelStore.cancelDownload { [weak self] in self?.syncLocalModels() }
                self.syncLocalModels()
                self.manageLocalModels()
            } else if answer == verifyIdx {
                LocalModelStore.verifyInstalledNow { [weak self] in self?.syncLocalModels() }
                self.syncLocalModels()
                self.manageLocalModels()
            } else if answer == revealIdx {
                self.revealLocalModelInFinder()
                self.manageLocalModels()
            } else if answer == deleteIdx {
                self.confirmDeleteLocalModel()
            } else {
                self.syncLocalModels()
                self.refocus()
            }
        }
    }
    #if OA_FULL
    /// Settings > Models > Executor Models & Weight Modules (CP_ASK_EXECUTOR_MODELS; #13924):
    /// displays live SHA-256 verification status, verification-class badges ([E]/[D]/[S]/[N]),
    /// and per-module lineage from oa_dc_exec_scan, with Verify Now and Install Built-ins actions.
    func manageExecutorModels() {
        let snap = ExecutorModelStore.scan()
        var body = snap.details
        if snap.tamperedModules > 0 {
            body += "\nWARNING: SHA-256 verification failed. Hybrid Executor is atomically refused until all weight artifacts pass verification."
        }
        if let err = ExecutorModelStore.lastError {
            body += "\n\nLast error: " + err
        }
        var d = CoderDialog(size: .lg, title: "Executor Models & Weight Modules", body: body)
        let installIdx = d.buttons.count
        d.buttons.append(.init(title: ExecutorModelStore.isInstalling ? "Installing…" : "Install Built-ins", aside: true))
        let verifyIdx = d.buttons.count
        d.buttons.append(.init(title: "Verify Now"))
        let doneIdx = d.buttons.count
        d.buttons.append(.init(title: "Done", kind: .primary))
        d.confirm = doneIdx; d.cancel = doneIdx
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == installIdx && !ExecutorModelStore.isInstalling {
                ExecutorModelStore.installBuiltins { [weak self] _ in
                    self?.syncLocalModels()
                    self?.manageExecutorModels()
                }
            } else if answer == verifyIdx {
                self.syncLocalModels()
                self.manageExecutorModels()
            } else {
                self.syncLocalModels()
                self.refocus()
            }
        }
    }
    /// Settings > Models > Pylon & Executor Network (CP_ASK_PYLON_EXECUTOR; #13928):
    /// displays live on-disk Pylon state, verified traces/windows, reconciled Spark sats
    /// (real_bitcoin_moved=true with explicit network badge), and signed Ch 224 shadow
    /// rollout state with atomic Promote and Rollback controls.
    func managePylonExecutorNetwork() {
        let snap = ExecutorReleaseUpdater.snapshot()
        var body = snap.details
        if let msg = ExecutorReleaseUpdater.lastMessage {
            body += "\nLast action: " + msg
        }
        var d = CoderDialog(size: .lg, title: "Pylon & Executor Network", body: body)
        var promoteIdx = -9
        var rollbackIdx = -9
        if snap.canPromote {
            promoteIdx = d.buttons.count
            d.buttons.append(.init(title: "Promote Staged", aside: true))
        }
        if snap.canRollback {
            rollbackIdx = d.buttons.count
            d.buttons.append(.init(title: "Rollback", kind: .danger, aside: true))
        }
        let refreshIdx = d.buttons.count
        d.buttons.append(.init(title: "Refresh"))
        let doneIdx = d.buttons.count
        d.buttons.append(.init(title: "Done", kind: .primary))
        d.confirm = doneIdx; d.cancel = doneIdx
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == promoteIdx {
                _ = ExecutorReleaseUpdater.promoteStaged()
                self.syncLocalModels()
                self.managePylonExecutorNetwork()
            } else if answer == rollbackIdx {
                _ = ExecutorReleaseUpdater.rollbackActive()
                self.syncLocalModels()
                self.managePylonExecutorNetwork()
            } else if answer == refreshIdx {
                self.syncLocalModels()
                self.managePylonExecutorNetwork()
            } else {
                self.syncLocalModels()
                self.refocus()
            }
        }
    }
    /// Coder > Claude Key: sets, replaces or removes the key Claude sub-agents use. It applies to the
    /// next Claude sub-agent; one already running keeps what it started with.
    func chooseKey() {
        var about = "Claude sub-agents use this key instead of the Claude Code login on this Mac. Paste an Anthropic API key (sk-ant-api…) or a token from claude setup-token (sk-ant-oat…). Coder keeps it in a file only you can read."
        if let launch = CoderKey.fromLaunch { about += "\n\nThis launch uses the key in CODER_CLAUDE_KEY (" + CoderKey.tail(launch) + "). It wins over a key set here until you open Coder without it." }
        else if let kept = CoderKey.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + "." }
        else { about += "\n\nNo key is set now: Claude sub-agents use the Claude Code login." }
        var d = CoderDialog(size: .md, title: "Claude Key", body: about)
        d.fields = [.init(hint: "sk-ant-…", secure: true)]
        let hadKey = CoderKey.saved != nil
        if hadKey { d.buttons.append(.init(title: "Remove Key", kind: .danger, aside: true)) }
        let remove = hadKey ? 0 : -9
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            let key: String
            switch answer {
            case save:
                key = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if key.isEmpty { self.refocus(); return }
            case remove: key = ""
            default: self.refocus(); return
            }
            if !CoderKey.save(key) {
                self.present(.notice("The key could not be saved.", "Coder could not write " + CoderKey.file.path + "."))
                return
            }
            self.refocus()
        }
    }
    /// Coder > OpenRouter Models: the key, the model for Microcoder (a composer picker entry) and the
    /// model for sub-agents (level openrouter). An empty model removes it. Each model is checked against
    /// OpenRouter's list first: it must take a strict JSON schema.
    func chooseOpenRouter() {
        var about = "Run Microcoder or its sub-agents on any OpenRouter model that takes a strict JSON schema. Use OpenRouter ids such as deepseek/deepseek-v4.1-flash-1017. The main model joins the composer's model picker. The sub-agent model is one more level Microcoder can hand tasks to: ask for OpenRouter by name."
        if let kept = CoderOpenRouter.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + ". Leave the key empty to keep it." }
        else if CoderOpenRouter.hasKey { about += "\n\nNo key is set here: Coder uses ~/work/.secrets/openrouter.env." }
        else { about += "\n\nNo key is set. Paste one from openrouter.ai/settings/keys." }
        let hadKey = CoderOpenRouter.saved != nil
        var d = CoderDialog(size: .md, title: "OpenRouter Models", body: about)
        d.fields = [
            .init(label: "Key", hint: hadKey ? "Leave empty to keep the key set now" : "sk-or-…", secure: true),
            .init(label: "Model for Microcoder", value: CoderOpenRouter.main, hint: "deepseek/deepseek-v4.1-flash-1017 (empty: none)"),
            .init(label: "Model for sub-agents", value: CoderOpenRouter.sub, hint: "deepseek/deepseek-v4.1-flash-1017 (empty: none)"),
        ]
        d.focus = hadKey ? 1 : 0
        if hadKey { d.buttons.append(.init(title: "Remove Key", kind: .danger, aside: true)) }
        let remove = hadKey ? 0 : -9
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == remove { _ = CoderOpenRouter.save(""); self.refocus(); return }
            guard answer == save, values.count == 3 else { self.refocus(); return }
            let k = values[0].trimmingCharacters(in: .whitespacesAndNewlines)
            let m = values[1].trimmingCharacters(in: .whitespacesAndNewlines), s = values[2].trimmingCharacters(in: .whitespacesAndNewlines)
            if !k.isEmpty && !CoderOpenRouter.save(k) {
                self.present(.notice("The key could not be saved.", "Coder could not write " + CoderOpenRouter.file.path + "."))
                return
            }
            CoderOpenRouter.unfit([m, s]) { bad in
                if let bad, !bad.isEmpty {
                    self.present(.notice("OpenRouter has no model " + bad.joined(separator: " or ") + " that takes a strict JSON schema.",
                                         "Microcoder sends one on every step. Check the id on openrouter.ai/models. The models were not changed."))
                    return
                }
                CoderOpenRouter.setModels(main: m, sub: s)
                self.applyOpenRouter()
                self.refocus()
            }
        }
    }
    /// The OpenRouter models to the window's page and every page running out of view.
    func applyOpenRouter() {
        coderApplyOpenRouter(page)
        for (_, r) in runs { coderApplyOpenRouter(r.page) }
        wake()
    }
    func chooseAntigravity() {
        let detected: String
        if let path = CoderAntigravity.resolvedBinaryPath() {
            if let v = CoderAntigravity.detectedVersion() {
                let note = CoderAntigravity.isVersionSupported(v) ? "ready" : "needs >= 1.5.0"
                detected = "Detected: \(path) (v\(v), \(note))"
            } else {
                detected = "Detected: \(path)"
            }
        } else {
            detected = "Antigravity CLI (agy) not found. Install Antigravity >= 1.5.0 or set a custom binary path below."
        }
        let d = CoderDialog(size: .md, title: "Antigravity CLI & Model",
                            body: detected + "\nLeave fields empty to use CLI defaults.",
                            fields: [
                                .init(label: "Binary path (optional)", value: CoderAntigravity.bin, hint: "Auto-detect agy from PATH or ~/.antigravity"),
                                .init(label: "Default model", value: CoderAntigravity.model, hint: "e.g. gemini-4-hydrogen"),
                                .init(label: "Reasoning effort", value: CoderAntigravity.effort, hint: "low, medium, or high")
                            ],
                            buttons: [.init(title: "Cancel"), .init(title: "Save", kind: .primary)],
                            confirm: 1, cancel: 0)
        present(d) { [weak self] btn, vals in
            guard let self, btn == 1, vals.count >= 3 else { return }
            let bin = vals[0].trimmingCharacters(in: .whitespacesAndNewlines)
            let model = vals[1].trimmingCharacters(in: .whitespacesAndNewlines)
            let effort = vals[2].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if !effort.isEmpty && effort != "low" && effort != "medium" && effort != "high" {
                self.present(.notice("Invalid reasoning effort.", "Use low, medium, high, or leave empty for the CLI default."))
                return
            }
            CoderAntigravity.bin = bin
            CoderAntigravity.model = model
            CoderAntigravity.effort = effort
            self.applyAntigravity()
            self.refocus()
        }
    }
    func chooseAntigravityAuth() {
        let envStatus = CoderAntigravity.hasEnvADC()
            ? "antigravity.env (ADC): configured at \(CoderAntigravity.envFileURL.path)"
            : "antigravity.env (ADC): not configured (\(CoderAntigravity.envFileURL.path))"
        let body = "Active profile: \(CoderAntigravity.effectiveProfilePath)\n\(envStatus)"
        let d = CoderDialog(size: .md, title: "Antigravity Sign-In & Profile",
                            body: body,
                            fields: [
                                .init(label: "Isolated profile directory (optional)", value: CoderAntigravity.profile, hint: CoderAntigravity.defaultProfileURL.path)
                            ],
                            buttons: [
                                .init(title: "Sign in…", aside: true),
                                .init(title: "Edit env", aside: true),
                                .init(title: "Cancel"),
                                .init(title: "Save", kind: .primary)
                            ],
                            confirm: 3, cancel: 2)
        present(d) { [weak self] btn, vals in
            guard let self else { return }
            let profileVal = vals.first?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            if btn == 0 {
                CoderAntigravity.profile = profileVal
                self.applyAntigravity()
                if !CoderAntigravity.launchSignIn() {
                    self.present(.notice("Antigravity CLI not found.", "Install agy (>= 1.5.0) or configure its binary path first."))
                }
                return
            }
            if btn == 1 {
                let url = CoderAntigravity.envFileURL
                let fm = FileManager.default
                try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                if !fm.fileExists(atPath: url.path) {
                    let template = "# Antigravity Google Cloud / ADC environment variables (KEY=VALUE)\n# GOOGLE_APPLICATION_CREDENTIALS=/path/to/adc.json\n"
                    try? template.write(to: url, atomically: true, encoding: .utf8)
                    try? fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
                }
                NSWorkspace.shared.open(url)
                self.syncAntigravitySettings()
                return
            }
            guard btn == 3 else { return }
            CoderAntigravity.profile = profileVal
            self.applyAntigravity()
            self.refocus()
        }
    }
    #endif
    /// Interactive Secret / Credential request card (page/permit.coax; GitHub #13873):
    /// prompts in a masked secure dialog, saves to a 0600 file under ~/Library/Application Support/Coder/secrets/
    /// (and known provider key stores when applicable) without ever logging or echoing the secret into the transcript.
    func enterRequestedSecret() {
        var nameBuf = [CChar](repeating: 0, count: 257)
        var titleBuf = [CChar](repeating: 0, count: 257)
        var bodyBuf = [CChar](repeating: 0, count: 1025)
        let nn = cd_permit_secret_name(page, &nameBuf, Int32(nameBuf.count))
        let tn = cd_permit_title(page, &titleBuf, Int32(titleBuf.count))
        let bn = cd_permit_body(page, &bodyBuf, Int32(bodyBuf.count))
        let secretName = nn > 0 ? String(cString: nameBuf) : "SECRET"
        let title = tn > 0 ? String(cString: titleBuf) : "Enter Secret"
        let reason = bn > 0 ? String(cString: bodyBuf) : "The agent requested a secret credential."
        let body = reason + "\n\nStored in a 0600 file under ~/Library/Application Support/Coder/secrets/ and never logged or echoed into the chat transcript."
        var d = CoderDialog(size: .md, title: title, body: body)
        d.fields = [.init(label: secretName, hint: "Paste secret value…", secure: true)]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save & Continue", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1 else { return }
            let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !raw.isEmpty else { return }
            let saved = Self.saveRequestedSecret(name: secretName, value: raw)
            guard saved else {
                self.present(.notice("Secret could not be saved.", "Coder could not write the 0600 secret file for \(secretName)."))
                return
            }
            #if OA_FULL
            if secretName.uppercased() == "GEMINI_API_KEY" { self.syncCloudAuth() }
            #endif
            _ = cd_permit_secret_done(self.page, 1)
            self.pump(self.run)
            self.wake()
        }
    }
    private static func saveRequestedSecret(name: String, value: String) -> Bool {
        #if OA_FULL
        let upper = name.uppercased()
        if upper == "GEMINI_API_KEY" { _ = CoderGemini.save(value) }
        else if upper == "ANTHROPIC_API_KEY" || upper == "CODER_CLAUDE_KEY" { _ = CoderKey.save(value) }
        else if upper == "OPENROUTER_API_KEY" { _ = CoderOpenRouter.save(value) }
        #endif
        let safe = name.lowercased().map { ch -> Character in
            (ch.isASCII && (ch.isLetter || ch.isNumber || ch == "-" || ch == "_")) ? ch : "-"
        }
        let slug = String(safe).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        let filename = slug.isEmpty ? "secret" : slug
        let fm = FileManager.default
        let base = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? fm.homeDirectoryForCurrentUser
        let dir = base.appendingPathComponent("Coder/secrets", isDirectory: true)
        do {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            let fileURL = dir.appendingPathComponent(filename)
            let data = Data((value + "\n").utf8)
            try data.write(to: fileURL, options: .atomic)
            try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
            return true
        } catch {
            return false
        }
    }
}
