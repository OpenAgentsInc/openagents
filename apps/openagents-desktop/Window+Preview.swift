// Firm reason: 6 (AppKit/WebKit native WKWebView host, DOM picker JS message handler, and macOS screencapture region SnapShot over the compiled page).
// Embedded Web Preview Pane (zero Chromium/Electron: native macOS WKWebView) with DOM element
// picker & annotation overlay, plus global SnapShot OS region capture (page/preview.coax; #13888).
import AppKit
import WebKit

final class PreviewScriptHandler: NSObject, WKScriptMessageHandler {
    weak var owner: CoderWindow?
    init(owner: CoderWindow) { self.owner = owner }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "oaDomPick", let body = message.body as? [String: Any] else { return }
        let selector = (body["selector"] as? String) ?? ""
        let tag = (body["tag"] as? String) ?? ""
        let text = (body["text"] as? String) ?? ""
        let rect = (body["rect"] as? String) ?? ""
        let url = (body["url"] as? String) ?? ""
        DispatchQueue.main.async { [weak self] in
            self?.owner?.handlePreviewDomPick(selector: selector, tag: tag, text: text, rect: rect, url: url)
        }
    }
}

final class PreviewNavigationDelegate: NSObject, WKNavigationDelegate {
    weak var owner: CoderWindow?
    init(owner: CoderWindow) { self.owner = owner }
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let scheme = navigationAction.request.url?.scheme?.lowercased(),
              scheme == "http" || scheme == "https" else {
            decisionHandler(.cancel)
            return
        }
        decisionHandler(.allow)
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard let owner, let url = webView.url?.absoluteString else { return }
        _ = cd_preview_set_url(owner.page, url)
        owner.previewLoadedURL = url
        if cd_preview_picking(owner.page) != 0 {
            owner.applyPreviewPickerJS(active: true)
        }
        owner.wake()
    }
}

extension CoderWindow {
    static let previewPickerEnableJS = """
    (function() {
      if (window.__oaPickerActive) return;
      window.__oaPickerActive = true;
      var box = document.createElement('div');
      box.id = '__oa_dom_picker_box';
      box.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #38bdf8;background:rgba(56,189,248,0.14);border-radius:3px;transition:all 40ms ease;display:none;';
      document.documentElement.appendChild(box);
      function cssPath(el) {
        if (!el || el.nodeType !== 1) return '';
        if (el.id) return '#' + el.id;
        var parts = [];
        var cur = el;
        while (cur && cur.nodeType === 1 && parts.length < 4) {
          var seg = cur.tagName.toLowerCase();
          if (cur.id) { parts.unshift('#' + cur.id); break; }
          if (cur.className && typeof cur.className === 'string') {
            var cls = cur.className.trim().split(/\\s+/).filter(Boolean).slice(0, 2);
            if (cls.length) seg += '.' + cls.join('.');
          }
          parts.unshift(seg);
          cur = cur.parentElement;
        }
        return parts.join(' > ');
      }
      window.__oaPickerMove = function(e) {
        var el = document.elementFromPoint(e.clientX, e.clientY);
        if (!el || el === box) return;
        var r = el.getBoundingClientRect();
        box.style.display = 'block';
        box.style.left = r.left + 'px';
        box.style.top = r.top + 'px';
        box.style.width = r.width + 'px';
        box.style.height = r.height + 'px';
      };
      window.__oaPickerClick = function(e) {
        e.preventDefault();
        e.stopPropagation();
        var el = document.elementFromPoint(e.clientX, e.clientY);
        if (window.__oaPickerCleanup) window.__oaPickerCleanup();
        if (!el) return;
        var r = el.getBoundingClientRect();
        var payload = {
          selector: cssPath(el),
          tag: (el.tagName || '').toLowerCase(),
          text: (el.innerText || el.textContent || '').trim().slice(0, 300),
          rect: Math.round(r.left) + ',' + Math.round(r.top) + ' ' + Math.round(r.width) + 'x' + Math.round(r.height),
          url: window.location.href || ''
        };
        if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.oaDomPick) {
          window.webkit.messageHandlers.oaDomPick.postMessage(payload);
        }
      };
      window.__oaPickerCleanup = function() {
        window.__oaPickerActive = false;
        document.removeEventListener('mousemove', window.__oaPickerMove, true);
        document.removeEventListener('click', window.__oaPickerClick, true);
        var b = document.getElementById('__oa_dom_picker_box');
        if (b && b.parentNode) b.parentNode.removeChild(b);
      };
      document.addEventListener('mousemove', window.__oaPickerMove, true);
      document.addEventListener('click', window.__oaPickerClick, true);
    })();
    """

