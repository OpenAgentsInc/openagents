// Firm reason: 6 (AppKit dialogs and git plumbing Process execution over page/checkpoint.coax; GitHub #13872).
import AppKit

extension CoderWindow {
    private static let checkpointQueue = DispatchQueue(label: "com.openagents.desktop.checkpoints", qos: .utility)

    // MARK: - Coax ref naming helpers

    private func checkpointRef(_ chatID: String, turn: Int) -> String {
        var buf = [CChar](repeating: 0, count: 256)
        let n = cd_ckpt_ref(chatID, Int32(turn), &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    private func checkpointBaseRef(_ chatID: String) -> String {
        var buf = [CChar](repeating: 0, count: 256)
        let n = cd_ckpt_base_ref(chatID, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    private func checkpointPrefix(_ chatID: String) -> String {
        var buf = [CChar](repeating: 0, count: 256)
        let n = cd_ckpt_prefix(chatID, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    private func checkpointDiffFrom(_ chatID: String, turn: Int) -> String {
        var buf = [CChar](repeating: 0, count: 256)
        let n = cd_ckpt_diff_from(page, chatID, Int32(turn), &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    // MARK: - Low-level git plumbing (temp GIT_INDEX_FILE, never mutates user index/HEAD/reflog)

    @discardableResult
    private static func runGitRaw(_ args: [String], in dir: String, extraEnv: [String: String] = [:]) -> (status: Int32, data: Data) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        p.arguments = args
        p.currentDirectoryURL = URL(fileURLWithPath: dir)
        var env = ProcessInfo.processInfo.environment
        env["GIT_TERMINAL_PROMPT"] = "0"
        for (k, v) in extraEnv { env[k] = v }
        p.environment = env
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        do { try p.run() } catch { return (-1, Data()) }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return (p.terminationStatus, data)
    }

    private static func runGitTrimmed(_ args: [String], in dir: String, extraEnv: [String: String] = [:]) -> (status: Int32, output: String) {
        let r = runGitRaw(args, in: dir, extraEnv: extraEnv)
        let text = (String(data: r.data, encoding: .utf8) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return (r.status, text)
    }

    private static func isGitRepo(_ dir: String) -> Bool {
        let r = runGitTrimmed(["rev-parse", "--is-inside-work-tree"], in: dir)
        return r.status == 0 && r.output == "true"
    }

    /// Captures the entire working tree (tracked + untracked, respecting .gitignore) into a
    /// lightweight commit object referenced by `ref` using a temporary GIT_INDEX_FILE.
    /// Does not modify `.git/index`, `HEAD`, user branches, or reflogs.
    @discardableResult
    private static func captureSnapshot(ref: String, message: String, in dir: String) -> Bool {
        guard !ref.isEmpty, isGitRepo(dir) else { return false }
        let tmpIndex = (NSTemporaryDirectory() as NSString).appendingPathComponent("oa-ckpt-idx-\(UUID().uuidString)")
        try? FileManager.default.removeItem(atPath: tmpIndex)
        defer { try? FileManager.default.removeItem(atPath: tmpIndex) }

        let idxEnv: [String: String] = [
            "GIT_INDEX_FILE": tmpIndex,
            "GIT_AUTHOR_NAME": "OpenAgents",
            "GIT_AUTHOR_EMAIL": "checkpoint@openagents.com",
            "GIT_COMMITTER_NAME": "OpenAgents",
            "GIT_COMMITTER_EMAIL": "checkpoint@openagents.com"
        ]
        let addRes = runGitTrimmed(["add", "-A"], in: dir, extraEnv: idxEnv)
        guard addRes.status == 0 else { return false }

        let treeRes = runGitTrimmed(["write-tree"], in: dir, extraEnv: idxEnv)
        guard treeRes.status == 0, !treeRes.output.isEmpty else { return false }

        let commitRes = runGitTrimmed(["commit-tree", treeRes.output, "-m", message], in: dir, extraEnv: idxEnv)
        guard commitRes.status == 0, !commitRes.output.isEmpty else { return false }

        let refRes = runGitTrimmed(["update-ref", "--no-deref", ref, commitRes.output], in: dir)
        return refRes.status == 0
    }

    // MARK: - Lifecycle hooks (turn start, turn end, chat open, chat delete)

    /// Captures the pre-turn-0 baseline checkpoint (`refs/openagents/checkpoints/<chat>/base`)
    /// once per chat when a turn starts in a git repository.
    func captureBaseCheckpointIfNeeded(for r: CoderRun) {
        let shown = r === run
        if shown && chat == nil && !r.preparingWorktree { keep() }
        guard let chatID = shown ? chat : r.chat, !chatID.isEmpty else { return }
        let dir = (shown ? folder : r.folder).path
        let baseRef = checkpointBaseRef(chatID)
        guard !baseRef.isEmpty else { return }

        Self.checkpointQueue.async { [weak self] in
            guard Self.isGitRepo(dir) else { return }
            let exists = Self.runGitTrimmed(["rev-parse", "--verify", "--quiet", baseRef], in: dir)
            let ok: Bool
            if exists.status == 0 {
                ok = true
            } else {
                ok = Self.captureSnapshot(ref: baseRef, message: "openagents checkpoint \(chatID)/base", in: dir)
            }
            guard ok else { return }
            DispatchQueue.main.async {
                guard let self, self.chat == chatID else { return }
                cd_ckpt_set_base(self.page, 1)
                self.wake()
            }
        }
    }

    /// Records a checkpoint ref for the just-completed turn covering tracked and untracked files.
    func captureTurnCheckpoint(for r: CoderRun) {
        let shown = r === run
        guard let chatID = shown ? chat : r.chat, !chatID.isEmpty else { return }
        let turnIdx = Int(cd_ckpt_nturns(r.page)) - 1
        guard turnIdx >= 0 else { return }
        let dir = (shown ? folder : r.folder).path
        let ref = checkpointRef(chatID, turn: turnIdx)
        guard !ref.isEmpty else { return }

        Self.checkpointQueue.async { [weak self] in
            let ok = Self.captureSnapshot(ref: ref, message: "openagents checkpoint \(chatID)/\(turnIdx)", in: dir)
            guard ok else { return }
            DispatchQueue.main.async {
                guard let self, self.chat == chatID else { return }
                cd_ckpt_mark(self.page, Int32(turnIdx), 1)
                self.wake()
            }
        }
    }

    /// Loads existing checkpoint refs for `chatID` in `dir` and syncs the page's per-turn badges.
    func refreshCheckpoints(for chatID: String, in dir: String) {
        guard !chatID.isEmpty else { return }
        let prefix = checkpointPrefix(chatID)
        guard !prefix.isEmpty else { return }

        Self.checkpointQueue.async { [weak self] in
            guard Self.isGitRepo(dir) else { return }
            let res = Self.runGitTrimmed(["for-each-ref", "--format=%(refname)", prefix], in: dir)
            let refs = res.status == 0 ? res.output : ""
            DispatchQueue.main.async {
                guard let self, self.chat == chatID else { return }
                _ = cd_ckpt_sync(self.page, chatID, refs)
                self.wake()
            }
        }
    }

    /// Deletes all checkpoint refs under `refs/openagents/checkpoints/<chat>/` when a chat is deleted.
    func cleanupCheckpoints(for chatID: String, in dir: String?) {
        guard let dir, !dir.isEmpty, !chatID.isEmpty else { return }
        let prefix = checkpointPrefix(chatID)
        guard !prefix.isEmpty else { return }

        Self.checkpointQueue.async {
            guard Self.isGitRepo(dir) else { return }
            let res = Self.runGitTrimmed(["for-each-ref", "--format=%(refname)", prefix], in: dir)
            guard res.status == 0, !res.output.isEmpty else { return }
            for line in res.output.split(separator: "\n") {
                let ref = String(line).trimmingCharacters(in: .whitespacesAndNewlines)
                guard ref.hasPrefix(prefix) else { continue }
                _ = Self.runGitTrimmed(["update-ref", "-d", ref], in: dir)
            }
        }
    }

    // MARK: - Host action dispatch (cd_action 53)

    func checkpointAsked() {
        let op = cd_ckpt_op(page)
        let turn = Int(cd_ckpt_target(page))
        switch op {
        case 1: showTurnCheckpointDiff(turn: turn)
        case 2: promptAndRewindToTurn(turn: turn)
        default: break
        }
    }

    /// Computes `git diff <from_ref> <turn_ref>` and displays it in the Changes/Review pane.
    private func showTurnCheckpointDiff(turn: Int) {
        guard let chatID = chat, turn >= 0 else { return }
        let dir = folder.path
        let toRef = checkpointRef(chatID, turn: turn)
        let fromRef = checkpointDiffFrom(chatID, turn: turn)
        guard !toRef.isEmpty, !fromRef.isEmpty else { return }
        let label = "Turn \(turn + 1) checkpoint diff"

        Self.checkpointQueue.async { [weak self] in
            let res = Self.runGitRaw(["diff", "--no-color", "--no-ext-diff", fromRef, toRef], in: dir)
            DispatchQueue.main.async {
                guard let self, self.chat == chatID else { return }
                guard res.status == 0 else {
                    cd_toast(self.page, "Could not load checkpoint diff for Turn \(turn + 1).", 1)
                    self.wake()
                    return
                }
                res.data.withUnsafeBytes { raw in
                    _ = cd_ckpt_diff_set(self.page, Int32(turn), raw.bindMemory(to: CChar.self).baseAddress, Int32(res.data.count), label)
                }
                self.wake()
            }
        }
    }

    /// Counts other active background runs sharing the same working-tree checkout as `dir` (#14026).
    private func sharedActiveCheckoutCount(for dir: String, excluding chatID: String) -> Int {
        var count = 0
        for (otherID, back) in runs {
            if otherID == chatID { continue }
            if cd_running(back.page) == 0 { continue }
            if cd_ckpt_same_checkout(dir, back.folder.path) != 0 { count += 1 }
        }
        return count
    }

    /// Preflight #authority CheckpointRestoreSafety check before touching files (#14026).
    private func verifyCheckpointRestoreSafety(chatID: String, turn: Int, in dir: String) -> Bool {
        let shared = sharedActiveCheckoutCount(for: dir, excluding: chatID)
        let canRollback = store.text(chatID) != nil
        let verdict = cd_ckpt_restore_safety(page, Int32(turn), Int32(shared), canRollback ? 1 : 0)
        switch verdict {
        case 0:
            return true
        case 1:
            cd_toast(page, "Cannot revert while a turn is running.", 1)
        case 3:
            cd_toast(page, "Cannot revert: another active chat is using this checkout.", 1)
        case 4:
            cd_toast(page, "Cannot revert: conversation history cannot be rolled back.", 1)
        default:
            cd_toast(page, "Checkpoint for Turn \(turn + 1) cannot be restored.", 1)
        }
        wake()
        return false
    }

    /// Checks for uncommitted working-tree changes or later turns and confirms before rewinding.
    private func promptAndRewindToTurn(turn: Int) {
        guard cd_running(page) == 0, let chatID = chat, turn >= 0 else { return }
        let dir = folder.path
        let nturns = Int(cd_ckpt_nturns(page))
        guard turn < nturns else { return }
        guard verifyCheckpointRestoreSafety(chatID: chatID, turn: turn, in: dir) else { return }
        let turnsDropped = max(0, nturns - 1 - turn)

        Self.checkpointQueue.async { [weak self] in
            let st = Self.runGitTrimmed(["status", "--porcelain", "--untracked-files=normal"], in: dir)
            let dirty = st.status == 0 ? st.output.split(separator: "\n").filter { !$0.isEmpty }.count : 0
            DispatchQueue.main.async {
                guard let self, self.chat == chatID, cd_running(self.page) == 0 else { return }
                if cd_ckpt_needs_confirm(Int32(dirty), Int32(turnsDropped)) != 0 {
                    var parts: [String] = []
                    if dirty > 0 {
                        parts.append(dirty == 1
                            ? "1 uncommitted or untracked file in the working tree will be overwritten or removed."
                            : "\(dirty) uncommitted or untracked files in the working tree will be overwritten or removed.")
                    }
                    if turnsDropped > 0 {
                        parts.append(turnsDropped == 1
                            ? "1 later conversation turn will be removed."
                            : "\(turnsDropped) later conversation turns will be removed.")
                    }
                    var d = CoderDialog(
                        title: "Revert to Turn \(turn + 1)?",
                        body: parts.joined(separator: " ") + " This restores the working tree and chat history to the end of Turn \(turn + 1)."
                    )
                    d.buttons = [.init(title: "Cancel"), .init(title: "Revert", kind: .danger)]
                    d.confirm = 0; d.cancel = 0
                    self.present(d) { [weak self] answer, _ in
                        guard let self else { return }
                        defer { self.refocus() }
                        guard answer == 1 else { return }
                        self.performRewind(chatID: chatID, turn: turn, in: dir)
                    }
                } else {
                    self.performRewind(chatID: chatID, turn: turn, in: dir)
                }
            }
        }
    }

    /// Restores the working tree to `refs/openagents/checkpoints/<chat>/<turn>` via a temporary
    /// GIT_INDEX_FILE, truncates conversation history to `turn`, cleans up orphaned refs for
    /// dropped turns, and resets worker recall so the next prompt sees the rewound history.
    private func performRewind(chatID: String, turn: Int, in dir: String) {
        guard verifyCheckpointRestoreSafety(chatID: chatID, turn: turn, in: dir) else { return }
        let targetRef = checkpointRef(chatID, turn: turn)
        let prefix = checkpointPrefix(chatID)
        guard !targetRef.isEmpty else { return }

        Self.checkpointQueue.async { [weak self] in
            let verify = Self.runGitTrimmed(["rev-parse", "--verify", "--quiet", targetRef], in: dir)
            guard verify.status == 0 else {
                DispatchQueue.main.async {
                    guard let self else { return }
                    cd_toast(self.page, "Checkpoint for Turn \(turn + 1) was not found.", 1)
                    self.wake()
                }
                return
            }

            // Use a temporary GIT_INDEX_FILE seeded with the current working tree (tracked + untracked)
            // so `read-tree --reset -u <targetRef>` removes files added after `turn` and restores
            // tracked + untracked files from `turn` without touching `.git/index` or `HEAD`.
            let tmpIndex = (NSTemporaryDirectory() as NSString).appendingPathComponent("oa-ckpt-rw-\(UUID().uuidString)")
            try? FileManager.default.removeItem(atPath: tmpIndex)
            defer { try? FileManager.default.removeItem(atPath: tmpIndex) }

            let idxEnv = ["GIT_INDEX_FILE": tmpIndex]
            let addCur = Self.runGitTrimmed(["add", "-A"], in: dir, extraEnv: idxEnv)
            guard addCur.status == 0 else {
                DispatchQueue.main.async {
                    guard let self else { return }
                    cd_toast(self.page, "Could not snapshot current working tree before revert.", 1)
                    self.wake()
                }
                return
            }
            let rt = Self.runGitTrimmed(["read-tree", "--reset", "-u", targetRef], in: dir, extraEnv: idxEnv)
            guard rt.status == 0 else {
                DispatchQueue.main.async {
                    guard let self else { return }
                    cd_toast(self.page, "Failed to restore working tree: \(rt.output)", 1)
                    self.wake()
                }
                return
            }
            // Refresh the real index's stat cache without changing staged entries or HEAD.
            _ = Self.runGitTrimmed(["update-index", "-q", "--refresh"], in: dir)

            // Clean up checkpoint refs for turns strictly greater than `turn`.
            if !prefix.isEmpty {
                let list = Self.runGitTrimmed(["for-each-ref", "--format=%(refname)", prefix], in: dir)
                if list.status == 0 {
                    for line in list.output.split(separator: "\n") {
                        let ref = String(line).trimmingCharacters(in: .whitespacesAndNewlines)
                        guard ref.hasPrefix(prefix) else { continue }
                        let slot = String(ref.dropFirst(prefix.count))
                        if let k = Int(slot), k > turn {
                            _ = Self.runGitTrimmed(["update-ref", "-d", ref], in: dir)
                        }
                    }
                }
            }

            DispatchQueue.main.async {
                guard let self, self.chat == chatID, cd_running(self.page) == 0 else { return }
                _ = cd_ckpt_rewind_to(self.page, Int32(turn))
                if self.run.worker.alive { self.run.worker.stop() }
                self.recallDue = true
                self.keep()
                self.refreshBranch()
                self.refreshCheckpoints(for: chatID, in: dir)
                cd_toast(self.page, "Reverted working tree and chat to Turn \(turn + 1)", 0)
                self.wake()
            }
        }
    }
}
