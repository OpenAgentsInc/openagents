// Firm reason: 6 (AppKit host bridge for page/browserpane.coax: CP_ASK_BR_* 230..249, BrowserViewportView placement with cd_over_rects cutouts, owner-only browser.sock attach server, and Phase 3 oa-browser-helper launcher).
// Spec: docs/browser/desktop-browser-pane-spec.md (§5–§11); Issues #14107, #14108, #14109, #14110.
import AppKit
import CryptoKit

final class BrowserWindowState {
    var attachServer: OpaquePointer?
    var attachTimer: Timer?
    var nextHostTabId: Int = 1
    var indexToTabId: [Int: Int] = [:]
    var tabIdToIndex: [Int: Int] = [:]
    var helperProc: Process?
}

extension CoderWindow {
    private var br: BrowserWindowState {
        if let st = brState { return st }
        let st = BrowserWindowState()
        brState = st
        return st
    }

    func classifyBrowserRoute(_ raw: String, caller: Int64 = 0, profile: Int64 = -1) -> (pane: Int, profile: Int, url: String) {
        let flags = cd_flags(page)
        var meta = [Int64](repeating: 0, count: 6)
        var urlBuf = [UInt8](repeating: 0, count: 4096)
        var jsonBuf = [UInt8](repeating: 0, count: 4608)
        DesktopCore.withCoaxString(raw) { s in
            DesktopCore.withS64Buf(&meta) { mv in
                DesktopCore.withByteBuf(&urlBuf) { uv in
                    DesktopCore.withByteBuf(&jsonBuf) { jv in
                        _ = oa_dc_br_route(s, caller, profile, flags, mv, uv, jv)
                    }
                }
            }
        }
        let norm = String(cString: urlBuf)
        return (Int(meta[0]), Int(meta[1]), norm)
    }

    func ensureBrowserViewport() -> BrowserViewportView {
        if let vp = brViewport {
            if vp.superview !== canvas { canvas.addSubview(vp) }
            return vp
        }
        let vp = BrowserViewportView(frame: .zero, workspacePath: folder.path)
        vp.isHidden = true
        vp.onAllowNavigation = { [weak self] _, url in
            guard let self else { return false }
            let r = self.classifyBrowserRoute(url)
            return r.pane == 0 || r.pane == 1 // BR_PANE_BROWSER or BR_PANE_PREVIEW
        }
        vp.onLoadState = { [weak self] tabId, url, title, loading, canBack, canFwd, progress in
            guard let self, let idx = self.br.tabIdToIndex[tabId] else { return }
            cd_br_sync_tab(self.page, Int32(idx), url, title, loading ? 1 : 0, canBack ? 1 : 0, canFwd ? 1 : 0, progress)
            self.wake()
        }
        vp.onConsole = { [weak self] _, level, text, src, line in
            guard let self else { return }
            cd_br_push_console(self.page, Int32(level), text, src, Int32(line))
            self.wake()
        }
        vp.onNetwork = { [weak self] _, method, status, url, ms in
            guard let self else { return }
            cd_br_push_network(self.page, method, Int32(status), url, Int32(ms))
            self.wake()
        }
        vp.onDomPick = { [weak self] _, payload in
            guard let self else { return }
            let sel = payload["selector"] as? String ?? ""
            let tag = payload["tag"] as? String ?? ""
            let txt = payload["text_excerpt"] as? String ?? ""
            let url = payload["url"] as? String ?? ""
            _ = cd_preview_dom_pick(self.page, sel, tag, txt, "", url, "")
            cd_toast(self.page, "DOM element attached to composer.", 0)
            self.focusComposer()
            self.wake()
        }
        vp.onHumanInteraction = { [weak self] _ in
            guard let self else { return }
            if cd_br_human_takeover(self.page) != 0 {
                cd_toast(self.page, "Human takeover: agent paused.", 0)
                self.wake()
            }
        }
        vp.onExternalTexture = { [weak self] tex in
            guard let self else { return }
            let ptr = tex.map { Unmanaged.passUnretained($0).toOpaque() }
            cd_br_set_ext_texture(self.page, 1, ptr) // slot 1 for Browser OSR alongside Metaverse slot 0
            self.wake()
        }
        canvas.addSubview(vp)
        brViewport = vp
        startBrowserAttachServerIfNeeded()
        return vp
    }

    func toggleBrowserPane() {
        if cd_br_shown(page) != 0 {
            cd_br_show(page, 0, 0)
        } else {
            cd_br_show(page, 1, 0)
        }
        wake()
    }