    static let previewPickerDisableJS = "if (window.__oaPickerCleanup) { window.__oaPickerCleanup(); }"

    /// View > Web Preview (⌥⌘P) or Command Palette.
    func toggleWebPreview() {
        cd_preview_toggle(page)
        let next = cd_action(page)
        if next != 0 { act(next) }
        wake()
    }

    /// Global / app SnapShot region capture (⇧⌘6 or Command Palette "SnapShot region").
    func triggerRegionSnapshot() {
        let shotsDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/shots", isDirectory: true)
        try? FileManager.default.createDirectory(at: shotsDir, withIntermediateDirectories: true)
        let ts = Int64(Date().timeIntervalSince1970)
        let outURL = shotsDir.appendingPathComponent("snapshot-\(ts).png")
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            proc.arguments = ["-i", "-x", outURL.path]
            proc.standardOutput = FileHandle.nullDevice
            proc.standardError = FileHandle.nullDevice
            do {
                try proc.run()
                proc.waitUntilExit()
            } catch {
                DispatchQueue.main.async {
                    guard let self else { return }
                    cd_toast(self.page, "SnapShot failed to launch screencapture.", 1)
                    self.wake()
                }
                return
            }
            let exists = FileManager.default.fileExists(atPath: outURL.path)
            let size = (try? FileManager.default.attributesOfItem(atPath: outURL.path)[.size] as? Int64) ?? 0
            DispatchQueue.main.async {
                guard let self else { return }
                guard proc.terminationStatus == 0 && exists && size > 0 else { return }
                _ = cd_preview_snapshot_done(self.page, outURL.path, ts)
                cd_toast(self.page, "SnapShot attached to composer.", 0)
                NSApp.activate(ignoringOtherApps: true)
                self.show()
                self.focusComposer()
                self.wake()
            }
        }
    }

    func previewAsked(_ what: Int32) {
        switch what {
        case 130: promptPreviewURL()
        case 131: previewWebView?.reload()
        case 132: applyPreviewPickerJS(active: cd_preview_picking(page) != 0)
        case 133: capturePreviewPaneShot()
        case 134, 136: loadPreviewCurrentURL(force: what == 134)
        case 135: triggerRegionSnapshot()
        case 137:
            applyPreviewPickerJS(active: false)
            previewWebView?.isHidden = true
        default: break
        }
    }

    private func currentPreviewURLString() -> String {
        var buf = [CChar](repeating: 0, count: 1024)
        let n = cd_preview_url(page, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : "http://localhost:3000"
    }

    private func ensurePreviewWebView() -> WKWebView {
        if let wv = previewWebView { return wv }
        let config = WKWebViewConfiguration()
        let ucc = WKUserContentController()
        let handler = PreviewScriptHandler(owner: self)
        ucc.add(handler, name: "oaDomPick")
        config.userContentController = ucc
        let wv = WKWebView(frame: .zero, configuration: config)
        let nav = PreviewNavigationDelegate(owner: self)
        wv.navigationDelegate = nav
        wv.wantsLayer = true
        wv.layer?.cornerRadius = 6
        wv.layer?.masksToBounds = true
        wv.isHidden = true
        canvas.addSubview(wv)
        previewScriptHandler = handler
        previewNavDelegate = nav
        previewWebView = wv
        return wv
    }

    private func loadPreviewCurrentURL(force: Bool = false) {
        let urlStr = currentPreviewURLString()
        guard let url = URL(string: urlStr),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https" else { return }
        let wv = ensurePreviewWebView()
        if force || previewLoadedURL != urlStr {
            previewLoadedURL = urlStr
            wv.load(URLRequest(url: url))
        }
    }

    private func promptPreviewURL() {
        var d = CoderDialog(size: .md, title: "Web Preview URL",
                            body: "Enter a local dev server or http(s) URL (e.g. localhost:3000 or :5173).")
        d.fields = [.init(label: "URL", value: currentPreviewURLString(), hint: "http://localhost:3000")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Open", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.wake() }
            guard answer == 1 else { return }
            let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            if cd_preview_set_url(self.page, raw) != 0 {
                self.loadPreviewCurrentURL(force: true)
            } else {
                cd_toast(self.page, "Invalid or unsupported URL (http/https only).", 1)
            }
        }
    }

    func applyPreviewPickerJS(active: Bool) {
        guard let wv = previewWebView else { return }
        let js = active ? CoderWindow.previewPickerEnableJS : CoderWindow.previewPickerDisableJS
        wv.evaluateJavaScript(js, completionHandler: nil)
    }

    func handlePreviewDomPick(selector: String, tag: String, text: String, rect: String, url: String) {
        cd_preview_set_picking(page, 0)
        var d = CoderDialog(size: .md, title: "Attach DOM Element",
                            body: "Picked <\(tag)> (\(selector)). Add an optional annotation note before attaching to the composer.")
        d.fields = [.init(label: "Annotation note (optional)", value: "", hint: "e.g. Increase padding or fix alignment")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Attach to Composer", kind: .primary)]
        d.confirm = 1; d.cancel = 0
        present(d) { [weak self] answer, values in
            guard let self else { return }
            defer { self.focusComposer(); self.wake() }
            guard answer == 1 else { return }
            let note = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            _ = cd_preview_dom_pick(self.page, selector, tag, text, rect, url, note)
            cd_toast(self.page, "DOM element attached to composer.", 0)
        }
    }

    private func capturePreviewPaneShot() {
        guard let wv = previewWebView, !wv.isHidden else {
            triggerRegionSnapshot()
            return
        }
        let shotsDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/shots", isDirectory: true)
        try? FileManager.default.createDirectory(at: shotsDir, withIntermediateDirectories: true)
        let ts = Int64(Date().timeIntervalSince1970)
        let outURL = shotsDir.appendingPathComponent("preview-\(ts).png")
        wv.takeSnapshot(with: nil) { [weak self] image, _ in
            guard let self, let image,
                  let tiff = image.tiffRepresentation,
                  let rep = NSBitmapImageRep(data: tiff),
                  let png = rep.representation(using: .png, properties: [:]) else {
                self?.triggerRegionSnapshot()
                return
            }
            do {
                try png.write(to: outURL, options: .atomic)
                _ = cd_preview_snapshot_done(self.page, outURL.path, ts)
                cd_toast(self.page, "Preview screenshot attached to composer.", 0)
                self.focusComposer()
                self.wake()
            } catch {
                cd_toast(self.page, "Failed to save preview screenshot.", 1)
                self.wake()
            }
        }
    }

    /// Keeps the native WKWebView aligned to the right-hand preview pane rect, hiding whenever
    /// Settings, Gym, or overlays cover the page (same pattern as SimulatorPaneView / SceneView).
    func syncPreviewFrame() {
        var rect = [Float](repeating: 0, count: 5)
        guard cd_preview_rect(page, &rect) != 0, rect[2] > 1, rect[3] > 1, rect[4] == 0 else {
            if let wv = previewWebView, !wv.isHidden { wv.isHidden = true }
            return
        }
        let wv = ensurePreviewWebView()
        let h = canvas.bounds.height
        let w = CGFloat(rect[2]).rounded(), ht = CGFloat(rect[3]).rounded()
        let target = NSRect(x: CGFloat(rect[0]).rounded(),
                            y: max(0, (h - CGFloat(rect[1]) - ht).rounded()),
                            width: max(1, w),
                            height: max(1, ht))
        if wv.frame != target { wv.frame = target }
        if previewLoadedURL.isEmpty { loadPreviewCurrentURL(force: false) }
        if wv.isHidden { wv.isHidden = false }
    }

    func retirePreview() {
        previewWebView?.configuration.userContentController.removeScriptMessageHandler(forName: "oaDomPick")
        previewWebView?.navigationDelegate = nil
        previewWebView?.removeFromSuperview()
        previewWebView = nil
        previewScriptHandler = nil
        previewNavDelegate = nil
    }
}
