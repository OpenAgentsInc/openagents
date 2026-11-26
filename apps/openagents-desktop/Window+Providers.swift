// Firm reason: 6 (AppKit dialogs, filesystem registry persistence, and worker environment wiring over page/providers.coax; GitHub #13878).
import AppKit

/// Detection & status probing for Codex CLI, Cursor ACP CLI, and user-registered ACP binaries.
enum ExternalProviders {
    static let providerCloud: Int32 = 0
    static let providerAgy: Int32 = 1
    static let providerInkling: Int32 = 2
    static let providerCodex: Int32 = 3
    static let providerCursor: Int32 = 4
    static let providerAcp: Int32 = 5
    static let providerClaude: Int32 = 7   // a chat on a Claude model: Claude Code, through claudeworker
    static let providerFlawed: Int32 = 8   // native pure-Coax Flawed Agent, through flawedworker

    static var registryURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/acp-registry.json")
    }

    static func findExecutable(_ names: [String]) -> String? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        var dirs = CoderWorker.path.split(separator: ":").map(String.init)
        for extra in [home + "/.local/bin", "/opt/homebrew/bin", "/usr/local/bin", home + "/.cursor/bin"] {
            if !dirs.contains(extra) { dirs.append(extra) }
        }
        for name in names {
            for dir in dirs {
                let candidate = dir + "/" + name
                if fm.isExecutableFile(atPath: candidate) { return candidate }
            }
        }
        return nil
    }

    static func codexBinaryPath() -> String? {
        if let env = ProcessInfo.processInfo.environment["CODER_CODEX_BIN"], !env.isEmpty,
           FileManager.default.isExecutableFile(atPath: (env as NSString).expandingTildeInPath) {
            return (env as NSString).expandingTildeInPath
        }
        return findExecutable(["codex"])
    }

    static func cursorBinaryPath() -> String? {
        if let env = ProcessInfo.processInfo.environment["CODER_CURSOR_BIN"], !env.isEmpty,
           FileManager.default.isExecutableFile(atPath: (env as NSString).expandingTildeInPath) {
            return (env as NSString).expandingTildeInPath
        }
        return findExecutable(["agent", "cursor-agent"])
    }

    static func codexStatus() -> (installed: Bool, status: String, auth: String) {
        guard let bin = codexBinaryPath() else {
            return (false, "Not installed", "Install Codex CLI (`npm i -g @openai/codex`) and run `codex login`.")
        }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hasAuthFile = FileManager.default.fileExists(atPath: home + "/.codex/auth.json")
        let hasKey = !(ProcessInfo.processInfo.environment["OPENAI_API_KEY"] ?? "").isEmpty
        let auth = hasAuthFile ? "Signed in (~/.codex/auth.json)" : (hasKey ? "OPENAI_API_KEY set" : "Run `codex login` to sign in")
        return (true, bin, auth)
    }

    static func cursorStatus() -> (installed: Bool, status: String, auth: String) {
        guard let bin = cursorBinaryPath() else {
            return (false, "Not installed", "Install Cursor CLI from cursor.com/install (`agent acp`).")
        }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hasCursorDir = FileManager.default.fileExists(atPath: home + "/.cursor")
        let hasKey = !(ProcessInfo.processInfo.environment["CURSOR_API_KEY"] ?? "").isEmpty
        let auth = hasKey ? "CURSOR_API_KEY set" : (hasCursorDir ? "Cursor profile detected (~/.cursor)" : "Run `agent login` to sign in")
        return (true, bin, auth)
    }

    static func loadRegistryJSON() -> String? {
        guard let data = try? Data(contentsOf: registryURL),
              let text = String(data: data, encoding: .utf8), !text.isEmpty else { return nil }
        return text
    }

    @discardableResult
    static func saveRegistryJSON(_ json: String) -> Bool {
        let fm = FileManager.default
        let dir = registryURL.deletingLastPathComponent()
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        do {
            try Data(json.utf8).write(to: registryURL, options: .atomic)
            return true
        } catch {
            return false
        }
    }

    static func providerDisplayName(_ kind: Int32) -> String {
        switch kind {
        case providerAgy: return "Antigravity"
        case providerInkling: return "Inkling 2 Mini"
        case providerCodex: return "Codex"
        case providerCursor: return "Cursor"
        case providerAcp: return "ACP Agent"
        case providerClaude: return "Claude"
        case providerFlawed: return "OpenAgents"
        default: return "OpenAgents"
        }
    }
}