    func openBrowserURI(_ raw: String, caller: Int64 = 0, profile: Int64 = -1) -> Bool {
        let r = classifyBrowserRoute(raw, caller: caller, profile: profile)
        guard r.pane == 0 else { return false } // BR_PANE_BROWSER
        let idx = Int(cd_br_open_tab(page, r.url, Int32(caller), Int32(r.profile)))
        guard idx >= 0 else { return false }
        let tid = br.nextHostTabId
        br.nextHostTabId += 1
        br.indexToTabId[idx] = tid
        br.tabIdToIndex[tid] = idx
        ensureBrowserViewport().openTab(id: tid, profile: r.profile, urlString: r.url)
        wake()
        return true
    }

    func browserAsked(_ what: Int32) {
        let vp = ensureBrowserViewport()
        let actIdx = Int(cd_br_active_tab(page))
        let tid = br.indexToTabId[actIdx] ?? -1
        switch what {
        case 230: // CP_ASK_BR_NAVIGATE
            var ubuf = [CChar](repeating: 0, count: 4096)
            if cd_br_tab_url(page, Int32(actIdx), &ubuf, Int32(ubuf.count)) > 0 {
                let u = String(cString: ubuf)
                if tid > 0 { vp.navigate(tabId: tid, urlString: u) }
            }
        case 231: if tid > 0 { vp.navAction(tabId: tid, action: 0) } // BACK
        case 232: if tid > 0 { vp.navAction(tabId: tid, action: 1) } // FORWARD
        case 233: if tid > 0 { vp.navAction(tabId: tid, action: 2) } // RELOAD
        case 234: if tid > 0 { vp.navAction(tabId: tid, action: 3) } // STOP
        case 235: // CP_ASK_BR_NEW_TAB
            var ubuf = [CChar](repeating: 0, count: 4096)
            let n = cd_br_tab_url(page, Int32(actIdx), &ubuf, Int32(ubuf.count))
            let u = n > 0 ? String(cString: ubuf) : ""
            let newId = br.nextHostTabId
            br.nextHostTabId += 1
            br.indexToTabId[actIdx] = newId
            br.tabIdToIndex[newId] = actIdx
            vp.openTab(id: newId, profile: 0, urlString: u)
        case 236: // CP_ASK_BR_CLOSE_TAB
            if tid > 0 { vp.closeTab(id: tid) }
        case 237: // CP_ASK_BR_SELECT_TAB
            if tid > 0 { vp.activateTab(id: tid) }
        case 238: // CP_ASK_BR_PICK_TOGGLE
            if tid > 0 { vp.setPicking(tabId: tid, enabled: true) }
        case 239: // CP_ASK_BR_SHOT
            if tid > 0 {
                let shotsDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/shots", isDirectory: true)
                try? FileManager.default.createDirectory(at: shotsDir, withIntermediateDirectories: true)
                let ts = Int64(Date().timeIntervalSince1970)
                let outURL = shotsDir.appendingPathComponent("browser-\(ts).png")
                vp.captureSnapshotPNG(tabId: tid, to: outURL) { [weak self] ok in
                    guard let self, ok else { return }
                    _ = cd_preview_snapshot_done(self.page, outURL.path, ts)
                    cd_toast(self.page, "Browser screenshot attached to composer.", 0)
                    self.focusComposer()
                    self.wake()
                }
            }
        case 242: // CP_ASK_BR_EXTERNAL
            var ubuf = [CChar](repeating: 0, count: 4096)
            if cd_br_tab_url(page, Int32(actIdx), &ubuf, Int32(ubuf.count)) > 0,
               let url = URL(string: String(cString: ubuf)) {
                NSWorkspace.shared.open(url)
            }
        case 248: // CP_ASK_BR_ENGINE_INSTALL (Phase 3 signed oa-browser-helper hydration check)
            launchVerifiedBrowserHelperIfInstalled()
        default: break
        }
    }

