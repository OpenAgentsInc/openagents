import UIKit

/// Paints the shared Rust Native controls and forwards their action identities.
final class ChatHomesController: UIViewController {
    private let app: AppModel
    private let homes: CoreChatHomes
    private let content = UIStackView()
    private weak var host: UIViewController?
    private var observation: AnyObject?
    private var acting = false

    init(app: AppModel, homes: CoreChatHomes, host: UIViewController) {
        self.app = app
        self.homes = homes
        self.host = host
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Runs on"
        view.backgroundColor = Palette.background
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        content.translatesAutoresizingMaskIntoConstraints = false
        content.axis = .vertical
        content.spacing = 16
        view.addSubview(scroll)
        scroll.addSubview(content)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor), scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 20),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -20),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40),
        ])
        observation = app.observe { [weak self] in self?.render() }
        render()
    }
    private func render() {
        guard !acting else { return }
        content.arrangedSubviews.forEach { $0.removeFromSuperview() }
        do {
            for json in [try homes.choices(), try homes.parts()] {
                guard let data = json.data(using: .utf8), let doc = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let root = doc["root"] as? [String: Any], let instance = doc["instance"] as? String,
                      let revision = doc["revision"] as? UInt64 else { continue }
                content.addArrangedSubview(mount(root, instance: instance, revision: revision))
            }
        } catch { dismiss(animated: true) }
    }
    private func mount(_ node: [String: Any], instance: String, revision: UInt64) -> UIView {
        let element = node["element"] as? [String: Any] ?? [:]
        let props = element["props"] as? [String: Any] ?? [:]
        switch element["kind"] as? String {
        case "stack":
            let stack = UIStackView()
            stack.axis = props["axis"] as? String == "horizontal" ? .horizontal : .vertical
            stack.spacing = 12
            for child in props["children"] as? [[String: Any]] ?? [] { stack.addArrangedSubview(mount(child, instance: instance, revision: revision)) }
            return stack
        case "text":
            let label = UILabel()
            label.text = props["value"] as? String
            label.numberOfLines = 0
            label.font = UIFont.preferredFont(forTextStyle: props["role"] as? String == "heading" ? .headline : .body)
            label.adjustsFontForContentSizeCategory = true
            label.textColor = Palette.text
            return label
        case "button":
            let button = UIButton(type: .system)
            var config = UIButton.Configuration.tinted()
            config.title = props["label"] as? String
            config.titleLineBreakMode = .byWordWrapping
            button.configuration = config
            button.isEnabled = props["enabled"] as? Bool ?? false
            let key = node["key"] as? String ?? ""
            button.accessibilityIdentifier = key
            button.addAction(UIAction { [weak self] _ in self?.activate(instance, revision, key) }, for: .touchUpInside)
            return button
        default: return UIView()
        }
    }
    private func activate(_ instance: String, _ revision: UInt64, _ key: String) {
        guard !acting else { return }
        acting = true
        content.isUserInteractionEnabled = false
        Task { @MainActor [weak self] in
            guard let self else { return }
            defer { self.acting = false; self.content.isUserInteractionEnabled = true }
            do {
                let action = try await self.homes.activate(instance: instance, revision: revision, node: key)
                switch action {
                case let .openComputer(id):
                    self.dismiss(animated: true) { self.host?.router?.openSession(id) }
                case let .openSaved(id):
                    try self.app.savedHistory?.openChatPart(id: id)
                    self.dismiss(animated: true) {
                        guard let host = self.host else { return }
                        if !(host is SavedTranscriptController) {
                            host.navigationController?.pushViewController(SavedTranscriptController(app: self.app), animated: true)
                        }
                    }
                case let .draft(draft):
                    let host = self.host
                    self.dismiss(animated: true) {
                        guard let host else { return }
                        let next = NewSessionViewController(app: self.app, prompt: nil, continuation: draft) { [weak host] id, _ in
                            host?.dismiss(animated: true) { host?.router?.openSession(id) }
                        }
                        host.present(UINavigationController(rootViewController: next), animated: true)
                    }
                }
            } catch { Toast.show(error.localizedDescription, in: self.view.window) }
        }
    }
}
