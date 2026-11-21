// Firm reason: 6 (AppKit dialogs, 60s scheduler Timer, ~/.openagents/desktop/scheduled-tasks.json I/O,
// 0600 webhook secret file, and CLI version probes over page/scheduled.coax; GitHub #13885).
import AppKit
import Foundation

enum DesktopScheduledStore {
    static var configURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/scheduled-tasks.json")
    }

    static var webhookSecretURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/webhook.secret")
    }

    static func loadJSON() -> String {
        guard let data = try? Data(contentsOf: configURL),
              let text = String(data: data, encoding: .utf8) else {
            return "{\"tasks\":[]}"
        }
        return text
    }

    static func saveJSON(_ json: String) {
        let dir = configURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? json.data(using: .utf8)?.write(to: configURL, options: .atomic)
    }

    static func loadWebhookSecret() -> String {
        guard let data = try? Data(contentsOf: webhookSecretURL),
              let text = String(data: data, encoding: .utf8) else { return "" }
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func saveWebhookSecret(_ secret: String) {
        let trimmed = secret.trimmingCharacters(in: .whitespacesAndNewlines)
        let dir = webhookSecretURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        if trimmed.isEmpty {
            try? FileManager.default.removeItem(at: webhookSecretURL)
            return
        }
        FileManager.default.createFile(atPath: webhookSecretURL.path, contents: trimmed.data(using: .utf8), attributes: [.posixPermissions: 0o600])
    }
}

final class DesktopSchedulerLoop {
    static let shared = DesktopSchedulerLoop()
    private var timer: Timer?
    private weak var window: CoderWindow?

    func attach(_ window: CoderWindow) {
        self.window = window
        if TestLaunch.active { return }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { [weak self] _ in
            self?.window?.tickScheduledTasks()
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }
}

extension CoderWindow {
    func startScheduledAutomation() {
        syncScheduledSettings()
        DesktopSchedulerLoop.shared.attach(self)
    }

    func syncScheduledSettings() {
        _ = cd_sched_load_json(page, DesktopScheduledStore.loadJSON())
        let secret = DesktopScheduledStore.loadWebhookSecret()
        if secret.isEmpty {
            cd_sched_webhook_set(page, 0, "Not configured")
        } else {
            cd_sched_webhook_set(page, 1, "HMAC-SHA256 secret configured (0600)")
        }
        syncDesktopUpdateTarget()
    }

    private func syncDesktopUpdateTarget() {
        let deskVer = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0.0"
        let leases = Int32(runningCount)
        let avail: Int32
        let latest: String
        let status: String
        switch updater.state {
        case .available(let ver, _):
            avail = 1
            latest = ver
            status = leases > 0 ? "\(deskVer) → \(ver) (deferred: \(leases) active)" : "\(deskVer) → \(ver) ready"
        case .ready(let ver, _, _):
            avail = 1
            latest = ver
            status = leases > 0 ? "\(deskVer) → \(ver) (deferred: \(leases) active)" : "\(deskVer) → \(ver) staged"
        case .downloading(let ver, _):
            avail = 0
            latest = ver
            status = "Downloading \(ver)…"
        case .error(let msg, _):
            avail = 0
            latest = ""
            status = msg
        case .hidden:
            avail = 0
            latest = deskVer
            status = "v\(deskVer) (up to date)"
        }
        cd_upd_all_set(page, 0, deskVer, latest, avail, leases, 0, status)
    }

    func saveScheduledTasks() {
        var buf = [CChar](repeating: 0, count: 65536)
        let n = cd_sched_save_json(page, &buf, Int32(buf.count))
        if n > 0 {
            DesktopScheduledStore.saveJSON(String(cString: buf))
        }
    }

    private func schedReadField(_ fn: (UnsafeMutableRawPointer?, Int32, UnsafeMutablePointer<CChar>?, Int32) -> Int32, idx: Int32, cap: Int = 1024) -> String {
        var buf = [CChar](repeating: 0, count: cap)
        return fn(page, idx, &buf, Int32(cap)) > 0 ? String(cString: buf) : ""
    }