extension CoderWindow {
    /// Syncs Codex, Cursor, and Custom ACP Registry availability onto a single page.
    func syncExternalProviders(on targetPage: UnsafeMutableRawPointer) {
        let cx = ExternalProviders.codexStatus()
        cd_set_codex(targetPage, cx.installed ? 1 : 0, cx.status, cx.auth)
        let cu = ExternalProviders.cursorStatus()
        cd_set_cursor(targetPage, cu.installed ? 1 : 0, cu.status, cu.auth)
        if let json = ExternalProviders.loadRegistryJSON() {
            _ = cd_acp_load(targetPage, json)
        }
    }

    /// Syncs Codex, Cursor, and Custom ACP Registry across the foreground page and all background runs.
    func syncExternalProvidersAll() {
        syncExternalProviders(on: page)
        for r in runs.values where r.page != page {
            syncExternalProviders(on: r.page)
        }
        wake()
    }

    /// Persists the current ACP Registry entries from `page` to `~/.openagents/desktop/acp-registry.json`
    /// and mirrors them to all background runs.
    private func persistAcpRegistryFromPage() {
        var buf = [CChar](repeating: 0, count: 16384)
        let n = cd_acp_save(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let json = String(cString: buf)
        _ = ExternalProviders.saveRegistryJSON(json)
        for r in runs.values where r.page != page {
            _ = cd_acp_load(r.page, json)
        }
    }

    /// Settings → Models → Codex CLI status dialog (CP_ASK_CODEX).
    func showCodexSettings() {
        let st = ExternalProviders.codexStatus()
        let body = st.installed
            ? "Binary: \(st.status)\nAuth: \(st.auth)\n\nCodex is available in the composer model picker (⌘/)."
            : "Codex CLI (`codex`) was not found on PATH.\n\n\(st.auth)"
        var d = CoderDialog(size: .md, title: "Codex CLI", body: body)
        d.buttons = [.init(title: "Re-check"), .init(title: "Done", kind: .primary)]
        d.cancel = 1; d.confirm = 1
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 0 { self.syncExternalProvidersAll() }
            self.refocus()
        }
    }

