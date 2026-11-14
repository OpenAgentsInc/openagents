import UIKit

/// The OpenAgents book's first milestone on the iPhone (chapters 6 and 11; GitHub #13516): the shared app
/// (apps/shared/openagents-app.coax, the same file the website runs at /milestone) drawn with UIKit
/// controls. Everything it decides is the shared app's: this screen draws its view (each element's id,
/// role, label and whether it can be pressed) and hands a button's press and a field's text to it.
///
/// Launched with `-OADrive SOCKET`, it answers coax drive on that socket (apps/shared/host/drive.c through
/// apple/phone-drive.c; on a physical iPhone `-OADrive tcp:PORT -OADriveToken TOKEN`, #13517): `tap ID` presses the UIButton with that accessibility identifier through its own
/// action, `type ID TEXT` types into the UITextField, and every other operation is the shared app's. With
/// `-OASharedApp YES` and no driver, a person uses it by hand and the fake services answer on their own.
/// Its services are fakes with recorded answers: it reaches no server and logs nobody in.
final class SharedAppViewController: UIViewController {
    /// The one app this process shows (coax drive's callbacks are C functions, so they find it here).
    static weak var shown: SharedAppViewController?

    private let host = UnsafeMutablePointer<coax_t_OapHost>.allocate(capacity: 1)
    private var view_: coax_v_t_OapHost { coax_v_t_OapHost(data: host, count: 1) }
    private let surface = NativeControls()
    private var controls: [String:UIView] {surface.controls}
    private var link: CADisplayLink?
    private let socket: String?
    private let token: String?

    init(socket: String?, token: String? = nil) {
        self.socket = socket
        self.token = token
        super.init(nibName: nil, bundle: nil)
        // zeroed in place: the struct is larger than a device's 1 MB main-thread stack, so `coax_t_OapHost()`
        // overflowed it on a physical iPhone (#13517) though the simulator's 8 MB stack held it
        UnsafeMutableRawPointer(host).initializeMemory(as: UInt8.self, repeating: 0, count: MemoryLayout<coax_t_OapHost>.stride)
        oap_host_open(view_, true, socket == nil)
        // a device check that holds a turn across the background slows the fakes down (#13517)
        let wait = UserDefaults.standard.double(forKey: "OASharedAppHandSeconds")
        if wait > 0 { oap_host_hand_wait(view_, Float(wait)) }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }
    deinit { host.deallocate() }

    override func viewDidLoad() {
        super.viewDidLoad()
        addChild(surface);surface.view.frame=view.bounds;surface.view.autoresizingMask=[.flexibleWidth,.flexibleHeight]
        view.addSubview(surface.view);surface.didMove(toParent:self)
        surface.press = { [weak self] id in self?.press(id) }
        surface.edit = { [weak self] id,text in self?.edit(id,text) }
        SharedAppViewController.shown = self
        render()
        if let socket {
            if phone_drive_start(socket, token, sharedAppAnswer, sharedAppStep) == 0 { NSLog("coax drive: could not listen on %@ (tcp:PORT needs -OADriveToken of 32 to 128 characters)", socket) }
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
        surface.render(elements().map { e in
            NativeElement(id:e.id,role:e.id.hasSuffix(".title") ? "heading":e.role,
                label:e.role == "message" ? "You: \(e.label)":e.label,value:e.field,disabled:e.off,
                complete:e.id == "account.code" ? "one-time-code":e.id == "account.email" ? "email":"",
                type:e.id == "account.code" ? "tel":e.id == "account.email" ? "email":"")
        })
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
    for (i, b) in bytes.enumerated() { out[i] = CChar(bitPattern: b) }
    return Int32(bytes.count)
}
/// One frame of `clock +MS`, on the main thread.
private func sharedAppStep(_ dt: Float) { MainActor.assumeIsolated { SharedAppViewController.shown?.step(dt) } }