    func tickScheduledTasks(now: Date = Date()) {
        let nowMs = Int64(now.timeIntervalSince1970 * 1000.0)
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(secondsFromGMT: 0)!
        let c = cal.dateComponents([.minute, .hour, .day, .month, .weekday], from: now)
        let minute = Int32(c.minute ?? 0)
        let hour = Int32(c.hour ?? 0)
        let dom = Int32(c.day ?? 1)
        let month = Int32(c.month ?? 1)
        let dow = Int32(((c.weekday ?? 1) - 1) % 7)

        var indices = [Int32](repeating: -1, count: 32)
        let dueCount = cd_sched_due(page, nowMs, minute, hour, dom, month, dow, &indices, 32)
        guard dueCount > 0 else { return }
        for k in 0..<Int(min(dueCount, 32)) {
            executeScheduledTask(at: indices[k], nowMs: nowMs)
        }
    }

    func executeScheduledTask(at idx: Int32, nowMs: Int64 = Int64(Date().timeIntervalSince1970 * 1000.0)) {
        let folderPath = schedReadField(cd_sched_get_folder, idx: idx, cap: 512)
        let prompt = schedReadField(cd_sched_get_prompt, idx: idx, cap: 1024)
        guard !prompt.isEmpty else { return }

        if settingsOpen { closeSettings() }
        if !folderPath.isEmpty {
            let url = URL(fileURLWithPath: (folderPath as NSString).expandingTildeInPath)
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), isDir.boolValue {
                setFolder(url, keep: true)
            }
        }
        newChat()
        let chatID = chat ?? "sched-\(nowMs)"
        _ = cd_sched_mark_run(page, idx, nowMs, chatID)
        saveScheduledTasks()
        if cd_set_text(page, prompt) != 0 {
            _ = cd_submit(page)
            act(cd_action(page))
        }
        wake()
    }

    // MARK: - Settings -> Scheduled Tasks Dialogs

    func schedAddDialog() {
        var d = CoderDialog(size: .md, title: "Add Scheduled Task",
                            body: "Create a 5-field UTC cron schedule (min hour dom month dow). Due tasks automatically launch in a new chat.")
        d.fields = [
            .init(label: "Name", value: "Daily Check", hint: "e.g. Nightly Audit"),
            .init(label: "Cron (UTC)", value: "0 9 * * 1-5", hint: "min hour dom month dow"),
            .init(label: "Project folder", value: folder.path, hint: "/path/to/repo"),
            .init(label: "Prompt", value: "", hint: "What should the agent run?", lines: 3),
        ]
        d.buttons = [.init(title: "Cancel"), .init(title: "Add Task", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self, answer == 1 else { self?.refocus(); return }
            let name = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let cron = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let dir = (values.count > 2 ? values[2] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let prompt = (values.count > 3 ? values[3] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !prompt.isEmpty else {
                self.present(.notice("Missing Prompt", "Enter a prompt for the scheduled task."))
                return
            }
            guard cd_sched_cron_valid(cron) != 0 else {
                self.present(.notice("Invalid Cron Expression", "\"\(cron)\" is not a valid 5-field UTC cron expression (e.g. \"0 9 * * 1-5\" or \"*/15 * * * *\")."))
                return
            }
            let id = "sched-" + UUID().uuidString.prefix(8).lowercased()
            let added = cd_sched_add(self.page, id, name.isEmpty ? "Scheduled Task" : name, cron, dir.isEmpty ? self.folder.path : dir, prompt, 1)
            if added < 0 {
                self.present(.notice("Task Limit Reached", "Up to 32 scheduled tasks are supported."))
                return
            }
            self.saveScheduledTasks()
            self.refocus()
        }
    }

    func schedToggleSelected() {
        saveScheduledTasks()
        wake()
    }

    func schedRunSelectedNow() {
        let idx = cd_sched_target(page)
        guard idx >= 0, idx < cd_sched_count(page) else { return }
        executeScheduledTask(at: idx)
    }

    func schedDeleteSelected() {
        let idx = cd_sched_target(page)
        guard idx >= 0, idx < cd_sched_count(page) else { return }
        let name = schedReadField(cd_sched_get_name, idx: idx, cap: 128)
        var d = CoderDialog(size: .sm, title: "Delete Scheduled Task?",
                            body: "Remove \"\(name)\" from ~/.openagents/desktop/scheduled-tasks.json?")
        d.buttons = [.init(title: "Cancel"), .init(title: "Delete", kind: .danger)]
        d.cancel = 0; d.confirm = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 {
                _ = cd_sched_remove(self.page, idx)
                self.saveScheduledTasks()
            }
            self.refocus()
        }
    }

    func schedWebhookDialog() {
        let hasSecret = !DesktopScheduledStore.loadWebhookSecret().isEmpty
        var d = CoderDialog(size: .md, title: "Webhook Triggers (HMAC-SHA256)",
                            body: hasSecret
                                ? "A webhook signing secret is saved in ~/.openagents/desktop/webhook.secret (0600). Enter a new secret to rotate it, or leave blank to clear."
                                : "Set a shared secret for verifying X-OpenAgents-Signature (HMAC-SHA256) webhook deliveries with 64-delivery replay deduplication.")
        d.fields = [.init(label: "Webhook secret", value: "", hint: "whsec_...", secure: true)]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save Secret", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1 {
                let secret = values.first ?? ""
                DesktopScheduledStore.saveWebhookSecret(secret)
                self.syncScheduledSettings()
            }
            self.refocus()
        }
    }

