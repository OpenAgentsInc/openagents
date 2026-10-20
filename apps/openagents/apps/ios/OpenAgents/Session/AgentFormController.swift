import LocalAuthentication
import UIKit

/// UIKit renders the Rust Native form; Rust resolves and validates every action.
final class AgentFormController: UIViewController, UITextViewDelegate {
    private let app: AppModel
    private let agents: CoreAgents?
    private let lab: CoreAgentLab?
    private let voiceAgent: CoreVoiceAgent?
    private let sharing: CoreSharing?
    private var profile: CorePublicProfile?
    private var payments: CoreBrowserPayments?
    private var lightning: CoreLightningAddress?
    private var poll: Timer?
    private var prefix: String { if lightning != nil { return "lightning-address" }; if payments != nil { return "browser-payments" }; if profile != nil { return "profile" }; if voiceAgent != nil { return "voice-agent" }; return sharing != nil ? "sharing" : (lab == nil ? "agent" : "lab") }
    private let stack = UIStackView()
    private var observation: AnyObject?
    private var instance = ""
    private var revision: UInt64 = 0
    private var controls: [String: UIView] = [:]
    private var fieldKeys: [ObjectIdentifier: String] = [:]

    init(app: AppModel, agents: CoreAgents) {
        self.voiceAgent = nil
        self.app = app
        self.agents = agents
        self.lab = nil
        self.sharing = nil
        super.init(nibName: nil, bundle: nil)
    }
    init(app: AppModel, lab: CoreAgentLab) {
        self.voiceAgent = nil
        self.app = app
        self.agents = nil
        self.lab = lab
        self.sharing = nil
        super.init(nibName: nil, bundle: nil)
    }
    init(app: AppModel, sharing: CoreSharing) {
        self.voiceAgent = nil
        self.app = app
        self.agents = nil
        self.lab = nil
        self.sharing = sharing
        super.init(nibName: nil, bundle: nil)
    }
    init(app: AppModel, profile: CorePublicProfile) {
        self.voiceAgent = nil
        self.app = app
        self.agents = nil
        self.lab = nil
        self.sharing = nil
        self.profile = profile
        super.init(nibName: nil, bundle: nil)
    }
    /// Wallet > Payments from your browser (#12310): Rust draws the card and
    /// decides; this asks for Face ID before a large Pay and polls while open.
    init(app: AppModel, payments: CoreBrowserPayments) {
        self.voiceAgent = nil
        self.app = app
        self.agents = nil
        self.lab = nil
        self.sharing = nil
        self.payments = payments
        super.init(nibName: nil, bundle: nil)
    }
    init(app: AppModel, voiceAgent: CoreVoiceAgent) {
        self.app = app
        self.agents = nil
        self.lab = nil
        self.sharing = nil
        self.voiceAgent = voiceAgent
        super.init(nibName: nil, bundle: nil)
    }

