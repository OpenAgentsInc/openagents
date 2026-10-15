import GameController
import UIKit
import WebKit

/// The game, edge to edge: no status bar, no zoom, no scrolling, the home
/// indicator out of the way. Saves (best time, settings, the tutorial seen)
/// live in the page's localStorage in the default persistent data store.
final class GameViewController: UIViewController, WKNavigationDelegate, WKUIDelegate {
    private var webView: WKWebView!
    private let pad = PadBridge()

    override func loadView() {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .default()
        configuration.setURLSchemeHandler(GameScheme(), forURLScheme: GameScheme.scheme)
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = []
        configuration.userContentController.addUserScript(
            WKUserScript(source: PadBridge.script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        configuration.userContentController.addUserScript(
            WKUserScript(source: GameViewController.noZoom, injectionTime: .atDocumentEnd, forMainFrameOnly: true))

        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.isOpaque = false
        webView.backgroundColor = .black
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = false
        webView.allowsLinkPreview = false
        let scroll = webView.scrollView
        scroll.isScrollEnabled = false
        scroll.bounces = false
        scroll.pinchGestureRecognizer?.isEnabled = false
        scroll.minimumZoomScale = 1
        scroll.maximumZoomScale = 1
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.delegate = NoZoom.shared
        #if DEBUG
        if #available(iOS 16.4, *) { webView.isInspectable = true }
        #endif
        self.webView = webView
        view = webView
        pad.webView = webView
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        webView.load(URLRequest(url: GameViewController.startURL))
        pad.start()
    }

    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge { .all }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        UserDefaults.standard.bool(forKey: "IceCreamLandscape") ? .landscape : .all
    }

    /// Store screenshots launch with arguments (never set in normal use):
    /// `-IceCreamFragment 'bot&seed=7'` hands the game a URL fragment (its
    /// bot, seed, and the like) and `-IceCreamLandscape YES` holds landscape.
    static var startURL: URL {
        guard let fragment = UserDefaults.standard.string(forKey: "IceCreamFragment"), !fragment.isEmpty,
              var parts = URLComponents(url: GameScheme.start, resolvingAgainstBaseURL: false) else {
            return GameScheme.start
        }
        parts.fragment = fragment
        return parts.url ?? GameScheme.start
    }

    // Links out of the game (how to play, the website) open in Safari.
    // The page's own links are site-relative ("/games/the-ice-cream-man/
    // how-to-play", "/"), so on this scheme they become openagents.com
    // links; the web view itself never leaves the game page.
    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = action.request.url else {
            decisionHandler(.cancel)
            return
        }
        if action.targetFrame?.isMainFrame == false {
            decisionHandler(url.scheme == GameScheme.scheme || url.scheme == "about" ? .allow : .cancel)
            return
        }
        switch url.scheme {
        case GameScheme.scheme:
            if url.path == GameScheme.start.path {
                decisionHandler(.allow)
                return
            }
            if let site = GameViewController.site(url) { UIApplication.shared.open(site) }
        case "about":
            decisionHandler(.allow)
            return
        case "https", "http", "mailto":
            UIApplication.shared.open(url)
        default:
            break
        }
        decisionHandler(.cancel)
    }

    /// icecream://game/some/path?q -> https://openagents.com/some/path?q
    static func site(_ url: URL) -> URL? {
        guard var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return nil }
        parts.scheme = "https"
        parts.host = "openagents.com"
        if parts.path.isEmpty { parts.path = "/" }
        return parts.url
    }

    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = action.request.url {
            if url.scheme == "https" || url.scheme == "http" {
                UIApplication.shared.open(url)
            } else if url.scheme == GameScheme.scheme, let site = GameViewController.site(url) {
                UIApplication.shared.open(site)
            }
        }
        return nil
    }

    // The game never needs a reload, but if WebKit's process dies, bring it back.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        webView.load(URLRequest(url: GameScheme.start))
    }

    /// Belt and braces beside the page's own viewport meta: no double-tap
    /// or pinch zoom, no text selection callout, no long-press menu.
    static let noZoom = """
    (function () {
      var meta = document.querySelector('meta[name=viewport]');
      var want = 'width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover';
      if (!meta) { meta = document.createElement('meta'); meta.name = 'viewport'; document.head.appendChild(meta); }
      meta.content = want;
      var style = document.createElement('style');
      style.textContent = 'html,body{touch-action:none;-webkit-user-select:none;user-select:none;-webkit-touch-callout:none;overscroll-behavior:none;}';
      document.head.appendChild(style);
      document.addEventListener('gesturestart', function (e) { e.preventDefault(); }, { passive: false });
    })();
    """
}

/// Keeps the web view's scroll view from zooming at all.
final class NoZoom: NSObject, UIScrollViewDelegate {
    static let shared = NoZoom()
    func viewForZooming(in scrollView: UIScrollView) -> UIView? { nil }
}
