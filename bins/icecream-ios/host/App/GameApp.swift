import AVFoundation
import UIKit

/// The Ice Cream Man: one full-screen web view running the game's
/// wasm/js bundle, which ships inside the app (the game folder). Nothing is
/// fetched from the network.
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Game sound mixes with other audio and follows the silent switch.
        try? AVAudioSession.sharedInstance().setCategory(.ambient)
        try? AVAudioSession.sharedInstance().setActive(true)
        return true
    }
}

/// One full-screen window scene holding the game.
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        // iPadOS 26's windowed multitasking would open the game as a small
        // window over the wallpaper; a game wants the whole screen, so the
        // smallest window it accepts is the screen itself.
        if scene.traitCollection.userInterfaceIdiom == .pad, let limits = scene.sizeRestrictions {
            let bounds = scene.screen.bounds.size
            let side = max(bounds.width, bounds.height)
            limits.minimumSize = CGSize(width: side, height: side)
        }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = GameViewController()
        window.backgroundColor = .black
        window.makeKeyAndVisible()
        self.window = window
    }
}
