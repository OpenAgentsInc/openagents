// Firm reason: 6 (AppKit Thread Proof, Effective-Model Honesty & Private-by-Default ATIF-v1.8 Trace Drawer
// over coax/lib/agent/desktop/thread-proof.coax and coax/lib/agent/desktop/proof-export.coax; GitHub #13895).
import AppKit
import Foundation

struct ThreadProofTurnSummary {
    let index: Int
    let askPreview: String
    let requestedModel: String
    let effectiveModel: String
    let fallbackDetected: Bool
    let badge: String
    let commands: [(text: String, failed: Bool, output: String)]
}

struct ThreadProofSnapshot {
    let chatTitle: String
    let providerName: String
    let execMode: String
    let goalStatusLabel: String
    let turns: [ThreadProofTurnSummary]
    let stepsDigest: String
    let stepCount: Int
    let verified: Bool
    let rawChatJSON: String
}

enum ThreadProofPrivacy {
    static let syncOptInKey = "openagents.traceSyncOptIn"

    /// Strictly private by default: nothing leaves the machine unless explicitly opted in (#13895).
    static var isSyncOptedIn: Bool {
        get { UserDefaults.standard.bool(forKey: syncOptInKey) }
        set { UserDefaults.standard.set(newValue, forKey: syncOptInKey) }
    }

    static var statusLabel: String {
        isSyncOptedIn ? "Encrypted Sync Enabled (Opt-In)" : "Local Only (Private by Default)"
    }

    static var localTracesDirectory: URL {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()
        return URL(fileURLWithPath: home).appendingPathComponent(".openagents/traces", isDirectory: true)
    }
}

enum ThreadProofModel {
    static func providerLabel(kind: Int32) -> (provider: String, mode: String) {
        switch kind {
        case 0: return ("OpenAgents", "cloud")
        case 1: return ("Claude Code", "cloud")
        case 2: return ("OpenRouter", "cloud")
        case 3: return ("Antigravity", "cloud")
        case 4: return ("Psionic Metal", "local")
        case 5: return ("llama.cpp Metal", "local")
        default: return ("OpenAgents", "local")
        }
    }

    static func goalVerdictLabel(_ status: Int32) -> String {
        switch status {
        case 0: return "No /goal active"
        case 1: return "/goal: active"
        case 2: return "/goal: paused"
        case 3: return "/goal: verified complete"
        case 4: return "/goal: blocked (needs owner input)"
        case 5: return "/goal: budget limited"
        case 6: return "/goal: failed (impossible)"
        default: return "/goal: status \(status)"
        }
    }

    /// Mirrors coax/lib/agent/desktop/thread-proof.coax `tp_format_effective_badge`:
    /// always reports the actual effective runner/model and discloses any fallback.
    static func formatEffectiveBadge(
        provider: String,
        requestedModel: String,
        effectiveModel: String,
        execMode: String
    ) -> (badge: String, fallback: Bool) {
        let actual = effectiveModel.isEmpty ? (requestedModel.isEmpty ? "unknown" : requestedModel) : effectiveModel
        let prov = provider.isEmpty ? "OpenAgents" : provider
        let mode = execMode.isEmpty ? "local" : execMode
        let fallback = !requestedModel.isEmpty && !effectiveModel.isEmpty && requestedModel != effectiveModel
        var s = "\(prov) / \(actual) [\(mode)]"
        if fallback {
            s += " (fallback from \(requestedModel))"
        }
        return (s, fallback)
    }

    static func proofExportExecutable() -> String? {
        if let res = Bundle.main.resourceURL?.appendingPathComponent("proof-export").path,
           FileManager.default.isExecutableFile(atPath: res) {
            return res
        }
        return nil
    }

