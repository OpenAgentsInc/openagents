// Firm reason: 6 (AppKit/WebKit native multi-tab WKWebView host with CAShapeLayer even-odd cutout masking, 3 isolated WKWebsiteDataStore profiles, unified oa-browser-bridge.js, Human Supremacy input interceptor, and IOSurface/MTLTexture Phase 3 OSR hook).
// Spec: docs/browser/desktop-browser-pane-spec.md (§5, §6.2, §7.1, §8); Issues #14107, #14108, #14109.
import AppKit
import WebKit
import Metal
import QuartzCore
import CryptoKit

final class BrowserScriptRelay: NSObject, WKScriptMessageHandler {
    weak var owner: BrowserViewportView?
    let tabId: Int
    init(owner: BrowserViewportView, tabId: Int) {
        self.owner = owner
        self.tabId = tabId
    }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let owner else { return }
        DispatchQueue.main.async {
            owner.handleScriptMessage(tabId: self.tabId, channel: message.name, body: message.body)
        }
    }
}

final class BrowserNavRelay: NSObject, WKNavigationDelegate {
    weak var owner: BrowserViewportView?
    let tabId: Int
    init(owner: BrowserViewportView, tabId: Int) {
        self.owner = owner
        self.tabId = tabId
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = navigationAction.request.url, let owner else {
            decisionHandler(.cancel)
            return
        }
        let allow = owner.allowNavigation(tabId: tabId, url: url.absoluteString)
        decisionHandler(allow ? .allow : .cancel)
    }
    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        owner?.notifyLoadState(tabId: tabId, webView: webView, loading: true)
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        owner?.notifyLoadState(tabId: tabId, webView: webView, loading: false)
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        owner?.onTabCrashed?(tabId, "web_content_process_terminated")
    }
}

/// Multi-tab AppKit Browser Viewport hosting isolated WKWebView instances behind CAShapeLayer
/// even-odd overlay cutouts (zero flashing when Coax popovers / ⌘K Command Palette open over live web content).
final class BrowserViewportView: NSView {
    struct TabEntry {
        let id: Int
        let profile: Int
        let webView: WKWebView
        let scriptRelay: BrowserScriptRelay
        let navRelay: BrowserNavRelay
        var hibernated: Bool = false
    }

    private var tabs: [Int: TabEntry] = [:]
    private var activeTabId: Int = -1
    private var overRectsInSelf: [CGRect] = []
    private let workspaceHashUUID: UUID

    var onAllowNavigation: ((Int, String) -> Bool)?
    var onLoadState: ((Int, String, String, Bool, Bool, Bool, Float) -> Void)?
    var onConsole: ((Int, Int, String, String, Int) -> Void)?
    var onNetwork: ((Int, String, Int, String, Int) -> Void)?
    var onDomPick: ((Int, [String: Any]) -> Void)?
    var onHumanInteraction: ((Int) -> Void)?
    var onTabCrashed: ((Int, String) -> Void)?
    var onExternalTexture: ((MTLTexture?) -> Void)?

    init(frame: NSRect, workspacePath: String) {
        // Deterministic UUID per workspace for isolated WKWebsiteDataStore partition
        let digest = SHA256.hash(data: Data(workspacePath.utf8))
        var bytes = Array(digest.prefix(16))
        bytes[6] = (bytes[6] & 0x0F) | 0x50
        bytes[8] = (bytes[8] & 0x3F) | 0x80
        let tuple: uuid_t = (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                             bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15])
        self.workspaceHashUUID = UUID(uuid: tuple)
        super.init(frame: frame)
        wantsLayer = true
        layer?.masksToBounds = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    override var isFlipped: Bool { true }

    private func dataStore(for profile: Int) -> WKWebsiteDataStore {
        switch profile {
        case 2: // BR_PROFILE_EPHEMERAL -> nonPersistent, wiped completely on tab close
            return WKWebsiteDataStore.nonPersistent()
        case 1: // BR_PROFILE_PERSONAL -> persistent user store
            return WKWebsiteDataStore.default()
        default: // BR_PROFILE_WORKSPACE -> isolated per-repo persistent store (macOS 14+)
            return WKWebsiteDataStore(forIdentifier: workspaceHashUUID)
        }
    }

