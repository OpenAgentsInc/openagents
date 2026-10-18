import UIKit

/// One saved agent's autonomy card (#12045): the shared Rust Native view the
/// desktop and the website show too. Rust resolves every action and returns
/// the fresh card; this only paints it.
final class AgentCardController: UIViewController {
    private let agents: CoreAgents?
    private let agentId: String
    private let content = UIStackView()
    private var json: String?
    private var acting = false

    /// The live card for `agentId`, read from the account.
    init(agents: CoreAgents, agentId: String) {
        self.agents = agents
        self.agentId = agentId
        super.init(nibName: nil, bundle: nil)
    }
    /// A fixed card, for the offline demo.
    init(json: String) {
        self.agents = nil
        self.agentId = ""
        self.json = json
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Agent card"
        view.backgroundColor = Palette.background
        view.accessibilityIdentifier = "agent-card"
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
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
        render()
        load()
    }

    private func load() {
        guard let agents else { return }
        let id = agentId
        Task { @MainActor [weak self] in
            do {
                self?.json = try await agents.autonomyCard(id: id)
                self?.render()
            } catch { Toast.show(error.localizedDescription, in: self?.view.window) }
        }
    }

    private func render() {
        content.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard let json, let data = json.data(using: .utf8),
              let doc = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let root = doc["root"] as? [String: Any], let instance = doc["instance"] as? String,
              let revision = doc["revision"] as? UInt64 else {
            let label = UILabel()
            label.text = "Loading…"
            label.textColor = Palette.text
            content.addArrangedSubview(label)
            return
        }
        content.addArrangedSubview(mount(root, instance: instance, revision: revision))
    }

    private func mount(_ node: [String: Any], instance: String, revision: UInt64) -> UIView {
        let element = node["element"] as? [String: Any] ?? [:]
        let props = element["props"] as? [String: Any] ?? [:]
        let key = node["key"] as? String ?? ""
        switch element["kind"] as? String {
        case "stack":
            let stack = UIStackView()
            let horizontal = props["axis"] as? String == "horizontal"
            stack.axis = horizontal ? .horizontal : .vertical
            stack.spacing = 12
            stack.alignment = horizontal ? .center : .fill
            for child in props["children"] as? [[String: Any]] ?? [] { stack.addArrangedSubview(mount(child, instance: instance, revision: revision)) }
            return stack
        case "text":
            let label = UILabel()
            label.text = props["value"] as? String
            label.numberOfLines = 0
            let role = props["role"] as? String
            label.font = UIFont.preferredFont(forTextStyle: role == "heading" ? .headline : role == "status" ? .footnote : .body)
            label.adjustsFontForContentSizeCategory = true
            label.textColor = role == "status" ? Palette.secondary : Palette.text
            label.accessibilityIdentifier = key
            return label
        case "button":
            let button = UIButton(type: .system)
            var config = UIButton.Configuration.tinted()
            config.title = props["label"] as? String
            button.configuration = config
            button.isEnabled = (props["enabled"] as? Bool ?? false) && agents != nil
            button.accessibilityIdentifier = key
            button.setContentHuggingPriority(.required, for: .horizontal)
            button.addAction(UIAction { [weak self] _ in self?.activate(instance, revision, key) }, for: .touchUpInside)
            return button
        default: return UIView()
        }
    }

    private func activate(_ instance: String, _ revision: UInt64, _ key: String) {
        guard let agents, !acting else { return }
        acting = true
        content.isUserInteractionEnabled = false
        let id = agentId
        Task { @MainActor [weak self] in
            guard let self else { return }
            defer { self.acting = false; self.content.isUserInteractionEnabled = true }
            do {
                self.json = try await agents.actOnCard(id: id, instance: instance, revision: revision, node: key)
                self.render()
            } catch { Toast.show(error.localizedDescription, in: self.view.window) }
        }
    }
}
