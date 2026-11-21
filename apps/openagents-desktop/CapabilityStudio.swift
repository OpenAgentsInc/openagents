// Firm reason: 6 (AppKit Local Capability Authoring & Two-Arm Eval Studio over
// coax/lib/agent/desktop/capability-eval.coax and #authority capability-gate.coax; GitHub #13896).
import AppKit
import Foundation

final class CapabilityStudio {
    static let shared = CapabilityStudio()

    static var skillsRoot: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/skills", isDirectory: true)
    }

    static var experimentsRoot: URL {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? NSHomeDirectory()
        return URL(fileURLWithPath: home).appendingPathComponent(".openagents/capabilities/experiments", isDirectory: true)
    }

    static func capabilityEvalExecutable() -> String? {
        if let res = Bundle.main.resourceURL?.appendingPathComponent("capability-eval").path,
           FileManager.default.isExecutableFile(atPath: res) {
            return res
        }
        return nil
    }

    func present(window: CoderWindow?) {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Author Local Capability & Run Two-Arm Eval"
        alert.informativeText = "Complete the authoring interview and define positive + held-out eval commands. Passing candidates promote into ~/.claude/skills/ via the #authority gate; regressed candidates are rejected with ATIF-v1.8 evidence preserved."

        let form = NSView(frame: NSRect(x: 0, y: 0, width: 520, height: 276))
        func field(_ label: String, y: CGFloat, value: String) -> NSTextField {
            let lbl = NSTextField(labelWithString: label)
            lbl.frame = NSRect(x: 0, y: y + 2, width: 150, height: 20)
            lbl.font = NSFont.systemFont(ofSize: 11, weight: .medium)
            form.addSubview(lbl)
            let tf = NSTextField(string: value)
            tf.frame = NSRect(x: 156, y: y, width: 364, height: 24)
            tf.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
            form.addSubview(tf)
            return tf
        }

        let slugField = field("Capability Slug:", y: 248, value: "my-local-skill")
        let goalField = field("Goal:", y: 218, value: "Automate repeatable local check")
        let inputsField = field("Inputs:", y: 188, value: "Repository path or file")
        let outputsField = field("Outputs:", y: 158, value: "Verified report")
        let nongoalsField = field("Non-Goals / Guardrails:", y: 128, value: "Never modify tracked files")
        let posBaseField = field("Positive (Baseline cmd):", y: 92, value: "exit 1")
        let posCandField = field("Positive (Candidate cmd):", y: 62, value: "exit 0")
        let negBaseField = field("Held-out (Baseline cmd):", y: 32, value: "exit 0")
        let negCandField = field("Held-out (Candidate cmd):", y: 2, value: "exit 0")

        alert.accessoryView = form
        alert.addButton(withTitle: "Run Two-Arm Eval & Gate")
        alert.addButton(withTitle: "Cancel")

        guard alert.runModal() == .alertFirstButtonReturn else { return }

        guard let exe = Self.capabilityEvalExecutable() else {
            let err = NSAlert()
            err.alertStyle = .warning
            err.messageText = "capability-eval helper not found"
            err.informativeText = "Rebuild OpenAgents.app with apps/openagents-desktop/build.sh to bundle capability-eval."
            err.runModal()
            return
        }

        let slug = slugField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let tmpDraft = FileManager.default.temporaryDirectory.appendingPathComponent("oa-cap-draft-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: tmpDraft) }

        let draft = runProcess(exe, args: [
            "draft",
            "--slug", slug,
            "--goal", goalField.stringValue,
            "--inputs", inputsField.stringValue,
            "--outputs", outputsField.stringValue,
            "--nongoals", nongoalsField.stringValue,
            "--out-dir", tmpDraft.path
        ])
        guard draft.status == 0 else {
            showResult(title: "Draft Rejected", body: draft.stderr.isEmpty ? draft.stdout : draft.stderr, ok: false)
            return
        }

        let skillMD = tmpDraft.appendingPathComponent("SKILL.md").path
        let evalRes = runProcess(exe, args: [
            "run",
            "--slug", slug,
            "--skill-md", skillMD,
            "--pos-base", posBaseField.stringValue,
            "--pos-cand", posCandField.stringValue,
            "--neg-base", negBaseField.stringValue,
            "--neg-cand", negCandField.stringValue,
            "--skills-dir", Self.skillsRoot.path,
            "--exp-dir", Self.experimentsRoot.path
        ])

        var promoted = false
        var summary = evalRes.stdout.isEmpty ? evalRes.stderr : evalRes.stdout
        if let data = evalRes.stdout.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            promoted = (obj["promoted"] as? Bool) ?? false
            let verdict = (obj["verdict"] as? String) ?? ""
            let bp = (obj["baseline_passed"] as? Int) ?? 0
            let cp = (obj["candidate_passed"] as? Int) ?? 0
            let bh = (obj["baseline_heldout_passed"] as? Int) ?? 0
            let ch = (obj["candidate_heldout_passed"] as? Int) ?? 0
            let digest = (obj["steps_digest"] as? String) ?? ""
            let evPath = (obj["evidence_path"] as? String) ?? ""
            summary = """
            \(verdict)

            Baseline passed: \(bp) (held-out: \(bh))
            Candidate passed: \(cp) (held-out: \(ch))
            ATIF-v1.8 steps_digest: \(digest)
            Evidence: \(evPath)
            """
        }

        if promoted {
            window?.syncMcpSkills()
        }
        showResult(
            title: promoted ? "Capability Promoted to Skills" : "Capability Rejected by #authority Gate",
            body: summary,
            ok: promoted
        )
    }

    private func runProcess(_ exe: String, args: [String]) -> (status: Int32, stdout: String, stderr: String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = args
        let outP = Pipe()
        let errP = Pipe()
        p.standardOutput = outP
        p.standardError = errP
        do {
            try p.run()
            p.waitUntilExit()
        } catch {
            return (-1, "", error.localizedDescription)
        }
        let outStr = String(data: outP.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let errStr = String(data: errP.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        return (p.terminationStatus, outStr, errStr)
    }

    private func showResult(title: String, body: String, ok: Bool) {
        let a = NSAlert()
        a.alertStyle = ok ? .informational : .warning
        a.messageText = title
        a.informativeText = body
        a.runModal()
    }
}
