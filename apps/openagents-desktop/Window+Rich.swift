// Firm reason: 6 (AppKit host hooks for prompt stash/history persistence, slash commands, and Side Chat pane; #13884).
import AppKit

private enum RichDefaultsKeys {
    static let promptStash = "openagents.desktop.rich.promptStash"
    static let promptHistory = "openagents.desktop.rich.promptHistory"
}

extension CoderWindow {
    /// Restore persisted prompt stash and prompt history from UserDefaults on startup.
    func setupRichComposer() {
        if let savedStash = UserDefaults.standard.string(forKey: RichDefaultsKeys.promptStash), !savedStash.isEmpty {
            _ = cd_stash_load(page, savedStash)
        }
        if let savedHist = UserDefaults.standard.string(forKey: RichDefaultsKeys.promptHistory), !savedHist.isEmpty {
            _ = cd_hist_load(page, savedHist)
        }
    }

    /// Persist prompt stash and prompt history to UserDefaults.
    func saveRichPersistence() {
        var stashBuf = [CChar](repeating: 0, count: 16384)
        let sn = cd_stash_save(page, &stashBuf, Int32(stashBuf.count))
        let stashStr = sn > 0 ? String(cString: stashBuf) : ""
        UserDefaults.standard.set(stashStr, forKey: RichDefaultsKeys.promptStash)

        var histBuf = [CChar](repeating: 0, count: 36864)
        let hn = cd_hist_save(page, &histBuf, Int32(histBuf.count))
        let histStr = hn > 0 ? String(cString: histBuf) : ""
        UserDefaults.standard.set(histStr, forKey: RichDefaultsKeys.promptHistory)
    }

    /// Handle /compact: ask the running session to summarize & compact context, or notify when idle/empty.
    func handleRichCompact() {
        guard cd_running(page) == 0 else {
            cd_toast(page, "Wait for the active reply to finish before compacting.", 0)
            wake()
            return
        }
        let compactPrompt = "Summarize our conversation so far into a concise context handoff (goals, files changed, decisions, and next steps) so we can continue cleanly."
        if cd_set_text(page, compactPrompt) != 0 {
            _ = cd_submit(page)
            cd_toast(page, "Compacting conversation context…", 0)
        }
        wake()
    }

    /// Handle /clear: start a fresh thread cleanly.
    func handleRichClear() {
        guard cd_running(page) == 0 else {
            cd_toast(page, "Stop the running reply before clearing the thread.", 0)
            wake()
            return
        }
        newChat()
        cd_toast(page, "Cleared thread.", 0)
        wake()
    }

    /// Handle /review: open the Changes review pane and refresh live Git diff.
    func handleRichReview() {
        refreshGitDiff()
        wake()
    }

    /// Handle /usage: open the Usage & quota dashboard and refresh live quota telemetry.
    func handleRichUsage() {
        refreshUsageDashboard()
        wake()
    }

    /// Load a stored chat from CoderStore into the right-hand Side Chat split pane.
    func openSideChat(_ id: String) {
        let title = store.title(id) ?? (id == Foreman.chatID ? Foreman.title : "Linked Chat")
        cd_side_chat_open(page, id, title)
        if let raw = store.text(id),
           let data = raw.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let turns = obj["turns"] as? [[String: Any]] {
            for t in turns.suffix(32) {
                let ask = (t["ask"] as? String) ?? ""
                let reply = (t["reply"] as? String) ?? ""
                if !ask.isEmpty || !reply.isEmpty {
                    _ = cd_side_chat_add_turn(page, ask, reply)
                }
            }
        }
        wake()
    }

    /// Promote the currently open Side Chat into the main conversation pane.
    func handleRichSideOpenMain() {
        var buf = [CChar](repeating: 0, count: 128)
        guard cd_side_chat_id(page, &buf, Int32(buf.count)) > 0 else { return }
        let id = String(cString: buf)
        cd_side_chat_close(page)
        open(id)
    }
}
