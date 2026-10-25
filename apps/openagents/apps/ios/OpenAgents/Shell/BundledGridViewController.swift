import CryptoKit
import os
import UIKit
@preconcurrency import WebKit

/// Platform host for the app's pinned Aethyr C++/Wasm Grid. Scene behavior stays
/// in that release; Rust owns shell actions, service URLs, and local suggestions.
final class BundledGridViewController: UIViewController, WKNavigationDelegate, WKUIDelegate {
    private let core: CoreBundledGrid
    private let openSessions: () -> Void
    private var web: WKWebView!
    private var assets: GridBundleAssets?
    private let cardHost = UIView()
    private let card = UIStackView()
    private let metrics = UILabel()
    private var boot = ""
    private var timeout: DispatchWorkItem?
    private var notifications: [NSObjectProtocol] = []
    private let probe = ProcessInfo.processInfo.arguments.contains("-grid-probe")
    private static let log = Logger(subsystem: "com.openagents.app", category: "aethyr-grid")

    init(openSessions: @escaping () -> Void) {
        self.openSessions = openSessions
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("AethyrGrid")
        core = CoreBundledGrid(storageDirectory: directory.path)
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }
    deinit { notifications.forEach(NotificationCenter.default.removeObserver); timeout?.cancel() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        view.accessibilityIdentifier = "aethyr-grid"
        let bar = UIView()
        bar.backgroundColor = Palette.background
        let title = UILabel()
        title.text = "The Grid"
        title.textColor = Palette.text
        title.font = Fonts.ui(.sansSemibold, 17)
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.accessibilityLabel = "Close Grid"
        close.accessibilityIdentifier = "grid-close"
        close.tintColor = Palette.text
        close.addAction(UIAction { [weak self] _ in self?.finish() }, for: .touchUpInside)
        let sessions = UIButton(type: .system)
        sessions.setTitle("Sessions", for: .normal)
        sessions.accessibilityIdentifier = "grid-sessions"
        sessions.tintColor = Palette.text
        sessions.addAction(UIAction { [weak self] _ in self?.finish(sessions: true) }, for: .touchUpInside)

        let config = WKWebViewConfiguration()
        // A persistent store belonging only to this bundled world. Native app
        // credentials and other zones' cookies are never copied into it.
        config.websiteDataStore = WKWebsiteDataStore(forIdentifier: UUID(uuidString: "B3BC81F5-0339-4E90-943C-9BD6E6E64E20")!)
        let relay = GridPageRelay(self)
        config.userContentController.add(relay, contentWorld: .page, name: "gridShell")
        config.userContentController.addScriptMessageHandler(relay, contentWorld: .page, name: "gridSuggestion")
        let script = "window.__gridNativeTest=\(probe ? "true" : "false");\n" + core.iosScript()
        config.userContentController.addUserScript(WKUserScript(source: script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        if let bundle = Bundle.main.url(forResource: "AethyrGrid", withExtension: "bundle") {
            assets = try? GridBundleAssets(root: bundle, core: core)
        }
        if let assets { config.setURLSchemeHandler(assets, forURLScheme: "aethyr-grid") }
        web = WKWebView(frame: .zero, configuration: config)
        web.accessibilityIdentifier = "grid-web"
        web.navigationDelegate = self
        web.uiDelegate = self
        web.isOpaque = false
        web.backgroundColor = .black
        web.scrollView.backgroundColor = .black
        web.scrollView.isScrollEnabled = false
        web.scrollView.bounces = false
        web.scrollView.contentInsetAdjustmentBehavior = .never
        #if DEBUG
        web.isInspectable = true
        #endif
        cardHost.backgroundColor = Palette.background
        card.axis = .vertical
        card.spacing = 12
        for element in [bar, title, close, sessions, web!, cardHost, card] as [UIView] { element.translatesAutoresizingMaskIntoConstraints = false }
        view.addSubview(web)
        view.addSubview(cardHost)
        cardHost.addSubview(card)
        view.addSubview(bar)
        [title, close, sessions].forEach(bar.addSubview)
        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.topAnchor), bar.leadingAnchor.constraint(equalTo: view.leadingAnchor), bar.trailingAnchor.constraint(equalTo: view.trailingAnchor), bar.bottomAnchor.constraint(equalTo: safe.topAnchor, constant: 44),
            close.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 8), close.bottomAnchor.constraint(equalTo: bar.bottomAnchor), close.widthAnchor.constraint(equalToConstant: 44), close.heightAnchor.constraint(equalToConstant: 44),
            sessions.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -12), sessions.bottomAnchor.constraint(equalTo: bar.bottomAnchor), sessions.heightAnchor.constraint(equalToConstant: 44),
            title.centerXAnchor.constraint(equalTo: bar.centerXAnchor), title.centerYAnchor.constraint(equalTo: close.centerYAnchor),
            web.topAnchor.constraint(equalTo: bar.bottomAnchor), web.leadingAnchor.constraint(equalTo: view.leadingAnchor), web.trailingAnchor.constraint(equalTo: view.trailingAnchor), web.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            cardHost.topAnchor.constraint(equalTo: bar.bottomAnchor), cardHost.leadingAnchor.constraint(equalTo: view.leadingAnchor), cardHost.trailingAnchor.constraint(equalTo: view.trailingAnchor), cardHost.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            card.centerYAnchor.constraint(equalTo: cardHost.centerYAnchor), card.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 20), card.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -20),
        ])
        if probe {
            metrics.frame = CGRect(x: 1, y: 1, width: 2, height: 2)
            metrics.text = "Grid scene"
            metrics.textColor = .clear
            metrics.font = .systemFont(ofSize: 1)
            metrics.accessibilityIdentifier = "grid-metrics"
            metrics.accessibilityValue = "{}"
            view.addSubview(metrics)
        }
        notifications.append(NotificationCenter.default.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in self?.lifecycle("background") })
        notifications.append(NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in self?.lifecycle("foreground") })
        load()
    }

    private func load() {
        timeout?.cancel()
        core.retry()
        boot = UUID().uuidString
        refresh()
        guard assets != nil else { core.failed(); return refresh() }
        var components = URLComponents(string: "aethyr-grid://bundle/grid.html")!
        components.queryItems = [URLQueryItem(name: "boot", value: boot)]
        if ProcessInfo.processInfo.arguments.contains("-grid-offline") { components.queryItems?.append(URLQueryItem(name: "offline", value: "1")) }
        web.load(URLRequest(url: components.url!))
        let request = DispatchWorkItem { [weak self] in self?.core.failed(); self?.refresh() }
        timeout = request
        DispatchQueue.main.asyncAfter(deadline: .now() + 90, execute: request)
    }
    private func refresh() {
        let json = try? core.card()
        cardHost.isHidden = json == nil
        if let json {
            RustNativeCard.mount(json, in: card) { [weak self] instance, revision, key in
                guard let self, let action = try? self.core.activate(instance: instance, revision: revision, node: key) else { return }
                switch action { case .retry: self.load(); case .close: self.finish(); default: break }
            }
        }
    }
    private func lifecycle(_ phase: String) {
        web?.callAsyncJavaScript("window.aethyrGridLifecycle?.(phase)", arguments: ["phase": phase], in: nil, in: .page, completionHandler: nil)
    }
    private func finish(sessions: Bool = false) {
        timeout?.cancel()
        lifecycle("close")
        web.stopLoading()
        web.configuration.userContentController.removeAllScriptMessageHandlers()
        let open = openSessions
        dismiss(animated: true) { if sessions { open() } }
    }
    fileprivate func accepts(_ message: WKScriptMessage) -> Bool {
        message.frameInfo.isMainFrame && core.allowsNavigation(url: message.frameInfo.request.url?.absoluteString ?? "")
    }
    fileprivate func receive(_ message: WKScriptMessage) {
        guard accepts(message), let value = message.body as? [String: Any], value["boot"] as? String == boot else { return }
        switch value["phase"] as? String {
        case "ready": timeout?.cancel(); core.ready(); refresh()
        case "failed": timeout?.cancel(); core.failed(); refresh()
        case "metrics": if probe { metrics.accessibilityValue = value["value"] as? String }
        default: break
        }
    }
    fileprivate func suggestion(_ message: WKScriptMessage, reply: @escaping (Any?, String?) -> Void) {
        guard accepts(message), let body = message.body as? String, body.utf8.count <= 20_000 else { return reply(nil, "The suggestion could not be saved.") }
        do { reply(try core.suggestion(body: body), nil) } catch { reply(nil, "The suggestion could not be saved.") }
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { failed(error) }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { failed(error) }
    private func failed(_ error: Error) {
        guard (error as NSError).code != NSURLErrorCancelled else { return }
        Self.log.error("Grid page failed: \(error.localizedDescription, privacy: .public)")
        timeout?.cancel(); core.failed(); refresh()
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { timeout?.cancel(); core.failed(); refresh() }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.cancel) }
        if core.allowsNavigation(url: url.absoluteString) { return decisionHandler(.allow) }
        decisionHandler(.cancel)
        if action.navigationType == .linkActivated, let external = core.externalUrl(url: url.absoluteString), let target = URL(string: external) { UIApplication.shared.open(target) }
    }
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let value = action.request.url?.absoluteString, let external = core.externalUrl(url: value), let target = URL(string: external) { UIApplication.shared.open(target) }
        return nil
    }
}

