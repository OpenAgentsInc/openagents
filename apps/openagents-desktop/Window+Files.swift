// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The file in the pane on the window's right (page/filepane.coax, GitHub #13760): the page draws
// and edits it; this reads it from disk, writes it back, opens it in the Mac's own app for it, and
// asks before changes are lost. A press on a file a row or a reply names comes here, and so does a
// .coax file the Mac opens with Coder (AppDelegate.swift, #13759).
import AppKit
import UniformTypeIdentifiers

extension CoderWindow {
    /// A file a row, a reply or an attachment names: opens in the pane (viewer-first syntax-highlighted
    /// code up to 4 MB, explicit inline edit up to 128 KB, or rich read-only preview for CSV/TSV, .ipynb,
    /// images, PDFs, binary hex).
    func openNamedFile(_ url: URL, jumpRange: (Int, Int)? = nil) {
        #if OA_FULL
        if (FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder)) && openInMetaverseIfApplicable(url, autoOpened: false) {
            return
        }
        if PleSceneLoader.isSceneFile(url) || PleSceneLoader.isIfcFile(url) {
            openFileInPane(url, jumpRange: nil)
            return
        }
        #endif
        openFileInPane(url, jumpRange: jumpRange)
    }

    /// Parses `#Lstart-Lend`, `#Lstart`, `:start-end`, `:line:col`, or `:line` suffixes out of a raw file link (#13992).
    static func parseFileLink(_ raw: String) -> (path: String, startLine: Int, endLine: Int) {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return ("", 0, 0) }
        var startLine = 0
        var endLine = 0
        let parts = trimmed.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)
        let noFrag = String(parts[0])
        if parts.count == 2 {
            let frag = String(parts[1])
            let body = (frag.hasPrefix("L") || frag.hasPrefix("l")) ? String(frag.dropFirst()) : frag
            let rangeParts = body.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
            if let s = Int(rangeParts[0]), s > 0 {
                startLine = s
                if rangeParts.count == 2 {
                    let rhs = String(rangeParts[1])
                    let rhsDigits = (rhs.hasPrefix("L") || rhs.hasPrefix("l")) ? String(rhs.dropFirst()) : rhs
                    endLine = Int(rhsDigits) ?? s
                } else {
                    endLine = s
                }
            }
        }
        var pathStr: String
        if noFrag.lowercased().hasPrefix("file://") {
            if let u = URL(string: noFrag), u.isFileURL {
                pathStr = u.path
            } else {
                let dropped = String(noFrag.dropFirst(7))
                pathStr = dropped.removingPercentEncoding ?? dropped
            }
        } else {
            pathStr = noFrag
            // Strip trailing :start-end, :line:col, or :line when the full string is not an existing file.
            if !FileManager.default.fileExists(atPath: (pathStr as NSString).expandingTildeInPath) {
                if let colon = pathStr.lastIndex(of: ":"), colon > pathStr.startIndex {
                    let tail = String(pathStr[pathStr.index(after: colon)...])
                    if let dash = tail.firstIndex(of: "-") {
                        let a = String(tail[..<dash])
                        let b = String(tail[tail.index(after: dash)...])
                        if let s = Int(a), let e = Int(b), s > 0, e >= s {
                            pathStr = String(pathStr[..<colon])
                            if startLine == 0 { startLine = s; endLine = e }
                        }
                    } else if let n1 = Int(tail), n1 > 0, tail.allSatisfy({ $0.isNumber }) {
                        var base = String(pathStr[..<colon])
                        var lineVal = n1
                        if let colon2 = base.lastIndex(of: ":"), colon2 > base.startIndex {
                            let mid = String(base[base.index(after: colon2)...])
                            if let n0 = Int(mid), n0 > 0, mid.allSatisfy({ $0.isNumber }) {
                                base = String(base[..<colon2])
                                lineVal = n0
                            }
                        }
                        pathStr = base
                        if startLine == 0 { startLine = lineVal; endLine = lineVal }
                    }
                }
            }
        }
        if startLine > 0 && endLine < startLine { endLine = startLine }
        return (pathStr, startLine, endLine)
    }

    /// Resolves a `file://` URL, absolute path, tilde path, or repo-relative path (optionally carrying `#L10-L20` or `:10`)
    /// and opens it in the right-pane File Viewer, jumping smoothly to any line anchor (#13992).
    func openFileLink(_ raw: String) {
        let parsed = Self.parseFileLink(raw)
        guard !parsed.path.isEmpty else { return }
        let expanded = (parsed.path as NSString).expandingTildeInPath
        let url: URL
        if expanded.hasPrefix("/") {
            url = URL(fileURLWithPath: expanded)
        } else {
            let candidate = folder.appendingPathComponent(expanded)
            if FileManager.default.fileExists(atPath: candidate.path) {
                url = candidate
            } else if let src = worktreeSource(folder.path) {
                let fallback = URL(fileURLWithPath: src).appendingPathComponent(expanded)
                url = FileManager.default.fileExists(atPath: fallback.path) ? fallback : candidate
            } else {
                url = candidate
            }
        }
        let jump: (Int, Int)? = parsed.startLine > 0 ? (parsed.startLine, parsed.endLine) : nil
        openNamedFile(url, jumpRange: jump)
    }

    /// A file into the pane. The open one, if it has changes, is asked about first; the same file
    /// opened again comes forward with its changes, or is read again when it has none.
    func openFileInPane(_ url: URL, jumpRange: (Int, Int)? = nil) {
        let url = url.standardizedFileURL
        if let open = fileURL, cd_file_mode(page) != 0, cd_file_dirty(page) != 0 {
            if open == url {
                _ = cd_file_front(page)
                if let r = jumpRange { cd_file_jump(page, Int32(r.0), Int32(r.1)) }
                wake()
                return
            }
            askToSave { [weak self] in self?.readIntoPane(url, jumpRange: jumpRange) }
            return
        }
        readIntoPane(url, jumpRange: jumpRange)
    }

    /// Reads url and hands it to the page: its bytes, or why it can't be shown.
    private func readIntoPane(_ url: URL, jumpRange: (Int, Int)? = nil) {
        stopFileWatcher()
        fileURL = url
        // Pass full path so Row 2 breadcrumbs render Users › ... › dir › filename (#13992).
        let shown = url.path
        let name = url.lastPathComponent
        var isFolder: ObjCBool = false
        guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isFolder) else {
            #if OA_FULL
            hideScenePanel()
            #endif
            cd_file_failed(page, shown, "Coder couldn't find \(name). It may have been moved or deleted."); wake(); return
        }
        if isFolder.boolValue {
            #if OA_FULL
            hideScenePanel()
            #endif
            cd_file_failed(page, shown, "\(name) is a folder, not a file."); wake(); return
        }
        #if OA_FULL
        if FeatureFlags.isOn(.scenes) && (PleSceneLoader.isSceneFile(url) || PleSceneLoader.isIfcFile(url)) {
            openSceneInPane(url, shown: shown)
            return
        }
        hideScenePanel()
        #endif
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        let size = (attrs?[.size] as? NSNumber)?.int64Value ?? 0
        let mtime = attrs?[.modificationDate] as? Date
        let most = Int64(cd_file_most())
        if size > most {
            let f = ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
            let capLabel = most >= 1024 * 1024 ? "\(most / (1024 * 1024)) MB" : "\(most / 1024) KB"
            cd_file_failed(page, shown, "\(name) is \(f). Coder shows files up to \(capLabel) here; open it in its own app with the button above.")
            wake(); return
        }
        let data: Data
        do { data = try Data(contentsOf: url) } catch {
            cd_file_failed(page, shown, "Coder couldn't read \(name): \(error.localizedDescription)"); wake(); return
        }
        let ok = data.withUnsafeBytes { raw in
            cd_file_open(page, shown, raw.bindMemory(to: CChar.self).baseAddress, Int32(data.count), Int64(size))
        }
        if ok == 0 { cd_file_failed(page, shown, "Coder couldn't show \(name) here.") }
        else {
            fileWatchMtime = mtime
            fileWatchSize = size
            startFileWatcher(for: url)
            if let r = jumpRange {
                cd_file_jump(page, Int32(r.0), Int32(r.1))
            }
            if cd_file_mode(page) == 1 { window.makeFirstResponder(canvas) }
            #if OA_FULL
            if url.pathExtension.lowercased() == "lean" { refreshFilePaneLeanGoal() }
            #endif
        }
        wake()
    }

    // MARK: - Live File Watcher (DispatchSourceFileSystemObject, #13992)

    func stopFileWatcher() {
        fileWatchWork?.cancel()
        fileWatchWork = nil
        fileWatchSource?.cancel()
        fileWatchSource = nil
        fileWatchMtime = nil
        fileWatchSize = -1
    }

    private func startFileWatcher(for url: URL) {
        fileWatchWork?.cancel()
        fileWatchWork = nil
        fileWatchSource?.cancel()
        fileWatchSource = nil
        let fd = Darwin.open(url.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .rename, .delete, .attrib],
            queue: .main
        )
        src.setCancelHandler { Darwin.close(fd) }
        src.setEventHandler { [weak self, weak src] in
            guard let self, let src else { return }
            let flags = src.data
            let rearm = flags.contains(.rename) || flags.contains(.delete)
            self.scheduleFileWatchCheck(for: url, rearm: rearm)
        }
        fileWatchSource = src
        src.resume()
    }

    private func scheduleFileWatchCheck(for url: URL, rearm: Bool) {
        fileWatchWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.fileURL == url, cd_file_mode(self.page) != 0 else { return }
            if rearm { self.startFileWatcher(for: url) }
            guard let attrs = try? FileManager.default.attributesOfItem(atPath: url.path) else { return }
            let mtime = attrs[.modificationDate] as? Date
            let size = (attrs[.size] as? NSNumber)?.int64Value ?? -1
            if mtime == self.fileWatchMtime && size == self.fileWatchSize { return }
            self.fileWatchMtime = mtime
            self.fileWatchSize = size
            if cd_file_dirty(self.page) != 0 {
                cd_file_disk_changed(self.page)
                self.wake()
            } else {
                self.reloadFileFromDisk(url)
            }
        }
        fileWatchWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: work)
    }

    /// Re-reads url into the open File Viewer via cd_file_reload, preserving scroll offset and code folds (#13992).
    private func reloadFileFromDisk(_ url: URL) {
        guard fileURL == url else { return }
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        let size = (attrs?[.size] as? NSNumber)?.int64Value ?? 0
        let most = Int64(cd_file_most())
        guard size <= most, let data = try? Data(contentsOf: url) else { return }
        fileWatchMtime = attrs?[.modificationDate] as? Date
        fileWatchSize = size
        _ = data.withUnsafeBytes { raw in
            cd_file_reload(page, raw.bindMemory(to: CChar.self).baseAddress, Int32(data.count), size)
        }
        #if OA_FULL
        if url.pathExtension.lowercased() == "lean" { refreshFilePaneLeanGoal() }
        #endif
        wake()
    }

    #if OA_FULL
    /// Asynchronously queries oa-leanhost (`lean-run goal`) for the open .lean file at cursor line (#13947).
    func refreshFilePaneLeanGoal() {
        guard let url = fileURL, url.pathExtension.lowercased() == "lean" else { return }
        let line = max(1, Int(cd_file_line(page)))
        runLeanTool(["goal", url.path, "\(line)", "5", "--json"]) { [weak self] out in
            guard let self, self.fileURL == url else { return }
            guard let out, let data = out.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
                cd_file_lean_goal(self.page, Int32(line), 0, 0, "oa-leanhost unavailable (install elan / Lean 4)")
                self.wake()
                return
            }
            let ok: Int32 = (obj["ok"] as? Bool) == true ? 1 : 0
            let done: Int32 = (obj["accomplished"] as? Bool) == true ? 1 : 0
            let summary = (obj["summary"] as? String) ?? ((obj["error"] as? String) ?? "No tactic goal at cursor")
            cd_file_lean_goal(self.page, Int32(line), ok, done, summary)
            self.wake()
        }
    }
    #endif

    /// What the pane asked (cd_action 40..43, 164..169).
    func fileAsked(_ what: Int32) {
        switch what {
        case 40: _ = saveOpenFile()
        case 41: openFileExternally()
        case 42: closeFilePane()
        case 43: window.makeFirstResponder(canvas); wake()
        case 164: reloadOpenFile()
        case 165: copyOpenFilePath()
        case 166: listBreadcrumbDir()
        case 167: copyOpenFileRelativePath()
        case 168: copyOpenFileContents()
        case 169: revealOpenFileInFinder()
        default: break
        }
    }

    /// Re-reads the currently open file from disk, preserving scroll offset & folds (asks first if dirty).
    func reloadOpenFile() {
        guard let url = fileURL else { return }
        if cd_file_dirty(page) != 0 {
            askToSave { [weak self] in self?.reloadFileFromDisk(url) }
            return
        }
        reloadFileFromDisk(url)
    }

    /// Populates the breadcrumb sibling-directory popover (CP_ASK_FILE_DIR_LIST = 166, #13992).
    func listBreadcrumbDir() {
        var buf = [CChar](repeating: 0, count: 1024)
        let n = cd_file_crumb_dir(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let dirPath = String(cString: buf)
        let dirURL = URL(fileURLWithPath: dirPath)
        let fm = FileManager.default
        guard let items = try? fm.contentsOfDirectory(
            at: dirURL,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            _ = cd_file_dir_set(page, "")
            wake()
            return
        }
        var entries: [(isDir: Bool, name: String)] = items.map { item in
            let isDir = (try? item.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            return (isDir, item.lastPathComponent)
        }
        entries.sort { a, b in
            if a.isDir != b.isDir { return a.isDir }
            return a.name.localizedCaseInsensitiveCompare(b.name) == .orderedAscending
        }
        let payload = entries.prefix(64).map { ($0.isDir ? "D\t" : "F\t") + $0.name }.joined(separator: "\n")
        _ = cd_file_dir_set(page, payload)
        wake()
    }

    /// Active file's path relative to the workspace folder (or full path when outside workspace).
    func openFileRelativePath() -> String {
        let full: String
        if let url = fileURL {
            full = url.standardizedFileURL.path
        } else {
            var out = [CChar](repeating: 0, count: 1024)
            guard cd_file_path(page, &out, Int32(out.count)) > 0 else { return "" }
            full = String(cString: out)
        }
        let base = folder.standardizedFileURL.path
        let prefix = base.hasSuffix("/") ? base : base + "/"
        if full.hasPrefix(prefix) {
            return String(full.dropFirst(prefix.count))
        }
        return full
    }

    /// Copies the active file's full path to the system clipboard.
    func copyOpenFilePath() {
        let path: String
        if let url = fileURL {
            path = url.path
        } else {
            var out = [CChar](repeating: 0, count: 1024)
            guard cd_file_path(page, &out, Int32(out.count)) > 0 else { return }
            path = String(cString: out)
        }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(path, forType: .string)
    }

    /// Copies the active file's workspace-relative path to the system clipboard (#13992).
    func copyOpenFileRelativePath() {
        let rel = openFileRelativePath()
        guard !rel.isEmpty else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(rel, forType: .string)
    }

    /// Copies the active file's full contents (edited buffer if in edit mode, else file on disk) to clipboard (#13992).
    func copyOpenFileContents() {
        let text: String?
        if cd_file_mode(page) == 1 {
            let cap = max(Int(cd_file_edit_most()), 16384) + 1
            var out = [CChar](repeating: 0, count: cap)
            let n = cd_file_text(page, &out, Int32(out.count))
            if n >= 0 {
                let bytes = out.withUnsafeBufferPointer { Data(buffer: UnsafeBufferPointer(rebasing: $0[0..<Int(n)])) }
                text = String(data: bytes, encoding: .utf8)
            } else {
                text = nil
            }
        } else if let url = fileURL, let data = try? Data(contentsOf: url) {
            text = String(data: data, encoding: .utf8)
        } else {
            text = nil
        }
        guard let text else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
    }

    /// Reveals the active file in Finder (#13992).
    func revealOpenFileInFinder() {
        guard let url = fileURL, FileManager.default.fileExists(atPath: url.path) else { return }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    /// File > Save works while the pane shows a file with changes.
    var canSaveOpenFile: Bool { cd_file_shown(page) != 0 && cd_file_dirty(page) != 0 }
    /// The file has changes that are not saved (quitting asks first).
    var openFileDirty: Bool { cd_file_mode(page) != 0 && cd_file_dirty(page) != 0 }
    var openFileName: String {
        var out = [CChar](repeating: 0, count: 1024)
        return cd_file_name(page, &out, Int32(out.count)) > 0 ? String(cString: out) : "the file"
    }

    /// Writes what the editor holds to the file, in place (its permissions and links kept). True when it was written.
    @discardableResult
    func saveOpenFile() -> Bool {
        guard let url = fileURL, cd_file_mode(page) == 1 else { return false }
        let cap = max(Int(cd_file_edit_most()), 16384) + 1
        var out = [CChar](repeating: 0, count: cap)
        let n = cd_file_text(page, &out, Int32(out.count))
        guard n >= 0 else { return false }
        let data = out.withUnsafeBufferPointer { Data(buffer: UnsafeBufferPointer(rebasing: $0[0..<Int(n)])) }
        do {
            try data.write(to: url)
        } catch {
            cd_file_save_failed(page, "Coder couldn't save \(url.lastPathComponent): \(error.localizedDescription)")
            wake(); return false
        }
        if let attrs = try? FileManager.default.attributesOfItem(atPath: url.path) {
            fileWatchMtime = attrs[.modificationDate] as? Date
            fileWatchSize = (attrs[.size] as? NSNumber)?.int64Value ?? Int64(data.count)
        }
        data.withUnsafeBytes { raw in cd_file_saved(page, raw.bindMemory(to: CChar.self).baseAddress, Int32(data.count)) }
        wake()
        return true
    }

    /// Close: with changes, Save, Don't Save or Cancel first.
    func closeFilePane() {
        guard cd_file_mode(page) != 0 else { return }
        if cd_file_dirty(page) != 0 { askToSave { [weak self] in self?.dropFile() }; return }
        dropFile()
    }
    func dropFile() {
        let hadKeys = cd_wants_text(page) != 0
        stopFileWatcher()
        #if OA_FULL
        hideScenePanel()
        #endif
        cd_file_close(page); fileURL = nil
        if hadKeys { refocus() } else { wake() }
    }

    /// Asks whether to save the file's changes, then (unless Cancel, or a save that failed) does next;
    /// cancelled runs when it does not.
    func askToSave(then next: @escaping () -> Void, cancelled: (() -> Void)? = nil) {
        let name = openFileName
        var d = CoderDialog(title: "Save changes to \(name)?", body: "Your changes are lost if you don't save them.")
        d.buttons = [CoderDialog.Button(title: "Don't Save", kind: .danger, aside: true),
                     CoderDialog.Button(title: "Cancel"),
                     CoderDialog.Button(title: "Save", kind: .primary)]
        d.confirm = 2; d.cancel = 1
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            switch answer {
            case 2: if self.saveOpenFile() { next() } else { cancelled?() }
            case 0: next()
            default: cancelled?()
            }
            self.refocus()
        }
    }

    /// The file in the Mac's own app for it. When that is Coder (a .coax file), its text editor instead.
    func openFileExternally() {
        guard let url = fileURL, FileManager.default.fileExists(atPath: url.path) else { return }
        let me = Bundle.main.bundleURL.standardizedFileURL
        let ws = NSWorkspace.shared
        if let app = ws.urlForApplication(toOpen: url), app.standardizedFileURL != me { ws.open(url); return }
        if let editor = ws.urlForApplication(toOpen: .plainText), editor.standardizedFileURL != me {
            ws.open([url], withApplicationAt: editor, configuration: NSWorkspace.OpenConfiguration())
            return
        }
        ws.activateFileViewerSelecting([url])
    }

    // MARK: - File Explorer Tree, Multi-Tab & Open-in-Editor Picker (page/filetree.coax, GitHub #13881)

    /// Handles File Explorer / Open-in-Editor host asks (cd_action 86..89).
    func fileTreeAsked(_ what: Int32) {
        switch what {
        case 86: refreshFileTree()
        case 87: openFileTreeRequestedPath()
        case 88: launchPickedEditor()
        case 89: closeFileTreeTab()
        default: break
        }
    }

    /// Populates the collapsible File Explorer directory tree from git ls-files / folder walk
    /// (respecting .gitignore) and refreshes the detected installed editors catalog.
    func refreshFileTree() {
        detectInstalledEditors()
        let root = folder
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let lines = CoderWindow.folderFiles(root, most: 1024)
            DispatchQueue.main.async {
                guard let self, self.folder == root else { return }
                _ = cd_filetree_set(self.page, lines)
                self.wake()
            }
        }
    }

    /// Detects installed macOS IDEs/terminals and hands the installed mask to page/filetree.coax.
    func detectInstalledEditors() {
        let fm = FileManager.default
        var mask: Int32 = 0
        let checks: [(Int32, [String])] = [
            (0, ["/Applications/Cursor.app", "/usr/local/bin/cursor", "/opt/homebrew/bin/cursor"]),
            (1, ["/Applications/Visual Studio Code.app", "/usr/local/bin/code", "/opt/homebrew/bin/code"]),
            (2, ["/Applications/Zed.app", "/usr/local/bin/zed", "/opt/homebrew/bin/zed"]),
            (3, ["/Applications/Xcode.app", "/usr/bin/xed"]),
            (4, ["/Applications/Ghostty.app", "/opt/homebrew/bin/ghostty"]),
            (5, ["/System/Library/CoreServices/Finder.app"]),
        ]
        for (bit, paths) in checks {
            if paths.contains(where: { fm.fileExists(atPath: $0) }) {
                mask |= (1 << bit)
            }
        }
        let custom = UserDefaults.standard.string(forKey: "openagents.customEditor") ?? ""
        let sshHost = UserDefaults.standard.string(forKey: "openagents.sshHost") ?? ""
        cd_editors_set(page, mask, custom, sshHost)
    }

    /// Opens the path selected in the File Explorer tree or multi-tab bar.
    private func openFileTreeRequestedPath() {
        var buf = [CChar](repeating: 0, count: 1024)
        let n = cd_filetree_req_path(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        openFileLink(String(cString: buf))
    }

    /// Closes a tab from the multi-tab bar, asking to save if it is the currently open dirty file.
    private func closeFileTreeTab() {
        closeFileTreeTabAt(cd_filetree_close_tab(page))
    }

    /// Closes the currently active tab (Cmd+W when the File Viewer is open, #13992).
    func closeActiveFileTab() {
        guard cd_file_shown(page) != 0, cd_file_mode(page) != 0 else { return }
        var curBuf = [CChar](repeating: 0, count: 1024)
        let activePath = cd_file_path(page, &curBuf, Int32(curBuf.count)) > 0 ? String(cString: curBuf) : (fileURL?.path ?? "")
        for i: Int32 in 0..<16 {
            var pbuf = [CChar](repeating: 0, count: 1024)
            let pn = cd_filetree_tab_path(page, i, &pbuf, Int32(pbuf.count))
            if pn <= 0 { break }
            let tp = String(cString: pbuf)
            if tp == activePath || tp == (fileURL?.path ?? "") {
                closeFileTreeTabAt(i)
                return
            }
        }
        closeFilePane()
    }

    private func closeFileTreeTabAt(_ idx: Int32) {
        guard idx >= 0 else { return }
        var pbuf = [CChar](repeating: 0, count: 1024)
        let pn = cd_filetree_tab_path(page, idx, &pbuf, Int32(pbuf.count))
        let tabPath = pn > 0 ? String(cString: pbuf) : ""
        let activePath = fileURL?.path ?? ""
        let activeTilde = fileURL.map { ($0.path as NSString).abbreviatingWithTildeInPath } ?? ""
        let isActiveTab = (tabPath == activePath || tabPath == activeTilde)
        let finishDrop: () -> Void = { [weak self] in
            guard let self else { return }
            let nextIdx = cd_filetree_tab_drop(self.page, idx)
            if isActiveTab {
                if nextIdx >= 0 {
                    var nbuf = [CChar](repeating: 0, count: 1024)
                    if cd_filetree_tab_path(self.page, nextIdx, &nbuf, Int32(nbuf.count)) > 0 {
                        let nextRaw = String(cString: nbuf)
                        let nextURL = nextRaw.hasPrefix("~")
                            ? URL(fileURLWithPath: (nextRaw as NSString).expandingTildeInPath)
                            : (nextRaw.hasPrefix("/") ? URL(fileURLWithPath: nextRaw) : self.folder.appendingPathComponent(nextRaw))
                        self.openFileInPane(nextURL)
                        return
                    }
                }
                self.dropFile()
            } else {
                self.wake()
            }
        }
        if isActiveTab && cd_file_dirty(page) != 0 {
            askToSave(then: finishDrop)
        } else {
            finishDrop()
        }
    }

    /// Launches the editor picked in the Open-in-Editor dropdown at the active file & caret line
    /// (or the active worktree folder if no file is open).
    func launchPickedEditor() {
        let edId = cd_editor_pick_id(page)
        guard edId >= 0 else { return }
        let targetURL = fileURL ?? folder
        let line = max(Int(cd_file_line(page)), 1)
        let path = targetURL.path
        let fm = FileManager.default
        let ws = NSWorkspace.shared

        func runCLI(_ candidates: [String], args: [String], appFallback: String?) {
            for bin in candidates where fm.isExecutableFile(atPath: bin) {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: bin)
                p.arguments = args
                p.standardOutput = FileHandle.nullDevice
                p.standardError = FileHandle.nullDevice
                if (try? p.run()) != nil { return }
            }
            if let app = appFallback, fm.fileExists(atPath: app) {
                ws.open([targetURL], withApplicationAt: URL(fileURLWithPath: app), configuration: NSWorkspace.OpenConfiguration())
            }
        }

        switch edId {
        case 0: // Cursor
            runCLI(["/usr/local/bin/cursor", "/opt/homebrew/bin/cursor", "/Applications/Cursor.app/Contents/Resources/app/bin/cursor"],
                   args: fileURL != nil ? ["-g", "\(path):\(line)"] : [path],
                   appFallback: "/Applications/Cursor.app")
        case 1: // VS Code
            runCLI(["/usr/local/bin/code", "/opt/homebrew/bin/code", "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"],
                   args: fileURL != nil ? ["-g", "\(path):\(line)"] : [path],
                   appFallback: "/Applications/Visual Studio Code.app")
        case 2: // Zed
            runCLI(["/usr/local/bin/zed", "/opt/homebrew/bin/zed", "/Applications/Zed.app/Contents/MacOS/cli"],
                   args: fileURL != nil ? ["\(path):\(line)"] : [path],
                   appFallback: "/Applications/Zed.app")
        case 3: // Xcode
            runCLI(["/usr/bin/xed"],
                   args: fileURL != nil ? ["-l", "\(line)", path] : [path],
                   appFallback: "/Applications/Xcode.app")
        case 4: // Ghostty
            let dir = fileURL?.deletingLastPathComponent() ?? folder
            runCLI(["/usr/bin/open"], args: ["-a", "Ghostty", dir.path], appFallback: "/Applications/Ghostty.app")
        case 5: // Finder
            ws.activateFileViewerSelecting([targetURL])
        case 6: // Custom Editor
            var cmdBuf = [CChar](repeating: 0, count: 1024)
            let cn = cd_editor_cmd(page, edId, path, Int32(fileURL != nil ? line : 0), &cmdBuf, Int32(cmdBuf.count))
            if cn > 0 {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: "/bin/sh")
                p.arguments = ["-c", String(cString: cmdBuf)]
                p.standardOutput = FileHandle.nullDevice
                p.standardError = FileHandle.nullDevice
                _ = try? p.run()
            }
        default:
            break
        }
    }

    #if OA_FULL
    // MARK: - 3D PLE Scene Viewport (SceneView.swift, GitHub #13792)

    /// Opens a `.ple-scene.json`, `.ple-scene.v1.json`, or `.ifc` file in the right-hand pane's
    /// embedded Metal 3D viewport.
    func openSceneInPane(_ url: URL, shown: String? = nil) {
        let displayPath = shown ?? (url.path as NSString).abbreviatingWithTildeInPath
        let name = url.lastPathComponent
        do {
            let doc: PleScene
            if PleSceneLoader.isIfcFile(url) {
                doc = try PleSceneLoader.loadIfc(from: url, searchRoot: folder)
            } else {
                doc = try PleSceneLoader.load(from: url)
            }
            guard cd_file_open_scene(page, displayPath) != 0 else {
                hideScenePanel()
                cd_file_failed(page, displayPath, "Coder couldn't open 3D scene \(name).")
                wake()
                return
            }
            let panel = ensureScenePanel()
            panel.loadScene(doc)
            panel.isHidden = false
            syncScenePanelFrame()
            wake()
        } catch {
            hideScenePanel()
            cd_file_failed(page, displayPath, "Coder couldn't parse 3D scene \(name): \(error.localizedDescription)")
            wake()
        }
    }

    /// File > Open 3D Scene… panel.
    func chooseSceneFile() {
        let panel = NSOpenPanel()
        panel.title = "Open 3D PLE Scene"
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.directoryURL = folder
        panel.allowedContentTypes = [.json, UTType(filenameExtension: "ifc") ?? .data]
        panel.beginSheetModal(for: window) { [weak self] resp in
            guard resp == .OK, let url = panel.url else { return }
            self?.openFileInPane(url)
        }
    }

    private func ensureScenePanel() -> PleScenePanelView {
        if let existing = scenePanel {
            if existing.superview !== canvas {
                canvas.addSubview(existing)
            }
            return existing
        }
        let panel = PleScenePanelView(frame: .zero)
        panel.isHidden = true
        canvas.addSubview(panel)
        scenePanel = panel
        return panel
    }

    func hideScenePanel() {
        scenePanel?.isHidden = true
    }

    /// Keeps the embedded Metal 3D viewport aligned to the Coax filepane's body rect.
    func syncScenePanelFrame() {
        guard let panel = scenePanel else { return }
        var rect = [Float](repeating: 0, count: 4)
        let shown = cd_file_scene_rect(page, &rect)
        guard shown != 0, rect[2] > 1, rect[3] > 1 else {
            if !panel.isHidden { panel.isHidden = true }
            return
        }
        // Coax layout uses top-left logical points; AppKit canvas uses bottom-left points.
        let canvasH = canvas.bounds.height
        let x = CGFloat(rect[0]).rounded()
        let w = CGFloat(rect[2]).rounded()
        let h = CGFloat(rect[3]).rounded()
        let y = (canvasH - CGFloat(rect[1]) - h).rounded()
        let target = NSRect(x: x, y: max(0, y), width: max(1, w), height: max(1, h))
        if panel.frame != target {
            panel.frame = target
        }
        panel.setHoles(pageOverRects(in: panel))
        if panel.isHidden {
            panel.isHidden = false
        }
    }

    // MARK: - The live 3D card in the transcript (page/shows.coax, GitHub #13812)

    /// Mounts a compact PleScenePanelView over the live card the page drew last frame (a command's
    /// `OA-SHOW-LIVE: on` card, while Experimental is on), cut to the conversation's clip, and reloads
    /// it, keeping the camera, when the file changes (checked twice a second). With no live card on
    /// screen it hides; the card's picture is under it either way.
    func syncLiveScene() {
        var r = [Float](repeating: 0, count: 8)
        var buf = [CChar](repeating: 0, count: 4096)
        let n = cd_live_scene(page, &r, &buf, Int32(buf.count))
        guard n > 0, r[2] > 1, r[3] > 1, r[6] > 1, r[7] > 1 else {
            if let box = liveSceneBox, !box.isHidden { box.isHidden = true }
            return
        }
        let path = String(cString: buf)
        let now = CACurrentMediaTime()
        if path != liveScenePath || now - liveSceneCheckedAt > 0.5 {
            liveSceneCheckedAt = now
            loadLiveScene(path)
        }
        guard liveSceneStamp != nil, let panel = liveScenePanel, let box = liveSceneBox else {
            liveSceneBox?.isHidden = true
            return
        }
        // the page's rects are top-left points; the canvas's, bottom-left
        let canvasH = canvas.bounds.height
        let clip = Self.snapped(CGFloat(r[4]), canvasH - CGFloat(r[5]) - CGFloat(r[7]), CGFloat(r[6]), CGFloat(r[7]))
        let card = Self.snapped(CGFloat(r[0]) - clip.minX, canvasH - CGFloat(r[1]) - CGFloat(r[3]) - clip.minY, CGFloat(r[2]), CGFloat(r[3]))
        if box.frame != clip { box.frame = clip }
        if panel.frame != card { panel.frame = card }
        panel.setHoles(pageOverRects(in: panel))
        if box.isHidden { box.isHidden = false }
    }

    /// A rect with each edge on the nearest whole point (not grown, as NSRect.integral is).
    static func snapped(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> NSRect {
        let x0 = x.rounded(), y0 = y.rounded()
        return NSRect(x: x0, y: y0, width: (x + w).rounded() - x0, height: (y + h).rounded() - y0)
    }

    /// Loads path into the live card's viewport when it is a new file or the file changed; a file
    /// that fails to load leaves the picture showing until it changes.
    private func loadLiveScene(_ path: String) {
        let attrs = try? FileManager.default.attributesOfItem(atPath: path)
        let size = (attrs?[.size] as? NSNumber)?.doubleValue ?? -1
        let time = (attrs?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? -1
        let same = path == liveScenePath
        if same, let st = liveSceneStamp, st == [size, time] { return }
        if same, liveSceneStamp == nil, liveSceneFailedAt == [size, time] { return }
        liveScenePath = path
        let panel = ensureLiveScenePanel()
        do {
            let doc = try PleSceneLoader.load(from: URL(fileURLWithPath: path))
            panel.loadScene(doc, preserveView: same && liveSceneStamp != nil)
            liveSceneStamp = [size, time]
        } catch {
            liveSceneStamp = nil
            liveSceneFailedAt = [size, time]
        }
    }

    private func ensureLiveScenePanel() -> PleScenePanelView {
        if let panel = liveScenePanel, let box = liveSceneBox {
            if box.superview !== canvas { canvas.addSubview(box) }
            return panel
        }
        let box = PassThroughView(frame: .zero)
        box.wantsLayer = true
        box.layer?.masksToBounds = true
        box.isHidden = true
        let panel = PleScenePanelView(frame: .zero)
        panel.makeCompact()
        box.addSubview(panel)
        canvas.addSubview(box)
        liveSceneBox = box
        liveScenePanel = panel
        return panel
    }

    /// What the page drew over everything in its last frame (coder_page_over_rects: menus,
    /// popovers, dialogs, toasts) in view's coordinates, for a native view over the page to cut out.
    func pageOverRects(in view: NSView) -> [NSRect] {
        var r = [Float](repeating: 0, count: 32)
        let n = Int(cd_over_rects(page, &r, 8))
        guard n > 0 else { return [] }
        let canvasH = canvas.bounds.height
        return (0..<n).map { i in
            let c = NSRect(x: CGFloat(r[i * 4]), y: canvasH - CGFloat(r[i * 4 + 1]) - CGFloat(r[i * 4 + 3]), width: CGFloat(r[i * 4 + 2]), height: CGFloat(r[i * 4 + 3]))
            return view.convert(c, from: canvas)
        }
    }
    #endif
}

#if OA_FULL
/// A view that holds others and takes no press itself: the live 3D card's clip (#13812), so a press
/// outside its viewport, or in a hole the page cut in it, reaches the page under it.
final class PassThroughView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}
#endif
