// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The Foreman chat: its tier, its history, its audit log, and opening it.
import AppKit

extension CoderWindow {
    func setForemanTier(_ tier: ForemanTier) {
        let previous = foremanTier
        foremanTier = tier
        tier.save()
        daemonClient.updateForeman(tier: tier.rawValue)
        if isForemanShown {
            syncSkillsNote(for: page, isForeman: true)
            configureWorkerEnv(run, isForeman: true)
        }
        if previous != tier {
            _ = foremanEvents.emit(type: "tier.changed", chatID: nil, data: ["from": previous.wireName, "to": tier.wireName])
        }
        syncForemanUI()
        wake()
    }
    func clearForemanHistory() {
        var d = CoderDialog(title: "Clear Foreman history?", body: "This removes the Foreman conversation history. Managed chats are not deleted.")
        d.buttons = [.init(title: "Cancel"), .init(title: "Clear History", kind: .danger)]
        d.confirm = 0; d.cancel = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 { self.resetForeman() }
            self.refocus()
        }
    }
    /// Clears the Foreman conversation with no question asked: the menu asks first, the operator API does not.
    func resetForeman() {
        if chat == Foreman.chatID {
            if cd_running(page) != 0 { stopRun(run) }
            _ = cd_new(page)
            recallDue = false
            var out = [CChar](repeating: 0, count: 256)
            let n = cd_forget_line(&out, Int32(out.count)); run.worker.write(out, count: Int(n))
        } else if let back = runs.removeValue(forKey: Foreman.chatID) {
            stopRun(back); back.stop(); cd_close(back.page)
        }
        store.remove(Foreman.chatID)
        cd_chats(page, chatLines); syncForemanUI(); wake()
    }
    func openForemanAuditLog() {
        let url = foremanAudit.fileURL
        let fm = FileManager.default
        if !fm.fileExists(atPath: url.path) {
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            fm.createFile(atPath: url.path, contents: Data())
        }
        NSWorkspace.shared.open(url)
    }
    func openForeman() {
        if chat == Foreman.chatID { cd_current(page, Foreman.chatID); syncForemanUI(); window.makeFirstResponder(editor); wake(); return }
        guard detach() else { return }
        if let back = runs[Foreman.chatID] {
            guard cd_take(page, back.page) != 0 else { return }
            runs[Foreman.chatID] = nil; cd_close(back.page); back.page = page
            run.stop()
            run = back
            chat = Foreman.chatID; recallDue = false; cd_current(page, Foreman.chatID)
            setFolder(back.folder, keep: false)
            configureWorkerEnv(run, isForeman: true)
            syncSkillsNote(for: page, isForeman: true)
            cd_chats(page, chatLines); syncForemanUI()
            window.makeFirstResponder(editor); wake()
            return
        }
        if let text = store.text(Foreman.chatID), cd_load(page, text) != 0 {
            recallDue = true
        } else {
            _ = cd_new(page)
            recallDue = false
            var out = [CChar](repeating: 0, count: 256)
            let n = cd_forget_line(&out, Int32(out.count)); run.worker.write(out, count: Int(n))
        }
        chat = Foreman.chatID; cd_current(page, Foreman.chatID)
        var isFolder: ObjCBool = false
        if let kept = store.folder(Foreman.chatID), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue {
            setFolder(URL(fileURLWithPath: kept), keep: false)
        }
        configureWorkerEnv(run, isForeman: true)
        syncSkillsNote(for: page, isForeman: true)
        cd_chats(page, chatLines); syncForemanUI()
        window.makeFirstResponder(editor); wake()
    }
}
