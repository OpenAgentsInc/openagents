import UIKit

final class SignInViewController: UIViewController {
    private let app: AppModel
    private var observation: AnyObject?
    private let status = UILabel()
    private let code = UILabel()
    private let signIn = UIButton(type: .system)
    private let website = UIButton(type: .system)
    private let cancel = UIButton(type: .system)
    private let demo = UIButton(type: .system)

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
        status.font = Fonts.ui(.sans, 17)
        status.textColor = Palette.secondary
        status.numberOfLines = 3
        status.textAlignment = .center
        status.accessibilityIdentifier = "signed-out"
        code.font = .monospacedSystemFont(ofSize: 28, weight: .semibold)
        code.textColor = Palette.text
        code.accessibilityIdentifier = "account-code"
        var config = UIButton.Configuration.filled()
        config.title = "Sign in with OpenAgents"
        config.baseBackgroundColor = Palette.text
        config.baseForegroundColor = Palette.background
        config.cornerStyle = .capsule
        signIn.configuration = config
        signIn.accessibilityIdentifier = "account-sign-in"
        signIn.addAction(UIAction { [weak self] _ in self?.begin() }, for: .touchUpInside)
        website.setTitle("Open website", for: .normal)
        website.addAction(UIAction { [weak self] _ in self?.app.openAccountWebsite() }, for: .touchUpInside)
        cancel.setTitle("Cancel", for: .normal)
        cancel.addAction(UIAction { [weak self] _ in self?.app.cancelAccountSignIn() }, for: .touchUpInside)
        demo.setTitle("Try the demo", for: .normal)
        demo.accessibilityIdentifier = "open-demo"
        demo.addAction(UIAction { [weak self] _ in self?.app.openDemo() }, for: .touchUpInside)
        let stack = UIStackView(arrangedSubviews: [title, status, code, signIn, website, cancel, demo])
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
        observation = app.observe { [weak self] in self?.render() }
        render()
    }
    private func begin() {
        if case .failed(_, true)? = app.accountSnapshot?.state { app.restoreAccount() }
        else { app.beginAccountSignIn() }
    }
    private func render() {
        code.isHidden = true
        website.isHidden = true
        cancel.isHidden = true
        signIn.isHidden = false
        signIn.isEnabled = true
        demo.isHidden = false
        signIn.configuration?.title = "Sign in with OpenAgents"
        switch app.accountSnapshot?.state {
        case let .signedOut(notice): status.text = notice ?? "You're signed out."
        case .restoring, .starting:
            status.text = "Signing in…"
            signIn.isEnabled = false
        case let .waiting(value, _):
            status.text = "Approve this code on the website."
            code.text = value
            code.isHidden = false
            website.isHidden = false
            cancel.isHidden = false
            signIn.isHidden = true
            demo.isHidden = true
        case let .failed(message, _):
            status.text = message
            signIn.configuration?.title = "Try again"
        case .signedIn: status.text = "You're signed in."
        case nil: status.text = "You're signed out."
        }
    }
}
