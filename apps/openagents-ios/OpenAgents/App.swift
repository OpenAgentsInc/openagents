import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "OpenAgents", sessionRole: session.role)
        configuration.delegateClass = SceneDelegate.self
        return configuration
    }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private var home: HomeViewController? { window?.rootViewController as? HomeViewController }

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = HomeViewController()
        window.makeKeyAndVisible()
        self.window = window
        // Opened by a sign-in link: openagents:// or a universal link to openagents.com.
        for context in options.urlContexts { open(context.url) }
        for activity in options.userActivities { take(activity) }
        #if DEBUG
        // Tests: -OASignInOpen URL opens that code as if the system had opened it.
        if let code = UserDefaults.standard.string(forKey: "OASignInOpen") { home?.openSignInCode(code) }
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
    private func open(_ url: URL) { home?.openSignInCode(url.absoluteString) }
}
