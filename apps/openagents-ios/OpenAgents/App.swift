import UIKit
import UserNotifications

@main
final class AppDelegate: UIResponder, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        NoticePush.start()
        return true
    }
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "OpenAgents", sessionRole: session.role)
        configuration.delegateClass = SceneDelegate.self
        return configuration
    }

    // MARK: security notices as push (#13233)

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) { NoticePush.took(deviceToken) }
    /// No token (no network, or a build without the push entitlement): notices still come by email and Nostr.
    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {}
    /// A notice that arrives while the app is open still shows.
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions { [.banner, .list, .sound] }
    /// A tapped notice opens the Security screen.
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        await MainActor.run { if AppAuth.noticeOpens(info) { NoticePush.openSecurity() } }
    }
}

/// Security notices as push (#13233, docs/auth "Notify"). When someone is logged in, the app asks
/// Apple for this install's push token and gives it to the server (`AppAuth.pushTarget`); the
/// account's security notices then reach this phone. Tapping one opens the Security screen
/// (`SecurityViewController`), where this login cancels a recovery or freezes the account.
enum NoticePush {
    /// A test run never registers, so no permission question appears: unit tests run inside
    /// XCTest, and the UI tests pass -OAGridSeen or -OAAPIOrigin.
    static var off: Bool {
        let p = ProcessInfo.processInfo
        return p.environment["XCTestConfigurationFilePath"] != nil || p.arguments.contains("-OAGridSeen") || p.arguments.contains("-OAAPIOrigin")
    }
    private static var wanted = false

    static func start() {
        guard !off else { return }
        NotificationCenter.default.addObserver(forName: AppAuth.didLogIn, object: nil, queue: .main) { _ in register() }
        register()
    }
    /// Asks Apple for the token when someone is logged in. The token itself needs no permission.
    static func register() {
        guard !off, AppAuth.shared.loggedIn else { return }
        UIApplication.shared.registerForRemoteNotifications()
    }
    /// Apple gave the token. Once the server keeps it for this device, the phone asks (once)
    /// whether notices may show. A login the server won't send notices to asks nothing.
    static func took(_ token: Data) {
        Task {
            do { try await AppAuth.shared.pushTarget(token) } catch { return }
            _ = try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])
        }
    }
    /// Shows the Security screen over whatever is up; when the window isn't there yet (a tap that
    /// launched the app), it shows as soon as it is.
    @MainActor static func openSecurity() {
        guard AppAuth.shared.loggedIn else { return }
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        guard var top = scene?.windows.first(where: { $0.isKeyWindow })?.rootViewController ?? scene?.windows.first?.rootViewController else { wanted = true; return }
        wanted = false
        while let shown = top.presentedViewController { top = shown }
        if (top as? UINavigationController)?.viewControllers.first is SecurityViewController { return }
        let nav = UINavigationController(rootViewController: SecurityViewController())
        nav.overrideUserInterfaceStyle = .dark
        nav.view.tintColor = .white
        top.present(nav, animated: true)
    }
    /// The window is up: a notice tapped before it was opens now.
    @MainActor static func windowShown() { if wanted { openSecurity() } }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private var home: HomeViewController? { window?.rootViewController as? HomeViewController }
    private var grid: WebGridViewController? { window?.rootViewController as? WebGridViewController }

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        AppAnalytics.shared.launch()
        // iOS 26 draws the web's WebGPU pages in the app: open straight on the
        // web's Grid. Earlier systems keep the native home.
        if #available(iOS 26, *), !UserDefaults.standard.bool(forKey: "OANativeHome") {
            window.rootViewController = WebGridViewController()
        } else {
            // Straight into the Grid: no Start screen (tests ask for it with -OAGridSeen NO).
            UserDefaults.standard.set(true, forKey: "OAGridSeen")
            AppAnalytics.shared.gridFallback(UserDefaults.standard.bool(forKey: "OANativeHome") ? "native-home-set" : "before-ios-26")
            window.rootViewController = HomeViewController()
        }
        window.makeKeyAndVisible()
        self.window = window
        NoticePush.windowShown()
        // Opened by a sign-in link: openagents:// or a universal link to openagents.com.
        for context in options.urlContexts { open(context.url) }
        for activity in options.userActivities { take(activity) }
        #if DEBUG
        // Tests: -OASignInOpen URL opens that code as if the system had opened it.
        if let code = UserDefaults.standard.string(forKey: "OASignInOpen") { openCode(code) }
        #endif
    }
    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        for context in contexts { open(context.url) }
    }
    func scene(_ scene: UIScene, continue activity: NSUserActivity) { take(activity) }

    private func take(_ activity: NSUserActivity) {
        guard activity.activityType == NSUserActivityTypeBrowsingWeb, let url = activity.webpageURL else { return }
        open(url)
    }
    private func open(_ url: URL) { openCode(url.absoluteString) }
    private func openCode(_ text: String) {
        if let grid { grid.openSignInCode(text) } else { home?.openSignInCode(text) }
    }
}
