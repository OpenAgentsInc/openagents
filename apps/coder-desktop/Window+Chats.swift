// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Chats: opening, keeping, renaming, deleting, the working folder and text size.
import AppKit

extension CoderWindow {
    var canFolder: Bool { cd_running(page) == 0 }
    /// View > Bigger Text, Smaller Text, Actual Size: by is a row up or down, nil the usual size.
    func zoom(_ by: Int32?) {
        let row = by.map { cd_zoom(page, $0) } ?? cd_zoom_set(page, 2)
        UserDefaults.standard.set(Int(row), forKey: "zoom")
        var style = [Float](repeating: 0, count: 14); _ = cd_style(page, &style)
        line = CGFloat(style[1])
        if let font = editor.font { editor.font = NSFont(descriptor: font.fontDescriptor, size: CGFloat(style[0])) }
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = line; paragraph.maximumLineHeight = line
        editor.defaultParagraphStyle = paragraph
        editor.textStorage?.addAttribute(.paragraphStyle, value: paragraph, range: NSRange(location: 0, length: editor.textStorage?.length ?? 0))
        editor.typingAttributes[.paragraphStyle] = paragraph
        wake()
    }
    func setFolder(_ url: URL, keep: Bool) {
        folder = url.standardizedFileURL
        if keep { UserDefaults.standard.set(folder.path, forKey: "folder") }
        let home = FileManager.default.homeDirectoryForCurrentUser.path, path = folder.path
        cd_folder(page, path == home ? "~" : path.hasPrefix(home + "/") ? "~" + path.dropFirst(home.count) : path)
        wake()
    }
    struct BinaryIdentity: Equatable {
        let path: String
        let dev: dev_t
        let ino: ino_t
    }
    /// The running executable's path and inode on disk, or nil if the bundle was moved or deleted.
    /// macOS's out-of-process NSOpenPanel (openAndSavePanelService) validates the host binary on disk via
    /// SecCodeCopyGuestWithAttributes; if the bundle was replaced or removed while running, ViewBridge faults
    /// and blocks the main thread for ~10s (spinning beachball) before failing to display the panel.
    static func currentBinaryIdentity() -> BinaryIdentity? {
        var buf = [CChar](repeating: 0, count: Int(MAXPATHLEN) * 4)
        let n = proc_pidpath(getpid(), &buf, UInt32(buf.count))
        guard n > 0 else { return nil }
        let path = String(cString: buf)
        var st = stat()
        guard stat(path, &st) == 0 else { return nil }
        return BinaryIdentity(path: path, dev: st.st_dev, ino: st.st_ino)
    }
    private func canPresentOpenPanel() -> Bool {
        guard let initial = launchBinary, let current = CoderWindow.currentBinaryIdentity() else { return false }
        return initial == current
    }
    private func chooseFolderFallback() {
        let alert = NSAlert()
        alert.messageText = "Choose Working Folder"
        alert.informativeText = "Enter the path to the working folder (commands start there). Restart Coder to restore the Finder folder picker after an app update."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        field.stringValue = folder.path
        field.placeholderString = FileManager.default.homeDirectoryForCurrentUser.path
        alert.accessoryView = field
        alert.addButton(withTitle: "Choose")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            self.choosingFolder = false
            defer { self.refocus() }
            guard answer == .alertFirstButtonReturn else { return }
            let raw = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !raw.isEmpty else { return }
            let expanded = (raw as NSString).expandingTildeInPath
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: expanded, isDirectory: &isDir), isDir.boolValue {
                self.setFolder(URL(fileURLWithPath: expanded, isDirectory: true), keep: true)
            }
        }
    }
    func chooseFolder() {
        guard cd_running(page) == 0, !choosingFolder, window.attachedSheet == nil else { return }
        choosingFolder = true
        // Defer presentation out of the display link's .common run loop tick.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            guard cd_running(self.page) == 0, self.window.attachedSheet == nil else { self.choosingFolder = false; return }
            guard self.canPresentOpenPanel() else { self.chooseFolderFallback(); return }
            let panel = NSOpenPanel()
            panel.canChooseDirectories = true; panel.canChooseFiles = false; panel.allowsMultipleSelection = false; panel.canCreateDirectories = true
            var isDir: ObjCBool = false
            panel.directoryURL = (FileManager.default.fileExists(atPath: self.folder.path, isDirectory: &isDir) && isDir.boolValue)
                ? self.folder : FileManager.default.homeDirectoryForCurrentUser
            panel.prompt = "Choose"; panel.message = "Choose the working folder. Commands start there."
            panel.beginSheetModal(for: self.window) { [weak self] answer in
                guard let self else { return }
                self.choosingFolder = false
                if answer == .OK, let url = panel.url { self.setFolder(url, keep: true) }
                self.refocus()
            }
        }
    }
    /// File > New Chat (Command-N): what the New chat button does.
    func newChat() { if cd_new(page) != 0 { window.makeFirstResponder(editor) }; wake() }
    /// Opens the Nth chat from the top of the sidebar list (1 is the first).
    func openChat(at index: Int) {
        let ordered = store.orderedList
        guard index >= 1, index <= ordered.count else { return }
        open(ordered[index - 1].id)
    }
    /// Opens a kept chat: it becomes the chat that is shown, with the working folder it had. A reply
    /// running in the shown chat goes on out of view; a chat whose reply runs out of view comes back
    /// with its run.
    func open(_ id: String) {
        if id == Foreman.chatID { openForeman(); return }
        guard id != chat, detach() else { return }
        if let back = runs[id] {
            guard cd_take(page, back.page) != 0 else { return }
            runs[id] = nil; cd_close(back.page); back.page = page
            run.stop()   // the window's idle worker: a message in a chat opened later starts another
            run = back
            unread.remove(id)
            chat = id; recallDue = false; cd_current(page, id)
            setFolder(back.folder, keep: false)
            configureWorkerEnv(run, isForeman: false)
            syncSkillsNote(for: page, isForeman: false)
            cd_chats(page, chatLines); syncForemanUI()
            window.makeFirstResponder(editor); wake()
            return
        }
        guard let text = store.text(id), cd_load(page, text) != 0 else {
            let failed = NSAlert()
            failed.messageText = "This chat could not be opened."
            failed.informativeText = "“" + (store.title(id) ?? "Chat") + "” could not be read."
            failed.beginSheetModal(for: window)
            return
        }
        let clearedUnread = unread.remove(id) != nil
        chat = id; recallDue = true; cd_current(page, id)
        configureWorkerEnv(run, isForeman: false)
        syncSkillsNote(for: page, isForeman: false)
        syncForemanUI()
        if clearedUnread { cd_chats(page, chatLines) }
        var isFolder: ObjCBool = false
        if let kept = store.folder(id), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue { setFolder(URL(fileURLWithPath: kept), keep: false) }
        showKeptQueue(id)
        window.makeFirstResponder(editor)
        wake()
    }
    /// The chat that is shown, when it is kept: File > Rename Chat acts on it.
    var canRename: Bool { chat != nil && chat != Foreman.chatID }
    func renameShown() { if let id = chat, id != Foreman.chatID { rename(id) } }
    /// The chat that is shown, when it is kept and no reply is running: File > Delete Chat acts on it.
    var canDelete: Bool { chat != nil && chat != Foreman.chatID && cd_running(page) == 0 }
    func deleteShown() { if let id = chat { delete(id) } }
    /// Asks for a new title, then renames a kept chat.
    func rename(_ id: String) {
        guard let current = store.title(id) else { return }
        let alert = NSAlert()
        alert.messageText = "Rename Chat"
        alert.informativeText = "Enter a new title for this chat."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        field.stringValue = current
        field.placeholderString = "Chat title"
        alert.accessoryView = field
        alert.addButton(withTitle: "Rename")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == .alertFirstButtonReturn else { return }
            let title = CoderStore.cleanTitle(field.stringValue)
            guard !title.isEmpty else { return }
            self.store.rename(id, to: title)
            self.daemonClient.updateSessionMeta(sessionID: id, title: title)
            cd_chats(self.page, self.chatLines)
            self.wake()
        }
    }
    /// Whether a reply is running in a chat, shown or out of view: it can't be deleted then.
    func busy(_ id: String) -> Bool { runs[id] != nil || (id == chat && cd_running(page) != 0) }
    /// Asks, then deletes a kept chat. Deleting the one that is shown starts a new chat. Not while a
    /// reply is running in it.
    func delete(_ id: String) {
        if busy(id) { return }
        let alert = NSAlert()
        alert.messageText = "Delete this chat?"
        alert.informativeText = "“" + (store.title(id) ?? "New chat") + "” will be deleted from this Mac. You can't undo this."
        alert.addButton(withTitle: "Delete").hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self, answer == .alertFirstButtonReturn else { return }
            if id == self.chat { guard cd_running(self.page) == 0 else { return }; self.chat = nil; _ = cd_new(self.page); cd_current(self.page, ""); self.syncForemanUI() }
            guard !self.busy(id) else { return }
            self.unread.remove(id)
            self.store.remove(id)
            self.daemonClient.updateSessionMeta(sessionID: id, archived: true)
            cd_chats(self.page, self.chatLines)
            self.refocus(); self.wake()
        }
    }
    /// Keeps a chat: its file, and its entry in the list (at the top when new, in place when updated).
    /// The shown one, or one whose reply runs out of view.
    func keep(_ which: CoderRun? = nil, closing: Bool = false) {
        guard closing || !retired else { return }
        let r = which ?? run, shown = r === run
        let id = (shown ? chat : r.chat) ?? UUID().uuidString.lowercased()
        var out = [CChar](repeating: 0, count: 32_000_000)   // 24 messages with every row and every sub-agent's rows full fit
        guard cd_save(r.page, &out, Int32(out.count)) > 0 else { return }
        let text = String(cString: out)
        var name = [CChar](repeating: 0, count: 256)
        let title = cd_title(r.page, &name, Int32(name.count)) > 0 ? String(cString: name) : "New chat"
        store.keep(id: id, title: title, folder: (shown ? folder : r.folder).path, text: text)
        if shown { chat = id; cd_current(page, id) } else { r.chat = id; if id != chat { unread.insert(id) } }
        cd_chats(page, chatLines); syncForemanUI()
    }
}
