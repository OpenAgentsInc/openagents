// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Dialogs: a chat's budget and goal, the Claude key and the OpenRouter models, drawn by the page
// (page/overlay.coax, GitHub #13746). The field with the keyboard is the page's own editor
// (page/editor.coax, GitHub #13742): a secure one shows dots and never copies.
import AppKit

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

/// Local Psionic Metal checkpoint store (~/.openagents/models) for OpenAgents Desktop (#13825, #13826).
enum LocalModelStore {
    static var rootDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("models", isDirectory: true)
    }
    static var inklingDir: URL { rootDir.appendingPathComponent("inkling-2-mini", isDirectory: true) }
    static var weightsURL: URL { inklingDir.appendingPathComponent("Inkling-2-Mini-IQ4_NL.gguf") }
    static var partialURL: URL { inklingDir.appendingPathComponent("Inkling-2-Mini-IQ4_NL.gguf.part") }
    static var manifestURL: URL { inklingDir.appendingPathComponent("manifest.json") }
    static let downloadURL = "https://huggingface.co/ggml-org/Inkling-2-Mini-GGUF/resolve/main/Inkling-2-Mini-IQ4_NL.gguf"
    static let expectedBytes: Int64 = 17_627_940_768
    static let minValidBytes: Int64 = 16_000_000_000
    static var physicalMemoryGB: Int { Int(ProcessInfo.processInfo.physicalMemory / (1024 * 1024 * 1024)) }
    static var hardwareSupported: Bool { physicalMemoryGB >= 32 }
    static var weightsBytes: Int64 {
        (try? FileManager.default.attributesOfItem(atPath: weightsURL.path)[.size] as? Int64) ?? 0
    }
    static var partialBytes: Int64 {
        (try? FileManager.default.attributesOfItem(atPath: partialURL.path)[.size] as? Int64) ?? 0
    }
    static var isInstalled: Bool { weightsBytes >= minValidBytes }
    static var isSelectable: Bool { hardwareSupported && isInstalled }
    private static var downloadProcess: Process?
    static var isDownloading: Bool { downloadProcess?.isRunning == true }
    static var lastError: String?

    static func statusSummary() -> String {
        if !hardwareSupported { return "Requires 32 GB RAM" }
        if isDownloading {
            let gb = Double(partialBytes) / 1_073_741_824.0
            let pct = Int(min(99, max(0, (Double(partialBytes) / Double(expectedBytes)) * 100.0)))
            return String(format: "Downloading %.1f / 16.4 GB (%d%%)", gb, pct)
        }
        if isInstalled { return "Installed (16.4 GB · Metal)" }
        return "Not downloaded (16.4 GB)"
    }
    static func writeManifest() {
        try? FileManager.default.createDirectory(at: inklingDir, withIntermediateDirectories: true)
        let manifest = "{\"model_id\":\"inkling-2-mini\",\"weights\":\"Inkling-2-Mini-IQ4_NL.gguf\",\"bytes\":\(weightsBytes),\"loops\":3}\n"
        try? manifest.write(to: manifestURL, atomically: true, encoding: .utf8)
    }
    static func startDownload(onUpdate: @escaping () -> Void) {
        guard !isDownloading else { return }
        lastError = nil
        do {
            try FileManager.default.createDirectory(at: inklingDir, withIntermediateDirectories: true)
        } catch {
            lastError = "Could not create " + inklingDir.path
            onUpdate()
            return
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
        proc.arguments = ["-fSL", "-C", "-", "-o", partialURL.path, downloadURL]
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        proc.terminationHandler = { p in
            DispatchQueue.main.async {
                downloadProcess = nil
                if p.terminationStatus == 0 && partialBytes >= minValidBytes {
                    try? FileManager.default.removeItem(at: weightsURL)
                    try? FileManager.default.moveItem(at: partialURL, to: weightsURL)
                    writeManifest()
                } else if p.terminationStatus != 0 && p.terminationReason != .uncaughtSignal {
                    lastError = "Download failed (curl exit \(p.terminationStatus))."
                }
                onUpdate()
            }
        }
        do {
            try proc.run()
            downloadProcess = proc
            onUpdate()
        } catch {
            lastError = "Could not launch curl: \(error.localizedDescription)"
            onUpdate()
        }
    }
    static func cancelDownload() {
        downloadProcess?.terminate()
        downloadProcess = nil
    }
    static func removeInkling() {
        cancelDownload()
        LocalModelServer.shared.stop()
        try? FileManager.default.removeItem(at: inklingDir)
    }
}

/// Resident local Metal inference server for Inkling 2 Mini on 127.0.0.1:18794.
final class LocalModelServer {
    static let shared = LocalModelServer()
    static let host = "127.0.0.1"
    static let port = 18794
    static var baseURL: String { "http://\(host):\(port)" }
    private var process: Process?
    private let queue = DispatchQueue(label: "openagents.local-model-server")

