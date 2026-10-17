import CoreImage.CIFilterBuiltins
import LocalAuthentication
import UIKit
import UniformTypeIdentifiers

/// The Wallet page. Rust (`wallet` in the mobile core) builds the whole
/// screen as a Rust Native view and decides every action; this controller
/// mounts that view, sends typed values and taps back, and does the three
/// things only the platform can: the pasteboard, Face ID, and the QR image.
final class WalletViewController: UIViewController, UITextViewDelegate {
    private let app: AppModel
    private let wallet: CoreWallet
    private let scroll = UIScrollView()
    private let content = UIStackView()
    private var observation: AnyObject?
    private var instance = ""
    private var revision: UInt64 = 0
    private var page = ""
    private var views: [String: UIView] = [:]
    private var fieldKeys: [ObjectIdentifier: String] = [:]

    init(app: AppModel, wallet: CoreWallet) {
        self.app = app
        self.wallet = wallet
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Wallet"
        view.backgroundColor = .systemBackground
        view.tintColor = WalletAppearance.accent
        view.accessibilityIdentifier = "wallet"
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.keyboardDismissMode = .interactive
        scroll.accessibilityIdentifier = "wallet-scroll"
        let refresh = UIRefreshControl()
        refresh.addAction(UIAction { [weak self, weak refresh] _ in
            self?.wallet.refresh()
            refresh?.endRefreshing()
        }, for: .valueChanged)
        scroll.refreshControl = refresh
        content.axis = .vertical
        content.spacing = 24
        content.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        scroll.addSubview(content)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40),
        ])
        observation = app.observeWallet { [weak self] in self?.render() }
        render()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        wallet.open()
        render()
    }

    // MARK: Mounting

    private func render() {
        guard let json = try? wallet.snapshot(), let data = json.data(using: .utf8),
              let document = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let root = document["root"] as? [String: Any] else { return }
        instance = document["instance"] as? String ?? ""
        revision = (document["revision"] as? NSNumber)?.uint64Value ?? 0
        let nextPage = root["key"] as? String ?? "wallet"
        let changedPage = page != nextPage
        page = nextPage
        var used = Set<String>()
        let children = (props(root)["children"] as? [[String: Any]]) ?? []
        arrange(content, children.compactMap { mount($0, used: &used) })
        views = views.filter { used.contains($0.key) }
        fieldKeys = fieldKeys.filter { id, _ in views.values.contains { view in
            (view as? UIStackView)?.arrangedSubviews.contains { ObjectIdentifier($0) == id } == true
        } }
        if changedPage {
            scroll.setContentOffset(CGPoint(x: 0, y: -scroll.adjustedContentInset.top), animated: false)
            UIAccessibility.post(notification: .screenChanged, argument: content)
        }
    }

    private func props(_ node: [String: Any]) -> [String: Any] {
        ((node["element"] as? [String: Any])?["props"] as? [String: Any]) ?? [:]
    }

    /// Change a stack's children only where they differ, so a focused field
    /// keeps its keyboard while the rest of the screen updates.
    private func arrange(_ stack: UIStackView, _ children: [UIView]) {
        if stack.arrangedSubviews.elementsEqual(children, by: ===) { return }
        for child in stack.arrangedSubviews where !children.contains(where: { $0 === child }) {
            stack.removeArrangedSubview(child)
            child.removeFromSuperview()
        }
        for (index, child) in children.enumerated() {
            if index < stack.arrangedSubviews.count, stack.arrangedSubviews[index] === child { continue }
            if stack.arrangedSubviews.contains(where: { $0 === child }) { stack.removeArrangedSubview(child) }
            stack.insertArrangedSubview(child, at: index)
        }
    }

    private func cached<T: UIView>(_ key: String, _ make: () -> T) -> T {
        if let view = views[key] as? T { return view }
        let view = make()
        views[key] = view
        return view
    }

    private func mount(_ node: [String: Any], used: inout Set<String>) -> UIView? {
        guard let key = node["key"] as? String,
              let kind = (node["element"] as? [String: Any])?["kind"] as? String else { return nil }
        let p = props(node)
        used.insert(key)
        switch kind {
        case "stack":
            let card = cached(key) { Self.card(key) }
            guard let stack = (card as? UIStackView) ?? card.subviews.compactMap({ $0 as? UIStackView }).first else { return nil }
            let horizontal = (p["axis"] as? String) == "horizontal"
                && !traitCollection.preferredContentSizeCategory.isAccessibilityCategory
            stack.axis = horizontal ? .horizontal : .vertical
            stack.distribution = stack.axis == .horizontal ? .fillEqually : .fill
            let children = (p["children"] as? [[String: Any]]) ?? []
            arrange(stack, children.compactMap { mount($0, used: &used) })
            return card
        case "text":
            let value = p["value"] as? String ?? ""
            let role = p["role"] as? String ?? "body"
            if key == "qr" { return code(key, value) }
            let label = cached(key) { UILabel() }
            label.numberOfLines = 0
            label.text = value
            label.adjustsFontForContentSizeCategory = true
            label.accessibilityIdentifier = "wallet-\(key)"
            switch role {
            case "heading":
                let size: CGFloat = key == "balance" ? 36 : (key.hasSuffix("heading") || key == "heading" ? 28 : 21)
                label.font = UIFontMetrics(forTextStyle: .title1).scaledFont(for: .systemFont(ofSize: size, weight: .semibold))
                label.textColor = .label
                label.accessibilityTraits = .header
            case "status":
                label.font = .preferredFont(forTextStyle: .subheadline)
                label.textColor = key.hasSuffix("error") ? .systemRed : .secondaryLabel
            case "code":
                label.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 15, weight: .regular))
                label.lineBreakMode = .byCharWrapping
                label.textColor = .label
            default:
                label.font = .preferredFont(forTextStyle: .body)
                label.textColor = .label
            }
            if ["balance", "balance-label", "wallet-mark"].contains(key) { label.textAlignment = .center }
            if key == "wallet-mark" {
                label.font = UIFontMetrics(forTextStyle: .largeTitle).scaledFont(for: .systemFont(ofSize: 64, weight: .medium))
                label.textColor = WalletAppearance.accent
                label.accessibilityLabel = "Bitcoin"
            }
            if key.hasPrefix("payment-") {
                label.font = .preferredFont(forTextStyle: .body)
            }
            return label
        case "button":
            let button = cached(key) { () -> UIButton in
                let button = UIButton(type: .system)
                button.addAction(UIAction { [weak self] _ in self?.activate(key) }, for: .touchUpInside)
                return button
            }
            let label = p["label"] as? String ?? ""
            let primary = ["create", "pay", "invoice", "review", "trust-ok", "send-done", "restore", "receive-open"].contains(key)
            let plain = ["home", "restore-open", "restore-cancel", "send-cancel", "trust", "refresh", "activity-open"].contains(key)
            var config = WalletAppearance.button(primary: primary, plain: plain)
            config.title = label
            if label.hasPrefix("✓ ") {
                config.baseBackgroundColor = WalletAppearance.accent.withAlphaComponent(0.12)
                config.background.strokeColor = WalletAppearance.accent
                button.accessibilityTraits.insert(.selected)
            } else {
                button.accessibilityTraits.remove(.selected)
            }
            if key == "receive-open" { config.image = UIImage(systemName: "arrow.down.left") }
            if key == "send-open" { config.image = UIImage(systemName: "arrow.up.right") }
            if key == "home" { config.image = UIImage(systemName: "chevron.left") }
            if ["activity-open", "trust"].contains(key) {
                config.image = UIImage(systemName: "chevron.right")
                config.imagePlacement = .trailing
            }
            button.contentHorizontalAlignment = plain && key != "restore-open" ? .leading : .center
            config.imagePadding = 8
            button.configuration = config
            button.titleLabel?.adjustsFontForContentSizeCategory = true
            button.titleLabel?.numberOfLines = 0
            button.isEnabled = p["enabled"] as? Bool ?? true
            button.accessibilityIdentifier = "wallet-\(key)"
            return button
        case "field":
            let label = p["label"] as? String ?? ""
            let box = cached(key) { () -> UIStackView in
                let box = UIStackView()
                box.axis = .vertical
                box.spacing = 6
                let heading = UILabel()
                heading.font = .preferredFont(forTextStyle: .subheadline)
                heading.textColor = .secondaryLabel
                let field = UITextView()
                field.delegate = self
                field.font = .preferredFont(forTextStyle: .body)
                field.backgroundColor = .secondarySystemBackground
                field.layer.cornerRadius = 8
                field.layer.borderWidth = 1
                field.layer.borderColor = UIColor.separator.cgColor
                field.adjustsFontForContentSizeCategory = true
                field.isScrollEnabled = false
                field.textContainerInset = UIEdgeInsets(top: 10, left: 6, bottom: 10, right: 6)
                // Recovery words and payment codes must not reach the
                // keyboard's learned words or suggestions.
                field.autocorrectionType = .no
                field.spellCheckingType = .no
                field.autocapitalizationType = .none
                field.smartQuotesType = .no
                field.smartDashesType = .no
                field.inlinePredictionType = .no
                field.heightAnchor.constraint(greaterThanOrEqualToConstant: (p["multiline"] as? Bool == true) ? 88 : 44).isActive = true
                let toolbar = UIToolbar()
                toolbar.items = [UIBarButtonItem(systemItem: .flexibleSpace),
                                 UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.view.endEditing(true) })]
                toolbar.sizeToFit()
                field.inputAccessoryView = toolbar
                fieldKeys[ObjectIdentifier(field)] = key
                box.addArrangedSubview(heading)
                box.addArrangedSubview(field)
                return box
            }
            (box.arrangedSubviews.first as? UILabel)?.text = label
            if let field = box.arrangedSubviews.last as? UITextView {
                let value = p["value"] as? String ?? ""
                if field.text != value { field.text = value }
                field.isEditable = p["enabled"] as? Bool ?? true
                field.accessibilityLabel = label
                field.accessibilityHint = p["placeholder"] as? String
                field.accessibilityIdentifier = "wallet-\(key)"
            }
            return box
        default:
            return nil
        }
    }

    /// A section: a rounded box for cards and sections, a plain column for
    /// the rest.
    private static func card(_ key: String) -> UIView {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.spacing = key == "history" ? 20 : 16
        stack.accessibilityIdentifier = "wallet-\(key)"
        guard ["backup-card", "payment-review"].contains(key) else { return stack }
        let box = UIView()
        box.backgroundColor = .secondarySystemBackground
        box.layer.cornerRadius = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        box.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: box.topAnchor, constant: 16),
            stack.bottomAnchor.constraint(equalTo: box.bottomAnchor, constant: -16),
            stack.leadingAnchor.constraint(equalTo: box.leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(equalTo: box.trailingAnchor, constant: -16),
        ])
        box.accessibilityIdentifier = "wallet-\(key)"
        return box
    }

    /// A payment code: its QR image above the text.
    private func code(_ key: String, _ value: String) -> UIView {
        let box = cached(key) { () -> UIStackView in
            let box = UIStackView()
            box.axis = .vertical
            box.spacing = 10
            box.alignment = .center
            let image = UIImageView()
            image.layer.magnificationFilter = .nearest
            image.backgroundColor = .white
            image.contentMode = .scaleAspectFit
            image.isAccessibilityElement = true
            image.layer.cornerRadius = 8
            image.clipsToBounds = true
            image.widthAnchor.constraint(equalToConstant: 240).isActive = true
            image.heightAnchor.constraint(equalTo: image.widthAnchor).isActive = true
            let label = UILabel()
            label.numberOfLines = 0
            label.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 15, weight: .medium))
            label.adjustsFontForContentSizeCategory = true
            label.textColor = .label
            label.lineBreakMode = .byCharWrapping
            label.accessibilityIdentifier = "wallet-code"
            box.addArrangedSubview(image)
            box.addArrangedSubview(label)
            label.widthAnchor.constraint(equalTo: box.widthAnchor).isActive = true
            return box
        }
        if let label = box.arrangedSubviews.last as? UILabel, label.text != value {
            label.text = value
            (box.arrangedSubviews.first as? UIImageView)?.image = Self.qr(value)
            box.arrangedSubviews.first?.accessibilityLabel = "Payment request QR code"
        }
        return box
    }

    private static func qr(_ text: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        // Upper case fits the compact alphanumeric mode; these codes ignore case.
        filter.message = Data(text.uppercased().utf8)
        filter.correctionLevel = "M"
        guard let code = filter.outputImage else { return nil }
        let white = CIImage(color: .white).cropped(to: code.extent.insetBy(dx: -4, dy: -4))
        let output = code.composited(over: white).transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        guard let image = CIContext().createCGImage(output, from: output.extent) else { return nil }
        return UIImage(cgImage: image)
    }

    // MARK: Events

    func textViewDidChange(_ textView: UITextView) {
        guard let key = fieldKeys[ObjectIdentifier(textView)] else { return }
        // A refused edit puts Rust's value back on the next render.
        try? wallet.changeField(instance: instance, revision: revision, key: key, value: textView.text)
        render()
    }

    private func activate(_ key: String) {
        view.endEditing(true)
        do {
            let json = try wallet.activate(instance: instance, revision: revision, key: key)
            guard let data = json.data(using: .utf8),
                  let effect = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return render() }
            switch effect["kind"] as? String {
            case "copy":
                UIPasteboard.general.string = effect["text"] as? String
                Toast.show("Copied", in: view.window)
            case "backup":
                showWords()
            default: break
            }
        } catch {
            Toast.show("That didn't work. Try again.", in: view.window)
        }
        render()
    }

    /// The recovery words, after Face ID or the passcode.
    private func showWords() {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            Toast.show("Set a passcode on this phone to see your recovery words.", in: view.window)
            return
        }
        context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: "Show your recovery words") { [weak self] success, _ in
            DispatchQueue.main.async {
                guard let self, success, let words = try? self.wallet.recoveryWords() else { return }
                let sheet = RecoveryWordsController(words: words, expiry: TimeInterval(self.wallet.wordsPasteboardSeconds())) { [weak self] in
                    self?.wallet.wordsSaved()
                }
                let nav = UINavigationController(rootViewController: sheet)
                nav.modalPresentationStyle = .pageSheet
                nav.isModalInPresentation = true
                self.present(nav, animated: true)
            }
        }
    }
}

