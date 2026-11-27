// Firm reason: 6 (AppKit dialogs/menus and Process execution over page/gitactions.coax; GitHub #13871).
import AppKit

extension CoderWindow {
    /// Refreshes Git repository status, branch list, ahead/behind counts, and worktree status for
    /// the active chat's working folder off the main thread.
    func refreshGitStatus() {
        let dir = folder.path
        let wtEnabled = ChatWorktree.enabled
        cd_git_mode_set(page, wtEnabled ? 1 : 0)
        cd_worktree_set(page, wtEnabled ? 1 : 0)
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let inside = ChatWorktree.git(["rev-parse", "--is-inside-work-tree"], in: dir)
            guard inside.status == 0, inside.output == "true" else {
                DispatchQueue.main.async {
                    guard let self, self.folder.path == dir else { return }
                    cd_git_status(self.page, 0, 0, 0, 0, 0)
                    cd_git_branches(self.page, "")
                    self.wake()
                }
                return
            }
            let branch = CoderWindow.gitBranch(URL(fileURLWithPath: dir))
            let statusOut = ChatWorktree.git(["status", "--porcelain", "--untracked-files=normal"], in: dir)
            let dirty = statusOut.status == 0 ? statusOut.output.split(separator: "\n").filter { !$0.isEmpty }.count : 0
            var ahead = 0, behind = 0
            let ab = ChatWorktree.git(["rev-list", "--left-right", "--count", "@{upstream}...HEAD"], in: dir)
            if ab.status == 0 {
                let parts = ab.output.split(whereSeparator: { $0 == "\t" || $0 == " " })
                if parts.count >= 2 {
                    behind = Int(parts[0]) ?? 0
                    ahead = Int(parts[1]) ?? 0
                }
            }
            let branchesRes = ChatWorktree.git(["branch", "--format=%(refname:short)\t%(if)%(HEAD)%(then)1%(else)0%(end)"], in: dir)
            let branchLines = branchesRes.status == 0 ? branchesRes.output : ""
            let isWorktree = ChatWorktree.mainCheckout(of: dir) == nil && ChatWorktree.repository(of: dir) != nil
            let cloneCount = ChatWorktree.repository(of: dir).map { ChatWorktree.clonePaths(repo: $0).count } ?? 0
            let wtNote = isWorktree
                ? (cloneCount > 0 ? "Isolated worktree · \(cloneCount) CoW hydrated" : "Isolated worktree")
                : "Local checkout"
            DispatchQueue.main.async {
                guard let self, self.folder.path == dir else { return }
                cd_branch(self.page, ChatWorktree.shown(branch))
                cd_git_status(self.page, 1, isWorktree ? 1 : 0, Int32(dirty), Int32(ahead), Int32(behind))
                cd_git_mode_set(self.page, ChatWorktree.enabled ? 1 : 0)
                cd_worktree_set(self.page, ChatWorktree.enabled ? 1 : 0)
                _ = cd_git_branches(self.page, branchLines)
                cd_git_wt_status(self.page, 1, wtNote)
                self.wake()
            }
        }
    }

    /// Dispatches a Git Actions toolbar request from Coax (cd_action 49).
    func gitAsked() {
        let op = cd_git_op(page)
        var anchor = [Float](repeating: 0, count: 2)
        _ = cd_git_anchor(page, &anchor)
        switch op {
        case 1: refreshGitStatus()
        case 2: showBranchMenu(at: anchor)
        case 3: toggleWorktreeMode()
        case 4: showGitActionsMenu(at: anchor)
        case 5: gitCommitFlow()
        case 6: gitPushFlow()
        case 7: gitPullFlow()
        case 8: gitCreatePRFlow()
        case 9: gitWorktreeStatusAndCleanup()
        case 10: gitStopRunning()
        default: break
        }
    }

    /// Toggles Local checkout vs. Worktree mode directly in UI (stays synced with new-chat checkbox).
    private func toggleWorktreeMode() {
        let worktreeOn = cd_git_mode(page) == 1
        ChatWorktree.enabled = worktreeOn
        cd_worktree_set(page, worktreeOn ? 1 : 0)
        for r in runs.values where r.page != page { cd_worktree_set(r.page, worktreeOn ? 1 : 0) }
        if worktreeOn { prewarmWorktree(for: folder) }
        let msg = worktreeOn
            ? "Worktree mode enabled: new chats start in isolated Git worktrees"
            : "Local checkout mode enabled: new chats run directly in the chosen folder"
        cd_toast(page, msg, 0)
        wake()
    }

    /// Interactive Branch picker & creator menu anchored to the toolbar's branch chip.
    private func showBranchMenu(at anchor: [Float]) {
        let dir = folder.path
        let current = CoderWindow.gitBranch(folder)
        let res = ChatWorktree.git(["branch", "--format=%(refname:short)"], in: dir)
        let branches = res.status == 0
            ? res.output.split(separator: "\n").map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
            : []
        typealias Item = CoderOverlay.MenuItem
        var items: [Item] = [Item(label: "Switch Branch", kind: 1)]
        var acts: [() -> Void] = [{}]
        for b in branches.prefix(32) {
            items.append(Item(label: b, checked: b == current))
            acts.append { [weak self] in
                guard let self, b != current else { return }
                self.runGitStreaming(label: "Switching to \(b)…", executable: "/usr/bin/git", args: ["checkout", b], in: dir)
            }
        }
        items.append(Item(kind: 2)); acts.append({})
        items.append(Item(label: "New Branch…"))
        acts.append { [weak self] in self?.promptCreateBranch() }
        items.append(Item(kind: 2)); acts.append({})
        let wtOn = ChatWorktree.enabled
        items.append(Item(label: "Worktree Mode (Isolated per chat)", checked: wtOn))
        acts.append { [weak self] in
            guard let self else { return }
            cd_git_mode_set(self.page, 1)
            self.toggleWorktreeMode()
        }
        items.append(Item(label: "Local Checkout Mode", checked: !wtOn))
        acts.append { [weak self] in
            guard let self else { return }
            cd_git_mode_set(self.page, 0)
            self.toggleWorktreeMode()
        }
        overlay.presentMenu(items, x: anchor[0], y: anchor[1]) { chosen in
            if chosen >= 0 && chosen < acts.count { acts[chosen]() }
        }
    }

    private func promptCreateBranch() {
        let dir = folder.path
        var d = CoderDialog(size: .sm, title: "Create Branch", body: "Create and switch to a new Git branch from HEAD.")
        d.fields = [.init(label: "Branch name", value: "", hint: "feat/my-change")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Create & Switch", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1 else { return }
            let name = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard cd_git_valid_branch(name) != 0 else {
                cd_toast(self.page, "Invalid branch name: \(name)", 1); self.wake(); return
            }
            self.runGitStreaming(label: "Creating \(name)…", executable: "/usr/bin/git", args: ["checkout", "-b", name], in: dir)
        }
    }

    /// Compact Git Actions menu (also includes Worktree status & cleanup and output toggle).
    private func showGitActionsMenu(at anchor: [Float]) {
        typealias Item = CoderOverlay.MenuItem
        let items: [Item] = [
            Item(label: "Git Actions", kind: 1),
            Item(label: "Stage & Commit…"),
            Item(label: "Push"),
            Item(label: "Pull"),
            Item(label: "Create Pull Request…"),
            Item(kind: 2),
            Item(label: "Worktree Status & Cleanup…"),
            Item(label: "Refresh Git Status"),
        ]
        overlay.presentMenu(items, x: anchor[0], y: anchor[1]) { [weak self] chosen in
            guard let self else { return }
            switch chosen {
            case 1: self.gitCommitFlow()
            case 2: self.gitPushFlow()
            case 3: self.gitPullFlow()
            case 4: self.gitCreatePRFlow()
            case 6: self.gitWorktreeStatusAndCleanup()
            case 7: self.refreshGitStatus()
            default: break
            }
        }
    }

    // MARK: - Commit, Push, Pull, PR

    func gitCommitFlow() {
        let dir = folder.path
        let branch = CoderWindow.gitBranch(folder)
        let st = ChatWorktree.git(["status", "--porcelain", "--untracked-files=normal"], in: dir)
        guard st.status == 0, !st.output.isEmpty else {
            cd_toast(page, "Working tree is clean — nothing to commit", 0); wake(); return
        }
        if cd_git_needs_guard(branch, 5) != 0 {
            var d = CoderDialog(size: .sm, title: "Commit directly to \(branch)?",
                                body: "You are on the default branch (\(branch)). Create a feature branch or confirm committing directly.")
            d.buttons = [
                .init(title: "Cancel"),
                .init(title: "New Branch…"),
                .init(title: "Commit to \(branch)", kind: .danger),
            ]
            d.confirm = 0; d.cancel = 0
            present(d) { [weak self] answer, _ in
                guard let self else { return }
                if answer == 1 { self.promptCreateBranch() }
                else if answer == 2 { self.beginAICommitDialog(in: dir, branch: branch) }
                else { self.refocus() }
            }
            return
        }
        beginAICommitDialog(in: dir, branch: branch)
    }

    private func beginAICommitDialog(in dir: String, branch: String) {
        cd_git_busy(page, 1, "Drafting commit…")
        wake()
        generateAICommitDraft(in: dir) { [weak self] subject, body, source in
            guard let self else { return }
            cd_git_busy(self.page, 0, "")
            self.wake()
            self.presentEditableCommitDialog(in: dir, branch: branch, subject: subject, body: body, source: source)
        }
    }

    private func presentEditableCommitDialog(in dir: String, branch: String, subject: String, body: String, source: String) {
        var d = CoderDialog(size: .md, title: "Commit on \(branch)",
                            body: "Stages all changes and runs git commit with live hook output. Draft source: \(source).")
        d.fields = [
            .init(label: "Commit subject (Conventional Commits)", value: subject, hint: "feat(scope): summary"),
            .init(label: "Description (optional)", value: body, hint: "Additional context…", lines: 4),
        ]
        d.buttons = [
            .init(title: "Cancel"),
            .init(title: "Regenerate AI", aside: true),
            .init(title: "Stage All & Commit", kind: .primary),
        ]
        d.confirm = 2; d.cancel = 0; d.focus = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1 {
                self.beginAICommitDialog(in: dir, branch: branch)
                return
            }
            defer { self.refocus() }
            guard answer == 2 else { return }
            let sub = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let desc = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard cd_git_valid_commit(sub) != 0 else {
                cd_toast(self.page, "Commit message cannot be empty", 1); self.wake(); return
            }
            var args = ["commit", "-m", sub]
            if !desc.isEmpty { args.append(contentsOf: ["-m", desc]) }
            self.runGitStreaming(label: "Staging…", executable: "/usr/bin/git", args: ["add", "-A"], in: dir) { ok in
                guard ok else { return }
                self.runGitStreaming(label: "Committing…", executable: "/usr/bin/git", args: args, in: dir)
            }
        }
    }

    func gitPushFlow() {
        let dir = folder.path
        let branch = CoderWindow.gitBranch(folder)
        guard !branch.isEmpty else {
            cd_toast(page, "Cannot push detached HEAD", 1); wake(); return
        }
        if cd_git_needs_guard(branch, 6) != 0 {
            var d = CoderDialog(size: .sm, title: "Push directly to \(branch)?",
                                body: "You are pushing directly to the default branch (\(branch)). Confirm to proceed.")
            d.buttons = [.init(title: "Cancel"), .init(title: "Push to \(branch)", kind: .danger)]
            d.confirm = 0; d.cancel = 0
            present(d) { [weak self] answer, _ in
                guard let self else { return }
                defer { self.refocus() }
                guard answer == 1 else { return }
                self.runGitStreaming(label: "Pushing \(branch)…", executable: "/usr/bin/git", args: ["push", "-u", "origin", branch], in: dir)
            }
            return
        }
        runGitStreaming(label: "Pushing \(branch)…", executable: "/usr/bin/git", args: ["push", "-u", "origin", branch], in: dir)
    }

    func gitPullFlow() {
        let dir = folder.path
        runGitStreaming(label: "Pulling…", executable: "/usr/bin/git", args: ["pull", "--ff-only"], in: dir)
    }

    func gitCreatePRFlow() {
        let dir = folder.path
        let branch = CoderWindow.gitBranch(folder)
        guard !branch.isEmpty else {
            cd_toast(page, "Switch to a branch before creating a PR", 1); wake(); return
        }
        guard let ghPath = Self.resolveGHBinary() else {
            cd_toast(page, "GitHub CLI (gh) not found in PATH — install with brew install gh", 1); wake(); return
        }
        cd_git_busy(page, 1, "Drafting PR…")
        wake()
        generateAIPRDraft(in: dir, branch: branch) { [weak self] title, body, source in
            guard let self else { return }
            cd_git_busy(self.page, 0, "")
            self.wake()
            self.presentEditablePRDialog(in: dir, branch: branch, ghPath: ghPath, title: title, body: body, source: source)
        }
    }

    private func presentEditablePRDialog(in dir: String, branch: String, ghPath: String, title: String, body: String, source: String) {
        var d = CoderDialog(size: .lg, title: "Create Pull Request (\(branch))",
                            body: "Pushes the branch if needed and runs gh pr create. Draft source: \(source).")
        d.fields = [
            .init(label: "PR Title", value: title, hint: "Short summary of changes"),
            .init(label: "PR Description (Markdown)", value: body, hint: "## Summary\n- …", lines: 6),
            .init(label: "Base branch", value: "main", hint: "main"),
        ]
        d.buttons = [
            .init(title: "Cancel"),
            .init(title: "Regenerate AI", aside: true),
            .init(title: "Push & Create PR", kind: .primary),
        ]
        d.confirm = 2; d.cancel = 0; d.focus = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1 {
                self.gitCreatePRFlow()
                return
            }
            defer { self.refocus() }
            guard answer == 2 else { return }
            let prTitle = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let prBody = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            let base = (values.count > 2 ? values[2] : "main").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !prTitle.isEmpty else {
                cd_toast(self.page, "PR title cannot be empty", 1); self.wake(); return
            }
            self.runGitStreaming(label: "Pushing \(branch)…", executable: "/usr/bin/git", args: ["push", "-u", "origin", branch], in: dir) { ok in
                guard ok else { return }
                let prArgs = ["pr", "create", "--title", prTitle, "--body", prBody, "--base", base.isEmpty ? "main" : base]
                self.runGitStreaming(label: "Creating PR…", executable: ghPath, args: prArgs, in: dir) { prOK in
                    if prOK { cd_toast(self.page, "Pull request created", 0); self.wake() }
                }
            }
        }
    }

    // MARK: - Worktree status & cleanup UI

    func gitWorktreeStatusAndCleanup() {
        let dir = folder.path
        let repo = ChatWorktree.repository(of: dir) ?? dir
        let wtList = ChatWorktree.git(["worktree", "list", "--porcelain"], in: repo)
        let clones = ChatWorktree.clonePaths(repo: repo)
        let wtCount = wtList.output.components(separatedBy: "\nworktree ").count
        let mode = ChatWorktree.enabled ? "Worktree (isolated per chat)" : "Local checkout"
        let cloneSummary = clones.isEmpty ? "None configured (.coder/worktree.json)" : clones.joined(separator: ", ")
        let body = "Mode: \(mode)\nRepository: \(repo)\nActive worktrees: \(wtCount)\nCoW hydrate paths: \(cloneSummary)"
        var d = CoderDialog(size: .md, title: "Worktree Status & Cleanup", body: body)
        d.buttons = [
            .init(title: "Close"),
            .init(title: "Prune Stale Worktrees", kind: .primary),
        ]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1 else { return }
            self.runGitStreaming(label: "Pruning worktrees…", executable: "/usr/bin/git", args: ["worktree", "prune", "-v"], in: repo) { ok in
                if ok { cd_toast(self.page, "Pruned stale worktrees", 0); self.wake() }
            }
        }
    }

    // MARK: - Live streaming command execution

    func gitStopRunning() {
        gitProcess?.terminate()
        gitProcess = nil
        cd_git_output_append(page, "\n[stopped by user]\n")
        cd_git_output_done(page, 1)
        wake()
    }

    private func runGitStreaming(label: String, executable: String, args: [String], in dir: String, completion: ((Bool) -> Void)? = nil) {
        if gitProcess?.isRunning == true {
            cd_toast(page, "Another Git command is already running", 1); wake(); return
        }
        cd_git_output_clear(page)
        cd_git_busy(page, 1, label)
        cd_git_output_append(page, "$ \(URL(fileURLWithPath: executable).lastPathComponent) \(args.joined(separator: " "))\n")
        wake()

        let p = Process()
        p.executableURL = URL(fileURLWithPath: executable)
        p.arguments = args
        p.currentDirectoryURL = URL(fileURLWithPath: dir)
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        env["GIT_TERMINAL_PROMPT"] = "0"
        p.environment = env
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        gitProcess = p

        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let chunk = String(data: data, encoding: .utf8) else { return }
            DispatchQueue.main.async {
                guard let self else { return }
                cd_git_output_append(self.page, chunk)
                self.wake()
            }
        }
        p.terminationHandler = { [weak self] proc in
            pipe.fileHandleForReading.readabilityHandler = nil
            let tail = pipe.fileHandleForReading.readDataToEndOfFile()
            let tailStr = String(data: tail, encoding: .utf8) ?? ""
            let ok = proc.terminationStatus == 0
            DispatchQueue.main.async {
                guard let self else { return }
                if self.gitProcess === proc { self.gitProcess = nil }
                if !tailStr.isEmpty { cd_git_output_append(self.page, tailStr) }
                cd_git_output_done(self.page, ok ? 0 : 1)
                self.refreshGitStatus()
                self.wake()
                completion?(ok)
            }
        }
        do {
            try p.run()
        } catch {
            gitProcess = nil
            cd_git_output_append(page, "Failed to launch \(executable): \(error.localizedDescription)\n")
            cd_git_output_done(page, 1)
            wake()
            completion?(false)
        }
    }

    // MARK: - AI Commit & PR Generation

    private func generateAICommitDraft(in dir: String, done: @escaping (String, String, String) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let status = ChatWorktree.git(["status", "--short"], in: dir).output
            let diffStat = ChatWorktree.git(["diff", "--stat", "HEAD"], in: dir).output
            let diffRaw = ChatWorktree.git(["diff", "HEAD"], in: dir).output
            let recent = ChatWorktree.git(["log", "-n", "5", "--oneline"], in: dir).output
            let diffClipped = String(diffRaw.prefix(10000))
            let prompt = """
            Generate a Conventional Commits commit message for the following git changes.
            Match the repository's recent commit style. Return ONLY valid JSON with keys "subject" (<=72 chars, Conventional Commits format like feat(scope): ...) and "body" (short bullet list or empty string).

            Recent commits:
            \(recent)

            git status --short:
            \(status)

            git diff --stat:
            \(diffStat)

            git diff (truncated):
            \(diffClipped)
            """
            Self.callTextModel(prompt: prompt) { aiText, provider in
                if let aiText, let parsed = Self.parseJSONPair(aiText, key1: "subject", key2: "body"), !parsed.0.isEmpty {
                    DispatchQueue.main.async { done(parsed.0, parsed.1, provider) }
                    return
                }
                let fallback = Self.heuristicCommitDraft(status: status, diffStat: diffStat)
                DispatchQueue.main.async { done(fallback.0, fallback.1, "Diff summary (configure Gemini or OpenRouter key for AI)") }
            }
        }
    }

    private func generateAIPRDraft(in dir: String, branch: String, done: @escaping (String, String, String) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let log = ChatWorktree.git(["log", "origin/main..HEAD", "--oneline"], in: dir).output
            let diffStat = ChatWorktree.git(["diff", "--stat", "origin/main...HEAD"], in: dir).output
            let diffRaw = ChatWorktree.git(["diff", "origin/main...HEAD"], in: dir).output
            let diffClipped = String(diffRaw.prefix(10000))
            let prompt = """
            Generate a concise GitHub Pull Request title and Markdown description for branch "\(branch)".
            Return ONLY valid JSON with keys "title" (<=72 chars) and "body" (Markdown with ## Summary and bullet points).

            Commits:
            \(log)

            Diff stat:
            \(diffStat)

            Diff (truncated):
            \(diffClipped)
            """
            Self.callTextModel(prompt: prompt) { aiText, provider in
                if let aiText, let parsed = Self.parseJSONPair(aiText, key1: "title", key2: "body"), !parsed.0.isEmpty {
                    DispatchQueue.main.async { done(parsed.0, parsed.1, provider) }
                    return
                }
                let fallback = Self.heuristicPRDraft(branch: branch, log: log, diffStat: diffStat)
                DispatchQueue.main.async { done(fallback.0, fallback.1, "Branch summary (configure Gemini or OpenRouter key for AI)") }
            }
        }
    }

    /// Calls Gemini API or OpenRouter API when a key is configured to generate commit/PR text.
    private static func callTextModel(prompt: String, done: @escaping (String?, String) -> Void) {
        let savedGemini = CoderGemini.saved
        if let geminiKey = savedGemini ?? ProcessInfo.processInfo.environment["GEMINI_API_KEY"], !geminiKey.isEmpty,
           let url = URL(string: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key=\(geminiKey)") {
            var req = URLRequest(url: url, timeoutInterval: 15)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            let body: [String: Any] = ["contents": [["parts": [["text": prompt]]]]]
            req.httpBody = try? JSONSerialization.data(withJSONObject: body)
            URLSession.shared.dataTask(with: req) { data, _, _ in
                if let data,
                   let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let cands = root["candidates"] as? [[String: Any]],
                   let content = cands.first?["content"] as? [String: Any],
                   let parts = content["parts"] as? [[String: Any]],
                   let text = parts.first?["text"] as? String {
                    done(text, "Gemini AI")
                    return
                }
                done(nil, "")
            }.resume()
            return
        }
        if let orKey = CoderOpenRouter.saved, !orKey.isEmpty,
           let url = URL(string: "https://openrouter.ai/api/v1/chat/completions") {
            var req = URLRequest(url: url, timeoutInterval: 15)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.setValue("Bearer \(orKey)", forHTTPHeaderField: "Authorization")
            let model = CoderOpenRouter.main.isEmpty ? "google/gemini-2.5-flash" : CoderOpenRouter.main
            let body: [String: Any] = ["model": model, "messages": [["role": "user", "content": prompt]]]
            req.httpBody = try? JSONSerialization.data(withJSONObject: body)
            URLSession.shared.dataTask(with: req) { data, _, _ in
                if let data,
                   let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let choices = root["choices"] as? [[String: Any]],
                   let msg = choices.first?["message"] as? [String: Any],
                   let text = msg["content"] as? String {
                    done(text, "OpenRouter AI")
                    return
                }
                done(nil, "")
            }.resume()
            return
        }
        done(nil, "")
    }

    private static func parseJSONPair(_ raw: String, key1: String, key2: String) -> (String, String)? {
        guard let start = raw.firstIndex(of: "{"), let end = raw.lastIndex(of: "}") else { return nil }
        let slice = String(raw[start...end])
        guard let data = slice.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let v1 = (obj[key1] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let v2 = (obj[key2] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return (v1, v2)
    }

    private static func heuristicCommitDraft(status: String, diffStat: String) -> (String, String) {
        let files = status.split(separator: "\n").compactMap { line -> String? in
            let trimmed = line.dropFirst(3).trimmingCharacters(in: .whitespaces)
            return trimmed.isEmpty ? nil : trimmed
        }
        let scope = files.first?.split(separator: "/").first.map(String.init) ?? "repo"
        let subject = files.count == 1
            ? "chore(\(scope)): update \(URL(fileURLWithPath: files[0]).lastPathComponent)"
            : "chore(\(scope)): update \(files.count) files"
        return (subject, diffStat)
    }

    private static func heuristicPRDraft(branch: String, log: String, diffStat: String) -> (String, String) {
        let firstCommit = log.split(separator: "\n").first.map { line -> String in
            let parts = line.split(separator: " ", maxSplits: 1)
            return parts.count > 1 ? String(parts[1]) : String(line)
        } ?? "Changes from \(branch)"
        var body = "## Summary\n"
        if !log.isEmpty {
            for line in log.split(separator: "\n").prefix(10) { body += "- \(line)\n" }
        } else {
            body += "- Updates on branch `\(branch)`\n"
        }
        if !diffStat.isEmpty { body += "\n## Diff\n```\n\(diffStat)\n```" }
        return (firstCommit, body)
    }

    private static func resolveGHBinary() -> String? {
        for dir in CoderWorker.path.split(separator: ":") {
            let candidate = "\(dir)/gh"
            if FileManager.default.isExecutableFile(atPath: candidate) { return candidate }
        }
        for p in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"] {
            if FileManager.default.isExecutableFile(atPath: p) { return p }
        }
        return nil
    }
}
