// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Dialogs: a chat's budget and goal, the Claude key and the OpenRouter models.
import AppKit

extension CoderWindow {
    /// Asks for a new max budget (USD) for the shown chat thread.
    func editBudget() {
        let currentCost = Double(cd_cost_usd_micros(page)) / 1_000_000.0
        let currentBudget = Double(cd_budget_usd_micros(page)) / 1_000_000.0
        let alert = NSAlert()
        alert.messageText = "Chat Max Budget"
        alert.informativeText = String(format: "Set the maximum spend (USD) for this chat thread. Current spend: $%.2f.", currentCost)
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 220, height: 24))
        field.stringValue = String(format: "%.2f", currentBudget)
        field.placeholderString = "50.00"
        alert.accessoryView = field
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == .alertFirstButtonReturn else { return }
            let cleaned = field.stringValue
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .replacingOccurrences(of: "$", with: "")
                .replacingOccurrences(of: ",", with: "")
            guard let dollars = Double(cleaned), dollars.isFinite, dollars > 0, dollars <= 1_000_000 else { return }
            let micros = Int64((dollars * 1_000_000.0).rounded())
            guard micros > 0 else { return }
            _ = cd_set_budget_usd_micros(self.page, micros)
            self.wake()
        }
    }
    /// Opens the thread goal editor (/goal or /goal edit) to create, edit, pause/resume, or clear a thread goal.
    func editGoal() {
        let status = cd_goal_status(page)
        let elapsed = cd_goal_time(page)
        var buf = [CChar](repeating: 0, count: 4097)
        let n = cd_goal_objective(page, &buf, Int32(buf.count))
        let current = n > 0 ? String(cString: buf) : ""
        let alert = NSAlert()
        alert.messageText = status == 0 ? "Set Thread Goal" : "Edit Thread Goal"
        if status == 0 {
            alert.informativeText = "Describe a long-running objective. Coder will work autonomously across turns until it is complete, paused, or hits the chat budget."
        } else {
            let label: String
            switch status {
            case 1: label = "Active"
            case 2: label = "Paused"
            case 3: label = "Complete"
            case 4: label = "Blocked"
            case 5: label = "Budget limit"
            default: label = "None"
            }
            let timeStr = elapsed < 60 ? "\(elapsed)s" : "\(elapsed / 60)m"
            alert.informativeText = "Status: \(label) · Elapsed: \(timeStr). Update the objective below or change the goal state."
        }
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 420, height: 110))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        let tv = NSTextView(frame: NSRect(x: 0, y: 0, width: 420, height: 110))
        tv.isRichText = false
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.font = NSFont.systemFont(ofSize: 13)
        tv.string = current
        scroll.documentView = tv
        alert.accessoryView = scroll
        alert.addButton(withTitle: status == 0 ? "Start Goal" : "Save Goal")
        if status != 0 {
            alert.addButton(withTitle: status == 1 ? "Pause" : "Resume")
            alert.addButton(withTitle: "Clear")
        }
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = tv
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.refocus() }
            if answer == .alertFirstButtonReturn {
                let text = tv.string.trimmingCharacters(in: .whitespacesAndNewlines)
                if !text.isEmpty { _ = cd_goal_set(self.page, text) }
            } else if status != 0 && answer == .alertSecondButtonReturn {
                _ = cd_goal_set_status(self.page, status == 1 ? 2 : 1)
            } else if status != 0 && answer == .alertThirdButtonReturn {
                _ = cd_goal_clear(self.page)
            }
            self.wake()
        }
    }
    /// Coder > Claude Key: sets, replaces or removes the key Claude sub-agents use. It applies to the
    /// next Claude sub-agent; one already running keeps what it started with.
    func chooseKey() {
        let alert = NSAlert()
        alert.messageText = "Claude Key"
        var about = "Claude sub-agents use this key instead of the Claude Code login on this Mac. Paste an Anthropic API key (sk-ant-api…) or a token from claude setup-token (sk-ant-oat…). Coder keeps it in a file only you can read."
        if let launch = CoderKey.fromLaunch { about += "\n\nThis launch uses the key in CODER_CLAUDE_KEY (" + CoderKey.tail(launch) + "). It wins over a key set here until you open Coder without it." }
        else if let kept = CoderKey.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + "." }
        else { about += "\n\nNo key is set now: Claude sub-agents use the Claude Code login." }
        alert.informativeText = about
        let field = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        field.placeholderString = "sk-ant-…"
        alert.accessoryView = field
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        let hadKey = CoderKey.saved != nil
        if hadKey { alert.addButton(withTitle: "Remove Key").hasDestructiveAction = true }
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.refocus() }
            let key: String
            switch answer {
            case .alertFirstButtonReturn:
                key = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
                if key.isEmpty { return }
            case .alertThirdButtonReturn where hadKey: key = ""
            default: return
            }
            if !CoderKey.save(key) {
                let failed = NSAlert(); failed.messageText = "The key could not be saved."
                failed.informativeText = "Coder could not write " + CoderKey.file.path + "."
                failed.beginSheetModal(for: self.window)
            }
        }
    }
    /// Coder > OpenRouter Models: the key, the model for Microcoder (a composer picker entry) and the
    /// model for sub-agents (level openrouter). An empty model removes it. Each model is checked against
    /// OpenRouter's list first: it must take a strict JSON schema.
    func chooseOpenRouter() {
        let alert = NSAlert()
        alert.messageText = "OpenRouter Models"
        var about = "Run Microcoder or its sub-agents on any OpenRouter model that takes a strict JSON schema. Use OpenRouter ids such as deepseek/deepseek-v4.1-flash-1017. The main model joins the composer's model picker. The sub-agent model is one more level Microcoder can hand tasks to: ask for OpenRouter by name."
        if let kept = CoderOpenRouter.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + ". Leave the key empty to keep it." }
        else if CoderOpenRouter.hasKey { about += "\n\nNo key is set here: Coder uses ~/work/.secrets/openrouter.env." }
        else { about += "\n\nNo key is set. Paste one from openrouter.ai/settings/keys." }
        alert.informativeText = about
        // Three labeled rows at fixed frames: an alert's accessory view gets no Auto Layout width,
        // so a stack view there shrinks its fields to slivers.
        let width: CGFloat = 340, rowH: CGFloat = 46
        let box = NSView(frame: NSRect(x: 0, y: 0, width: width, height: rowH * 3))
        func field(_ row: Int, _ title: String, _ secure: Bool, _ value: String, _ hint: String) -> NSTextField {
            let top = rowH * CGFloat(2 - row)
            let label = NSTextField(labelWithString: title)
            label.font = NSFont.systemFont(ofSize: NSFont.smallSystemFontSize)
            label.frame = NSRect(x: 0, y: top + 26, width: width, height: 16)
            let f = secure ? NSSecureTextField(frame: NSRect(x: 0, y: top + 2, width: width, height: 22)) : NSTextField(frame: NSRect(x: 0, y: top + 2, width: width, height: 22))
            f.stringValue = value; f.placeholderString = hint
            box.addSubview(label); box.addSubview(f)
            return f
        }
        let key = field(0, "Key", true, "", CoderOpenRouter.saved != nil ? "Leave empty to keep the key set now" : "sk-or-…")
        let main = field(1, "Model for Microcoder", false, CoderOpenRouter.main, "deepseek/deepseek-v4.1-flash-1017 (empty: none)")
        let sub = field(2, "Model for sub-agents", false, CoderOpenRouter.sub, "deepseek/deepseek-v4.1-flash-1017 (empty: none)")
        alert.accessoryView = box
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        let hadKey = CoderOpenRouter.saved != nil
        if hadKey { alert.addButton(withTitle: "Remove Key").hasDestructiveAction = true }
        alert.window.initialFirstResponder = hadKey ? main : key
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            if answer == .alertThirdButtonReturn && hadKey { _ = CoderOpenRouter.save(""); self.refocus(); return }
            guard answer == .alertFirstButtonReturn else { self.refocus(); return }
            let k = key.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let m = main.stringValue.trimmingCharacters(in: .whitespacesAndNewlines), s = sub.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            if !k.isEmpty && !CoderOpenRouter.save(k) {
                let failed = NSAlert(); failed.messageText = "The key could not be saved."
                failed.informativeText = "Coder could not write " + CoderOpenRouter.file.path + "."
                failed.beginSheetModal(for: self.window); return
            }
            CoderOpenRouter.unfit([m, s]) { bad in
                if let bad, !bad.isEmpty {
                    let failed = NSAlert(); failed.messageText = "OpenRouter has no model " + bad.joined(separator: " or ") + " that takes a strict JSON schema."
                    failed.informativeText = "Microcoder sends one on every step. Check the id on openrouter.ai/models. The models were not changed."
                    failed.beginSheetModal(for: self.window); return
                }
                CoderOpenRouter.setModels(main: m, sub: s)
                self.applyOpenRouter()
                self.refocus()
            }
        }
    }
    /// The OpenRouter models to the window's page and every page running out of view.
    func applyOpenRouter() {
        coderApplyOpenRouter(page)
        for (_, r) in runs { coderApplyOpenRouter(r.page) }
        wake()
    }
}
