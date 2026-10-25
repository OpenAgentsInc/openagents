import UIKit

/// The Grid-only experiment (Swift flag `GRID_ONLY`, see apps/ios/README.md):
/// the app opens on one Aethyr-styled home screen whose only control is
/// "Enter the Grid", which opens the bundled Aethyr Grid full screen. No
/// account, chat, drawer, or other app surface is created in this variant.
/// Compiled into every build so the normal app keeps it building; only the
/// `GRID_ONLY` scene configuration uses it.
@objc(OpenAgentsGridOnlySceneDelegate)
final class GridOnlySceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.overrideUserInterfaceStyle = .dark
        window.rootViewController = GridOnlyHomeViewController()
        self.window = window
        window.makeKeyAndVisible()
    }
}

/// Aethyr UI (aethyr-cpp `web/app.css` / `player-controls.css`): black ink,
/// #ededed content, and a solid content-colored pill set in Michroma.
final class GridOnlyHomeViewController: UIViewController {
    static let ink = UIColor.black
    static let content = UIColor(red: 0xed / 255, green: 0xed / 255, blue: 0xed / 255, alpha: 1)
    private let enter = UIButton(type: .custom)
    private var autoEntered = false

    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Self.ink
        view.accessibilityIdentifier = "grid-home"

        var config = UIButton.Configuration.filled()
        config.cornerStyle = .capsule
        config.baseBackgroundColor = Self.content
        config.baseForegroundColor = Self.ink
        config.contentInsets = NSDirectionalEdgeInsets(top: 16, leading: 30, bottom: 16, trailing: 30)
        config.attributedTitle = AttributedString("Enter the Grid", attributes: AttributeContainer([
            .font: BundledGridViewController.controlFont(15),
            .kern: 1.6,
        ]))
        enter.configuration = config
        enter.configurationUpdateHandler = { button in
            // `button.solid:hover` brightens to white.
            button.configuration?.baseBackgroundColor = button.isHighlighted ? .white : Self.content
        }
        enter.accessibilityIdentifier = "grid-enter"
        enter.addAction(UIAction { [weak self] _ in self?.openGrid(animated: true) }, for: .touchUpInside)
        enter.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(enter)
        NSLayoutConstraint.activate([
            enter.centerXAnchor.constraint(equalTo: view.safeAreaLayoutGuide.centerXAnchor),
            enter.centerYAnchor.constraint(equalTo: view.safeAreaLayoutGuide.centerYAnchor),
        ])
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // `-enter-grid` opens the Grid at launch (screenshots, UI tests).
        guard !autoEntered, ProcessInfo.processInfo.arguments.contains("-enter-grid") else { return }
        autoEntered = true
        openGrid(animated: false)
    }

    private func openGrid(animated: Bool) {
        // Close returns here; there is no Sessions button in this variant.
        let grid = BundledGridViewController(openSessions: nil)
        grid.modalPresentationStyle = .fullScreen
        present(grid, animated: animated)
    }
}