    static func runProofExport(chatJSON: String, outPath: String?, provider: String, mode: String, goalStatus: Int32) -> (digest: String, steps: Int, ok: Bool, error: String?) {
        guard let exe = proofExportExecutable() else {
            return ("(proof-export helper not bundled)", 0, false, "proof-export binary not found in bundle")
        }
        let tmpDir = FileManager.default.temporaryDirectory
        let tmpChat = tmpDir.appendingPathComponent("oa-proof-\(UUID().uuidString).json")
        let tmpOut = outPath.map { URL(fileURLWithPath: $0) } ?? tmpDir.appendingPathComponent("oa-proof-\(UUID().uuidString).atif.jsonl")
        defer {
            try? FileManager.default.removeItem(at: tmpChat)
            if outPath == nil { try? FileManager.default.removeItem(at: tmpOut) }
        }
        do {
            try Data(chatJSON.utf8).write(to: tmpChat, options: .atomic)
        } catch {
            return ("", 0, false, error.localizedDescription)
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: exe)
        proc.arguments = [
            "export",
            "--chat", tmpChat.path,
            "--out", tmpOut.path,
            "--act-provider", provider,
            "--exec-mode", mode,
            "--goal-status", String(goalStatus)
        ]
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        do {
            try proc.run()
            proc.waitUntilExit()
        } catch {
            return ("", 0, false, error.localizedDescription)
        }
        let outData = outPipe.fileHandleForReading.readDataToEndOfFile()
        let errText = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        guard proc.terminationStatus == 0,
              let obj = try? JSONSerialization.jsonObject(with: outData) as? [String: Any] else {
            return ("", 0, false, errText.isEmpty ? "proof-export exited \(proc.terminationStatus)" : errText)
        }
        let digest = (obj["steps_digest"] as? String) ?? ""
        let steps = (obj["steps"] as? Int) ?? 0
        let verified = (obj["verified"] as? Bool) ?? false
        return (digest, steps, verified, nil)
    }