    /// Settings → Models → Cursor ACP CLI status dialog (CP_ASK_CURSOR).
    func showCursorSettings() {
        let st = ExternalProviders.cursorStatus()
        let body = st.installed
            ? "Binary: \(st.status)\nAuth: \(st.auth)\n\nCursor ACP (`agent acp`) is available in the composer model picker (⌘/)."
            : "Cursor ACP CLI (`agent` / `cursor-agent`) was not found on PATH.\n\n\(st.auth)"
        var d = CoderDialog(size: .md, title: "Cursor ACP CLI", body: body)
        d.buttons = [.init(title: "Re-check"), .init(title: "Done", kind: .primary)]
        d.cancel = 1; d.confirm = 1
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 0 { self.syncExternalProvidersAll() }
            self.refocus()
        }
    }

    /// Settings → Models → ACP Registry Add/Remove dialog (CP_ASK_ACP_REGISTRY).
    func manageAcpRegistry() {
        var buf = [CChar](repeating: 0, count: 16384)
        let n = cd_acp_save(page, &buf, Int32(buf.count))
        let currentJSON = n > 0 ? String(cString: buf) : "[]"
        let count = cd_acp_count(page)
        let body = "Register any ACP-compliant CLI agent (e.g. OpenCode, Kiro, custom ACP binaries) into the model picker. Stored in ~/.openagents/desktop/acp-registry.json.\n\nCurrently registered (\(count)/8): \(currentJSON)"
        var d = CoderDialog(size: .lg, title: "ACP Registry", body: body)
        d.fields = [
            .init(label: "Agent Name (to add) or ID (to remove)", value: "", hint: "OpenCode"),
            .init(label: "Program Path", value: "", hint: "/usr/local/bin/opencode"),
            .init(label: "Arguments (optional)", value: "acp", hint: "acp")
        ]
        d.buttons = [
            .init(title: "Remove by ID", kind: .danger, aside: true),
            .init(title: "Cancel"),
            .init(title: "Add Agent", kind: .primary)
        ]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            let nameOrId = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let program = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let args = (values.count > 2 ? values[2] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            if answer == 2 {
                if !nameOrId.isEmpty && !program.isEmpty {
                    let expanded = (program as NSString).expandingTildeInPath
                    if cd_acp_add(self.page, "", nameOrId, expanded, args, 1) != 0 {
                        self.persistAcpRegistryFromPage()
                        cd_toast(self.page, "Added ACP agent \(nameOrId)", 0)
                    } else {
                        cd_toast(self.page, "Could not add ACP agent (max 8 entries).", 1)
                    }
                }
            } else if answer == 0 {
                if !nameOrId.isEmpty {
                    if cd_acp_remove(self.page, nameOrId) != 0 {
                        self.persistAcpRegistryFromPage()
                        cd_toast(self.page, "Removed ACP agent \(nameOrId)", 0)
                    } else {
                        cd_toast(self.page, "ACP agent '\(nameOrId)' not found.", 1)
                    }
                }
            }
            self.syncSettingsInfo()
            self.refocus()
        }
    }

    /// Composer → Fan-out chip dialog (CP_ASK_FANOUT_PICK): select 2..4 model IDs for parallel fan-out.
    func openFanoutPicker() {
        let n = Int(cd_fanout_count(page))
        var current: [String] = []
        var tb = [CChar](repeating: 0, count: 128)
        for i in 0..<n {
            if cd_fanout_target(page, Int32(i), &tb, Int32(tb.count)) > 0 {
                current.append(String(cString: tb))
            }
        }
        let defaultValue = current.isEmpty ? "gemini-4-neon, codex" : current.joined(separator: ", ")
        let body = "Enter 2 to 4 comma-separated model IDs to run in parallel side-by-side chats on your next prompt (e.g. gemini-4-neon, gemini-4-carbon, antigravity, codex, cursor, acp:<id>)."
        var d = CoderDialog(size: .md, title: "Multi-Model Fan-Out", body: body)
        d.fields = [.init(label: "Target Models (2–4)", value: defaultValue, hint: "gemini-4-neon, codex")]
        d.buttons = [
            .init(title: "Clear Fan-Out", kind: .danger, aside: true),
            .init(title: "Cancel"),
            .init(title: "Enable Fan-Out", kind: .primary)
        ]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                cd_fanout_clear(self.page)
                cd_toast(self.page, "Multi-model fan-out cleared.", 0)
            } else if answer == 2 {
                let raw = values.first ?? ""
                let ids = raw.split(separator: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
                cd_fanout_clear(self.page)
                var added = 0
                for id in ids.prefix(4) {
                    if cd_fanout_toggle(self.page, id) != 0 { added += 1 }
                }
                if added >= 2 {
                    cd_fanout_set_active(self.page, 1)
                    cd_toast(self.page, "Fan-out armed for \(added) models.", 0)
                } else {
                    cd_fanout_clear(self.page)
                    cd_toast(self.page, "Select at least 2 models for fan-out.", 1)
                }
            }
            self.refocus()
        }
    }

    /// Configures `r.worker` program and environment for the currently selected provider on `r.page`.
    /// Returns the desired worker program name and whether a cross-provider budgeted handoff summary
    /// should be injected before the task line.
    func resolveProviderWorker(_ r: CoderRun) -> (program: String, providerKind: Int32, handoffJSON: String?) {
        let kind = cd_model_provider(r.page)
        let prevKind = r.lastProviderKind
        var program = "mcworker"
        switch kind {
        case ExternalProviders.providerAgy:
            program = "agyworker"
        case ExternalProviders.providerCursor:
            program = "agyworker"
            if let bin = ExternalProviders.cursorBinaryPath() {
                r.worker.extraEnv["CODER_AGY_BIN"] = bin
            }
            r.worker.extraEnv.removeValue(forKey: "CODER_AGY_PROFILE")
            r.worker.extraEnv.removeValue(forKey: "CODER_AGY_PROFILE_DIR")
            r.worker.extraEnv["CODER_ACP_PROVIDER"] = "cursor"
        case ExternalProviders.providerAcp:
            program = "agyworker"
            var pbuf = [CChar](repeating: 0, count: 512)
            var abuf = [CChar](repeating: 0, count: 256)
            if cd_model_acp_program(r.page, &pbuf, Int32(pbuf.count)) > 0 {
                r.worker.extraEnv["CODER_AGY_BIN"] = String(cString: pbuf)
            }
            if cd_model_acp_args(r.page, &abuf, Int32(abuf.count)) > 0 {
                r.worker.extraEnv["CODER_ACP_ARGS"] = String(cString: abuf)
            } else {
                r.worker.extraEnv.removeValue(forKey: "CODER_ACP_ARGS")
            }
            r.worker.extraEnv.removeValue(forKey: "CODER_AGY_PROFILE")
            r.worker.extraEnv.removeValue(forKey: "CODER_AGY_PROFILE_DIR")
            r.worker.extraEnv["CODER_ACP_PROVIDER"] = "acp"
        case ExternalProviders.providerClaude:
            program = "claudeworker"
            r.worker.extraEnv.removeValue(forKey: "CODER_PROVIDER")
        case ExternalProviders.providerFlawed:
            program = "oa-worker"
            r.worker.extraEnv.removeValue(forKey: "CODER_PROVIDER")
        case ExternalProviders.providerInkling:
            program = "inky"
            r.worker.extraEnv.removeValue(forKey: "CODER_PROVIDER")
        case ExternalProviders.providerCodex:
            program = "mcworker"
            if let bin = ExternalProviders.codexBinaryPath() {
                r.worker.extraEnv["CODER_CODEX_BIN"] = bin
            }
            r.worker.extraEnv["CODER_PROVIDER"] = "codex"
        default:
            program = "mcworker"
            r.worker.extraEnv.removeValue(forKey: "CODER_PROVIDER")
        }

        var handoffJSON: String? = nil
        if prevKind >= 0 && cd_handoff_needed(r.page, prevKind, kind) != 0 {
            let fromName = ExternalProviders.providerDisplayName(prevKind)
            let toName = ExternalProviders.providerDisplayName(kind)
            var hbuf = [CChar](repeating: 0, count: 16384)
            let hn = cd_handoff_compose(r.page, fromName, toName, 8192, &hbuf, Int32(hbuf.count))
            if hn > 0 {
                let summary = String(cString: hbuf)
                let entry: [[String: String]] = [["task": "[Cross-provider handoff context]", "reply": summary]]
                if let data = try? JSONSerialization.data(withJSONObject: ["recall": entry]),
                   let line = String(data: data, encoding: .utf8) {
                    handoffJSON = line + "\n"
                }
            }
        }
        return (program, kind, handoffJSON)
    }

    /// Multi-model fan-out handler (CP_ASK_FANOUT): spawns parallel linked chats across the selected
    /// target models/providers from one composer submission.
    func runMultiModelFanout() {
        var pbuf = [CChar](repeating: 0, count: 8192)
        let pn = cd_fanout_prompt(page, &pbuf, Int32(pbuf.count))
        guard pn > 0 else { return }
        let prompt = String(cString: pbuf)
        let targetCount = Int(cd_fanout_count(page))
        guard targetCount >= 2 else { return }
        var targets: [String] = []
        var tbuf = [CChar](repeating: 0, count: 128)
        for i in 0..<targetCount {
            if cd_fanout_target(page, Int32(i), &tbuf, Int32(tbuf.count)) > 0 {
                targets.append(String(cString: tbuf))
            }
        }
        guard targets.count >= 2 else { return }
        // Clear active fan-out so the spawned turns submit normally without recursing.
        cd_fanout_clear(page)

        var spawned = 0
        for (idx, modelID) in targets.enumerated() {
            guard let back = cd_headless() else { continue }
            AgentSettings.apply(to: back)
            syncExternalProviders(on: back)
            coderApplyOpenRouter(back)
            coderApplyClaude(back)
            syncLocalState(on: back)
            if ComposerModelVisibility.showOtherModels { syncAntigravityModels(on: back) }
            let execSnap = ExecutorModelStore.scan()
            cd_settings_executor(back, execSnap.status, execSnap.lineage, execSnap.badge)
            cd_hybrid_exec(back, execSnap.selectable ? 1 : 0)
            cd_experimental_set(back, Experimental.on ? 1 : 0)
            cd_cloud_authed_set(back, (Experimental.on && MacAccount.shared.loggedIn) ? 1 : 0)
            cd_settings_gemini(back, CoderGemini.saved.map { "Ends " + String($0.suffix(4)) } ?? "")
            CoderProbes.apply(back)
            syncSkillsNote(for: back, isForeman: false)
            _ = cd_pick_model(back, modelID)
            guard cd_set_text(back, prompt) != 0, cd_submit(back) != 0 else {
                cd_close(back)
                continue
            }
            _ = cd_action(back)   // consume CP_ASK_SEND
            let newID = UUID().uuidString.lowercased()
            let shortPrompt = prompt.prefix(44)
            let title = "[\(modelID)] \(shortPrompt)"
            var sbuf = [CChar](repeating: 0, count: 262144)
            if cd_save(back, &sbuf, Int32(sbuf.count)) > 0 {
                store.keep(id: newID, title: title, folder: folder.path, text: String(cString: sbuf))
            }
            let r = CoderRun(page: back, folder: folder)
            r.chat = newID
            configureWorkerEnv(r, isForeman: false)
            wire(r)
            runs[newID] = r
            sendTask(r)
            spawned += 1
            if idx == 0 {
                // Keep sidebar fresh and show the first fan-out chat immediately if current chat is empty.
                cd_chats(page, chatLines)
            }
        }
        cd_chats(page, chatLines)
        if spawned > 0 {
            cd_toast(page, "Started \(spawned) parallel chats for multi-model fan-out.", 0)
        }
        wake()
    }
}