/// The recovery words, numbered. They exist only while this sheet is open.
/// They are drawn inside a secure text field's layer, so screenshots and
/// screen recordings show a blank area instead.
final class RecoveryWordsController: UIViewController {
    private var words: [String]
    private let expiry: TimeInterval
    private let saved: () -> Void
    private let secure = UITextField()

    init(words: [String], expiry: TimeInterval, saved: @escaping () -> Void) {
        self.words = words
        self.expiry = expiry
        self.saved = saved
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Recovery words"
        view.backgroundColor = .systemBackground
        view.tintColor = WalletAppearance.accent
        view.accessibilityIdentifier = "wallet-words"
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.close()
        })
        let note = UILabel()
        note.numberOfLines = 0
        note.font = .preferredFont(forTextStyle: .body)
        note.adjustsFontForContentSizeCategory = true
        note.textColor = Palette.text
        note.text = "Write these \(words.count) words down in order and keep them somewhere safe and offline. Anyone with them can take your bitcoin."
        let grid = UIStackView()
        grid.axis = .vertical
        grid.spacing = 8
        let half = (words.count + 1) / 2
        for row in 0..<half {
            let line = UIStackView()
            line.axis = .horizontal
            line.distribution = .fillEqually
            line.spacing = 12
            for index in [row, row + half] where index < words.count {
                let label = UILabel()
                label.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 17, weight: .medium))
                label.adjustsFontForContentSizeCategory = true
                label.numberOfLines = 0
                label.backgroundColor = .secondarySystemBackground
                label.layer.cornerRadius = 8
                label.clipsToBounds = true
                label.textColor = .label
                label.text = "\(index + 1). \(words[index])"
                line.addArrangedSubview(label)
            }
            grid.addArrangedSubview(line)
        }
        // Put the grid inside the secure field's own drawing layer.
        secure.isSecureTextEntry = true
        secure.isUserInteractionEnabled = false
        secure.layoutIfNeeded()
        // The secure field draws its text in a view that captures skip; the
        // grid goes inside it. Without one, the grid still shows normally.
        let canvas = (secure.layer.sublayers?.first?.delegate as? UIView) ?? secure.subviews.first ?? UIView()
        canvas.isUserInteractionEnabled = false
        grid.translatesAutoresizingMaskIntoConstraints = false
        canvas.addSubview(grid)
        let host = UIView()
        host.translatesAutoresizingMaskIntoConstraints = false
        canvas.translatesAutoresizingMaskIntoConstraints = false
        host.addSubview(canvas)
        NSLayoutConstraint.activate([
            canvas.topAnchor.constraint(equalTo: host.topAnchor),
            canvas.bottomAnchor.constraint(equalTo: host.bottomAnchor),
            canvas.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            grid.topAnchor.constraint(equalTo: canvas.topAnchor),
            grid.bottomAnchor.constraint(equalTo: canvas.bottomAnchor),
            grid.leadingAnchor.constraint(equalTo: canvas.leadingAnchor),
            grid.trailingAnchor.constraint(equalTo: canvas.trailingAnchor),
        ])
        var copyConfig = WalletAppearance.button(primary: false)
        copyConfig.title = "Copy words"
        copyConfig.cornerStyle = .large
        let copy = UIButton(configuration: copyConfig, primaryAction: UIAction { [weak self] _ in self?.copyWords() })
        copy.accessibilityIdentifier = "wallet-copy-words"
        var doneConfig = WalletAppearance.button(primary: true)
        doneConfig.title = "I wrote them down"
        doneConfig.cornerStyle = .large
        let done = UIButton(configuration: doneConfig, primaryAction: UIAction { [weak self] _ in
            self?.saved()
            self?.close()
        })
        done.accessibilityIdentifier = "wallet-words-saved"
        let stack = UIStackView(arrangedSubviews: [note, host, copy, done])
        stack.axis = .vertical
        stack.spacing = 20
        stack.setCustomSpacing(28, after: host)
        stack.translatesAutoresizingMaskIntoConstraints = false
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 20),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -24),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -48),
        ])
    }

    /// One line of words, on this phone's pasteboard only (never Universal
    /// Clipboard), gone after a minute.
    private func copyWords() {
        UIPasteboard.general.setItems(
            [[UTType.plainText.identifier: words.joined(separator: " ")]],
            options: [.localOnly: true, .expirationDate: Date().addingTimeInterval(expiry)])
        Toast.show("Copied for one minute", in: view.window)
    }

    private func close() {
        words = []
        dismiss(animated: true)
    }
}

/// WalletUI's filled, outlined, and plain hierarchy, adapted to native text sizing.
private enum WalletAppearance {
    static let accent = UIColor(red: 0.97, green: 0.58, blue: 0.10, alpha: 1)

    static func button(primary: Bool, plain: Bool = false) -> UIButton.Configuration {
        var config = primary ? UIButton.Configuration.filled() : UIButton.Configuration.plain()
        config.baseBackgroundColor = accent
        config.baseForegroundColor = primary ? .black : .label
        config.cornerStyle = .fixed
        config.background.cornerRadius = 8
        if !primary && !plain {
            config.background.strokeWidth = 1
            config.background.strokeColor = .separator
        }
        config.contentInsets = NSDirectionalEdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 16)
        config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
            var outgoing = incoming
            outgoing.font = UIFont.preferredFont(forTextStyle: .headline)
            return outgoing
        }
        return config
    }
}