    func placeBrowserViewport() {
        guard FeatureFlags.isOn(.browser) else {
            brViewport?.isHidden = true
            return
        }
        var rect = [Float](repeating: 0, count: 4)
        let visible = cd_br_rect(page, &rect) != 0
        guard visible else {
            brViewport?.isHidden = true
            cd_br_set_ext_texture(page, 1, nil) // 0 Hz compositing while hidden (Invariant I-4)
            return
        }
        let vp = ensureBrowserViewport()
        let ch = canvas.bounds.height
        // Round left up so the native WKWebView frame never intrudes into the left divider drag strip.
        let left = CGFloat(rect[0]).rounded(.up)
        let right = (CGFloat(rect[0]) + CGFloat(rect[2])).rounded()
        let top = CGFloat(rect[1]).rounded()
        let bottom = (CGFloat(rect[1]) + CGFloat(rect[3])).rounded()
        let w = max(1, right - left)
        let h = max(1, bottom - top)
        // Convert top-left Coax coordinates to AppKit bottom-left frame without implicit CA lag during drag-resize.
        let frame = NSRect(x: left, y: max(0, ch - bottom), width: w, height: h)
        if vp.frame != frame {
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            vp.frame = frame
            CATransaction.commit()
        }
        vp.isHidden = false

        // Feed cd_over_rects into CAShapeLayer even-odd cutout mask (zero flashing over ⌘K / menus!)
        var ov = [Float](repeating: 0, count: 32)
        let n = Int(cd_over_rects(page, &ov, 8))
        var cutouts: [CGRect] = []
        for i in 0..<n {
            let rx = CGFloat(ov[i * 4]) - left
            let ry = CGFloat(ov[i * 4 + 1]) - top
            let rw = CGFloat(ov[i * 4 + 2])
            let rh = CGFloat(ov[i * 4 + 3])
            let r = CGRect(x: rx, y: ry, width: rw, height: rh)
            if r.intersects(CGRect(origin: .zero, size: frame.size)) {
                cutouts.append(r)
            }
        }
        vp.updateOverlayCutouts(cutouts)
    }

    // ---- Phase 2: Owner-only ~/.openagents/desktop/browser.sock attach server ----

    private func startBrowserAttachServerIfNeeded() {
        guard br.attachServer == nil else { return }
        let ctxPtr = Unmanaged.passUnretained(self).toOpaque()
        let srv = oa_br_attach_start(nil, { ctx, reqPtr, respBuf, respCap in
            guard let ctx, let reqPtr, let respBuf else { return -1 }
            let win = Unmanaged<CoderWindow>.fromOpaque(ctx).takeUnretainedValue()
            let req = String(cString: reqPtr)
            let reply = win.handleBrowserAttachCommand(req)
            var red = [CChar](repeating: 0, count: Int(respCap))
            let n = oa_br_redact_secrets(reply, &red, respCap)
            memcpy(respBuf, &red, Int(n) + 1)
            return n
        }, ctxPtr)
        br.attachServer = srv
        if srv != nil {
            br.attachTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
                guard let srv = self?.br.attachServer else { return }
                _ = oa_br_attach_poll(srv)
            }
        }
    }

    fileprivate func handleBrowserAttachCommand(_ reqJson: String) -> String {
        guard let data = reqJson.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let verb = obj["verb"] as? String else {
            return "{\"ok\":false,\"error\":\"invalid_json\"}"
        }
        let url = obj["url"] as? String ?? ""
        cd_br_push_agent(page, verb, "\(verb) \(url)", 1)
        if verb == "browser.open" || verb == "browser.navigate" {
            let profName = obj["profile"] as? String ?? "ephemeral"
            let prof: Int64 = (profName == "workspace") ? 0 : (profName == "personal" ? 1 : 2)
            let r = classifyBrowserRoute(url, caller: 1, profile: prof) // BR_CALLER_AGENT
            guard r.pane == 0 else {
                return "{\"ok\":false,\"error\":\"route_rejected\"}"
            }
            _ = openBrowserURI(r.url, caller: 1, profile: Int64(r.profile))
            return "{\"ok\":true,\"verb\":\"\(verb)\",\"url\":\"\(r.url)\",\"profile\":\"\(profName)\"}"
        }
        return "{\"ok\":true,\"verb\":\"\(verb)\",\"tabs\":\(cd_br_tab_count(page))}"
    }

    // ---- Phase 3: Signed on-demand oa-browser-helper launcher ----

    private func launchVerifiedBrowserHelperIfInstalled() {
        let helperBin = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/engines/browser/oa-browser-helper")
        guard FileManager.default.isExecutableFile(atPath: helperBin.path) else {
            cd_toast(page, "Chromium Pro helper not installed in ~/.openagents/engines/browser/", 1)
            return
        }
        let proc = Process()
        proc.executableURL = helperBin
        proc.arguments = ["--ipc"]
        try? proc.run()
        br.helperProc = proc
        cd_toast(page, "Chromium Pro (CEF OSR) helper active.", 0)
    }
}
