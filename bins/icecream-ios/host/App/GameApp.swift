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
        // iPadOS 26's "Windowed Apps" multitasking (Settings > Multitasking &
        // Gestures) opens apps as resizable windows; a game wants the whole
        // screen, so the smallest window it accepts is the screen.
        SceneDelegate.fillScreen(scene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = GameViewController()
        window.backgroundColor = .black
        window.makeKeyAndVisible()
        self.window = window
    }

    // Rotation swaps the screen's width and height; follow it, or a
    // minimum taller than the screen makes iPadOS shrink the whole scene.
    func windowScene(
        _ windowScene: UIWindowScene, didUpdate previousCoordinateSpace: UICoordinateSpace,
        interfaceOrientation previousInterfaceOrientation: UIInterfaceOrientation,
        traitCollection previousTraitCollection: UITraitCollection
    ) {
        SceneDelegate.fillScreen(windowScene)
    }

    static func fillScreen(_ scene: UIWindowScene) {
        guard scene.traitCollection.userInterfaceIdiom == .pad, let limits = scene.sizeRestrictions else { return }
        let screen = scene.screen.bounds.size
        if limits.minimumSize != screen { limits.minimumSize = screen }
    }
}