    func openTab(id: Int, profile: Int, urlString: String) {
        let cfg = WKWebViewConfiguration()
        cfg.websiteDataStore = dataStore(for: profile)
        let ucc = WKUserContentController()
        let bridgeSrc = String(cString: oa_br_unified_bridge_js())
        let userScript = WKUserScript(source: bridgeSrc, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        ucc.addUserScript(userScript)
        let relay = BrowserScriptRelay(owner: self, tabId: id)
        ucc.add(relay, name: "oaBrConsole")
        ucc.add(relay, name: "oaBrNetwork")
        ucc.add(relay, name: "oaBrDomPick")
        cfg.userContentController = ucc

        let wv = WKWebView(frame: bounds, configuration: cfg)
        wv.autoresizingMask = [.width, .height]
        let nav = BrowserNavRelay(owner: self, tabId: id)
        wv.navigationDelegate = nav
        wv.isHidden = true
        addSubview(wv)

        tabs[id] = TabEntry(id: id, profile: profile, webView: wv, scriptRelay: relay, navRelay: nav)
        activateTab(id: id)
        if !urlString.isEmpty && !urlString.hasPrefix("openagents://") {
            navigate(tabId: id, urlString: urlString)
        }
    }

    func activateTab(id: Int) {
        activeTabId = id
        for (tid, entry) in tabs {
            entry.webView.isHidden = (tid != id) || entry.hibernated
        }
    }

    func closeTab(id: Int) {
        guard let entry = tabs.removeValue(forKey: id) else { return }
        entry.webView.stopLoading()
        entry.webView.configuration.userContentController.removeScriptMessageHandler(forName: "oaBrConsole")
        entry.webView.configuration.userContentController.removeScriptMessageHandler(forName: "oaBrNetwork")
        entry.webView.configuration.userContentController.removeScriptMessageHandler(forName: "oaBrDomPick")
        entry.webView.removeFromSuperview()
    }

    func navigate(tabId: Int, urlString: String) {
        guard let entry = tabs[tabId], let url = URL(string: urlString) else { return }
        entry.webView.load(URLRequest(url: url))
    }

    func navAction(tabId: Int, action: Int) {
        guard let wv = tabs[tabId]?.webView else { return }
        switch action {
        case 0: wv.goBack()
        case 1: wv.goForward()
        case 2: wv.reload()
        case 3: wv.stopLoading()
        default: break
        }
    }

    func setPicking(tabId: Int, enabled: Bool) {
        guard let wv = tabs[tabId]?.webView else { return }
        wv.evaluateJavaScript("window.__oaBrowserBridge && window.__oaBrowserBridge.setPicking(\(enabled ? "true" : "false"))", completionHandler: nil)
    }

    func toggleHints(tabId: Int, enabled: Bool) {
        guard let wv = tabs[tabId]?.webView else { return }
        wv.evaluateJavaScript("window.__oaBrowserBridge && window.__oaBrowserBridge.toggleHints(\(enabled ? "true" : "false"))", completionHandler: nil)
    }

    func setZoom(tabId: Int, zoomPct: Int) {
        guard let wv = tabs[tabId]?.webView else { return }
        wv.pageZoom = CGFloat(max(25, min(500, zoomPct))) / 100.0
    }

    func evaluateBridge(tabId: Int, js: String, completion: @escaping (String) -> Void) {
        guard let wv = tabs[tabId]?.webView else { completion("{\"ok\":false,\"error\":\"tab_not_found\"}"); return }
        wv.evaluateJavaScript(js) { res, err in
            if let err {
                completion("{\"ok\":false,\"error\":\"\(err.localizedDescription.replacingOccurrences(of: "\"", with: "'"))\"}")
                return
            }
            if let obj = res, JSONSerialization.isValidJSONObject(obj),
               let data = try? JSONSerialization.data(withJSONObject: obj),
               let str = String(data: data, encoding: .utf8) {
                completion(str)
            } else if let s = res as? String {
                completion(s)
            } else {
                completion("{\"ok\":true}")
            }
        }
    }

    func captureSnapshotPNG(tabId: Int, to url: URL, completion: @escaping (Bool) -> Void) {
        guard let wv = tabs[tabId]?.webView else { completion(false); return }
        wv.takeSnapshot(with: nil) { image, _ in
            guard let image, let tiff = image.tiffRepresentation,
                  let rep = NSBitmapImageRep(data: tiff),
                  let png = rep.representation(using: .png, properties: [:]) else {
                completion(false); return
            }
            do { try png.write(to: url); completion(true) }
            catch { completion(false) }
        }
    }

    /// Phase 3 zero-copy IOSurfaceRef -> MTLTexture bridge for composition-v2 (slot 1).
    func publishIOSurfaceFrame(_ surface: IOSurfaceRef, width: Int, height: Int, device: MTLDevice?) {
        guard let device, width > 0, height > 0 else { onExternalTexture?(nil); return }
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        desc.usage = [.shaderRead]
        let tex = device.makeTexture(descriptor: desc, iosurface: surface, plane: 0)
        onExternalTexture?(tex)
    }

    /// Updates CAShapeLayer even-odd cutout mask from cd_over_rects so Coax menus/⌘K palette float with ZERO flashing.
    func updateOverlayCutouts(_ rectsInSelf: [CGRect]) {
        if overRectsInSelf == rectsInSelf && (rectsInSelf.isEmpty ? layer?.mask == nil : layer?.mask?.frame == bounds) {
            return
        }
        overRectsInSelf = rectsInSelf
        guard let lyr = layer else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        if rectsInSelf.isEmpty {
            lyr.mask = nil
        } else {
            let path = CGMutablePath()
            path.addRect(bounds)
            for r in rectsInSelf { path.addRect(r) }
            let mask = (lyr.mask as? CAShapeLayer) ?? CAShapeLayer()
            mask.frame = bounds
            mask.fillRule = .evenOdd
            mask.path = path
            lyr.mask = mask
        }
        CATransaction.commit()
    }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for entry in tabs.values where entry.webView.frame != bounds {
            entry.webView.frame = bounds
        }
        if !overRectsInSelf.isEmpty, let mask = layer?.mask as? CAShapeLayer {
            let path = CGMutablePath()
            path.addRect(bounds)
            for r in overRectsInSelf { path.addRect(r) }
            mask.frame = bounds
            mask.path = path
        }
        CATransaction.commit()
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let pt = convert(point, from: superview)
        guard bounds.contains(pt) else { return nil }
        for r in overRectsInSelf where r.contains(pt) {
            return nil // Pass clicks directly through to Coax overlay (menus, dialogs, ⌘K palette)
        }
        // Human Supremacy (Invariant I-5): any click inside an agent-leased viewport immediately notifies host
        if activeTabId > 0, NSApp.currentEvent?.type == .leftMouseDown {
            onHumanInteraction?(activeTabId)
        }
        return super.hitTest(point)
    }

    fileprivate func allowNavigation(tabId: Int, url: String) -> Bool {
        return onAllowNavigation?(tabId, url) ?? false
    }

    fileprivate func notifyLoadState(tabId: Int, webView: WKWebView, loading: Bool) {
        let url = webView.url?.absoluteString ?? ""
        let title = webView.title ?? url
        onLoadState?(tabId, url, title, loading, webView.canGoBack, webView.canGoForward, Float(webView.estimatedProgress))
    }

    fileprivate func handleScriptMessage(tabId: Int, channel: String, body: Any) {
        guard let dict = body as? [String: Any] else { return }
        switch channel {
        case "oaBrConsole":
            onConsole?(tabId, dict["level"] as? Int ?? 0, dict["text"] as? String ?? "", dict["source"] as? String ?? "", dict["line"] as? Int ?? 0)
        case "oaBrNetwork":
            onNetwork?(tabId, dict["method"] as? String ?? "GET", dict["status"] as? Int ?? 0, dict["url"] as? String ?? "", dict["ms"] as? Int ?? 0)
        case "oaBrDomPick":
            onDomPick?(tabId, dict)
        default: break
        }
    }
}
