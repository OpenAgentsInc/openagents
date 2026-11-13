import UIKit

/// The OpenAgents book's first milestone on the iPhone (chapters 6 and 11; GitHub #13516): the shared app
/// (apps/shared/openagents-app.coax, the same file the website runs at /milestone) drawn with UIKit
/// controls. Everything it decides is the shared app's: this screen draws its view (each element's id,
/// role, label and whether it can be pressed) and hands a button's press and a field's text to it.
///
/// Launched with `-OADrive SOCKET`, it answers coax drive on that socket (apps/shared/host/drive.c through
/// apple/phone-drive.c): `tap ID` presses the UIButton with that accessibility identifier through its own
/// action, `type ID TEXT` types into the UITextField, and every other operation is the shared app's. With
/// `-OASharedApp YES` and no driver, a person uses it by hand and the fake services answer on their own.
/// Its services are fakes with recorded answers: it reaches no server and logs nobody in.
final class SharedAppViewController: UIViewController {
    /// The one app this process shows (coax drive's callbacks are C functions, so they find it here).
    static weak var shown: SharedAppViewController?

    private let host = UnsafeMutablePointer<coax_t_OapHost>.allocate(capacity: 1)
    private var view_: coax_v_t_OapHost { coax_v_t_OapHost(data: host, count: 1) }
    private let stack = UIStackView()
    private var controls: [String: UIView] = [:]
    private var link: CADisplayLink?
    private let socket: String?

