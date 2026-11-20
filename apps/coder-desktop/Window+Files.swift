// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The file in the pane on the window's right (page/filepane.coax, GitHub #13760): the page draws
// and edits it; this reads it from disk, writes it back, opens it in the Mac's own app for it, and
// asks before changes are lost. A press on a file a row or a reply names comes here, and so does a
// .coax file the Mac opens with Coder (AppDelegate.swift, #13759).
import AppKit
import UniformTypeIdentifiers

extension CoderWindow {
    /// A file a row, a reply or an attachment names: a text file opens in the pane, anything else
    /// (a picture, a PDF) in the Mac's own app for it, as before.
    func openNamedFile(_ url: URL) {
        if let type = UTType(filenameExtension: url.pathExtension), type.isDeclared, !type.conforms(to: .text) {
            if FileManager.default.fileExists(atPath: url.path) { NSWorkspace.shared.open(url) }
            return
        }
        openFileInPane(url)
    }

    /// A file into the pane. The open one, if it has changes, is asked about first; the same file
    /// opened again comes forward with its changes, or is read again when it has none.
    func openFileInPane(_ url: URL) {
        let url = url.standardizedFileURL
        if let open = fileURL, cd_file_mode(page) != 0, cd_file_dirty(page) != 0 {
            if open == url { _ = cd_file_front(page); return }
            askToSave { [weak self] in self?.readIntoPane(url) }
            return
        }
        readIntoPane(url)
    }

    /// Reads url and hands it to the page: its bytes, or why it can't be shown.
    private func readIntoPane(_ url: URL) {
        fileURL = url
        let shown = (url.path as NSString).abbreviatingWithTildeInPath
        let name = url.lastPathComponent
        var isFolder: ObjCBool = false
        guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isFolder) else {
            cd_file_failed(page, shown, "Coder couldn't find \(name). It may have been moved or deleted."); wake(); return
        }
        if isFolder.boolValue { cd_file_failed(page, shown, "\(name) is a folder, not a file."); wake(); return }
        let size = ((try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? NSNumber)?.int64Value ?? 0
        let most = Int64(cd_file_most())
        if size > most {
            let f = ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
            cd_file_failed(page, shown, "\(name) is \(f). Coder shows files up to \(most / 1024) KB here; open it in its own app with the button above.")
            wake(); return
        }
        let data: Data
        do { data = try Data(contentsOf: url) } catch {
            cd_file_failed(page, shown, "Coder couldn't read \(name): \(error.localizedDescription)"); wake(); return
        }
        guard String(data: data, encoding: .utf8) != nil else {
            cd_file_failed(page, shown, "\(name) isn't text, so Coder can't show it here. Open it in its own app with the button above.")
            wake(); return
        }
        let ok = data.withUnsafeBytes { raw in
            cd_file_open(page, shown, raw.bindMemory(to: CChar.self).baseAddress, Int32(data.count), Int64(size))
        }
        if ok == 0 { cd_file_failed(page, shown, "Coder couldn't show \(name) here.") }
        wake()
    }

    /// What the pane asked (cd_action 40 to 43).
    func fileAsked(_ what: Int32) {
        switch what {
        case 40: _ = saveOpenFile()
        case 41: openFileExternally()
        case 42: closeFilePane()
        case 43: window.makeFirstResponder(canvas); wake()
        default: break
        }
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
        var out = [CChar](repeating: 0, count: 16384 + 1)
        let n = cd_file_text(page, &out, Int32(out.count))
        guard n >= 0 else { return false }
        let data = out.withUnsafeBufferPointer { Data(buffer: UnsafeBufferPointer(rebasing: $0[0..<Int(n)])) }
        do {
            try data.write(to: url)
        } catch {
            cd_file_save_failed(page, "Coder couldn't save \(url.lastPathComponent): \(error.localizedDescription)")
            wake(); return false
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
}
