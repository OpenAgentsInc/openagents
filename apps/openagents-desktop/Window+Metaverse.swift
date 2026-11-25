// Firm reason: 6 (AppKit lifecycle, file watcher, Metal viewport mount, and open/snapshot dialogs for Metaverse & GameBuilder pane).
// Bridges page/metaversepane.coax (cd_mv_* / CP_ASK_MV_* 200..215) to MetaverseViewportView (GitHub #14041).
import AppKit
import UniformTypeIdentifiers
import simd

#if OA_FULL
final class MetaverseWindowState {
    var scenes: [String: MetaverseTabScene] = [:]
    var gbSessions: [String: GameBuilderSession] = [:]
    var activePath: String = ""
    var watchPath: String = ""
    var watchMtime: Date?
    var watchSize: Int64 = 0
    var watchTimer: Timer?
    var attachPollTimer: Timer?
    var gbSnapIdx: Int32 = 3 // default 50cm snap
    var gbShareCode: String = ""
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
            activateGameBuilderTab(tabIdx: Int(tabIdx), path: path, gameId: gameId, levelId: levelId)
        }
        wake()
    }

    func activateGameBuilderTab(tabIdx: Int, path: String, gameId: String, levelId: String) {
        let vp = ensureMetaverseViewport()
        let session: GameBuilderSession
        if let existing = mv.gbSessions[path] {
            session = existing
        } else {
            let gid = gameId.isEmpty ? parseGbIds(from: path).game : gameId
            let lid = levelId.isEmpty ? parseGbIds(from: path).level : levelId
            session = GameBuilderSession(key: path, gameId: gid, levelId: lid, viewportSize: vp.bounds.size)
            mv.gbSessions[path] = session
        }
        mv.activePath = path
        _ = session.startAttachServer()
        startAttachPollTimer()
        vp.setActiveGameBuilder(session)
        syncGameBuilderChrome(session: session, tabIdx: tabIdx)
        wake()
    }

    private func parseGbIds(from path: String) -> (game: String, level: String) {
        let stripped = path.hasPrefix("gb:") ? String(path.dropFirst(3)) : path
        let parts = stripped.split(separator: "/").map(String.init)
        if parts.count >= 2 { return (parts[0], parts[1]) }
        if parts.count == 1 && !parts[0].isEmpty { return (parts[0], "level-1") }
        return ("scrappers-arena", "level-1")
    }

    func syncGameBuilderChrome(session: GameBuilderSession, tabIdx: Int) {
        let snap = session.snapshotUi()
        let fields64 = snap.fields.map { Int64($0) }
        fields64.withUnsafeBufferPointer { buf in
            cd_gb_sync_ui(
                page,
                snap.tool,
                mv.gbSnapIdx,
                snap.canUndo ? 1 : 0,
                snap.canRedo ? 1 : 0,
                snap.templateId,
                snap.playing ? 1 : 0,
                snap.selCount,
                snap.selKind,
                buf.baseAddress,
                Int32(buf.count),
                snap.selLabel,
                mv.gbShareCode,
                session.attachListening ? 1 : 0
            )
        }
        let rev = max(1, snap.savedRev)
        let stats = "\(snap.actorCount) actors · rev \(rev)\(session.gbmLoaded ? " · GBM" : "")\(session.farmLoaded ? " · Farm" : "")"
        cd_mv_tab_status(
            page,
            Int32(tabIdx),
            rev,
            rev,
            0, // CP_MV_REV_SYNCED
            snap.saveDue ? 1 : 0,
            snap.readOnly ? 1 : 0,
            snap.changeCount > 0 ? 1 : 0,
            snap.changeCount,
            stats,
            snap.statusNote
        )
        _ = cd_mv_set_tree(page, session.treeTSV())
    }

    private func startAttachPollTimer() {
        guard mv.attachPollTimer == nil else { return }
        mv.attachPollTimer = Timer.scheduledTimer(withTimeInterval: 0.12, repeats: true) { [weak self] _ in
            guard let self, let vp = self.mvViewport, let gb = vp.activeGameBuilder else { return }
            if gb.pollAttach() {
                _ = gb.drainPendingPictures(using: vp)
                let idx = Int(cd_mv_active_tab(self.page))
                if idx >= 0 { self.syncGameBuilderChrome(session: gb, tabIdx: idx) }
                vp.renderNow()
                self.wake()
            }
        }
    }

    private func loadMetaverseTab(tabIdx: Int, path: String, preserveCamera: Bool) {
        let vp = ensureMetaverseViewport()
        let prev = preserveCamera ? mv.scenes[path] : nil
        let scene = vp.loadScene(from: URL(fileURLWithPath: path), preserveCameraFrom: prev)
        // Phase 2 (#14042): Hand off camera pose from inline OA-SHOW-LIVE card when opening the same target
        if !preserveCamera, path == liveScenePath, let livePanel = liveScenePanel {
            let pose = livePanel.metalView.exportOrbitPose()
            scene.target = pose.target
            scene.distance = pose.distance
            scene.azimuth = pose.azimuth
            scene.elevation = pose.elevation
            scene.flyMode = false
        }
        mv.scenes[path] = scene
        mv.activePath = path
        MetaverseGpuCoordinator.shared.enforceBudget(scenes: mv.scenes, activePath: path)
        if scene.errorText.isEmpty {
            let rev = Int32(max(1, scene.savedRev))
            cd_mv_tab_status(page, Int32(tabIdx), rev, rev, 0, 0, 0, 0, 0, scene.statsText, "")
            _ = cd_mv_set_tree(page, scene.treeTSV)
            vp.setActiveScene(scene)
            startMetaverseWatcher(for: path, tabIdx: tabIdx)
        } else {
            cd_mv_tab_status(page, Int32(tabIdx), 0, 0, 2, 0, 0, 0, 0, "", scene.errorText)
            vp.setActiveScene(nil)
        }
        wake()
    }

    /// Queries #authority aethyr_inline_dedup so inline OA-SHOW-LIVE cards yield GPU ownership
    /// when the same target is actively shown in the right-side Metaverse pane (#14042).
    func shouldDeduplicateInlineLiveCard(for inlinePath: String) -> Bool {
        let shown = cd_mv_shown(page)
        let hidden = cd_mv_hidden(page)
        let activeIdx = cd_mv_active_tab(page)
        var activePath = mv.activePath
        if activeIdx >= 0 {
            var pbuf = [CChar](repeating: 0, count: 1024)
            if cd_mv_tab_path(page, activeIdx, &pbuf, Int32(pbuf.count)) > 0 {
                activePath = String(cString: pbuf)
            }
        }
        return oa_mv_policy_inline_dedup(inlinePath, shown, hidden, activePath) != 0
    }

    /// Transfers camera pose back from right-pane Metaverse tab to inline OA-SHOW-LIVE card when re-shown (#14042).
    func handoffCameraToLiveCard(for inlinePath: String, panel: PleScenePanelView) {
        guard let sc = mv.scenes[inlinePath] else { return }
        panel.metalView.importOrbitPose(
            target: sc.target,
            distance: sc.distance,
            azimuth: sc.azimuth,
            elevation: sc.elevation
        )
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
        vp.onGameBuilderMutated = { [weak self] in
            guard let self, let gb = vp.activeGameBuilder else { return }
            let idx = Int(cd_mv_active_tab(self.page))
            if idx >= 0 { self.syncGameBuilderChrome(session: gb, tabIdx: idx) }
            self.wake()
        }
        vp.onAttachSelectionRequested = { [weak self] in
            self?.attachMetaverseSelectionToChat()
        }
        canvas.addSubview(vp)
        mvViewport = vp
        return vp
    }

    func hideMetaverseViewport() {
        if let vp = mvViewport, !vp.isHidden {
            vp.isHidden = true
            vp.setPaneVisible(false)
        }
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
        if cd_mv_tab_count(page) == 0 {
            openMetaverseTarget(path: "gb:scrappers-arena/level-1", kind: 7, mode: 1, title: "Scrappers Arena", gameId: "scrappers-arena", levelId: "level-1", autoOpened: false)
        }
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
            vp.setPaneVisible(true)
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
            if cd_mv_tab_kind(page, Int32(idx)) == 7 {
                activateGameBuilderTab(tabIdx: idx, path: path, gameId: "", levelId: "")
            } else if let existing = mv.scenes[path] {
                mv.activePath = path
                let vp = ensureMetaverseViewport()
                vp.setActiveScene(existing)
                _ = cd_mv_set_tree(page, existing.treeTSV)
            } else {
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
        case 210: // CP_ASK_MV_HYDRATE (assets-sync --group gamebuilder)
            hydrateGameBuilderAssets()
        case 211: // CP_ASK_GB_CMD (dispatches strictly through GB_COMMANDS)
            handleGameBuilderCommand(cmd: cmd, arg: arg)
        case 212: // CP_ASK_GB_MODE (Inspect | Build | Play)
            handleGameBuilderMode(arg: arg)
        case 213: // CP_ASK_GB_NEW_GAME (12 canonical game types)
            handleGameBuilderNewGame(templateId: arg)
        case 214: // CP_ASK_GB_FOCUS_ISSUE (jump camera to play-test stuck-bot pin)
            handleGameBuilderFocusIssue(arg: arg)
        case 215: // CP_ASK_GB_NEW_CODE_AGENT
            handleGameBuilderCommand(cmd: "code-agent", arg: arg)
        default:
            break
        }
        wake()
    }

    private func activeOrDefaultGameBuilderSession() -> (GameBuilderSession, Int)? {
        let idx = Int(cd_mv_active_tab(page))
        if let gb = mvViewport?.activeGameBuilder, idx >= 0 { return (gb, idx) }
        guard idx >= 0 else { return nil }
        var pbuf = [CChar](repeating: 0, count: 1024)
        guard cd_mv_tab_path(page, Int32(idx), &pbuf, Int32(pbuf.count)) > 0 else { return nil }
        let path = String(cString: pbuf)
        if cd_mv_tab_kind(page, Int32(idx)) == 7 {
            activateGameBuilderTab(tabIdx: idx, path: path, gameId: "", levelId: "")
            if let gb = mvViewport?.activeGameBuilder { return (gb, idx) }
        }
        return nil
    }

    /// Enforces canonical GB_APPROVALS ("confirm" / destructive actions require explicit user approval).
    private func confirmGameBuilderActionIfRequired(cmd: String) -> Bool {
        let approval = oa_gb_get_verb_approval(cmd)
        let destructive: Set<String> = ["delete-game", "delete-level", "unpublish", "remove-published"]
        guard approval == 2 || destructive.contains(cmd) else { return true }
        let alert = NSAlert()
        alert.alertStyle = destructive.contains(cmd) ? .warning : .informational
        alert.messageText = "Confirm GameBuilder Action: \(cmd)"
        alert.informativeText = approval == 2
            ? "GameBuilder marks '\(cmd)' as requiring confirmation (asks the person first). Proceed?"
            : "This action ('\(cmd)') modifies or deletes GameBuilder state. Proceed?"
        alert.addButton(withTitle: "Proceed")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func handleGameBuilderCommand(cmd: String, arg: String) {
        guard let (gb, tabIdx) = activeOrDefaultGameBuilderSession() else { return }
        guard confirmGameBuilderActionIfRequired(cmd: cmd) else { return }
        switch cmd {
        case "open-game":
            let gid = arg.isEmpty ? "scrappers-arena" : arg
            gb.gameId = gid
            _ = gb.runVerb("open-game", argsJson: "{\"shipped\":\"\(gid)\"}")
            gb.hydrateLocalPacksIfAvailable()
        case "set-tool":
            _ = gb.runVerb("set-tool", argsJson: "{\"tool\":\"\(arg)\"}")
        case "add":
            let kind = arg.isEmpty ? "block" : arg
            _ = gb.runVerb("add", argsJson: "{\"kind\":\"\(kind)\"}")
        case "set-field":
            let parts = arg.split(separator: " ").map(String.init)
            if parts.count == 2, let val = Int(parts[1]) {
                _ = gb.runVerb("set-field", argsJson: "{\"field\":\"\(parts[0])\",\"value\":\(val)}")
            }
        case "snap":
            let labels = ["Free", "10 cm", "25 cm", "50 cm", "100 cm"]
            if let i = labels.firstIndex(of: arg) { mv.gbSnapIdx = Int32(i) }
        case "match-mode":
            let modes = ["Knockouts", "Last Stand", "Scrapball", "Hoops"]
            if let i = modes.firstIndex(of: arg) {
                gb.matchMode = Int32(i)
                if let h = gb.handle { _ = oa_gb_configure_play(h, gb.matchMode, gb.teamSize) }
            }
        case "fighter-class":
            let classes = ["Bolt", "Boulder", "Lumen"]
            if let i = classes.firstIndex(of: arg) { gb.fighterClass = Int32(i) }
        case "mute":
            gb.muted = (arg == "1")
        case "play":
            gb.muted = false
            if let h = gb.handle { _ = oa_gb_configure_play(h, gb.matchMode, gb.teamSize) }
            let classes = ["bolt", "boulder", "lumen"]
            let cls = classes[Int(gb.fighterClass) % classes.count]
            _ = gb.runVerb("play", argsJson: "{\"scrapper\":\"\(cls)\"}")
        case "stop":
            gb.muted = true
            _ = gb.runVerb("stop", argsJson: "")
        case "play-test":
            cd_gb_playtest_status(page, 1, 0, "Running deterministic play-test...")
            wake()
            let modes = ["knockouts", "last-stand", "scrapball", "hoops"]
            let modeWord = modes[Int(gb.matchMode) % modes.count]
            let res = gb.runVerb("play-test", argsJson: "{\"mode\":\"\(modeWord)\",\"size\":\(gb.teamSize),\"seconds\":20,\"seed\":20261116}")
            if res.ok {
                gb.parsePlaytestResult(res.json)
                cd_gb_playtest_status(page, 0, Int32(gb.playtestPins.count), gb.lastPlaytestSummary)
            } else {
                cd_gb_playtest_status(page, 0, 0, "Play-test refused")
            }
        case "undo-turn":
            let r = gb.runVerb("undo-turn", argsJson: "")
            if !r.ok { _ = gb.runVerb("undo", argsJson: "") }
        case "copy-share-url":
            if !arg.isEmpty {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString("https://gamebuilder.org/g/\(arg)", forType: .string)
            }
        default:
            let jsonArgs = arg.hasPrefix("{") ? arg : ""
            _ = gb.runVerb(cmd, argsJson: jsonArgs)
        }
        syncGameBuilderChrome(session: gb, tabIdx: tabIdx)
        mvViewport?.updateGbTimer()
        mvViewport?.renderNow()
    }

    private func handleGameBuilderMode(arg: String) {
        if arg == "play" {
            handleGameBuilderCommand(cmd: "play", arg: "")
        } else {
            handleGameBuilderCommand(cmd: "stop", arg: "")
        }
    }

    private func handleGameBuilderNewGame(templateId: String) {
        let tid = templateId.isEmpty ? "arena" : templateId
        let stamp = Int(Date().timeIntervalSince1970) % 10000
        let path = "gb:\(tid)-\(stamp)/level-1"
        let title = tid.replacingOccurrences(of: "-", with: " ").capitalized
        openMetaverseTarget(path: path, kind: 7, mode: 1, title: title, gameId: "\(tid)-\(stamp)", levelId: "level-1", autoOpened: false)
        if let (gb, tabIdx) = activeOrDefaultGameBuilderSession() {
            _ = gb.runVerb("new-game", argsJson: "{\"template\":\"\(tid)\"}")
            syncGameBuilderChrome(session: gb, tabIdx: tabIdx)
            mvViewport?.renderNow()
        }
    }

    private func handleGameBuilderFocusIssue(arg: String) {
        guard let (gb, _) = activeOrDefaultGameBuilderSession(), let h = gb.handle, !gb.playtestPins.isEmpty else { return }
        let idx = min(max(0, Int(arg) ?? 0), gb.playtestPins.count - 1)
        let pin = gb.playtestPins[idx]
        oa_gb_focus_cm(h, pin.xCm, pin.zCm)
        mvViewport?.renderNow()
    }

    private func hydrateGameBuilderAssets() {
        cd_gb_hydrate_progress(page, 1, 15)
        wake()
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            proc.arguments = ["bash", "-lc", "coax/web/tool.sh assets-sync --group gamebuilder"]
            let ok = (try? proc.run()) != nil
            if ok { proc.waitUntilExit() }
            DispatchQueue.main.async {
                guard let self else { return }
                cd_gb_hydrate_progress(self.page, 0, ok && proc.terminationStatus == 0 ? 100 : 0)
                if let gb = self.mvViewport?.activeGameBuilder {
                    gb.hydrateLocalPacksIfAvailable()
                    let idx = Int(cd_mv_active_tab(self.page))
                    if idx >= 0 { self.syncGameBuilderChrome(session: gb, tabIdx: idx) }
                    self.mvViewport?.renderNow()
                }
                self.wake()
            }
        }
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
        guard let vp = mvViewport else { return }
        var kindStr = "model"
        var pathStr = ""
        var gameId = ""
        var levelId = ""
        var revVal: Int64 = 1
        var selLabel = ""
        var coords = [Int64](repeating: 0, count: 9)

        if let gb = vp.activeGameBuilder {
            let snap = gb.snapshotUi()
            kindStr = "gamebuilder"
            pathStr = gb.key
            gameId = gb.gameId
            levelId = gb.levelId
            revVal = Int64(max(1, snap.savedRev))
            selLabel = snap.selId > 0 ? "shape-\(snap.selId)" : snap.selLabel
            coords = snap.camCoordsCm
        } else if let sc = vp.activeScene {
            pathStr = sc.path
            revVal = Int64(max(1, sc.savedRev))
            selLabel = (sc.selectedNode >= 0 && sc.selectedNode < sc.nodes.count) ? sc.nodes[sc.selectedNode].label : ""
            let hit = sc.lastHitWorld * 100.0
            let eye = sc.eyePosition * 100.0
            let at = sc.lookTarget * 100.0
            coords = [
                Int64(hit.x), Int64(hit.y), Int64(hit.z),
                Int64(eye.x), Int64(eye.y), Int64(eye.z),
                Int64(at.x),  Int64(at.y),  Int64(at.z),
            ]
        } else {
            return
        }
        var jsonBuf = [UInt8](repeating: 0, count: 2048)
        let n: Int64 = DesktopCore.withCoaxString(kindStr) { kS in
            DesktopCore.withCoaxString(pathStr) { pS in
                DesktopCore.withCoaxString(gameId) { gS in
                    DesktopCore.withCoaxString(levelId) { lS in
                        DesktopCore.withCoaxString(selLabel) { selS in
                            DesktopCore.withS64Buf(&coords) { cv in
                                DesktopCore.withByteBuf(&jsonBuf) { jv in
                                    oa_dc_mv_selection_json(kS, pS, gS, lS, revVal, selS, cv, jv)
                                }
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
