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
        view.backgroundColor = Palette.background
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
        content.spacing = 16
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
        var used = Set<String>()
        let children = (props(root)["children"] as? [[String: Any]]) ?? []
        arrange(content, children.compactMap { mount($0, used: &used) })
        views = views.filter { used.contains($0.key) }
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
                label.font = key == "balance" ? Fonts.ui(.sansSemibold, 40) : Fonts.ui(.sansSemibold, 20)
                label.textColor = Palette.text
                label.accessibilityTraits = .header
            case "status":
                label.font = .preferredFont(forTextStyle: .subheadline)
                label.textColor = key.hasSuffix("error") ? .systemRed : .secondaryLabel
            case "code":
                label.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
                label.textColor = Palette.text
            default:
                label.font = .preferredFont(forTextStyle: .body)
                label.textColor = Palette.text
            }
            return label
        case "button":
            let button = cached(key) { () -> UIButton in
                let button = UIButton(type: .system)
                button.addAction(UIAction { [weak self] _ in self?.activate(key) }, for: .touchUpInside)
                return button
            }
            var config = ["create", "restore", "pay", "invoice", "review", "trust-ok", "send-done"].contains(key)
                ? UIButton.Configuration.filled() : UIButton.Configuration.tinted()
            config.title = p["label"] as? String
            config.cornerStyle = .large
            button.configuration = config
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
                field.layer.cornerRadius = 10
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
        stack.spacing = 10
        guard ["receive", "send", "history", "backup-card", "trust-card"].contains(key) else { return stack }
        let box = UIView()
        box.backgroundColor = .secondarySystemBackground.withAlphaComponent(0.6)
        box.layer.cornerRadius = 16
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
            image.layer.cornerRadius = 8
            image.clipsToBounds = true
            image.widthAnchor.constraint(equalToConstant: 200).isActive = true
            image.heightAnchor.constraint(equalToConstant: 200).isActive = true
            let label = UILabel()
            label.numberOfLines = 0
            label.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
            label.textColor = Palette.text
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
            box.arrangedSubviews.first?.accessibilityLabel = "QR code"
        }
        return box
    }

    private static func qr(_ text: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        // Upper case fits the compact alphanumeric mode; these codes ignore case.
        filter.message = Data(text.uppercased().utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 8, y: 8)),
              let image = CIContext().createCGImage(output, from: output.extent) else { return nil }
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
        view.backgroundColor = Palette.background
        view.accessibilityIdentifier = "wallet-words"
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.close()
        })
        let note = UILabel()
        note.numberOfLines = 0
        note.font = .preferredFont(forTextStyle: .body)
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
                label.font = .monospacedSystemFont(ofSize: 17, weight: .medium)
                label.textColor = Palette.text
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
        var copyConfig = UIButton.Configuration.tinted()
        copyConfig.title = "Copy words"
        copyConfig.cornerStyle = .large
        let copy = UIButton(configuration: copyConfig, primaryAction: UIAction { [weak self] _ in self?.copyWords() })
        copy.accessibilityIdentifier = "wallet-copy-words"
        var doneConfig = UIButton.Configuration.filled()
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
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 20),
            stack.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
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
