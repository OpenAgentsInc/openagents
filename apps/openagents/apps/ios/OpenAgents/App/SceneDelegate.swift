import UIKit

@objc(OpenAgentsCoderSceneDelegate)
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private let app = AppModel()

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.overrideUserInterfaceStyle = UIUserInterfaceStyle(rawValue: UserDefaults.standard.integer(forKey: "appearance")) ?? .unspecified
        window.tintColor = Palette.accent
        self.window = window
        let push = PushNotifications.shared
        push.install(app: app)
        push.openChat = { [weak self] chatId in
            guard let router = self?.window?.rootViewController as? AppRouter else { return false }
            router.openSession(chatId)
            return true
        }
        app.onSignedIn = { [weak self] in
            self?.showRoot(animated: true)
            self?.shellShown()
        }
        app.onSignOut = { [weak self] in
            guard let self else { return }
            push.signingOut(client: self.app.client)
            self.app.signOutLocally()
            self.showRoot(animated: true)
        }
        let args = ProcessInfo.processInfo.arguments
        // `-wallpaper <path> [-wallpaper-effect ascii]`: set the wallpaper at
        // launch (screenshots, tests); `-wallpaper none` clears it.
        if let i = args.firstIndex(of: "-wallpaper"), i + 1 < args.count {
            if args[i + 1] == "none" {
                WallpaperStore.remove()
            } else if let image = UIImage(contentsOfFile: args[i + 1]) {
                WallpaperStore.set(image, name: (args[i + 1] as NSString).lastPathComponent)
            }
        }
        if let i = args.firstIndex(of: "-wallpaper-effect"), i + 1 < args.count,
           let effect = WallpaperStore.allEffects.first(where: { WallpaperStore.key($0) == args[i + 1] }) {
            WallpaperStore.effect = effect
        }
        if args.contains("-lab") {
            window.rootViewController = DrawerShellController.nav(TranscriptLabViewController())
        } else {
            showRoot(animated: false)
        }
        window.makeKeyAndVisible()
        if !args.contains("-lab") {
            shellShown()
            showTutorialIfNeeded()
        }
        if let activity = connectionOptions.userActivities.first(where: { $0.activityType == NSUserActivityTypeBrowsingWeb }) {
            DispatchQueue.main.async { [weak self] in self?.openCodeLink(activity.webpageURL) }
        }

        #if DEBUG
        if args.contains("-voice-preview") {
            app.voice.preview()
            if args.contains("-voice-stage") {
                DispatchQueue.main.async { [app] in window.rootViewController?.presentVoiceStage(app: app, source: nil) }
            }
        }
        #endif
        if let router = window.rootViewController as? AppRouter, let i = args.firstIndex(of: "-route"), i + 1 < args.count {
            let route = args[i + 1]
            DispatchQueue.main.async {
                if route.hasPrefix("chat:") { router.openSession(String(route.dropFirst(5))) }
                if route == "new" { router.presentNewSession(prompt: nil) }
                if route == "more" { router.showSettings() }
                if route == "search" { router.showSearch() }
                if route == "cloud" || route == "welcome" { router.showChat() }
            }
        }
    }

    /// The signed-in shell is up: refresh this phone's notification
    /// registration and open a session a notification was tapped for.
    private func shellShown() {
        guard app.isSignedIn else { return }
        if !app.isDemo { PushNotifications.shared.signedIn() }
        PushNotifications.shared.shellReady()
    }

    /// First launch: the tutorial over the shell, once (#11996). It ends on
    /// a new chat with the composer focused. `-route onboarding` shows it
    /// on the demo, starting at `-onboarding-step N`.
    private func showTutorialIfNeeded() {
        guard let onboarding = app.onboarding, onboarding.shouldShow(), let root = window?.rootViewController else { return }
        let args = ProcessInfo.processInfo.arguments
        var start = 0
        if let i = args.firstIndex(of: "-onboarding-step"), i + 1 < args.count { start = Int(args[i + 1]) ?? 0 }
        let tutorial = OnboardingViewController(steps: onboarding.steps(), start: start)
        tutorial.onScan = { [weak self, weak tutorial] in
            guard let self, let tutorial else { return }
            CodeAction.scan(from: tutorial, app: self.app)
        }
        tutorial.onFinish = { [weak self, weak tutorial] demo in
            guard let self else { return }
            onboarding.finish()
            let presenter = tutorial?.presentingViewController
            presenter?.dismiss(animated: true) {
                if demo {
                    self.app.openDemo()
                } else {
                    // A new chat; the chat page focuses its empty composer.
                    self.app.cloudChat?.newChat()
                    (self.window?.rootViewController as? AppRouter)?.showChat()
                }
            }
        }
        DispatchQueue.main.async {
            // Over anything already shown (a launch route's sheet).
            var top = root
            while let shown = top.presentedViewController { top = shown }
            top.present(tutorial, animated: false)
        }
    }

    private func showRoot(animated: Bool) {
        guard let window else { return }
        // iPad gets the split shell (it collapses to the drawer shell
        // at compact widths); iPhone the drawer shell directly. Nothing asks
        // the person to sign in (#11996), and the shell's first page is the
        // chat (#12009: there is no Home).
        let root: UIViewController = UIDevice.current.userInterfaceIdiom == .pad ? SplitRootController(app: app) : DrawerShellController(app: app)
        // Sheets (new session, Settings) belong to the old root: they'd
        // stay on top of the new one.
        if let old = window.rootViewController, old.presentedViewController != nil {
            old.dismiss(animated: false)
        }
        guard animated, window.rootViewController != nil else {
            window.rootViewController = root
            return
        }
        UIView.transition(with: window, duration: 0.35, options: [.transitionCrossDissolve, .allowAnimatedContent]) {
            window.rootViewController = root
        }
    }

    /// `openagents://voice` (the call's Live Activity) returns to the stage.
    func scene(_ scene: UIScene, openURLContexts URLContexts: Set<UIOpenURLContext>) {
        guard URLContexts.contains(where: { $0.url.scheme == "openagents" && $0.url.host == "voice" }),
              app.voice.live, let root = window?.rootViewController
        else { return }
        root.presentVoiceStage(app: app, source: nil)
    }

    /// `https://openagents.com/connect#…` (a computer's connect code) and
    /// `https://openagents.com/login/scan#…` (a website sign-in), opened by
    /// the phone's own camera as universal links (#12000, #12001). The link
    /// goes only to Rust; the app keeps no copy.
    func scene(_ scene: UIScene, continue userActivity: NSUserActivity) {
        guard userActivity.activityType == NSUserActivityTypeBrowsingWeb else { return }
        openCodeLink(userActivity.webpageURL)
    }

    private func openCodeLink(_ url: URL?) {
        guard let url, let presenter = topController() else { return }
        let text = url.absoluteString
        guard isWebsiteLogin(text: text) || computerCodeProblem(text: text) == nil else { return }
        CodeAction.run(text, from: presenter, app: app)
    }

    private func topController() -> UIViewController? {
        var top = window?.rootViewController
        while let shown = top?.presentedViewController { top = shown }
        return top
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        app.didEnterBackground()
    }

    func sceneWillEnterForeground(_ scene: UIScene) {
        app.willEnterForeground()
    }
}
