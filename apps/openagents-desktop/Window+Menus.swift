// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Right-click menus, drawn by the page (page/overlay.coax, GitHub #13746), and their handlers.
import AppKit

extension CoderWindow {
    private func togglePin(_ id: String) {
        store.togglePin(id)
        daemonClient.updateSessionMeta(sessionID: id, pinned: store.isPinned(id))
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
        if cd_foreman_at(page, px, py) != 0 {
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
            items.append(Item(label: "Clear Foreman History…", danger: true)); acts.append { [weak self] in self?.clearForemanHistory() }
            overlay.presentMenu(items, x: px, y: py) { chosen in if chosen >= 0 && chosen < acts.count { acts[chosen]() } }
            return true
        }
        var out = [CChar](repeating: 0, count: 128)
        guard cd_chat_at(page, px, py, &out, Int32(out.count)) > 0 else {
            // a right-click on nothing closes a menu that is up
            if overlay.menuOpen { _ = overlay.escape(); return true }
            return false
        }
        let id = String(cString: out)
        let items = [Item(label: store.isPinned(id) ? "Unpin Chat" : "Pin Chat"), Item(label: "Rename Chat…"), Item(label: "Fork Chat"), Item(kind: 2), Item(label: "Delete Chat…", disabled: busy(id), danger: true)]
        overlay.presentMenu(items, x: px, y: py) { [weak self] chosen in
            guard let self else { return }
            switch chosen {
            case 0: self.togglePin(id)
            case 1: self.rename(id)
            case 2: self.fork(id)
            case 4: self.delete(id)
            default: break
            }
        }
        return true
    }
}
