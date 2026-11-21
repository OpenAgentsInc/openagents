// Firm reason: 6 (AppKit Scheduled Tasks UI, background cron timer, CryptoKit HMAC-SHA256
// local webhook trigger listener, and unified "Update Everything" sheet over Coax #13839/#13885).
import AppKit
import CryptoKit
import Foundation

struct ScheduledTaskRecord: Codable {
    var id: String
    var name: String
    var cron: String
    var folder: String
    var prompt: String
    var model: String
    var enabled: Bool
    var last_run_min: Int
    var run_count: Int
}

final class ScheduledTaskManager {
    static let shared = ScheduledTaskManager()

    private weak var delegate: CoderDelegate?
    private var timer: Timer?
    private var dedupIDs: [String] = []
    private let dedupCap = 64

    static var storeURL: URL {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()
        return URL(fileURLWithPath: home)
            .appendingPathComponent(".openagents/coder/scheduled-tasks.jsonl")
    }

    private init() {}

    func start(delegate: CoderDelegate) {
        self.delegate = delegate
        if TestLaunch.active { return }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            self?.tick()
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    func loadTasks() -> [ScheduledTaskRecord] {
        guard let data = try? Data(contentsOf: Self.storeURL),
              let text = String(data: data, encoding: .utf8) else { return [] }
        let dec = JSONDecoder()
        return text.split(separator: "\n").compactMap { line in
            guard let d = line.data(using: .utf8) else { return nil }
            return try? dec.decode(ScheduledTaskRecord.self, from: d)
        }
    }

    func saveTasks(_ tasks: [ScheduledTaskRecord]) {
        let dir = Self.storeURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let enc = JSONEncoder()
        let lines = tasks.compactMap { t -> String? in
            guard let d = try? enc.encode(t) else { return nil }
            return String(data: d, encoding: .utf8)
        }
        let body = lines.joined(separator: "\n") + (lines.isEmpty ? "" : "\n")
        try? body.write(to: Self.storeURL, atomically: true, encoding: .utf8)
    }

    // Standard 5-field UTC cron matcher (mirrors coax/lib/agent/desktop/scheduled.coax).
    static func cronMatches(_ expr: String, date: Date = Date()) -> Bool {
        let parts = expr.split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
        guard parts.count == 5 else { return false }
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(secondsFromGMT: 0)!
        let c = cal.dateComponents([.minute, .hour, .day, .month, .weekday], from: date)
        let vals = [c.minute ?? 0, c.hour ?? 0, c.day ?? 1, c.month ?? 1, ((c.weekday ?? 1) - 1) % 7]
        let bounds = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 6)]
        for i in 0..<5 {
            if !fieldMatches(parts[i], lo: bounds[i].0, hi: bounds[i].1, val: vals[i]) { return false }
        }
        return true
    }

    private static func fieldMatches(_ field: String, lo: Int, hi: Int, val: Int) -> Bool {
        for rawPart in field.split(separator: ",") {
            let part = String(rawPart)
            let slashParts = part.split(separator: "/", maxSplits: 1).map(String.init)
            let base = slashParts[0]
            let step = slashParts.count == 2 ? (Int(slashParts[1]) ?? 0) : 1
            guard step > 0 else { return false }
            let a: Int, b: Int
            if base == "*" {
                a = lo; b = hi
            } else if base.contains("-") {
                let r = base.split(separator: "-", maxSplits: 1).compactMap { Int($0) }
                guard r.count == 2, r[0] >= lo, r[1] <= hi, r[0] <= r[1] else { return false }
                a = r[0]; b = r[1]
            } else if let v = Int(base), v >= lo, v <= hi {
                a = v; b = slashParts.count == 2 ? hi : v
            } else {
                return false
            }
            if val >= a && val <= b && (val - a) % step == 0 { return true }
        }
        return false
    }

    // Constant-time HMAC-SHA256 webhook signature verification + delivery deduplication (#13885).
    func verifyAndAdmitWebhook(secret: String, deliveryID: String, body: Data, signatureHeader: String) -> Bool {
        guard !secret.isEmpty, !deliveryID.isEmpty, signatureHeader.hasPrefix("sha256=") else { return false }
        let key = SymmetricKey(data: Data(secret.utf8))
        let mac = HMAC<SHA256>.authenticationCode(for: body, using: key)
        let expected = "sha256=" + mac.map { String(format: "%02x", $0) }.joined()
        let got = signatureHeader.lowercased()
        guard got.utf8.count == expected.utf8.count else { return false }
        var diff: UInt8 = 0
        for (a, b) in zip(got.utf8, expected.utf8) { diff |= (a ^ b) }
        guard diff == 0 else { return false }
        if dedupIDs.contains(deliveryID) { return false }
        dedupIDs.append(deliveryID)
        if dedupIDs.count > dedupCap { dedupIDs.removeFirst() }
        return true
    }

    func tick(now: Date = Date()) {
        let epochMin = Int(now.timeIntervalSince1970) / 60
        var tasks = loadTasks()
        var changed = false
        for i in tasks.indices {
            guard tasks[i].enabled, tasks[i].last_run_min != epochMin, Self.cronMatches(tasks[i].cron, date: now) else { continue }
            tasks[i].last_run_min = epochMin
            tasks[i].run_count += 1
            changed = true
            runTaskInDesktop(tasks[i])
        }
        if changed { saveTasks(tasks) }
    }

    func runTaskInDesktop(_ task: ScheduledTaskRecord) {
        DispatchQueue.main.async { [weak self] in
            guard let main = self?.delegate?.main else { return }
            if !task.folder.isEmpty {
                main.setFolder(URL(fileURLWithPath: task.folder), keep: true)
            }
            main.newChat()
            if !task.model.isEmpty {
                _ = cd_pick_model(main.page, task.model)
            }
            if cd_set_text(main.page, task.prompt) != 0 {
                _ = cd_submit(main.page)
            }
            main.wake()
        }
    }

    // MARK: - Scheduled Tasks Management Dialog

    func showScheduledTasksDialog() {
        let tasks = loadTasks()
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Scheduled Tasks & Automations"
        if tasks.isEmpty {
            alert.informativeText = "No scheduled tasks configured yet.\nStore: \(Self.storeURL.path)\nClick \"Add Task…\" to create a 5-field cron schedule."
        } else {
            let summary = tasks.map { t in
                "• [\(t.enabled ? "ON" : "PAUSED")] \(t.name) (\(t.cron)) — runs: \(t.run_count)\n  Prompt: \(t.prompt.prefix(72))"
            }.joined(separator: "\n")
            alert.informativeText = summary
        }
        alert.addButton(withTitle: "Add Task…")
        if !tasks.isEmpty {
            alert.addButton(withTitle: "Run First Now")
            alert.addButton(withTitle: "Toggle First Pause")
        }
        alert.addButton(withTitle: "Close")
        let resp = alert.runModal()
        if resp == .alertFirstButtonReturn {
            showAddTaskDialog()
        } else if !tasks.isEmpty && resp == .alertSecondButtonReturn {
            var updated = tasks
            updated[0].run_count += 1
            saveTasks(updated)
            runTaskInDesktop(updated[0])
        } else if !tasks.isEmpty && resp == .alertThirdButtonReturn {
            var updated = tasks
            updated[0].enabled.toggle()
            saveTasks(updated)
        }
    }

    private func showAddTaskDialog() {
        let alert = NSAlert()
        alert.messageText = "New Scheduled Task"
        alert.informativeText = "Enter a name, 5-field UTC cron expression (e.g. \"0 9 * * 1-5\"), and prompt."
        let stack = NSStackView(frame: NSRect(x: 0, y: 0, width: 340, height: 96))
        stack.orientation = .vertical
        stack.spacing = 6
        let nameField = NSTextField(string: "Daily Check")
        nameField.placeholderString = "Task name"
        let cronField = NSTextField(string: "0 9 * * 1-5")
        cronField.placeholderString = "5-field cron (min hour dom month dow)"
        let promptField = NSTextField(string: "")
        promptField.placeholderString = "Prompt to run..."
        stack.addArrangedSubview(nameField)
        stack.addArrangedSubview(cronField)
        stack.addArrangedSubview(promptField)
        alert.accessoryView = stack
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let cron = cronField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let prompt = promptField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty, Self.cronMatches(cron, date: Date()) || cron.split(separator: " ").count == 5 else { return }
        var tasks = loadTasks()
        let folder = delegate?.main?.run.folder.path ?? NSHomeDirectory()
        tasks.append(ScheduledTaskRecord(
            id: "sched-" + UUID().uuidString.prefix(8).lowercased(),
            name: nameField.stringValue.isEmpty ? "Scheduled Task" : nameField.stringValue,
            cron: cron,
            folder: folder,
            prompt: prompt,
            model: "",
            enabled: true,
            last_run_min: 0,
            run_count: 0
        ))
        saveTasks(tasks)
    }

    // MARK: - Unified "Update Everything" Dialog (#13885)

    func showUpdateEverythingDialog() {
        let running = delegate?.main?.runningCount ?? 0
        let agyVer = probeVersion("agy", arg: "--version")
        let claudeVer = probeVersion("claude", arg: "--version")
        let codexVer = probeVersion("codex", arg: "--version")
        let deskVer = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0.0"

        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Update Everything"
        alert.informativeText = """
        OpenAgents Desktop: v\(deskVer) (\(running > 0 ? "deferred: \(running) active session(s)" : "ready to check"))
        Antigravity CLI (agy): \(agyVer.isEmpty ? "not installed" : agyVer) (managed atomic switch via ~/.openagents/runtimes/agy)
        Claude Code CLI: \(claudeVer.isEmpty ? "not installed" : claudeVer)
        Codex CLI: \(codexVer.isEmpty ? "not installed" : codexVer)
        """
        alert.addButton(withTitle: "Check Desktop Updates")
        alert.addButton(withTitle: "Close")
        if alert.runModal() == .alertFirstButtonReturn {
            delegate?.main?.openSettings(4, row: "set.update")
            delegate?.main?.checkForUpdates()
        }
    }

    private func probeVersion(_ prog: String, arg: String) -> String {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        p.arguments = [prog, arg]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do {
            try p.run()
            p.waitUntilExit()
            guard p.terminationStatus == 0 else { return "" }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8)?.split(separator: "\n").first.map(String.init) ?? ""
        } catch {
            return ""
        }
    }
}
