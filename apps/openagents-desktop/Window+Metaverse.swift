// Firm reason: 6 (AppKit lifecycle, file watcher, Metal viewport mount, and open/snapshot dialogs for Metaverse & GameBuilder pane).
// Bridges page/metaversepane.coax (cd_mv_* / CP_ASK_MV_* 200..215) to MetaverseViewportView (GitHub #14041).
import AppKit
import UniformTypeIdentifiers
import simd

#if OA_FULL
final class MetaverseWindowState {
    var scenes: [String: MetaverseTabScene] = [:]
    var activePath: String = ""
    var watchPath: String = ""
    var watchMtime: Date?
    var watchSize: Int64 = 0
    var watchTimer: Timer?
}

extension CoderWindow {
    private var mv: MetaverseWindowState {
        if let st = mvState { return st }
        let st = MetaverseWindowState()
        mvState = st
        return st
    }

    private func classifyMetaverseRoute(_ raw: String) -> (pane: Int, kind: Int, mode: Int, readOnly: Bool) {
        let flags = cd_flags(page)
        var meta = [Int64](repeating: 0, count: 5)
        var badge = [UInt8](repeating: 0, count: 16)
        var json = [UInt8](repeating: 0, count: 512)
        DesktopCore.withCoaxString(raw) { s in
            DesktopCore.withS64Buf(&meta) { mv in
                DesktopCore.withByteBuf(&badge) { bv in
                    DesktopCore.withByteBuf(&json) { jv in
                        _ = oa_dc_mv_route(s, flags, mv, bv, jv)
                    }
                }
            }
        }
        return (Int(meta[0]), Int(meta[1]), Int(meta[2]), meta[3] != 0)
    }

    /// Checks via #authority oa_dc_mv_route whether a path/URI routes to the Metaverse pane under active feature flags.
    func openInMetaverseIfApplicable(_ url: URL, autoOpened: Bool = false) -> Bool {
        let r = classifyMetaverseRoute(url.path)
        guard r.pane == 1 else { return false } // 1 == MV_PANE_METAVERSE
        openMetaverseTarget(path: url.path, kind: r.kind, mode: r.mode, readOnly: r.readOnly, autoOpened: autoOpened)
        return true
    }

    /// Opens a file path or deep-link URI (openagents://metaverse/... or openagents://gamebuilder/...) in the Metaverse pane.
    func openMetaverseURI(_ raw: String, autoOpened: Bool = false) -> Bool {
        let r = classifyMetaverseRoute(raw)
        guard r.pane == 1 else { return false }
        openMetaverseTarget(path: raw, kind: r.kind, mode: r.mode, readOnly: r.readOnly, autoOpened: autoOpened)
        return true
    }

    func openMetaverseTarget(path: String, kind: Int, mode: Int, readOnly: Bool = false, title: String = "", gameId: String = "", levelId: String = "", autoOpened: Bool = false) {
        let shownTitle = title.isEmpty ? URL(fileURLWithPath: path).lastPathComponent : title
        let tabIdx = cd_mv_open_tab(page, Int32(kind), path, shownTitle, gameId, levelId, Int32(mode), autoOpened ? 1 : 0)
        guard tabIdx >= 0 else { return }
        if kind != 7 { // Non-GameBuilder 3D files load immediately into MetaverseViewportView
            loadMetaverseTab(tabIdx: Int(tabIdx), path: path, preserveCamera: false)
        } else {
            cd_mv_tab_status(page, tabIdx, 1, 1, 0, 0, readOnly ? 1 : 0, 0, 0, "GameBuilder ready", "")
        }
        wake()
    }