    static func findLlamaServer() -> String? {
        let homeBin = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/bin/llama-server").path
        let candidates = [
            homeBin,
            "/opt/homebrew/bin/llama-server",
            "/usr/local/bin/llama-server",
            Bundle.main.resourcePath.map { $0 + "/llama-server" } ?? "",
        ]
        for path in candidates where !path.isEmpty && FileManager.default.isExecutableFile(atPath: path) {
            return path
        }
        return nil
    }

    private func isHealthy() -> Bool {
        guard let url = URL(string: Self.baseURL + "/v1/models") else { return false }
        var req = URLRequest(url: url)
        req.timeoutInterval = 1.0
        let sem = DispatchSemaphore(value: 0)
        var ok = false
        URLSession.shared.dataTask(with: req) { _, resp, _ in
            if let http = resp as? HTTPURLResponse, http.statusCode == 200 { ok = true }
            sem.signal()
        }.resume()
        _ = sem.wait(timeout: .now() + 1.5)
        return ok
    }

    func ensureRunning(completion: @escaping (String?) -> Void) {
        queue.async {
            if self.isHealthy() {
                DispatchQueue.main.async { completion(nil) }
                return
            }
            guard LocalModelStore.isInstalled else {
                DispatchQueue.main.async { completion("Inkling 2 Mini weights are not downloaded.") }
                return
            }
            guard let bin = Self.findLlamaServer() else {
                DispatchQueue.main.async { completion("llama-server not found. Install it with: brew install llama.cpp") }
                return
            }
            if self.process?.isRunning != true {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: bin)
                p.arguments = [
                    "--model", LocalModelStore.weightsURL.path,
                    "--alias", "inkling-2-mini",
                    "--host", Self.host,
                    "--port", String(Self.port),
                    "--ctx-size", "32768",
                    "--n-gpu-layers", "99",
                    "--flash-attn", "on",
                ]
                p.standardOutput = FileHandle.nullDevice
                p.standardError = FileHandle.nullDevice
                do {
                    try p.run()
                    self.process = p
                } catch {
                    DispatchQueue.main.async { completion("Could not start llama-server: \(error.localizedDescription)") }
                    return
                }
            }
            for _ in 0..<90 {
                if self.isHealthy() {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }
                if self.process?.isRunning != true {
                    DispatchQueue.main.async { completion("Local Metal server exited before becoming ready.") }
                    return
                }
                Thread.sleep(forTimeInterval: 0.5)
            }
            DispatchQueue.main.async { completion("Timed out waiting for Inkling 2 Mini Metal server on \(Self.baseURL).") }
        }
    }

    func stop() {
        queue.async {
            self.process?.terminate()
            self.process = nil
        }
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
    /// Settings > Models > Local Models (#13826): download, verify, or remove local Psionic Metal checkpoints.
    func manageLocalModels() {
        let ramGB = LocalModelStore.physicalMemoryGB
        let supported = LocalModelStore.hardwareSupported
        let installed = LocalModelStore.isInstalled
        let downloading = LocalModelStore.isDownloading
        var about = "Inkling 2 Mini (26.4B total / 3.4B active looped MoE, IQ4_NL GGUF, 16.4 GB).\n"
        about += "Storage: " + LocalModelStore.inklingDir.path + "\n"
        about += "Hardware: " + String(ramGB) + " GB unified memory"
        if !supported {
            about += " (requires >= 32 GB RAM)."
        } else if downloading {
            about += " · " + LocalModelStore.statusSummary()
        } else if installed {
            about += " · Installed and ready in the composer model picker (Metal GPU)."
        } else {
            about += " · Ready to download from Hugging Face (16.4 GB, resumable)."
        }
        if let err = LocalModelStore.lastError {
            about += "\nLast error: " + err
        }
        var d = CoderDialog(size: .md, title: "Local Models — Inkling 2 Mini", body: about)
        var removeIdx = -9
        var downloadIdx = -9
        var cancelDlIdx = -9
        if installed {
            removeIdx = d.buttons.count
            d.buttons.append(.init(title: "Remove", kind: .danger, aside: true))
            let done = d.buttons.count
            d.buttons.append(.init(title: "Done", kind: .primary))
            d.confirm = done; d.cancel = done
        } else if downloading {
            cancelDlIdx = d.buttons.count
            d.buttons.append(.init(title: "Cancel Download", kind: .danger, aside: true))
            let done = d.buttons.count
            d.buttons.append(.init(title: "Close", kind: .primary))
            d.confirm = done; d.cancel = done
        } else if supported {
            d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
            downloadIdx = d.buttons.count; d.buttons.append(.init(title: "Download", kind: .primary))
            d.confirm = downloadIdx
        } else {
            let close = d.buttons.count; d.buttons.append(.init(title: "Close", kind: .primary))
            d.confirm = close; d.cancel = close
        }
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == downloadIdx {
                LocalModelStore.startDownload { [weak self] in
                    self?.syncLocalModels()
                }
                self.syncLocalModels()
            } else if answer == cancelDlIdx {
                LocalModelStore.cancelDownload()
                self.syncLocalModels()
            } else if answer == removeIdx {
                LocalModelStore.removeInkling()
                self.syncLocalModels()
            }
            self.refocus()
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
}
