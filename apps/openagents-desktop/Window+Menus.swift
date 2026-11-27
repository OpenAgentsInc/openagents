// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Right-click menus, drawn by the page (page/overlay.coax, GitHub #13746), and their handlers.
import AppKit

extension CoderWindow {
    private func togglePin(_ id: String) {
        store.togglePin(id)
        let pinned = store.isPinned(id)
        daemonClient.updateSessionMeta(sessionID: id, pinned: pinned)
        if id != Foreman.chatID { cloudSync.syncMeta(chatID: id, pinned: pinned) }
        cd_chats(page, chatLines)
        wake()
    }
    private func toggleOperatorTierRaise() {
        let on = !UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey)
        UserDefaults.standard.set(on, forKey: ForemanAPI.operatorTierRaiseKey)
        foremanAudit.record(action: "operator_tier_raise", tier: foremanTier, detail: ["allowed": on, "principal": "user"])
    }
    /// A right-click on the Foreman row or a chat in the sidebar opens its menu at the pointer. While
    /// a dialog is up the click is the dialog's; while a menu is up another opens where it was made.
    func rightClick(_ event: NSEvent) -> Bool {
        guard event.window === window else { return false }
        if overlay.dialogOpen { return true }
        let at = canvas.convert(event.locationInWindow, from: nil)
        guard canvas.bounds.contains(at) else { return false }
        let scale = window.backingScaleFactor
        let y = canvas.isFlipped ? at.y : canvas.bounds.height - at.y
        let px = Float(at.x * scale), py = Float(y * scale)
        typealias Item = CoderOverlay.MenuItem
        if FeatureFlags.isOn(.foreman) && cd_foreman_at(page, px, py) != 0 {
            let raise = UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey)
            var items = [Item(label: "Open Foreman"), Item(kind: 2)]
            var acts: [() -> Void] = [{ [weak self] in self?.openForeman() }, {}]
            for (tag, label) in [(0, "Tier 0: Observe (Read-Only)"), (1, "Tier 1: Managed (Default)"), (2, "Tier 2: Autonomous")] {
                items.append(Item(label: label, checked: foremanTier.rawValue == tag))
                acts.append { [weak self] in if let tier = ForemanTier(rawValue: tag) { self?.setForemanTier(tier) } }
            }
            items.append(Item(label: "Let Programs Raise the Tier", checked: raise)); acts.append { [weak self] in self?.toggleOperatorTierRaise() }
            items.append(Item(kind: 2)); acts.append {}
            items.append(Item(label: "View Audit Log…")); acts.append { [weak self] in self?.openForemanAuditLog() }
            items.append(Item(label: "Stop All Running Chats", disabled: runningCount == 0)); acts.append { [weak self] in self?.stopAllChats() }
            items.append(Item(kind: 2)); acts.append {}
            items.append(Item(label: "Copy Chat ID")); acts.append { [weak self] in self?.copyChatID(Foreman.chatID) }
            items.append(Item(label: "Copy Chat Path")); acts.append { [weak self] in self?.copyChatPath(Foreman.chatID) }
            items.append(Item(kind: 2)); acts.append {}
            items.append(Item(label: "Clear Foreman History…", danger: true)); acts.append { [weak self] in self?.clearForemanHistory() }
            overlay.presentMenu(items, x: px, y: py) { chosen in if chosen >= 0 && chosen < acts.count { acts[chosen]() } }
            return true
        }
        var out = [CChar](repeating: 0, count: 128)
        guard cd_chat_at(page, px, py, &out, Int32(out.count)) > 0 else {
            // Right-click with an active GPU transcript or File Viewer selection offers Copy (+ Copy with Path:Line in File Viewer, #13992).
            if cd_has_selection(page) != 0 {
                var lr: [Int32] = [0, 0]
                let isFileSel = cd_sel_file_lines(page, &lr) != 0 && lr[0] > 0
                let items = isFileSel
                    ? [Item(label: "Copy"), Item(label: "Copy with Path:Line")]
                    : [Item(label: "Copy")]
                overlay.presentMenu(items, x: px, y: py) { [weak self] chosen in
                    guard let self, chosen >= 0 else { return }
                    _ = cd_key(self.page, Int32(UnicodeScalar("c").value), 8)
                    if chosen == 1 && isFileSel {
                        let rel = self.openFileRelativePath()
                        let copied = NSPasteboard.general.string(forType: .string) ?? ""
                        let lineSpec = lr[1] > lr[0] ? "\(lr[0])-\(lr[1])" : "\(lr[0])"
                        let tagged = rel.isEmpty ? copied : "\(rel):\(lineSpec)\n\(copied)"
                        let pb = NSPasteboard.general
                        pb.clearContents()
                        pb.setString(tagged, forType: .string)
                    }
                    self.wake()
                }
                return true
            }
            // a right-click on nothing closes a menu that is up
            if overlay.menuOpen { _ = overlay.escape(); return true }
            return false
        }
        let id = String(cString: out)
        let isArchived = store.isArchived(id)
        let isSnoozed = store.snoozedUntil(id) > Int64(Date().timeIntervalSince1970 * 1000)
        var items: [Item] = [
            Item(label: store.isPinned(id) ? "Unpin Chat" : "Pin Chat"),
            Item(label: "Rename Chat…"),
            Item(label: "Regenerate Title"),
            Item(label: "Fork Chat"),
        ]
        var acts: [() -> Void] = [
            { [weak self] in self?.togglePin(id) },
            { [weak self] in self?.rename(id) },
            { [weak self] in self?.regenerateTitle(id) },
            { [weak self] in self?.fork(id) },
        ]
        if FeatureFlags.isOn(.rich) {
            items.append(Item(label: "Open in Side Pane"))
            acts.append { [weak self] in self?.openSideChat(id) }
        }
        items.append(Item(kind: 2)); acts.append {}
        items.append(Item(label: "Copy Chat ID"))
        acts.append { [weak self] in self?.copyChatID(id) }
        items.append(Item(label: "Copy Chat Path"))
        acts.append { [weak self] in self?.copyChatPath(id) }
        items.append(Item(kind: 2)); acts.append {}
        items.append(Item(label: isArchived ? "Unarchive Chat" : "Archive Chat", disabled: !isArchived && busy(id)))
        acts.append { [weak self] in if isArchived { self?.unarchiveChat(id) } else { self?.archiveChat(id) } }
        if !isArchived {
            if isSnoozed {
                items.append(Item(label: "Unsnooze Chat"))
                acts.append { [weak self] in self?.unsnoozeChat(id) }
            }
            if !isSnoozed {
                items.append(Item(label: "Snooze 1 Hour"))
                acts.append { [weak self] in self?.snoozeChat(id, durationSeconds: 3600) }
                items.append(Item(label: "Snooze 4 Hours"))
                acts.append { [weak self] in self?.snoozeChat(id, durationSeconds: 4 * 3600) }
                items.append(Item(label: "Snooze Until Tomorrow"))
                acts.append { [weak self] in self?.snoozeChatUntilTomorrow(id) }
                items.append(Item(label: "Snooze Custom…"))
                acts.append { [weak self] in self?.snoozeChatCustom(id) }
            }
        }
        items.append(Item(kind: 2)); acts.append {}
        items.append(Item(label: "Delete Chat…", disabled: busy(id), danger: true))
        acts.append { [weak self] in self?.delete(id) }
        overlay.presentMenu(items, x: px, y: py) { chosen in
            if chosen >= 0 && chosen < acts.count { acts[chosen]() }
        }
        return true
    }
}