    private func loadMetaverseTab(tabIdx: Int, path: String, preserveCamera: Bool) {
        let vp = ensureMetaverseViewport()
        let prev = preserveCamera ? mv.scenes[path] : nil
        let scene = vp.loadScene(from: URL(fileURLWithPath: path), preserveCameraFrom: prev)
        mv.scenes[path] = scene
        mv.activePath = path
        if scene.errorText.isEmpty {
            cd_mv_tab_status(page, Int32(tabIdx), 1, 1, 0, 0, 0, 0, 0, scene.statsText, "")
            _ = cd_mv_set_tree(page, scene.treeTSV)
            vp.setActiveScene(scene)
            startMetaverseWatcher(for: path, tabIdx: tabIdx)
        } else {
            cd_mv_tab_status(page, Int32(tabIdx), 0, 0, 2, 0, 0, 0, 0, "", scene.errorText)
            vp.setActiveScene(nil)
        }
        wake()
    }

    private func startMetaverseWatcher(for path: String, tabIdx: Int) {
        mv.watchTimer?.invalidate()
        mv.watchPath = path
        let attrs = try? FileManager.default.attributesOfItem(atPath: path)
        mv.watchMtime = attrs?[.modificationDate] as? Date
        mv.watchSize = (attrs?[.size] as? NSNumber)?.int64Value ?? 0
        mv.watchTimer = Timer.scheduledTimer(withTimeInterval: 0.75, repeats: true) { [weak self] _ in
            guard let self, self.mv.watchPath == path, cd_mv_shown(self.page) != 0 else { return }
            guard let a = try? FileManager.default.attributesOfItem(atPath: path) else { return }
            let mt = a[.modificationDate] as? Date
            let sz = (a[.size] as? NSNumber)?.int64Value ?? 0
            if mt != self.mv.watchMtime || sz != self.mv.watchSize {
                self.mv.watchMtime = mt
                self.mv.watchSize = sz
                let active = Int(cd_mv_active_tab(self.page))
                if active >= 0 { self.loadMetaverseTab(tabIdx: active, path: path, preserveCamera: true) }
            }
        }
    }

    func ensureMetaverseViewport() -> MetaverseViewportView {
        if let vp = mvViewport {
            if vp.superview !== canvas { canvas.addSubview(vp) }
            return vp
        }
        let vp = MetaverseViewportView(frame: .zero)
        vp.isHidden = true
        vp.onFocusRequested = { [weak self] in
            guard let self else { return }
            _ = cd_mv_focus_viewport(self.page)
            self.window.makeFirstResponder(self.canvas)
            self.wake()
        }
        vp.onNodePicked = { [weak self] idx, label, hit in
            guard let self else { return }
            if let sc = vp.activeScene {
                // Refresh TSV selected bit so drawer highlights clicked 3D node
                var lines: [String] = []
                for nd in sc.nodes {
                    lines.append("0\t\(nd.visible ? 1 : 0)\t\(nd.index == idx ? 1 : 0)\t\(nd.label)\t\(nd.label)")
                }
                _ = cd_mv_set_tree(self.page, lines.joined(separator: "\n") + "\n")
                self.wake()
            }
            _ = label; _ = hit
        }
        vp.onFilesDropped = { [weak self] urls in
            guard let self else { return }
            for u in urls { _ = self.openInMetaverseIfApplicable(u, autoOpened: false) }
        }
        canvas.addSubview(vp)
        mvViewport = vp
        return vp
    }

    func hideMetaverseViewport() {
        mvViewport?.isHidden = true
    }

    func toggleMetaversePane() {
        if cd_mv_shown(page) != 0 {
            cd_mv_hide(page)
        } else {
            cd_mv_show(page, 1, 0)
        }
        wake()
    }

    func toggleMetaverseExpanded() {
        if cd_mv_shown(page) != 0 && cd_mv_expanded(page) != 0 {
            cd_mv_show(page, 1, 0)
        } else {
            cd_mv_show(page, 1, 1)
        }
        wake()
    }

    func openGameBuilderStudio() {
        cd_mv_show(page, 1, 0)
        wake()
    }

