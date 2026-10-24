import os
import UIKit
import WebKit

/// The web runner (ADR 0010): one zone's web release in a `WKWebView`.
/// Rust decides what may load, whether this web view can run the zone
/// (WebGPU), which card to show, and every bridge frame. This view only
/// forwards WebKit's events and paints the result.
///
/// - Exact origin only: any other navigation is cancelled.
/// - No credentials: a non-persistent website data store, no cookies from
///   the app, no popups. Sign-in stays a native sheet.
/// - The `aethyrforge-host` bridge runs over a `WKScriptMessageHandler`
///   exposed to the page as `window.aethyrHost`.
final class ZoneWebViewController: UIViewController, WKNavigationDelegate, WKUIDelegate {
    private static let log = Logger(subsystem: "com.openagents.app", category: "zones")
    private let runner: CoreZoneRunner
    private var webView: WKWebView!
    private let bar = UIView()
    private let titleLabel = UILabel()
    private let back = UIButton(type: .system)
    private let close = UIButton(type: .system)
    private let cardHost = UIView()
    private let card = UIStackView()
    private let spinner = UIActivityIndicatorView(style: .medium)
    /// The recorded WebGPU adapter, for UI tests and device notes.
    private let gpuNote = UILabel()
    private var backObservation: NSKeyValueObservation?