    init(app: AppModel, lightning: CoreLightningAddress) {
        self.app = app
        self.agents = nil
        self.lab = nil
        self.voiceAgent = nil
        self.sharing = nil
        self.lightning = lightning
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = lightning != nil ? "Lightning address" : payments != nil ? "Payments from your browser" : profile != nil ? "Public profile" : voiceAgent != nil ? "Voice agent" : sharing != nil ? "Project sharing" : (lab == nil ? "Agent" : "Agent Lab")
        view.backgroundColor = .systemBackground
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
        let scroll = UIScrollView()
        scroll.accessibilityIdentifier = "\(prefix)-form-scroll"
        scroll.keyboardDismissMode = .interactive
        scroll.translatesAutoresizingMaskIntoConstraints = false
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 20),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -20),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40),
        ])
        observation = app.observe { [weak self] in self?.render() }
        render()
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        voiceAgent?.setVisible(visible: true)
        lab?.setVisible(visible: true)
        sharing?.setVisible(visible: true)
        if let lightning {
            let refresh = { Task { @MainActor [weak self] in try? await lightning.refresh(); self?.render() } }
            _ = refresh()
            poll = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { _ in _ = refresh() }
        }
        if let payments {
            let refresh = { Task { @MainActor [weak self] in try? await payments.refresh(); self?.render() } }
            _ = refresh()
            poll = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { _ in _ = refresh() }
        }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        voiceAgent?.setVisible(visible: false)
        lab?.setVisible(visible: false)
        sharing?.setVisible(visible: false)
        poll?.invalidate()
        poll = nil
        if lab != nil { app.customAgents?.refresh() }
    }
    private func render() {
        let snapshot = agents?.snapshot()
        guard let json = lightning.flatMap({ try? $0.snapshot() }) ?? payments.flatMap({ try? $0.snapshot() }) ?? profile.flatMap({ try? $0.snapshot() }) ?? voiceAgent.flatMap({ try? $0.snapshot() }) ?? sharing.flatMap({ try? $0.snapshot() }) ?? lab.flatMap({ try? $0.snapshot() }) ?? snapshot?.formJson else { dismiss(animated: true); return }
        guard let data = json.data(using: .utf8), let form = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let root = form["root"] as? [String: Any] else { return }
        instance = form["instance"] as? String ?? ""
        revision = (form["revision"] as? NSNumber)?.uint64Value ?? 0
        let focused = controls.first { $0.value.isFirstResponder }?.key
        let selection = focused.flatMap { (controls[$0] as? UITextView)?.selectedRange }
        var arranged: [UIView] = []
        var used = Set<String>()
        func append(_ node: [String: Any]) {
            guard let key = node["key"] as? String, let element = node["element"] as? [String: Any], let kind = element["kind"] as? String, let props = element["props"] as? [String: Any] else { return }
            let children = props["children"] as? [[String: Any]] ?? []
            if kind == "stack" || kind == "list" { children.forEach(append); return }
            let label = props["label"] as? String ?? ""
            let enabled = props["enabled"] as? Bool ?? true
            let control: UIView
            if kind == "field" {
                let headingKey = "label:\(key)"
                let heading = controls[headingKey] as? UILabel ?? UILabel()
                controls[headingKey] = heading
                used.insert(headingKey)
                heading.text = label
                heading.font = .preferredFont(forTextStyle: .headline)
                arranged.append(heading)
                let field = controls[key] as? UITextView ?? UITextView()
                field.delegate = self
                let value = props["value"] as? String ?? ""
                if field.text != value { field.text = value }
                field.font = .preferredFont(forTextStyle: .body)
                field.backgroundColor = .secondarySystemBackground
                field.layer.cornerRadius = 8
                field.isEditable = enabled
                field.isScrollEnabled = false
                if field.inputAccessoryView == nil {
                    let toolbar = UIToolbar()
                    toolbar.items = [UIBarButtonItem(systemItem: .flexibleSpace),
                        UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.view.endEditing(true) })]
                    toolbar.sizeToFit()
                    field.inputAccessoryView = toolbar
                }
                field.accessibilityLabel = label
                field.accessibilityIdentifier = "\(prefix)-\(key)"
                if field.constraints.isEmpty { field.heightAnchor.constraint(greaterThanOrEqualToConstant: (props["multiline"] as? Bool == true) ? 140 : 44).isActive = true }
                fieldKeys[ObjectIdentifier(field)] = key
                control = field
            } else if kind == "button" || kind == "choice" {
                let existing = controls[key] as? UIButton
                let button = existing ?? UIButton(type: .system)
                var config = UIButton.Configuration.tinted()
                config.title = label
                if props["selected"] as? Bool == true { config.image = UIImage(systemName: "checkmark.circle.fill"); config.imagePadding = 8 }
                button.configuration = config
                button.isEnabled = enabled
                button.accessibilityIdentifier = "\(prefix)-\(key)"
                if existing == nil { button.addAction(UIAction { [weak self] _ in self?.activate(key) }, for: .touchUpInside) }
                control = button
            } else if kind == "text" {
                let text = controls[key] as? UILabel ?? UILabel()
                text.text = props["value"] as? String
                text.font = .preferredFont(forTextStyle: .body)
                text.numberOfLines = 0
                control = text
            } else { return }
            controls[key] = control
            used.insert(key)
            arranged.append(control)
        }
        append(root)
        controls = controls.filter { used.contains($0.key) }
        if let notice = snapshot?.notice {
            let label = controls["notice"] as? UILabel ?? UILabel()
            controls["notice"] = label
            used.insert("notice")
            label.text = notice
            label.numberOfLines = 0
            label.textColor = .systemRed
            label.accessibilityIdentifier = "agent-notice"
            arranged.append(label)
        }
        for child in stack.arrangedSubviews where !arranged.contains(where: { $0 === child }) {
            stack.removeArrangedSubview(child)
            child.removeFromSuperview()
        }
        for (index, child) in arranged.enumerated() {
            if index < stack.arrangedSubviews.count && stack.arrangedSubviews[index] === child { continue }
            if stack.arrangedSubviews.contains(where: { $0 === child }) { stack.removeArrangedSubview(child) }
            stack.insertArrangedSubview(child, at: index)
        }
        controls = controls.filter { used.contains($0.key) }
        if let focused, let field = controls[focused] as? UITextView, field.isEditable {
            field.becomeFirstResponder()
            if let selection, NSMaxRange(selection) <= field.text.utf16.count { field.selectedRange = selection }
        }
    }
    func textViewDidChange(_ textView: UITextView) {
        guard let key = fieldKeys[ObjectIdentifier(textView)] else { return }
        do { if let profile { try profile.changeField(instance: instance, revision: revision, key: key, value: textView.text) }
            else if let sharing { try sharing.changeField(instance: instance, revision: revision, key: key, value: textView.text) }
            else if let lab { try lab.changeField(instance: instance, revision: revision, key: key, value: textView.text) }
            else { try agents?.changeField(instance: instance, revision: revision, key: key, value: textView.text) } }
        catch { /* Restore the Rust-owned value after a rejected edit. */ }
        render()
    }
    private func activate(_ key: String) {
        if let lightning {
            let currentInstance = instance
            let currentRevision = revision
            Task { @MainActor [weak self] in
                do {
                    let effect = try await lightning.activate(instance: currentInstance, revision: currentRevision, key: key)
                    if let data = effect.data(using: .utf8),
                       let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                       let text = value["copy"] as? String { UIPasteboard.general.string = text }
                    if let data = effect.data(using: .utf8),
                       let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                       let text = value["share"] as? String, let host = self {
                        let sheet = UIActivityViewController(activityItems: [text], applicationActivities: nil)
                        sheet.popoverPresentationController?.sourceView = host.view
                        sheet.popoverPresentationController?.sourceRect = CGRect(x: host.view.bounds.midX, y: host.view.bounds.midY, width: 1, height: 1)
                        host.present(sheet, animated: true)
                    }
                    self?.render()
                } catch { self?.showError(error); self?.render() }
            }
            return
        }
        if let payments {
            let currentInstance = instance
            let currentRevision = revision
            let run = {
                Task { @MainActor [weak self] in
                    do {
                        try await payments.activate(instance: currentInstance, revision: currentRevision, key: key)
                        self?.render()
                    } catch { self?.showError(error); self?.render() }
                }
            }
            guard payments.needsCheck(key: key) else { _ = run(); return }
            let context = LAContext()
            var problem: NSError?
            guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &problem) else {
                showError(NSError(domain: "OpenAgents", code: 1, userInfo: [NSLocalizedDescriptionKey: "Set a passcode on this phone to pay more than 1,000 sats."]))
                return
            }
            context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: "Pay what your browser asked for") { success, _ in
                if success { DispatchQueue.main.async { _ = run() } }
            }
            return
        }
        if let profile {
            let currentInstance = instance
            let currentRevision = revision
            Task { @MainActor [weak self] in
                do {
                    try await profile.activate(instance: currentInstance, revision: currentRevision, key: key)
                    self?.render()
                } catch { self?.showError(error); self?.render() }
            }
            return
        }
        if let voiceAgent {
            let currentInstance = instance
            let currentRevision = revision
            Task { @MainActor [weak self] in
                do {
                    try await voiceAgent.activate(instance: currentInstance, revision: currentRevision, key: key)
                    self?.render()
                } catch { self?.showError(error) }
            }
            return
        }
        if let sharing {
            let currentInstance = instance
            let currentRevision = revision
            Task { @MainActor [weak self] in
                do {
                    if let text = try await sharing.activate(instance: currentInstance, revision: currentRevision, key: key) {
                        UIPasteboard.general.string = text
                    }
                    self?.render()
                } catch { self?.showError(error) }
            }
            return
        }
        if let lab {
            let currentInstance = instance
            let currentRevision = revision
            Task { @MainActor [weak self] in
                do {
                    let json = try await lab.activate(instance: currentInstance, revision: currentRevision, key: key)
                    if let data = json.data(using: .utf8), let effect = try JSONSerialization.jsonObject(with: data) as? [String: Any], effect["kind"] as? String == "session", let id = effect["id"] as? String {
                        let appRouter = self?.router
                        self?.dismiss(animated: true) { appRouter?.openSession(id) }
                    }
                    if key == "promote" || key == "retry" { self?.app.customAgents?.refresh() }
                    self?.render()
                } catch { self?.showError(error) }
            }
            return
        }
        do { try agents?.activate(instance: instance, revision: revision, key: key); render() }
        catch {
            showError(error)
        }
    }
    private func showError(_ error: Error) {
        let alert = UIAlertController(title: "Couldn't complete that action", message: error.localizedDescription, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        if presentedViewController == nil { present(alert, animated: true) }
    }
}
