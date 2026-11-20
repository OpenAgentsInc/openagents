// Firm reason: 6 (AppKit's open panel and a background walk of the working folder, over the compiled page).
// The composer's + menu, files chosen for the draft, and the working folder's files for the @
// popover (page/attach.coax and page/mention.coax, GitHub #13699). Pasted and dropped files reach
// the page through the shared host (host.c cd_on_file); this file holds what needs AppKit.
import AppKit

extension CoderWindow {
    /// The draft as a follow-up sends it: its words, then the note on the files it carries and mentions.
    var draftMessage: String {
        var out = [CChar](repeating: 0, count: 16384 + 8192)
        return cd_draft_message(page, &out, Int32(out.count)) > 0 ? String(cString: out) : ""
    }
    /// The composer's + (cd_action 27): its menu under it.
    func attachMenu() {
        var at = [Float](repeating: 0, count: 2)
        guard cd_add_at(page, &at) != 0 else { return }
        typealias Item = CoderOverlay.MenuItem
        let items = [Item(label: "Attach Files…"), Item(label: "Change Folder…", disabled: cd_running(page) != 0)]
        overlay.presentMenu(items, x: at[0], y: at[1]) { [weak self] chosen in
            switch chosen {
            case 0: self?.chooseFiles()
            case 1: self?.chooseFolder()
            default: break
            }
        }
    }
    /// The open panel for files to attach; a dialog for a path where the panel can't open (after an
    /// app update the panel service refuses the moved binary, Window+Chats.swift).
    func chooseFiles() {
        guard !choosingFolder, window.attachedSheet == nil else { return }
        choosingFolder = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            guard self.window.attachedSheet == nil else { self.choosingFolder = false; return }
            guard self.canPresentOpenPanel() else { self.chooseFilesFallback(); return }
            let panel = NSOpenPanel()
            panel.canChooseFiles = true; panel.canChooseDirectories = false; panel.allowsMultipleSelection = true
            var isDir: ObjCBool = false
            panel.directoryURL = (FileManager.default.fileExists(atPath: self.folder.path, isDirectory: &isDir) && isDir.boolValue)
                ? self.folder : FileManager.default.homeDirectoryForCurrentUser
            panel.prompt = "Attach"; panel.message = "Choose files to attach to the message."
            panel.beginSheetModal(for: self.window) { [weak self] answer in
                guard let self else { return }
                self.choosingFolder = false
                if answer == .OK { for url in panel.urls { _ = cd_attach(self.page, url.path) } }
                self.refocus()
            }
        }
    }
    private func chooseFilesFallback() {
        var d = CoderDialog(size: .md, title: "Attach a File",
                            body: "Enter the path to a file to attach. Restart Coder to restore the Finder file picker after an app update.")
        d.fields = [.init(value: "", hint: folder.path + "/")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Attach", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            self.choosingFolder = false
            defer { self.refocus() }
            guard answer == 1 else { return }
            let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !raw.isEmpty else { return }
            let named = (raw as NSString).expandingTildeInPath
            let url = named.hasPrefix("/") ? URL(fileURLWithPath: named) : self.folder.appendingPathComponent(named)
            _ = cd_attach(self.page, url.path)
        }
    }
    /// The @ popover asks for the working folder's files (cd_action 28): Git's list where the folder
    /// is a repository (what .gitignore ignores left out), else a walk that skips .git, node_modules
    /// and target. Off the main thread, at most 4000 paths; given to the page while its folder is the same.
    func listFiles() {
        let root = folder
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let lines = CoderWindow.folderFiles(root, most: 4000)
            DispatchQueue.main.async {
                guard let self, self.folder == root else { return }
                cd_files(self.page, lines)
                self.wake()
            }
        }
    }
    static func folderFiles(_ root: URL, most: Int) -> String {
        if FileManager.default.fileExists(atPath: root.appendingPathComponent(".git").path), let listed = gitFiles(root, most: most) { return listed }
        var out = "", n = 0
        let skip: Set<String> = [".git", "node_modules", "target"]
        let began = Date()
        guard let walk = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsPackageDescendants]) else { return "" }
        let base = root.standardizedFileURL.path.count + 1
        for case let url as URL in walk {
            if n >= most || Date().timeIntervalSince(began) > 2 { break }
            let isDir = (try? url.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
            if skip.contains(url.lastPathComponent) { if isDir { walk.skipDescendants() }; continue }
            if isDir { continue }
            let path = url.standardizedFileURL.path
            guard path.count > base else { continue }
            out += String(path.dropFirst(base)) + "\n"; n += 1
        }
        return out
    }
    private static func gitFiles(_ root: URL, most: Int) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        p.arguments = ["-C", root.path, "ls-files", "--cached", "--others", "--exclude-standard"]
        let pipe = Pipe(); p.standardOutput = pipe; p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        guard p.terminationStatus == 0, let text = String(data: data, encoding: .utf8) else { return nil }
        var out = "", n = 0
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            if n >= most { break }
            let first = line.split(separator: "/").first.map(String.init) ?? ""
            if first == "node_modules" || first == "target" { continue }
            out += line + "\n"; n += 1
        }
        return out
    }
}
