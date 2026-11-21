// Firm reason: 6 (macOS NSApp.effectiveAppearance observation, NSOpenPanel theme import, UserDefaults & ~/.openagents/desktop/keybindings.json persistence over page/customize.coax; GitHub #13887).
import AppKit
import Foundation
import UniformTypeIdentifiers

/// Persisted customization settings (Theme Engine, VS Code/OpenVSX theme JSON, Keybindings, Locale).
enum CoderCustomizeStore {
    static let modeKey = "customize.theme.mode"
    static let skinKey = "customize.theme.skin"
    static let contrastKey = "customize.theme.contrast"
    static let vscodeJSONKey = "customize.theme.vscodeJSON"
    static let localeKey = "customize.locale"

    static var mode: Int32 {
        get {
            guard UserDefaults.standard.object(forKey: modeKey) != nil else { return 1 } // Dark
            return Int32(UserDefaults.standard.integer(forKey: modeKey))
        }
        set { UserDefaults.standard.set(Int(newValue), forKey: modeKey) }
    }

    static var skin: Int32 {
        get { Int32(UserDefaults.standard.integer(forKey: skinKey)) }
        set { UserDefaults.standard.set(Int(newValue), forKey: skinKey) }
    }

    static var highContrast: Bool {
        get { UserDefaults.standard.bool(forKey: contrastKey) }
        set { UserDefaults.standard.set(newValue, forKey: contrastKey) }
    }

    static var vscodeThemeJSON: String? {
        get { UserDefaults.standard.string(forKey: vscodeJSONKey) }
        set {
            if let v = newValue, !v.isEmpty {
                UserDefaults.standard.set(v, forKey: vscodeJSONKey)
            } else {
                UserDefaults.standard.removeObject(forKey: vscodeJSONKey)
            }
        }
    }

    static var locale: Int32 {
        get { Int32(UserDefaults.standard.integer(forKey: localeKey)) }
        set { UserDefaults.standard.set(Int(newValue), forKey: localeKey) }
    }

    static var keybindingsURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/keybindings.json")
    }

    struct KeybindingOverride: Codable {
        let id: String
        let chord: String
        let when: String
    }

    static func loadKeybindings() -> [KeybindingOverride] {
        guard let data = try? Data(contentsOf: keybindingsURL),
              let list = try? JSONDecoder().decode([KeybindingOverride].self, from: data) else {
            return []
        }
        return list
    }

    static func saveKeybindings(_ items: [KeybindingOverride]) {
        let url = keybindingsURL
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        if items.isEmpty {
            try? FileManager.default.removeItem(at: url)
            return
        }
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(items) {
            try? data.write(to: url, options: .atomic)
        }
    }

    static var systemIsDark: Bool {
        let match = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua])
        return match != .aqua
    }
}