    init(runner: CoreZoneRunner) {
        self.runner = runner
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        view.accessibilityIdentifier = "zone-runner"
        let manifest = runner.manifest()

        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = .all
        let scripts = config.userContentController
        let relay = ZoneMessageRelay(self)
        scripts.add(relay, contentWorld: .page, name: runner.channelName())
        scripts.add(relay, contentWorld: .defaultClient, name: runner.probeChannelName())
        scripts.addUserScript(WKUserScript(source: runner.iosChannelScript(), injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .page))
        scripts.addUserScript(WKUserScript(source: runner.probeScript(platform: .ios), injectionTime: .atDocumentEnd, forMainFrameOnly: true, in: .defaultClient))
        webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.isOpaque = false
        webView.backgroundColor = .black
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.bounces = false
        webView.accessibilityIdentifier = "zone-web"
        #if DEBUG
        if #available(iOS 16.4, *) { webView.isInspectable = true }
        #endif

        bar.backgroundColor = Palette.background
        titleLabel.text = manifest.title
        titleLabel.font = Fonts.ui(.sansSemibold, 17)
        titleLabel.textColor = Palette.text
        titleLabel.textAlignment = .center
        back.setImage(UIImage(systemName: "chevron.left"), for: .normal)
        back.tintColor = Palette.text
        back.accessibilityLabel = "Back"
        back.accessibilityIdentifier = "zone-back"
        back.isEnabled = false
        back.addAction(UIAction { [weak self] _ in self?.webView.goBack() }, for: .touchUpInside)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.tintColor = Palette.text
        close.accessibilityLabel = "Close"
        close.accessibilityIdentifier = "zone-close"
        close.addAction(UIAction { [weak self] _ in self?.finish() }, for: .touchUpInside)

        cardHost.backgroundColor = Palette.background
        card.axis = .vertical
        card.spacing = 12
        spinner.color = Palette.secondary
        gpuNote.isHidden = true
        gpuNote.accessibilityIdentifier = "zone-webgpu"

        for v in [webView!, bar, titleLabel, back, close, cardHost, card, spinner, gpuNote] as [UIView] {
            v.translatesAutoresizingMaskIntoConstraints = false
        }
        view.addSubview(webView)
        view.addSubview(cardHost)
        cardHost.addSubview(card)
        cardHost.addSubview(spinner)
        view.addSubview(bar)
        bar.addSubview(back)
        bar.addSubview(titleLabel)
        bar.addSubview(close)
        view.addSubview(gpuNote)
        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bar.bottomAnchor.constraint(equalTo: safe.topAnchor, constant: 44),
            back.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 8),
            back.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
            back.widthAnchor.constraint(equalToConstant: 44),
            back.heightAnchor.constraint(equalToConstant: 44),
            close.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -8),
            close.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
            close.widthAnchor.constraint(equalToConstant: 44),
            close.heightAnchor.constraint(equalToConstant: 44),
            titleLabel.centerXAnchor.constraint(equalTo: bar.centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: back.centerYAnchor),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: back.trailingAnchor, constant: 8),
            // The zone fills the rest, out to the bottom and side edges; it
            // reads its own safe-area insets through CSS env().
            webView.topAnchor.constraint(equalTo: bar.bottomAnchor),
            webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            cardHost.topAnchor.constraint(equalTo: bar.bottomAnchor),
            cardHost.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            cardHost.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            cardHost.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            card.centerYAnchor.constraint(equalTo: safe.centerYAnchor),
            card.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 20),
            card.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -20),
            spinner.centerXAnchor.constraint(equalTo: cardHost.centerXAnchor),
            spinner.bottomAnchor.constraint(equalTo: card.topAnchor, constant: -16),
            gpuNote.topAnchor.constraint(equalTo: bar.bottomAnchor),
            gpuNote.leadingAnchor.constraint(equalTo: view.leadingAnchor),
        ])
        backObservation = webView.observe(\.canGoBack, options: [.initial, .new]) { [weak self] web, _ in
            self?.back.isEnabled = web.canGoBack
        }
        load()
    }

    private func load() {
        guard let url = URL(string: runner.url()), runner.allowsNavigation(url: url.absoluteString) else {
            runner.pageFailed(offline: false)
            return refresh()
        }
        webView.load(URLRequest(url: url))
        refresh()
    }

    private func finish() {
        webView.stopLoading()
        webView.configuration.userContentController.removeAllScriptMessageHandlers()
        dismiss(animated: true)
    }

    /// Show the zone, or Rust's card over it.
    private func refresh() {
        let phase = runner.phase()
        let json = try? runner.card()
        cardHost.isHidden = json == nil
        if case .loading = phase { spinner.startAnimating() } else { spinner.stopAnimating() }
        if let json {
            RustNativeCard.mount(json, in: card) { [weak self] instance, revision, key in
                self?.activate(instance, revision, key)
            }
        }
        if let adapter = runner.gpuAdapter() {
            gpuNote.accessibilityValue = "webgpu: \(adapter.vendor) \(adapter.architecture) \(adapter.device) \(adapter.description)"
        } else if let reason = runner.gpuMissingReason() {
            gpuNote.accessibilityValue = "webgpu: none (\(reason))"
        }
    }

    private func activate(_ instance: String, _ revision: UInt64, _ key: String) {
        guard let action = try? runner.activate(instance: instance, revision: revision, node: key) else {
            return refresh()
        }
        switch action {
        case .retry: load()
        case .close: finish()
        case let .openInBrowser(url):
            if let url = URL(string: url) { UIApplication.shared.open(url) }
        }
    }

    // MARK: Bridge

    fileprivate func receive(_ message: WKScriptMessage) {
        guard let text = message.body as? String else { return }
        let origin = Self.origin(message.frameInfo.securityOrigin)
        let main = message.frameInfo.isMainFrame
        if message.name == runner.probeChannelName() {
            do {
                try runner.probe(origin: origin, mainFrame: main, text: text)
                let adapter = runner.gpuAdapter()
                Self.log.notice("zone \(self.runner.manifest().id, privacy: .public) webgpu \(adapter == nil ? "missing" : "available", privacy: .public) vendor=\(adapter?.vendor ?? "", privacy: .public) arch=\(adapter?.architecture ?? "", privacy: .public) reason=\(self.runner.gpuMissingReason() ?? "", privacy: .public) ios=\(UIDevice.current.systemVersion, privacy: .public)")
            } catch {}
            return refresh()
        }
        let now = UInt64(Date().timeIntervalSince1970 * 1000)
        let reply = runner.bridge(origin: origin, mainFrame: main, text: text, nowMs: now)
        for frame in reply.deliver {
            webView.callAsyncJavaScript(runner.iosDeliverScript(), arguments: ["text": frame], in: nil, in: .page, completionHandler: nil)
        }
    }

    private static func origin(_ o: WKSecurityOrigin) -> String {
        let port = o.port == 0 ? "" : ":\(o.port)"
        return "\(o.protocol)://\(o.host)\(port)"
    }

    // MARK: WKNavigationDelegate

    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let url = action.request.url?.absoluteString ?? ""
        decisionHandler(runner.allowsNavigation(url: url) ? .allow : .cancel)
    }

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        runner.pageStarted()
        refresh()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        runner.pageFinished()
        refresh()
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { failed(error) }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { failed(error) }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        runner.pageFailed(offline: false)
        refresh()
    }

    private func failed(_ error: Error) {
        let code = (error as NSError).code
        // A cancelled navigation is ours (exact-origin policy), not a failure.
        guard code != NSURLErrorCancelled, code != 102 else { return }
        runner.pageFailed(offline: code == NSURLErrorNotConnectedToInternet || code == NSURLErrorNetworkConnectionLost)
        refresh()
    }

    // MARK: WKUIDelegate

    /// No popups or new windows.
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? { nil }
}

/// Holds the controller weakly; WebKit retains script message handlers.
private final class ZoneMessageRelay: NSObject, WKScriptMessageHandler {
    private weak var owner: ZoneWebViewController?
    init(_ owner: ZoneWebViewController) { self.owner = owner }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        owner?.receive(message)
    }
}
