import UIKit

/// Signed-out screen while OpenAgents account sign-in is being connected.
final class SignInViewController: UIViewController {
    private let app: AppModel

    init(app: AppModel) {
        self.app = app
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        let title = UILabel()
        title.text = "OpenAgents Coder"
        title.font = Fonts.ui(.sansSemibold, 30)
        title.textColor = Palette.text
        let status = UILabel()
        status.text = "You're signed out."
        status.font = Fonts.ui(.sans, 17)
        status.textColor = Palette.secondary
        status.accessibilityIdentifier = "signed-out"
        let demo = UIButton(type: .system)
        var config = UIButton.Configuration.filled()
        config.title = "Try the demo"
        config.baseBackgroundColor = Palette.text
        config.baseForegroundColor = Palette.background
        config.cornerStyle = .capsule
        demo.configuration = config
        demo.accessibilityIdentifier = "open-demo"
        demo.addAction(UIAction { [weak self] _ in self?.app.openDemo() }, for: .touchUpInside)
        let stack = UIStackView(arrangedSubviews: [title, status, demo])
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 20
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
        ])
    }
}