    static func snapshot(window: CoderWindow) -> ThreadProofSnapshot {
        let chatJSON = window.savedChat(window.page, from: 1_048_576) ?? "{}"
        let goalStatus = cd_goal_status(window.page)
        let provKind = cd_model_provider(window.page)
        let (providerName, execMode) = providerLabel(kind: provKind)
        var chatTitle = "Current Thread"
        var defaultModel = ""
        var turns: [ThreadProofTurnSummary] = []

        if let data = chatJSON.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let m = obj["model"] as? String, !m.isEmpty { defaultModel = m }
            if let t = obj["title"] as? String, !t.isEmpty { chatTitle = t }
            if let rawTurns = obj["turns"] as? [[String: Any]] {
                for (i, t) in rawTurns.enumerated() {
                    let ask = ((t["ask"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                    let askPreview = ask.count > 72 ? String(ask.prefix(72)) + "…" : (ask.isEmpty ? "(turn \(i + 1))" : ask)
                    let reqModel = (t["model"] as? String) ?? defaultModel
                    let ranModel = (t["ran"] as? String) ?? ""
                    let effModel = ranModel.isEmpty ? reqModel : ranModel
                    let fmt = formatEffectiveBadge(provider: providerName, requestedModel: reqModel, effectiveModel: effModel, execMode: execMode)
                    var cmds: [(text: String, failed: Bool, output: String)] = []
                    if let rawCmds = t["cmds"] as? [[String: Any]] {
                        for c in rawCmds {
                            let ctext = (c["text"] as? String) ?? "(command)"
                            let errFlag = (c["err"] as? Bool) ?? ((c["ok"] as? Bool) == false)
                            let cout = (c["out"] as? String) ?? ""
                            cmds.append((text: ctext, failed: errFlag, output: cout))
                        }
                    }
                    turns.append(ThreadProofTurnSummary(
                        index: i + 1,
                        askPreview: askPreview,
                        requestedModel: reqModel,
                        effectiveModel: effModel,
                        fallbackDetected: fmt.fallback,
                        badge: fmt.badge,
                        commands: cmds
                    ))
                }
            }
        }

        let export = runProofExport(chatJSON: chatJSON, outPath: nil, provider: providerName, mode: execMode, goalStatus: goalStatus)
        return ThreadProofSnapshot(
            chatTitle: chatTitle,
            providerName: providerName,
            execMode: execMode,
            goalStatusLabel: goalVerdictLabel(goalStatus),
            turns: turns,
            stepsDigest: export.digest,
            stepCount: export.steps,
            verified: export.ok,
            rawChatJSON: chatJSON
        )
    }
}

final class ThreadProofDrawer {
    static let shared = ThreadProofDrawer()

    func present(window: CoderWindow?) {
        guard let window else { return }
        let snap = ThreadProofModel.snapshot(window: window)

        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Thread Proof & ATIF-v1.8 Trace"

        var lines: [String] = []
        lines.append("Privacy: \(ThreadProofPrivacy.statusLabel)")
        lines.append("Goal Verdict: \(snap.goalStatusLabel)")
        lines.append("ATIF-v1.8 steps_digest: \(snap.stepsDigest.isEmpty ? "(no steps yet)" : snap.stepsDigest)")
        lines.append("ATIF Verification: \(snap.verified ? "PASS (\(snap.stepCount) canonical steps)" : "Not yet verified (\(snap.stepCount) steps)")")
        lines.append("")
        if snap.turns.isEmpty {
            lines.append("No completed turns in this thread yet.")
        } else {
            for t in snap.turns {
                lines.append("Turn \(t.index): \(t.askPreview)")
                lines.append("  Effective Model: \(t.badge)")
                if t.commands.isEmpty {
                    lines.append("  Commands: none")
                } else {
                    for c in t.commands.prefix(8) {
                        let status = c.failed ? "FAIL (non-zero)" : "OK (exit 0)"
                        lines.append("  • [\(status)] \(c.text)")
                    }
                    if t.commands.count > 8 {
                        lines.append("  • … and \(t.commands.count - 8) more commands")
                    }
                }
            }
        }

        let container = NSView(frame: NSRect(x: 0, y: 0, width: 540, height: 320))
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 40, width: 540, height: 280))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        let tv = NSTextView(frame: NSRect(x: 0, y: 0, width: 520, height: 280))
        tv.isEditable = false
        tv.isSelectable = true
        tv.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        tv.string = lines.joined(separator: "\n")
        scroll.documentView = tv
        container.addSubview(scroll)

        let syncCheck = NSButton(checkboxWithTitle: "Opt in to encrypted trace backup (openagents traces) — off by default", target: nil, action: nil)
        syncCheck.frame = NSRect(x: 0, y: 8, width: 540, height: 24)
        syncCheck.state = ThreadProofPrivacy.isSyncOptedIn ? .on : .off
        container.addSubview(syncCheck)

        alert.accessoryView = container
        alert.addButton(withTitle: "Close")
        alert.addButton(withTitle: "Export .atif.jsonl…")
        alert.addButton(withTitle: "Copy Digest")

        let resp = alert.runModal()
        ThreadProofPrivacy.isSyncOptedIn = (syncCheck.state == .on)

        if resp == .alertSecondButtonReturn {
            exportATIF(snapshot: snap, window: window)
        } else if resp == .alertThirdButtonReturn, !snap.stepsDigest.isEmpty {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(snap.stepsDigest, forType: .string)
        }
    }

    private func exportATIF(snapshot: ThreadProofSnapshot, window: CoderWindow) {
        let panel = NSSavePanel()
        panel.title = "Export Canonical ATIF-v1.8 Trace"
        panel.nameFieldStringValue = "thread-trace.atif.jsonl"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }

        let goalStatus = cd_goal_status(window.page)
        let res = ThreadProofModel.runProofExport(
            chatJSON: snapshot.rawChatJSON,
            outPath: url.path,
            provider: snapshot.providerName,
            mode: snapshot.execMode,
            goalStatus: goalStatus
        )
        let done = NSAlert()
        done.alertStyle = res.ok ? .informational : .warning
        done.messageText = res.ok ? "ATIF-v1.8 Trace Exported & Verified" : "ATIF Export Failed"
        if res.ok {
            done.informativeText = "Saved verified ATIF-v1.8 log (\(res.steps) steps) to:\n\(url.path)\n\nsteps_digest:\n\(res.digest)"
        } else {
            done.informativeText = res.error ?? "Failed to export ATIF-v1.8 trace."
        }
        done.runModal()
    }
}