extension CoderWindow {
    /// Installs live macOS system appearance observation and applies saved theme, keybindings, and locale.
    func setupCustomize() {
        applyCustomize(to: page)
        appearanceObservation = NSApp.observe(\.effectiveAppearance, options: [.initial, .new]) { [weak self] _, _ in
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                let isDark: Int32 = CoderCustomizeStore.systemIsDark ? 1 : 0
                cd_cust_system_dark_set(self.page, isDark)
                for r in self.runs.values where r.page != self.page {
                    cd_cust_system_dark_set(r.page, isDark)
                }
                self.wake()
            }
        }
    }

    /// Applies saved theme, VS Code theme JSON, keybindings, and locale onto a single page.
    func applyCustomize(to targetPage: UnsafeMutableRawPointer) {
        cd_cust_system_dark_set(targetPage, CoderCustomizeStore.systemIsDark ? 1 : 0)
        cd_cust_theme_set(targetPage, CoderCustomizeStore.mode, CoderCustomizeStore.skin, CoderCustomizeStore.highContrast ? 1 : 0)
        if let json = CoderCustomizeStore.vscodeThemeJSON, !json.isEmpty {
            _ = cd_cust_import_vscode_theme(targetPage, json)
        } else {
            cd_cust_clear_vscode_theme(targetPage)
        }
        cd_cust_locale_set(targetPage, CoderCustomizeStore.locale)
        cd_cust_kb_reset_all(targetPage)
        for item in CoderCustomizeStore.loadKeybindings() {
            _ = cd_cust_kb_rebind_id(targetPage, item.id, item.chord, item.when, 1)
        }
    }

    /// Syncs current customization state across the shown page and all background run pages.
    func syncCustomize() {
        applyCustomize(to: page)
        for r in runs.values where r.page != page {
            applyCustomize(to: r.page)
        }
        wake()
    }

    /// Dispatches CP_ASK_CUSTOM_* (120..125) from Settings -> Appearance & Keybindings.
    func customizeAsked(_ what: Int32) {
        switch what {
        case 120: customThemeChanged()
        case 121: importVSCodeTheme()
        case 122: clearVSCodeTheme()
        case 123: rebindKeybinding()
        case 124: resetKeybindings()
        case 125: customLocaleChanged()
        default: break
        }
    }

    func customThemeChanged() {
        CoderCustomizeStore.mode = cd_cust_theme_mode(page)
        CoderCustomizeStore.skin = cd_cust_theme_skin(page)
        CoderCustomizeStore.highContrast = cd_cust_theme_contrast(page) != 0
        // If user switched directly to a built-in skin, clear any stored VS Code override.
        if CoderCustomizeStore.vscodeThemeJSON != nil {
            // Only clear if the page itself cleared it (i.e. skin click).
            let mode = CoderCustomizeStore.mode
            let skin = CoderCustomizeStore.skin
            for r in runs.values where r.page != page {
                cd_cust_theme_set(r.page, mode, skin, CoderCustomizeStore.highContrast ? 1 : 0)
            }
        } else {
            syncCustomize()
        }
        wake()
    }

    func importVSCodeTheme() {
        let panel = NSOpenPanel()
        panel.title = "Import VS Code / OpenVSX Theme JSON"
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.json]
        panel.beginSheetModal(for: window) { [weak self] resp in
            guard let self else { return }
            defer { self.refocus() }
            guard resp == .OK, let url = panel.url else { return }
            guard let text = try? String(contentsOf: url, encoding: .utf8) else {
                self.present(.notice("Could not read theme file", "Unable to read UTF-8 text from \(url.lastPathComponent)."))
                return
            }
            if cd_cust_import_vscode_theme(self.page, text) != 0 {
                CoderCustomizeStore.vscodeThemeJSON = text
                for r in self.runs.values where r.page != self.page {
                    _ = cd_cust_import_vscode_theme(r.page, text)
                }
                self.wake()
            } else {
                self.present(.notice("Invalid VS Code Theme JSON", "Expected a JSON object with a \"colors\" map containing hex color tokens such as \"editor.background\" or \"editor.foreground\"."))
            }
        }
    }

    func clearVSCodeTheme() {
        CoderCustomizeStore.vscodeThemeJSON = nil
        cd_cust_clear_vscode_theme(page)
        for r in runs.values where r.page != page {
            cd_cust_clear_vscode_theme(r.page)
        }
        wake()
    }

    func customLocaleChanged() {
        CoderCustomizeStore.locale = cd_cust_locale(page)
        for r in runs.values where r.page != page {
            cd_cust_locale_set(r.page, CoderCustomizeStore.locale)
        }
        wake()
    }

    private func custString(_ fn: (UnsafeMutablePointer<CChar>, Int32) -> Int32, cap: Int = 128) -> String {
        var buf = [CChar](repeating: 0, count: cap)
        let n = fn(&buf, Int32(cap))
        if n <= 0 { return "" }
        return String(cString: buf)
    }

    private func persistCurrentKeybindings() {
        let count = cd_cust_kb_count(page)
        var overrides: [CoderCustomizeStore.KeybindingOverride] = []
        for i in 0..<count {
            let id = custString { cd_cust_kb_id(page, i, $0, $1) }
            let chord = custString { cd_cust_kb_chord(page, i, $0, $1) }
            let when = custString { cd_cust_kb_when(page, i, $0, $1) }
            if !id.isEmpty && !chord.isEmpty {
                overrides.append(.init(id: id, chord: chord, when: when))
            }
        }
        CoderCustomizeStore.saveKeybindings(overrides)
        for r in runs.values where r.page != page {
            applyCustomize(to: r.page)
        }
    }

    func rebindKeybinding() {
        let idx = cd_cust_kb_target(page)
        guard idx >= 0 && idx < cd_cust_kb_count(page) else { return }
        let label = custString { cd_cust_kb_label(page, idx, $0, $1) }
        let curChord = custString { cd_cust_kb_chord(page, idx, $0, $1) }
        let curWhen = custString { cd_cust_kb_when(page, idx, $0, $1) }

        var body = "Edit the shortcut chord (for example: Cmd+Shift+K, Ctrl+`, Escape) and optional VS Code-style when-clause (editorFocus, terminalFocus, filePaneFocus, settingsFocus, !, &&, ||)."
        let conf = cd_cust_kb_conflict(page, idx)
        if conf >= 0 {
            let other = custString { cd_cust_kb_label(page, conf, $0, $1) }
            body += "\n\nWarning: Currently conflicts with \"\(other)\"."
        }

        var d = CoderDialog(size: .md, title: "Rebind: \(label)", body: body)
        d.fields = [
            .init(label: "Shortcut chord", value: curChord, hint: "Cmd+K"),
            .init(label: "When clause (optional)", value: curWhen, hint: "!settingsFocus"),
        ]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1 else { return }
            let newChord = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            let newWhen = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !newChord.isEmpty else { return }
            if cd_cust_kb_rebind(self.page, idx, newChord, newWhen, 1) != 0 {
                self.persistCurrentKeybindings()
                let newConf = cd_cust_kb_conflict(self.page, idx)
                if newConf >= 0 {
                    let other = self.custString { cd_cust_kb_label(self.page, newConf, $0, $1) }
                    self.present(.notice("Keybinding conflict detected", "\"\(label)\" (\(newChord)) overlaps with \"\(other)\" in the same focus context. Narrow one of their when-clauses or pick a different chord."))
                }
                self.wake()
            }
        }
    }

    func resetKeybindings() {
        CoderCustomizeStore.saveKeybindings([])
        cd_cust_kb_reset_all(page)
        for r in runs.values where r.page != page {
            cd_cust_kb_reset_all(r.page)
        }
        wake()
    }
}