    // MARK: - Settings -> About & updates -> "Update Everything"

    func checkUpdateEverything() {
        cd_upd_all_busy_set(page, 1)
        syncDesktopUpdateTarget()
        let activeRuns = Int32(runningCount)
        updater.checkNow(userInitiated: false)

        DispatchQueue.global(qos: .utility).async { [weak self] in
            let agyVer = Self.probeCliVersion("agy")
            let claudeVer = Self.probeCliVersion("claude")
            let codexVer = Self.probeCliVersion("codex")

            DispatchQueue.main.async {
                guard let self else { return }
                self.syncDesktopUpdateTarget()
                cd_upd_all_set(self.page, 1, agyVer, agyVer, 0, activeRuns, 1,
                               agyVer.isEmpty ? "Not installed" : "\(agyVer) (managed atomic switch)")
                cd_upd_all_set(self.page, 2, claudeVer, claudeVer, 0, activeRuns, 0,
                               claudeVer.isEmpty ? "Not installed" : "\(claudeVer)\(activeRuns > 0 ? " (lease guarded)" : "")")
                cd_upd_all_set(self.page, 3, codexVer, codexVer, 0, activeRuns, 0,
                               codexVer.isEmpty ? "Not installed" : "\(codexVer)\(activeRuns > 0 ? " (lease guarded)" : "")")
                cd_upd_all_busy_set(self.page, 0)
                self.wake()
            }
        }
    }

    func applyUpdateEverything() {
        syncDesktopUpdateTarget()
        let deskDec = cd_upd_all_decision(page, 0)
        let agyDec = cd_upd_all_decision(page, 1)
        let claudeDec = cd_upd_all_decision(page, 2)
        let codexDec = cd_upd_all_decision(page, 3)

        var deferred: [String] = []
        if deskDec == 2 { deferred.append("OpenAgents Desktop") }
        if claudeDec == 2 { deferred.append("Claude Code CLI") }
        if codexDec == 2 { deferred.append("Codex CLI") }

        if deskDec == 1 {
            handleUpdatePillClick()
        }
        if !deferred.isEmpty {
            present(.notice("Updates Deferred by Active Session Leases",
                            "Skipped disruptive updates for: \(deferred.joined(separator: ", ")). Finish or stop running tasks, then click Update all again."))
        } else if deskDec == 0 && agyDec == 0 && claudeDec == 0 && codexDec == 0 {
            checkUpdateEverything()
        }
    }

    private static func probeCliVersion(_ binary: String) -> String {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        p.arguments = [binary, "--version"]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do {
            try p.run()
            p.waitUntilExit()
            guard p.terminationStatus == 0 else { return "" }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8)?
                .split(separator: "\n")
                .first
                .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) } ?? ""
        } catch {
            return ""
        }
    }
}
