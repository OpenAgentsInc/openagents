// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Chats: opening, keeping, renaming, deleting, the working folder and text size.
import AppKit

extension CoderWindow {
    var canFolder: Bool { cd_running(page) == 0 }
    /// View > Bigger Text, Smaller Text, Actual Size: by is a row up or down, nil the usual size.
    func zoom(_ by: Int32?) {
        let row = by.map { cd_zoom(page, $0) } ?? cd_zoom_set(page, 2)
        UserDefaults.standard.set(Int(row), forKey: "zoom")
        wake()
    }
    func setFolder(_ url: URL, keep: Bool) {
        folder = url.standardizedFileURL
        if keep { UserDefaults.standard.set(folder.path, forKey: "folder") }
        let home = FileManager.default.homeDirectoryForCurrentUser.path, path = folder.path
        cd_folder(page, path == home ? "~" : path.hasPrefix(home + "/") ? "~" + path.dropFirst(home.count) : path)
        refreshBranch()
        wake()
    }
    /// The working folder's Git branch for the composer's foot (#13743): HEAD's branch, a detached
    /// HEAD's short commit, "" outside a repository. Read again when a reply ends.
    func refreshBranch() {
        cd_branch(page, ChatWorktree.shown(CoderWindow.gitBranch(folder)))
        refreshGitStatus()
    }
    static func gitBranch(_ folder: URL) -> String {
        var dir = folder.standardizedFileURL
        let fm = FileManager.default
        // At most 256 folders up: past "/" a directory URL's parent is "/../", whose path never
        // repeats, and an unbounded walk on the main thread hung launch and grew without end (#13752).
        for _ in 0..<256 {
            let dot = dir.appendingPathComponent(".git")
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: dot.path, isDirectory: &isDir) {
                var gitDir = dot
                if !isDir.boolValue {   // a worktree's or a submodule's .git file: "gitdir: <path>"
                    guard let line = try? String(contentsOf: dot, encoding: .utf8), line.hasPrefix("gitdir:") else { return "" }
                    let p = line.dropFirst(7).trimmingCharacters(in: .whitespacesAndNewlines)
                    gitDir = p.hasPrefix("/") ? URL(fileURLWithPath: p) : dir.appendingPathComponent(p)
                }
                guard let head = try? String(contentsOf: gitDir.appendingPathComponent("HEAD"), encoding: .utf8) else { return "" }
                let h = head.trimmingCharacters(in: .whitespacesAndNewlines)
                if h.hasPrefix("ref: refs/heads/") { return String(h.dropFirst(16)) }
                return String(h.prefix(7))
            }
            if dir.path == "/" || dir.path.isEmpty { return "" }
            let up = dir.deletingLastPathComponent().standardizedFileURL
            if up.path == dir.path { return "" }
            dir = up
        }
        return ""
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
    func canPresentOpenPanel() -> Bool {
        guard let initial = launchBinary, let current = CoderWindow.currentBinaryIdentity() else { return false }
        return initial == current
    }
    private func chooseFolderFallback() {
        var d = CoderDialog(size: .md, title: "Choose Working Folder",
                            body: "Enter the path to the working folder (commands start there). Restart Coder to restore the Finder folder picker after an app update.")
        d.fields = [.init(value: folder.path, hint: FileManager.default.homeDirectoryForCurrentUser.path)]
        d.buttons = [.init(title: "Cancel"), .init(title: "Choose", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            self.choosingFolder = false
            defer { self.refocus() }
            guard answer == 1 else { return }
            let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
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
    func newChat() { if cd_new(page) != 0 { focusComposer() }; wake() }
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
        if id == chat { closeGym(); focusComposer(); wake(); return }
        closeGym()
        guard detach() else { return }
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
            focusComposer(); wake()
            return
        }
        guard let text = store.text(id), cd_load(page, text) != 0 else {
            present(.notice("This chat could not be opened.", "“" + (store.title(id) ?? "Chat") + "” could not be read."))
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
        else { restoreWorktree(id) }
        showKeptQueue(id)
        focusComposer()
        wake()
    }
    /// The chat that is shown, when it is kept: File > Rename Chat acts on it.
    var canRename: Bool { chat != nil && chat != Foreman.chatID }
    func renameShown() { if let id = chat, id != Foreman.chatID { rename(id) } }
    var canFork: Bool { chat != Foreman.chatID && (chat != nil || savedText(of: page) != nil) }
    func forkShown() { guard canFork else { return }; fork(chat) }
    /// Forks a chat (the shown chat when `id == nil` or `id == chat`, or a sidebar chat by id) into a
    /// new independent chat with the same transcript and working folder, then opens it.
    func fork(_ id: String?) {
        if let id, id == Foreman.chatID { return }
        let text: String
        let baseTitle: String
        let baseFolder: String
        if id == nil || id == chat {
            guard let saved = savedText(of: page) else { return }
            text = saved
            baseTitle = (chat.flatMap { store.title($0) }) ?? titleFromFirstTurn(saved) ?? "New chat"
            baseFolder = (chat.flatMap { store.folder($0) }) ?? folder.path
        } else if let id, let saved = store.text(id) {
            text = saved
            baseTitle = store.title(id) ?? "New chat"
            baseFolder = store.folder(id) ?? folder.path
        } else {
            return
        }
        let newID = UUID().uuidString.lowercased()
        let newTitle = baseTitle.hasSuffix(" (fork)") ? baseTitle : baseTitle + " (fork)"
        store.keep(id: newID, title: newTitle, folder: baseFolder, text: text)
        cd_chats(page, chatLines)
        open(newID)
    }
    private func savedText(of p: UnsafeMutableRawPointer) -> String? {
        var cap = 262144
        while cap <= 128_000_000 {
            var buf = [CChar](repeating: 0, count: cap)
            let n = cd_save(p, &buf, Int32(buf.count))
            if n <= 0 { return nil }
            if n < Int32(cap) { return String(cString: buf) }
            cap *= 2
        }
        return nil
    }
    private func titleFromFirstTurn(_ saved: String) -> String? {
        guard let data = saved.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let turns = obj["turns"] as? [[String: Any]],
              let first = turns.first?["ask"] as? String,
              !first.isEmpty else { return nil }
        return first
    }
    /// The chat that is shown, when it is kept and no reply is running: File > Delete Chat acts on it.
    var canDelete: Bool { chat != nil && chat != Foreman.chatID && cd_running(page) == 0 }
    func deleteShown() { if let id = chat { delete(id) } }
    /// Asks for a new title, then renames a kept chat.
    func rename(_ id: String) {
        guard let current = store.title(id) else { return }
        var d = CoderDialog(title: "Rename Chat", body: "Enter a new title for this chat.")
        d.fields = [.init(value: current, hint: "Chat title")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Rename", kind: .primary)]
        d.confirm = 1; d.cancel = 0; d.selectAll = true
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.refocus() }
            guard answer == 1 else { return }
            let title = CoderStore.cleanTitle(values.first ?? "")
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
        // destructive: Return stays on Cancel
        var d = CoderDialog(title: "Delete this chat?", body: "“" + (store.title(id) ?? "New chat") + "” will be deleted from this Mac. You can't undo this.")
        d.buttons = [.init(title: "Cancel"), .init(title: "Delete", kind: .danger)]
        d.confirm = 0; d.cancel = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            guard answer == 1 else { self.refocus(); return }
            if id == self.chat { guard cd_running(self.page) == 0 else { return }; self.chat = nil; _ = cd_new(self.page); cd_current(self.page, ""); self.syncForemanUI() }
            guard !self.busy(id) else { return }
            self.unread.remove(id)
            if let w = self.store.worktree(id) { self.removeWorktree(w, title: self.store.title(id) ?? "New chat") }
            self.store.remove(id)
            self.daemonClient.updateSessionMeta(sessionID: id, archived: true)
            cd_chats(self.page, self.chatLines)
            self.refocus(); self.wake()
        }
    }
    /// A page's chat as the text the store keeps (cd_save). The buffer grows until the chat fits:
    /// a chat has no message cap (#13745), so it can outgrow any one size. Nil when it never fits.
    func savedChat(_ p: UnsafeMutableRawPointer, from size: Int = 32_000_000) -> String? {
        var cap = size
        while cap <= 1_536_000_000 {
            var out = [CChar](repeating: 0, count: cap)
            if cd_save(p, &out, Int32(cap)) > 0 { return String(cString: out) }
            cap *= 2
        }
        return nil
    }
    /// Keeps a chat: its file, and its entry in the list (at the top when new, in place when updated).
    /// The shown one, or one whose reply runs out of view.
    func keep(_ which: CoderRun? = nil, closing: Bool = false) {
        guard closing || !retired else { return }
        let r = which ?? run, shown = r === run
        let id = (shown ? chat : r.chat) ?? pendingChatID(r) ?? UUID().uuidString.lowercased()
        guard let text = savedChat(r.page) else { return }
        var name = [CChar](repeating: 0, count: 256)
        let title = cd_title(r.page, &name, Int32(name.count)) > 0 ? String(cString: name) : "New chat"
        store.keep(id: id, title: title, folder: (shown ? folder : r.folder).path, text: text, worktree: madeWorktree(r, for: id))
        if shown { chat = id; cd_current(page, id) } else { r.chat = id; if id != chat { unread.insert(id) } }
        // the worker's queue binds to the chat once it is kept, so its run names the chat: a new
        // chat's first message and a Foreman chat start before they have an id (#13755)
        if r.bound != id { r.bound = id; bindQueue(r) }
        cd_chats(page, chatLines); syncForemanUI()
    }
}
