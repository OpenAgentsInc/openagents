// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Right-click menus and their handlers.
import AppKit

extension CoderWindow {
    @objc private func togglePinFromMenu(_ item: NSMenuItem) {
        guard let id = item.representedObject as? String else { return }
        store.togglePin(id)
        daemonClient.updateSessionMeta(sessionID: id, pinned: store.isPinned(id))
        cd_chats(page, chatLines)
        wake()
    }
    @objc private func renameFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { rename(id) } }
    @objc private func deleteFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { delete(id) } }
    @objc private func openForemanFromMenu(_ item: NSMenuItem) { openForeman() }
    @objc private func setForemanTierFromMenu(_ item: NSMenuItem) {
        if let tier = ForemanTier(rawValue: item.tag) { setForemanTier(tier) }
    }
    @objc private func stopAllFromMenu(_ item: NSMenuItem) { stopAllChats() }
    @objc private func viewAuditFromMenu(_ item: NSMenuItem) { openForemanAuditLog() }
    @objc private func toggleOperatorTierRaiseFromMenu(_ item: NSMenuItem) {
        let on = !UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey)
        UserDefaults.standard.set(on, forKey: ForemanAPI.operatorTierRaiseKey)
        foremanAudit.record(action: "operator_tier_raise", tier: foremanTier, detail: ["allowed": on, "principal": "user"])
    }
    @objc private func clearForemanFromMenu(_ item: NSMenuItem) { clearForemanHistory() }
    /// A right-click on the Foreman row or a chat in the sidebar.
    func rightClick(_ event: NSEvent) -> Bool {
        guard event.window === window else { return false }
        let at = canvas.convert(event.locationInWindow, from: nil), scale = window.backingScaleFactor
        let y = canvas.isFlipped ? at.y : canvas.bounds.height - at.y
        let px = Float(at.x * scale), py = Float(y * scale)
        if cd_foreman_at(page, px, py) != 0 {
            let menu = NSMenu()
            let openItem = menu.addItem(withTitle: "Open Foreman", action: #selector(openForemanFromMenu(_:)), keyEquivalent: "")
            openItem.target = self
            menu.addItem(.separator())
            for (tag, label) in [(0, "Tier 0: Observe (Read-Only)"), (1, "Tier 1: Managed (Default)"), (2, "Tier 2: Autonomous")] {
                let ti = menu.addItem(withTitle: label, action: #selector(setForemanTierFromMenu(_:)), keyEquivalent: "")
                ti.target = self; ti.tag = tag
                ti.state = foremanTier.rawValue == tag ? .on : .off
            }
            let raiseItem = menu.addItem(withTitle: "Let Programs Raise the Tier", action: #selector(toggleOperatorTierRaiseFromMenu(_:)), keyEquivalent: "")
            raiseItem.target = self
            raiseItem.state = UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey) ? .on : .off
            menu.addItem(.separator())
            let auditItem = menu.addItem(withTitle: "View Audit Log…", action: #selector(viewAuditFromMenu(_:)), keyEquivalent: "")
            auditItem.target = self
            let stopAll = menu.addItem(withTitle: "Stop All Running Chats", action: #selector(stopAllFromMenu(_:)), keyEquivalent: "")
            stopAll.target = self; stopAll.isEnabled = runningCount > 0
            menu.addItem(.separator())
            let clearItem = menu.addItem(withTitle: "Clear Foreman History…", action: #selector(clearForemanFromMenu(_:)), keyEquivalent: "")
            clearItem.target = self
            menu.autoenablesItems = false
            NSMenu.popUpContextMenu(menu, with: event, for: canvas)
            return true
        }
        var out = [CChar](repeating: 0, count: 128)
        guard cd_chat_at(page, px, py, &out, Int32(out.count)) > 0 else { return false }
        let id = String(cString: out), menu = NSMenu()
        let pinItem = menu.addItem(withTitle: store.isPinned(id) ? "Unpin Chat" : "Pin Chat", action: #selector(togglePinFromMenu(_:)), keyEquivalent: "")
        pinItem.target = self; pinItem.representedObject = id
        let renameItem = menu.addItem(withTitle: "Rename Chat…", action: #selector(renameFromMenu(_:)), keyEquivalent: "")
        renameItem.target = self; renameItem.representedObject = id
        menu.addItem(.separator())
        let deleteItem = menu.addItem(withTitle: "Delete Chat…", action: #selector(deleteFromMenu(_:)), keyEquivalent: "")
        deleteItem.target = self; deleteItem.representedObject = id
        deleteItem.isEnabled = !busy(id)
        menu.autoenablesItems = false
        NSMenu.popUpContextMenu(menu, with: event, for: canvas)
        return true
    }
}
