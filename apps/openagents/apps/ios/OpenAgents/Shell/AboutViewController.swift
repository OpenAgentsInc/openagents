import UIKit

/// Settings → About: our name, version, copyright and license first; the MIT
/// notice for code the app is based on stays at the bottom, small, because
/// the license requires it to ship with the app (owner, 2026-10-17).
final class AboutViewController: UIViewController {
    private let version: String

    init(version: String) {
        self.version = version
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "About"
        view.backgroundColor = Palette.background
        let text = UITextView()
        text.isEditable = false
        text.backgroundColor = .clear
        text.textContainerInset = UIEdgeInsets(top: 24, left: 16, bottom: 32, right: 16)
        text.attributedText = body()
        text.accessibilityIdentifier = "about-text"
        text.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(text)
        NSLayoutConstraint.activate([
            text.topAnchor.constraint(equalTo: view.topAnchor),
            text.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            text.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            text.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])
    }

    private func body() -> NSAttributedString {
        let out = NSMutableAttributedString()
        func add(_ s: String, _ font: UIFont, _ color: UIColor, after: CGFloat) {
            let style = NSMutableParagraphStyle()
            style.paragraphSpacing = after
            out.append(NSAttributedString(string: s + "\n", attributes: [.font: font, .foregroundColor: color, .paragraphStyle: style]))
        }
        add("OpenAgents", Fonts.ui(.sansSemibold, 22), Palette.text, after: 4)
        add(version, Fonts.ui(.sans, 15), Palette.secondary, after: 20)
        add("© 2026 OpenAgents, Inc.", Fonts.ui(.sans, 15), Palette.text, after: 4)
        add("Licensed under the Apache License 2.0.", Fonts.ui(.sans, 15), Palette.text, after: 48)
        let notice = Bundle.main.url(forResource: "OpenAgents-MIT", withExtension: "txt")
            .flatMap { try? String(contentsOf: $0, encoding: .utf8) }
        if let notice {
            add("Third-party notices", Fonts.ui(.sansMedium, 12), Palette.tertiary, after: 6)
            add("Parts of this app are used under the MIT License:", Fonts.ui(.sans, 11), Palette.tertiary, after: 6)
            add(notice.trimmingCharacters(in: .whitespacesAndNewlines), Fonts.ui(.sans, 11), Palette.tertiary, after: 0)
        }
        return out
    }
}