    func syncMetaverseFrame() {
        var r = [Float](repeating: 0, count: 4)
        guard cd_mv_rect(page, &r) != 0, r[2] > 1, r[3] > 1 else {
            hideMetaverseViewport()
            return
        }
        let vp = ensureMetaverseViewport()
        let canvasH = canvas.bounds.height
        let frame = Self.snapped(CGFloat(r[0]), canvasH - CGFloat(r[1]) - CGFloat(r[3]), CGFloat(r[2]), CGFloat(r[3]))
        if vp.frame != frame { vp.frame = frame }
        vp.setHoles(pageOverRects(in: vp))
        if vp.isHidden {
            vp.isHidden = false
            vp.renderNow()
        }
    }

    /// Handles CP_ASK_MV_* (200..215) requests raised by page/metaversepane.coax.
    func metaverseAsked(_ what: Int32) {
        var cmdBuf = [CChar](repeating: 0, count: 256)
        var argBuf = [CChar](repeating: 0, count: 512)
        var tabIdx: Int32 = -1
        var val: Int32 = 0
        _ = cd_mv_pending_cmd(page, &cmdBuf, Int32(cmdBuf.count), &argBuf, Int32(argBuf.count), &tabIdx, &val)
        let cmd = String(cString: cmdBuf)
        let arg = String(cString: argBuf)

        switch what {
        case 200: // CP_ASK_MV_OPEN_DIALOG
            chooseMetaverseFile()
        case 201: // CP_ASK_MV_LOAD_TAB
            let idx = Int(cd_mv_active_tab(page))
            guard idx >= 0 else { return }
            var pbuf = [CChar](repeating: 0, count: 1024)
            guard cd_mv_tab_path(page, Int32(idx), &pbuf, Int32(pbuf.count)) > 0 else { return }
            let path = String(cString: pbuf)
            if let existing = mv.scenes[path] {
                mv.activePath = path
                let vp = ensureMetaverseViewport()
                vp.setActiveScene(existing)
                _ = cd_mv_set_tree(page, existing.treeTSV)
            } else if cd_mv_tab_kind(page, Int32(idx)) != 7 {
                loadMetaverseTab(tabIdx: idx, path: path, preserveCamera: false)
            }
        case 202: // CP_ASK_MV_CLOSE_TAB
            if cd_mv_tab_count(page) == 0 {
                mvViewport?.setActiveScene(nil)
                hideMetaverseViewport()
            } else {
                metaverseAsked(201)
            }
        case 203: // CP_ASK_MV_RELOAD
            let idx = Int(cd_mv_active_tab(page))
            guard idx >= 0 else { return }
            var pbuf = [CChar](repeating: 0, count: 1024)
            guard cd_mv_tab_path(page, Int32(idx), &pbuf, Int32(pbuf.count)) > 0 else { return }
            loadMetaverseTab(tabIdx: idx, path: String(cString: pbuf), preserveCamera: true)
        case 204: // CP_ASK_MV_SNAPSHOT
            snapshotMetaverseToChat()
        case 205: // CP_ASK_MV_ATTACH_SEL
            attachMetaverseSelectionToChat()
        case 206: // CP_ASK_MV_CAM_ACTION
            guard let vp = mvViewport, let sc = vp.activeScene else { return }
            if cmd == "cam_mode" { sc.flyMode = (arg == "fly") }
            else if cmd == "preset" || cmd == "focus" { sc.applyPreset(arg.isEmpty ? "iso" : arg) }
            vp.renderNow()
        case 207: // CP_ASK_MV_VIEW_OPT
            guard let vp = mvViewport, let sc = vp.activeScene else { return }
            if cmd == "shading" {
                let modes = ["lit", "clay", "normals", "unlit", "wire"]
                if let m = modes.firstIndex(of: arg) { sc.shadingMode = m }
            } else if cmd == "overlay" {
                if let dot = arg.lastIndex(of: "."), let bit = Int(arg[arg.index(after: dot)...]) {
                    sc.overlayMask ^= (1 << bit)
                }
            } else if cmd == "select_node" {
                if let idx = sc.nodes.firstIndex(where: { $0.label == arg }) {
                    sc.selectedNode = idx
                    if let h = sc.handle { oa_mv_select_node(h, Int32(idx)) }
                    for i in 0..<sc.nodes.count { sc.nodes[i].selected = (i == idx) }
                }
            } else if cmd == "toggle_node" {
                if let idx = sc.nodes.firstIndex(where: { $0.label == arg }) {
                    sc.nodes[idx].visible.toggle()
                    if let h = sc.handle { oa_mv_set_node_visible(h, Int32(idx), sc.nodes[idx].visible ? 1 : 0) }
                }
            }
            vp.renderNow()
        case 208: // CP_ASK_MV_EXTERNAL
            let idx = Int(cd_mv_active_tab(page))
            guard idx >= 0 else { return }
            var pbuf = [CChar](repeating: 0, count: 1024)
            if cd_mv_tab_path(page, Int32(idx), &pbuf, Int32(pbuf.count)) > 0 {
                NSWorkspace.shared.open(URL(fileURLWithPath: String(cString: pbuf)))
            }
        case 209: // CP_ASK_MV_OPEN_AS_TEXT
            let idx = Int(cd_mv_active_tab(page))
            guard idx >= 0 else { return }
            var pbuf = [CChar](repeating: 0, count: 1024)
            if cd_mv_tab_path(page, Int32(idx), &pbuf, Int32(pbuf.count)) > 0 {
                let u = URL(fileURLWithPath: String(cString: pbuf))
                cd_mv_hide(page)
                openFileInPane(u)
            }
        default:
            // 210..215: GameBuilder commands (wired to embedded gamebuilder-core.o in #14043)
            break
        }
        wake()
    }