private final class GridPageRelay: NSObject, WKScriptMessageHandler, WKScriptMessageHandlerWithReply {
    weak var owner: BundledGridViewController?
    init(_ owner: BundledGridViewController) { self.owner = owner }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) { owner?.receive(message) }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let owner else { return replyHandler(nil, "The Grid is closed.") }
        owner.suggestion(message, reply: replyHandler)
    }
}

/// WebKit's asset adapter. It serves only hashed bundle members and the three
/// public read URLs selected by Rust, with no app authentication or cookies.
@MainActor private final class GridBundleAssets: NSObject, WKURLSchemeHandler {
    struct Record: Decodable { let bytes: Int; let sha256: String }
    struct Manifest: Decodable { let format: String; let repository: String; let revision: String; let files: [String: Record] }
    let root: URL
    let manifest: Manifest
    let core: CoreBundledGrid
    private var active = Set<ObjectIdentifier>()
    private let session: URLSession
    init(root: URL, core: CoreBundledGrid) throws {
        self.root = root; self.core = core
        manifest = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: root.appendingPathComponent("manifest.json")))
        guard manifest.format == "openagents.aethyr-grid.bundle.v1", manifest.repository == "https://github.com/OpenAgentsInc/aethyr-cpp", manifest.revision.count == 40, manifest.files.count <= 40 else { throw URLError(.cannotDecodeContentData) }
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.timeoutIntervalForRequest = 12
        config.httpMaximumConnectionsPerHost = 3
        session = URLSession(configuration: config)
        super.init()
    }
    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        let identity = ObjectIdentifier(task)
        active.insert(identity)
        guard let url = task.request.url, url.scheme == "aethyr-grid", url.host == "bundle", task.request.httpMethod == "GET" else { return complete(task, identity, status: 404, data: Data()) }
        if let endpoint = core.serviceUrl(path: url.path), let target = URL(string: endpoint) {
            Task { [weak self] in
                guard let self else { return }
                do {
                    var request = URLRequest(url: target)
                    request.setValue("application/json", forHTTPHeaderField: "Accept")
                    let (bytes, response) = try await self.session.bytes(for: request)
                    guard (response as? HTTPURLResponse)?.statusCode == 200, response.expectedContentLength <= 512 * 1024 else { throw URLError(.badServerResponse) }
                    var data = Data()
                    for try await byte in bytes { guard data.count < 512 * 1024 else { throw URLError(.dataLengthExceedsMaximum) }; data.append(byte) }
                    await MainActor.run { self.complete(task, identity, status: 200, data: data, mime: "application/json") }
                } catch { await MainActor.run { self.complete(task, identity, status: 503, data: Data("{}".utf8), mime: "application/json") } }
            }
            return
        }
        let path = String(url.path.dropFirst())
        guard let record = manifest.files[path], record.bytes > 0, record.bytes <= 64 * 1024 * 1024, !path.split(separator: "/").contains("..") else { return complete(task, identity, status: 404, data: Data()) }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            do {
                let data = try Data(contentsOf: self.root.appendingPathComponent(path), options: .mappedIfSafe)
                guard data.count == record.bytes, SHA256.hash(data: data).map({ String(format: "%02x", $0) }).joined() == record.sha256 else { throw URLError(.cannotDecodeContentData) }
                let mime: String
                switch (path as NSString).pathExtension { case "html": mime = "text/html"; case "css": mime = "text/css"; case "js": mime = "application/javascript"; case "wasm": mime = "application/wasm"; case "txt": mime = "text/plain"; default: mime = "application/octet-stream" }
                DispatchQueue.main.async { self.complete(task, identity, status: 200, data: data, mime: mime) }
            } catch { DispatchQueue.main.async { self.complete(task, identity, status: 500, data: Data()) } }
        }
    }
    private func complete(_ task: WKURLSchemeTask, _ identity: ObjectIdentifier, status: Int, data: Data, mime: String = "application/octet-stream") {
        guard active.contains(identity), let url = task.request.url else { return }
        task.didReceive(HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": mime, "Content-Length": String(data.count), "Access-Control-Allow-Origin": "*"])!)
        task.didReceive(data)
        task.didFinish()
        active.remove(identity)
    }
    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) { active.remove(ObjectIdentifier(task)) }
}
