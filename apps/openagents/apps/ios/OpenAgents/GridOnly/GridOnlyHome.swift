import UIKit
@preconcurrency import WebKit

/// The app's only scene: Aethyr home and the bundled Grid. No account or
/// session model is constructed on this path.
@objc(OpenAgentsGridOnlySceneDelegate)
final class GridOnlySceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.overrideUserInterfaceStyle = .dark
        window.rootViewController = GridOnlyRootController()
        self.window = window
        window.makeKeyAndVisible()
    }
}

/// Navigation replaces the child screen. Grid is never presented as a modal.
final class GridOnlyRootController: UIViewController {
    private var screen: UIViewController?
    override var prefersStatusBarHidden: Bool { true }
    override var childForStatusBarHidden: UIViewController? { screen }
    override func viewDidLoad() { super.viewDidLoad(); view.backgroundColor = .black; home() }
    private func show(_ next: UIViewController) {
        let visible = view.window != nil
        if visible { screen?.beginAppearanceTransition(false, animated:false) }
        screen?.willMove(toParent: nil)
        screen?.view.removeFromSuperview()
        screen?.removeFromParent()
        if visible { screen?.endAppearanceTransition() }
        screen = next
        addChild(next)
        if visible { next.beginAppearanceTransition(true, animated:false) }
        next.view.frame = view.bounds
        next.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(next.view)
        next.didMove(toParent: self)
        if visible { next.endAppearanceTransition() }
        setNeedsStatusBarAppearanceUpdate()
    }
    private func home() {
        show(GridOnlyHomeViewController { [weak self] in self?.enter() })
    }
    private func enter() {
        show(BundledGridViewController(openSessions: nil, standalone: true, onClose: { [weak self] in self?.home() }))
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if ProcessInfo.processInfo.arguments.contains("-enter-grid"), screen is GridOnlyHomeViewController { enter() }
    }
}

/// Thin WebKit host for the Rust Aethyr af-home renderer and its home UI.
final class GridOnlyHomeViewController: UIViewController, WKScriptMessageHandler, WKNavigationDelegate {
    private let enter: () -> Void
    private var web: GridGameWebView!
    private let metrics = UILabel()
    private var homeURL: URL?
    private var notifications: [NSObjectProtocol] = []
    init(enter: @escaping () -> Void) { self.enter = enter; super.init(nibName:nil,bundle:nil) }
    required init?(coder: NSCoder) { fatalError() }
    override var prefersStatusBarHidden: Bool { true }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        view.accessibilityIdentifier = "grid-home"
        let config = GridGameWebView.configuration()
        config.websiteDataStore = .nonPersistent()
        config.userContentController.add(GridHomeRelay(self), name: "gridHome")
        web = GridGameWebView(frame: view.bounds, configuration: config)
        web.autoresizingMask = [.flexibleWidth,.flexibleHeight]
        web.navigationDelegate = self
        web.accessibilityIdentifier = "grid-home-web"
        view.addSubview(web)
        if ProcessInfo.processInfo.arguments.contains("-grid-probe") {
            metrics.frame = CGRect(x:1,y:1,width:2,height:2)
            metrics.textColor = .clear
            metrics.text = "Aethyr home"
            metrics.accessibilityIdentifier = "grid-home-metrics"
            metrics.accessibilityValue = "{}"
            view.addSubview(metrics)
        }
        if let bundle = Bundle.main.url(forResource:"AethyrHome",withExtension:"bundle") {
            homeURL = bundle.appendingPathComponent("home.html")
            web.loadFileURL(homeURL!, allowingReadAccessTo: bundle)
        }
        for (event, phase) in [(UIApplication.willResignActiveNotification,"background"),(UIApplication.didBecomeActiveNotification,"foreground")] {
            notifications.append(NotificationCenter.default.addObserver(forName:event,object:nil,queue:.main) { [weak self] _ in
                self?.web.evaluateJavaScript("window.aethyrHomeLifecycle?.('\(phase)')",completionHandler:nil)
            })
        }
    }
    deinit { notifications.forEach(NotificationCenter.default.removeObserver) }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        web.evaluateJavaScript("window.dispatchEvent(new Event('pagehide'))",completionHandler:nil)
        web.stopLoading()
        web.configuration.userContentController.removeAllScriptMessageHandlers()
    }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, message.frameInfo.request.url == homeURL,
              let value = message.body as? [String:Any] else { return }
        if value["kind"] as? String == "enter" { enter() }
        if value["kind"] as? String == "ready", let data = try? JSONSerialization.data(withJSONObject:value) {
            metrics.accessibilityValue = String(data:data,encoding:.utf8)
        }
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { self.web.enforceGameInput() }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy)->Void) {
        decisionHandler(action.request.url == homeURL ? .allow : .cancel)
    }
}
private final class GridHomeRelay: NSObject, WKScriptMessageHandler {
    weak var owner: GridOnlyHomeViewController?
    init(_ owner: GridOnlyHomeViewController) { self.owner = owner }
    func userContentController(_ c: WKUserContentController, didReceive message: WKScriptMessage) { owner?.userContentController(c,didReceive:message) }
}
