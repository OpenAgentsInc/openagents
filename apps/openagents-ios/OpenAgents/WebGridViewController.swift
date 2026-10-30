import UIKit
import WebKit

/// Where the app opens on iOS 26 and later: the web's Grid, the same page
/// openagents.com/realms/the-grid draws, straight in with no Start screen.
/// Turning the phone turns the camera (the gyro, through coaxial.tilt in
/// coax/web/host.js), as well as the right thumb. LOG IN and the wallet stay
/// native, under the page's GALAXY.
final class WebGridViewController: UIViewController, WKNavigationDelegate, WKUIDelegate {
    static let start = URL(string: "https://openagents.com/realms/the-grid")!

    private var web: WKWebView!
    private let signIn = HudButton("Log in", "signin.open")
    private let balance = HudButton("₿–", "wallet.open")
    private let wallet = SparkHost()
    private var walletTask: Task<Void, Never>?
    private let turn = PhoneTurn()
    private var clock: CADisplayLink?
    private var pendingCode: String?
    private var urlWatch: NSKeyValueObservation?

    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.applicationNameForUserAgent = "OpenAgentsApp/\(Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "0")"
        web = WKWebView(frame: view.bounds, configuration: config)
        web.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        web.isOpaque = false
        web.backgroundColor = .black
        web.scrollView.backgroundColor = .black
        web.scrollView.isScrollEnabled = false
        web.scrollView.bounces = false
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.navigationDelegate = self
        web.uiDelegate = self
        web.accessibilityIdentifier = "grid.web"
        #if DEBUG
        web.isInspectable = true
        #endif
        view.addSubview(web)
        web.load(URLRequest(url: Self.start))
        // LOG IN and the wallet sit under the Grid's GALAXY; other pages have
        // their own corners (Everglade's HP and WANTED), so they hide there.
        urlWatch = web.observe(\.url, options: [.initial, .new]) { [weak self] web, _ in
            DispatchQueue.main.async { self?.placeCorner(web.url) }
        }

        signIn.accessibilityLabel = "Log in to openagents.com"
        signIn.addAction(UIAction { [weak self] _ in self?.openSignIn() }, for: .touchUpInside)
        balance.isHidden = true
        balance.accessibilityLabel = "Wallet"
        balance.addAction(UIAction { [weak self] _ in self?.openWallet() }, for: .touchUpInside)
        for b in [signIn, balance] { b.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(b) }
        NSLayoutConstraint.activate([
            signIn.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 52),
            signIn.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            balance.topAnchor.constraint(equalTo: signIn.bottomAnchor, constant: 8),
            balance.trailingAnchor.constraint(equalTo: signIn.trailingAnchor),
        ])

        NotificationCenter.default.addObserver(self, selector: #selector(resume), name: UIApplication.didBecomeActiveNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(suspend), name: UIApplication.willResignActiveNotification, object: nil)
        resume()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if let code = pendingCode { pendingCode = nil; SignInRouter.open(code, from: self) }
    }

    private func placeCorner(_ url: URL?) {
        let onGrid = url?.path.hasPrefix("/realms/the-grid") ?? true
        signIn.isHidden = !onGrid
        if !onGrid { balance.isHidden = true } else { walletTask = nil; refreshBalance() }
    }

    /// A sign-in code or project link opened from outside (a URL, a universal link).
    func openSignInCode(_ text: String) {
        guard isViewLoaded, view.window != nil else { pendingCode = text; return }
        SignInRouter.open(text, from: self)
    }

    // MARK: the gyro

    @objc private func resume() {
        refreshBalance()
        guard clock == nil else { return }
        turn.view = view
        turn.start()
        let link = CADisplayLink(target: Tick(owner: self), selector: #selector(Tick.tick))
        link.add(to: .main, forMode: .common); clock = link
    }
    @objc private func suspend() {
        turn.stop()
        clock?.invalidate(); clock = nil
    }

    fileprivate func frame() {
        let (y, p) = turn.take()
        guard abs(y) + abs(p) > 0.0002 else { return }
        web.evaluateJavaScript("globalThis.coaxial?.tilt?.(\(y), \(p))", completionHandler: nil)
    }

    // MARK: the page

    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        // openagents.com stays in the app; anything else opens in Safari.
        guard let url = action.request.url, let host = url.host?.lowercased() else { decisionHandler(.allow); return }
        if host == "openagents.com" || host.hasSuffix(".openagents.com") || url.scheme == "about" { decisionHandler(.allow); return }
        decisionHandler(.cancel)
        UIApplication.shared.open(url)
    }
    func webView(_ webView: WKWebView, requestDeviceOrientationAndMotionPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        decisionHandler(origin.host.hasSuffix("openagents.com") ? .grant : .prompt)
    }
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        // A link meant for a new tab opens here instead.
        if let url = action.request.url { webView.load(URLRequest(url: url)) }
        return nil
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { webView.reload() }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // A web view without WebGPU can't draw the Grid: hand over to the
        // native one, straight in (no Start screen), so the app always plays.
        webView.callAsyncJavaScript("return !!(navigator.gpu && await navigator.gpu.requestAdapter())", arguments: [:], in: nil, in: .page) { [weak self] result in
            guard let self, case .success(let value) = result, (value as? Bool) == false, let window = self.view.window else { return }
            NSLog("OpenAgents grid: no WebGPU in the web view; using the native Grid")
            UserDefaults.standard.set(true, forKey: "OAGridSeen")
            self.suspend()
            window.rootViewController = HomeViewController()
        }
    }

    // MARK: log in and the wallet

    private func openSignIn() {
        let sheet = UINavigationController(rootViewController: SignInViewController())
        sheet.overrideUserInterfaceStyle = .dark
        sheet.view.tintColor = .white
        present(sheet, animated: true)
    }
    private func refreshBalance() {
        guard walletTask == nil else { return }
        walletTask = Task { [weak self, wallet] in
            let state = await wallet.balance()
            await MainActor.run {
                guard let self, !Task.isCancelled else { return }
                // The wallet shows once there is one on this phone.
                if case .balance(let sats) = state { self.balance.setTitle("₿" + String(sats), for: .normal); self.balance.isHidden = self.signIn.isHidden }
                else { self.balance.isHidden = true }
                self.walletTask = nil
            }
        }
    }
    private func openWallet() {
        let screen = WalletViewController(host: wallet)
        screen.closed = { [weak self] in self?.refreshBalance() }
        let sheet = UINavigationController(rootViewController: screen)
        sheet.overrideUserInterfaceStyle = .dark
        sheet.view.tintColor = .white
        present(sheet, animated: true)
    }
}

/// The display link holds this, not the screen, so the screen can go.
private final class Tick {
    weak var owner: WebGridViewController?
    init(owner: WebGridViewController) { self.owner = owner }
    @objc func tick() { owner?.frame() }
}
