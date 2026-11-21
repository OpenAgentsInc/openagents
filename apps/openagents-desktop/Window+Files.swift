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
        if PleSceneLoader.isSceneFile(url) || PleSceneLoader.isIfcFile(url) {
            openFileInPane(url)
            return
        }
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
            hideScenePanel()
            cd_file_failed(page, shown, "Coder couldn't find \(name). It may have been moved or deleted."); wake(); return
        }
        if isFolder.boolValue {
            hideScenePanel()
            cd_file_failed(page, shown, "\(name) is a folder, not a file."); wake(); return
        }
        if PleSceneLoader.isSceneFile(url) || PleSceneLoader.isIfcFile(url) {
            openSceneInPane(url, shown: shown)
            return
        }
        hideScenePanel()
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
        else if cd_file_mode(page) == 1 { window.makeFirstResponder(canvas) }
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
        hideScenePanel()
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
}

/// A view that holds others and takes no press itself: the live 3D card's clip (#13812), so a press
/// outside its viewport, or in a hole the page cut in it, reaches the page under it.
final class PassThroughView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}