    private func chooseMetaverseFile() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Open in Metaverse"
        panel.message = "Choose a 3D model, scene, asset pack, or GameBuilder file."
        panel.beginSheetModal(for: window) { [weak self] resp in
            guard let self, resp == .OK, let url = panel.url else { return }
            if !self.openInMetaverseIfApplicable(url, autoOpened: false) {
                self.openMetaverseTarget(path: url.path, kind: 1, mode: 0, autoOpened: false)
            }
        }
    }

    private func snapshotMetaverseToChat() {
        guard let vp = mvViewport, let png = vp.capturePNG() else { return }
        let stamp = Int(Date().timeIntervalSince1970)
        let outURL = FileManager.default.temporaryDirectory.appendingPathComponent("openagents-3d-snapshot-\(stamp).png")
        do {
            try png.write(to: outURL)
            _ = cd_attach(page, outURL.path)
            wake()
        } catch {}
    }

    private func attachMetaverseSelectionToChat() {
        guard let vp = mvViewport, let sc = vp.activeScene else { return }
        let selLabel = (sc.selectedNode >= 0 && sc.selectedNode < sc.nodes.count) ? sc.nodes[sc.selectedNode].label : ""
        let hit = sc.lastHitWorld * 100.0
        let eye = sc.eyePosition * 100.0
        let at = sc.lookTarget * 100.0
        var coords: [Int64] = [
            Int64(hit.x), Int64(hit.y), Int64(hit.z),
            Int64(eye.x), Int64(eye.y), Int64(eye.z),
            Int64(at.x),  Int64(at.y),  Int64(at.z),
        ]
        var jsonBuf = [UInt8](repeating: 0, count: 2048)
        let n: Int64 = DesktopCore.withCoaxString("model") { kS in
            DesktopCore.withCoaxString(sc.path) { pS in
                DesktopCore.withCoaxString("") { eS in
                    DesktopCore.withCoaxString(selLabel) { selS in
                        DesktopCore.withS64Buf(&coords) { cv in
                            DesktopCore.withByteBuf(&jsonBuf) { jv in
                                oa_dc_mv_selection_json(kS, pS, eS, eS, 1, selS, cv, jv)
                            }
                        }
                    }
                }
            }
        }
        guard n > 0 else { return }
        let stamp = Int(Date().timeIntervalSince1970)
        let outURL = FileManager.default.temporaryDirectory.appendingPathComponent("openagents-3d-selection-\(stamp).json")
        let data = Data(jsonBuf[0..<Int(n)])
        try? data.write(to: outURL)
        _ = cd_attach(page, outURL.path)
        wake()
    }
}
#endif
