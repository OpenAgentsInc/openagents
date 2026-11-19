// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Settings (settings.coax): opening and closing, the search field, and what only the app knows.
import AppKit

extension CoderWindow {
    // MARK: - Settings (settings.coax)
    /// Coder > Settings… (Command-comma), and the menu items that open one of its pages: 0 General,
    /// 1 Models, 2 Foreman, 3 Account, 4 About & updates (-1 the last one shown), at a row ("" the top).
    func openSettings(_ which: Int32 = -1, row: String = "") {
        syncSettingsInfo()
        cd_settings_open(page, which, row)
        act(cd_action(page)); wake()
    }
    /// View > Back to Chats (Command-[), Esc and the gear: the chat as it was.
    func closeSettings() { cd_settings_open(page, -2, ""); act(cd_action(page)); wake() }
    var settingsOpen: Bool { cd_settings_page(page) >= 0 }
    /// Edit > Find (Command-F) in settings: the search field takes the keyboard.
    func findInSettings() { if settingsOpen { window.makeFirstResponder(search); wake() } }
    /// Esc in settings: a search is emptied first, then settings closes.
    func escape() {
        if overlay.escape() { return }   // a dialog or a menu over settings closes first
        if !search.string.isEmpty { search.synchronize { search.string = "" }; cd_search_set(page, ""); window.makeFirstResponder(search); wake(); return }
        closeSettings()
    }
    /// Settings opened or closed: the keyboard goes to the page or back to the composer.
    func settingsMoved() {
        if settingsOpen {
            syncSettingsInfo()
            if window.firstResponder === editor { window.makeFirstResponder(nil) }
        } else {
            search.synchronize { search.string = "" }; search.isHidden = true
            window.makeFirstResponder(editor)
        }
    }
    /// After a sheet: the composer takes the keyboard back, unless settings is open.
    func refocus() { if !settingsOpen { window.makeFirstResponder(editor) } else { syncSettingsInfo(); wake() } }
    /// What settings' pages show that only the app knows: the keys' ends and the version.
    func syncSettingsInfo() {
        func ends(_ key: String) -> String { "Ends " + String(key.suffix(4)) }
        var claude = ""
        if let launch = CoderKey.fromLaunch { claude = ends(launch) + " (this launch)" } else if let kept = CoderKey.saved { claude = ends(kept) }
        var openrouter = ""
        if let kept = CoderOpenRouter.saved { openrouter = ends(kept) } else if CoderOpenRouter.hasKey { openrouter = "From openrouter.env" }
        let info = Bundle.main.infoDictionary
        var version = info?["CFBundleShortVersionString"] as? String ?? ""
        if let build = info?["CFBundleVersion"] as? String { version += " (" + build + ")" }
        cd_settings_info(page, claude, openrouter, version, Float(window.frame.height - window.contentLayoutRect.height))
    }
    /// The search field over the box the page drew for it, at the page's text size.
    func placeSearch() {
        var box = [Float](repeating: 0, count: 4)
        guard cd_search_box(page, &box) != 0 else {
            if !search.isHidden { if window.firstResponder === search { window.makeFirstResponder(nil) }; search.isHidden = true }
            return
        }
        search.isHidden = false
        let size = CGFloat(box[3]) * 12 / 16
        if let font = search.font, abs(font.pointSize - size) > 0.01 {
            search.font = NSFont(descriptor: font.fontDescriptor, size: size)
            let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = CGFloat(box[3]); paragraph.maximumLineHeight = CGFloat(box[3])
            search.defaultParagraphStyle = paragraph; search.typingAttributes[.paragraphStyle] = paragraph
        }
        let frame = NSRect(x: CGFloat(box[0]), y: canvas.bounds.height - CGFloat(box[1] + box[3]), width: CGFloat(box[2]), height: CGFloat(box[3]))
        if search.frame != frame { search.frame = frame; wake() }
    }
}