    init(socket: String?) {
        self.socket = socket
        super.init(nibName: nil, bundle: nil)
        host.initialize(to: coax_t_OapHost())
        oap_host_open(view_, true, socket == nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }
    deinit { host.deallocate() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        overrideUserInterfaceStyle = .dark
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.keyboardDismissMode = .interactive
        view.addSubview(scroll)
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        let guide = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: guide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            scroll.leadingAnchor.constraint(equalTo: guide.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: guide.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 24),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            stack.leadingAnchor.constraint(equalTo: scroll.frameLayoutGuide.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: scroll.frameLayoutGuide.trailingAnchor, constant: -20),
        ])
        SharedAppViewController.shown = self
        render()
        if let socket {
            if phone_drive_start(socket, sharedAppAnswer, sharedAppStep) == 0 { NSLog("coax drive: could not listen on %@", socket) }
        } else {
            let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
            link.add(to: .main, forMode: .common)
            self.link = link
        }
    }

    // MARK: the shared app

    private func put(_ text: String, at: Int = 0) -> Int {
        let bytes = Array(text.utf8)
        let room = 4096 - at   // OapHost.in (OAP_IN)
        let n = min(bytes.count, max(0, room))
        withUnsafeMutableBytes(of: &host.pointee.`in`) { raw in
            for i in 0..<n { raw[at + i] = bytes[i] }
        }
        return at + n
    }
    private func out(_ n: Int64) -> String {
        withUnsafeBytes(of: &host.pointee.out) { raw in
            String(decoding: raw.prefix(Int(max(0, n))), as: UTF8.self)
        }
    }
    @discardableResult func press(_ id: String) -> String {
        let answer = out(oap_host_press(view_, Int64(put(id))))
        copyIfAsked()
        render()
        return answer
    }
    func edit(_ id: String, _ text: String) {
        let k = put(id)
        _ = oap_host_edit(view_, Int64(k), Int64(put(text, at: k)))
        render()
    }
    func drive(_ op: String) -> String {
        let answer = out(oap_host_drive(view_, Int64(put(op))))
        render()
        return answer
    }
    func step(_ dt: Float) { oap_host_step(view_, dt) }
    /// What Copy put on the clipboard goes on the phone's, by hand only: a driven run never touches the
    /// pasteboard, which the simulator shares with the Mac.
    private func copyIfAsked() {
        let n = oap_host_clip(view_)
        if n > 0 && socket == nil { UIPasteboard.general.string = out(n) }
    }
    @objc private func tick(_ link: CADisplayLink) {
        if oap_host_tick(view_, Float(link.targetTimestamp - link.timestamp)) { render() }
    }

    // MARK: drawing the view

    private struct Element { let id: String; let role: String; let off: Bool; let label: String; let field: String }
    private func elements() -> [Element] {
        let count = Int(oap_host_view(view_))
        return (0..<count).map { k in
            let parts = out(oap_host_el(view_, Int64(k))).split(separator: "\t", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
            let field = parts.count > 1 && parts[1] == "field" ? out(oap_host_field(view_, Int64(k))) : ""
            return Element(id: parts[0], role: parts.count > 1 ? parts[1] : "text", off: parts.count > 2 && parts[2] == "1", label: parts.count > 3 ? parts[3] : "", field: field)
        }
    }
    /// Each element's control, kept by id across draws (a field keeps its caret), in the view's order.
    private func render() {
        let now = elements()
        var kept: [String: UIView] = [:]
        for e in now {
            let control = controls[e.id].flatMap { fits($0, e.role) ? $0 : nil } ?? make(e)
            update(control, e)
            kept[e.id] = control
        }
        for (id, old) in controls where kept[id] == nil { old.removeFromSuperview() }
        controls = kept
        for (i, e) in now.enumerated() {
            guard let control = kept[e.id] else { continue }
            if i < stack.arrangedSubviews.count, stack.arrangedSubviews[i] === control { continue }
            stack.insertArrangedSubview(control, at: i)
        }
    }
    private func fits(_ control: UIView, _ role: String) -> Bool {
        switch role {
        case "button": return control is UIButton
        case "field": return control is UITextField
        default: return control is UILabel
        }
    }
    private func make(_ e: Element) -> UIView {
        let control: UIView
        switch e.role {
        case "button":
            var config = UIButton.Configuration.bordered()
            config.baseForegroundColor = .white
            let button = UIButton(configuration: config)
            let id = e.id
            button.addAction(UIAction { [weak self] _ in self?.press(id) }, for: .primaryActionTriggered)
            control = button
        case "field":
            let field = UITextField()
            field.borderStyle = .roundedRect
            field.autocapitalizationType = .none
            field.autocorrectionType = .no
            field.textColor = .white
            let id = e.id
            field.addAction(UIAction { [weak self, weak field] _ in self?.edit(id, field?.text ?? "") }, for: .editingChanged)
            if e.id == "account.email" { field.keyboardType = .emailAddress; field.textContentType = .emailAddress }
            if e.id == "account.code" { field.keyboardType = .numberPad; field.textContentType = .oneTimeCode }
            control = field
        default:
            let label = UILabel()
            label.numberOfLines = 0
            label.textColor = .white
            control = label
        }
        control.accessibilityIdentifier = e.id
        return control
    }
    private func update(_ control: UIView, _ e: Element) {
        if let button = control as? UIButton {
            button.configuration?.title = e.label
            button.isEnabled = !e.off
        } else if let field = control as? UITextField {
            field.placeholder = e.label
            field.accessibilityLabel = e.label
            if field.text != e.field { field.text = e.field }
        } else if let label = control as? UILabel {
            label.text = e.role == "message" ? "You: \(e.label)" : e.label
            label.font = e.role == "text" && e.id.hasSuffix(".title") ? .preferredFont(forTextStyle: .largeTitle) : .preferredFont(forTextStyle: e.role == "notice" ? .footnote : .body)
            label.textColor = e.role == "notice" ? .secondaryLabel : .white
        }
    }

    // MARK: coax drive

    /// tap and type go through the control itself: the button's own action, the field's own text input.
    /// A control that isn't shown, or a button that can't be pressed, is the shared app's to refuse.
    func answer(_ op: String) -> String {
        let words = op.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: true).map(String.init)
        guard words.count >= 2, words[0] == "tap" || words[0] == "type", let control = controls[words[1]] else { return drive(op) }
        _ = drive("input")   // nothing from an earlier input is left to read
        if words[0] == "tap", let button = control as? UIButton, button.isEnabled {
            button.sendActions(for: .primaryActionTriggered)
        } else if words[0] == "type", let field = control as? UITextField {
            field.insertText(words.count > 2 ? words[2] : "")
            field.sendActions(for: .editingChanged)
        }
        let said = drive("input")
        return said == "no input reached the app" ? drive(op) : said
    }
}

/// coax drive's answer for one operation (apple/phone-drive.c calls it on the main thread).
private func sharedAppAnswer(_ op: UnsafePointer<CChar>?, _ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    guard let op, let out, cap > 0 else { return -1 }
    let text = MainActor.assumeIsolated { SharedAppViewController.shown?.answer(String(cString: op)) } ?? "the app is not shown yet"
    let bytes = Array(text.utf8.prefix(Int(cap) - 1))
    out.withMemoryRebound(to: UInt8.self, capacity: max(1, bytes.count)) { o in for (i, b) in bytes.enumerated() { o[i] = b } }
    return Int32(bytes.count)
}
/// One frame of `clock +MS`, on the main thread.
private func sharedAppStep(_ dt: Float) { MainActor.assumeIsolated { SharedAppViewController.shown?.step(dt) } }
