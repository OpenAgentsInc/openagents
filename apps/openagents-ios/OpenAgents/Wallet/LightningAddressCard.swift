import UIKit

/// The wallet screen's Lightning Address card (docs/auth/lightning-addresses.md, "The one-tap
/// claim"; #13209). Offered at the top of the wallet while the account has no address: a suggested
/// name, a field that checks as you type, one Claim. Nothing is claimed without the tap. Then the
/// address with Copy, Share, a QR and Turn off / Turn on. The limits are on the card before use.
/// LightningAddressClient talks to the server; SparkHost signs the claim's proof.
@MainActor
final class LightningAddressCardView: UIView, UITextFieldDelegate {
    /// What the card shows.
    enum Phase: Equatable { case loading, loggedOut, unclaimed, claimed, hidden }
    /// "Not now" hides the offer until the app starts again.
    static var notNow = false

    private let client: LightningAddressClient
    private let sign: LightningAddressClient.Sign
    /// Presents a sheet (Share, Log in) from the wallet screen.
    var present: ((UIViewController, UIView) -> Void)?
    /// The card changed size.
    var changed: (() -> Void)?

    private(set) var phase: Phase = .loading
    private(set) var card: LightningAddress.Card?
    private var checked: LightningAddress.Availability?
    private var checking: Task<Void, Never>?
    private var busy = false

    private let heading = Look.label("", Look.title(15))
    private let about = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    let field = Look.field("yourname", "lnaddr.name")
    private let domain = Look.label("@openagents.com", .preferredFont(forTextStyle: .body), Look.dim)
    let note = Look.label("", .preferredFont(forTextStyle: .footnote), Look.dim)
    private let address = Look.label("", .monospacedSystemFont(ofSize: 17, weight: .medium))
    let status = Look.label("", .preferredFont(forTextStyle: .footnote), Look.ink)
    private let code = UIImageView()
    let limits = Look.label("", .preferredFont(forTextStyle: .footnote), Look.dim)
    let claim = Look.button("Claim", "lnaddr.claim", primary: true)
    private let later = Look.button("Not now", "lnaddr.later")
    private let laterOut = Look.button("Not now", "lnaddr.later")
    private let login = Look.button("Log in", "lnaddr.login", primary: true)
    private let copy = Look.button("Copy", "lnaddr.copy")
    private let share = Look.button("Share", "lnaddr.share")
    let toggle = Look.button("Turn off", "lnaddr.toggle")
    private let column = UIStackView()
    private lazy var nameRow = UIStackView(arrangedSubviews: [field, domain])
    private lazy var offerRow = UIStackView(arrangedSubviews: [claim, later])
    private lazy var outRow = UIStackView(arrangedSubviews: [login, laterOut])
    private lazy var shareRow = UIStackView(arrangedSubviews: [copy, share])

    init(client: LightningAddressClient, sign: @escaping LightningAddressClient.Sign) {
        self.client = client; self.sign = sign
        super.init(frame: .zero)
        backgroundColor = UIColor(white: 1, alpha: 0.06)
        layer.cornerRadius = 12; layer.borderWidth = 1; layer.borderColor = Look.line.cgColor
        accessibilityIdentifier = "lnaddr.card"
        column.axis = .vertical; column.spacing = 10
        column.translatesAutoresizingMaskIntoConstraints = false
        addSubview(column)
        NSLayoutConstraint.activate([
            column.topAnchor.constraint(equalTo: topAnchor, constant: 16), column.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -16),
            column.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16), column.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
        ])
        field.autocapitalizationType = .none; field.autocorrectionType = .no; field.spellCheckingType = .no
        field.keyboardType = .asciiCapable; field.returnKeyType = .done; field.delegate = self
        field.textContentType = .username
        field.addAction(UIAction { [weak self] _ in self?.typed() }, for: .editingChanged)
        domain.setContentHuggingPriority(.required, for: .horizontal)
        domain.setContentCompressionResistancePriority(.required, for: .horizontal)
        nameRow.spacing = 8; nameRow.alignment = .center
        note.accessibilityIdentifier = "lnaddr.note"; status.accessibilityIdentifier = "lnaddr.status"
        limits.accessibilityIdentifier = "lnaddr.limits"; heading.accessibilityIdentifier = "lnaddr.heading"
        address.accessibilityIdentifier = "lnaddr.address"; address.lineBreakMode = .byCharWrapping
        code.contentMode = .scaleAspectFit; code.backgroundColor = .white; code.layer.cornerRadius = 10; code.clipsToBounds = true
        code.isAccessibilityElement = true; code.accessibilityLabel = "Lightning Address QR code"; code.accessibilityIdentifier = "lnaddr.qr"
        code.widthAnchor.constraint(equalToConstant: 180).isActive = true
        code.heightAnchor.constraint(equalTo: code.widthAnchor).isActive = true
        let qrRow = UIStackView(arrangedSubviews: [code]); qrRow.alignment = .center; qrRow.axis = .vertical
        for row in [offerRow, outRow, shareRow] { row.spacing = 12; row.distribution = .fillEqually }
        claim.addAction(UIAction { [weak self] _ in self?.claimIt() }, for: .touchUpInside)
        for b in [later, laterOut] { b.addAction(UIAction { [weak self] _ in Self.notNow = true; self?.show(.hidden) }, for: .touchUpInside) }
        login.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            let nav = LoginSheet(rootViewController: SignInViewController())
            nav.overrideUserInterfaceStyle = .dark
            nav.closed = { [weak self] in Task { await self?.load() } }
            self.present?(nav, self.login)
        }, for: .touchUpInside)
        copy.addAction(UIAction { [weak self] _ in
            guard let a = self?.card?.address else { return }
            UIPasteboard.general.string = a
            self?.note.text = "Copied."
            UIAccessibility.post(notification: .announcement, argument: "Copied")
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        }, for: .touchUpInside)
        share.addAction(UIAction { [weak self] _ in
            guard let self, let a = self.card?.address else { return }
            self.present?(UIActivityViewController(activityItems: [a], applicationActivities: nil), self.share)
        }, for: .touchUpInside)
        toggle.addAction(UIAction { [weak self] _ in self?.turn() }, for: .touchUpInside)
        [heading, about, nameRow, note, address, status, qrRow, shareRow, toggle, offerRow, outRow, limits].forEach(column.addArrangedSubview)
        show(.loading)
    }
    required init?(coder: NSCoder) { fatalError() }

    // MARK: what the server says

    /// Asks for the account's address and shows what came back.
    func load() async {
        guard !busy else { return }
        guard client.isLoggedIn else { show(Self.notNow ? .hidden : .loggedOut); return }
        do { apply(try await client.card()) }
        catch {
            // Logged out on the server too: offer the login. Anything else: say what happened.
            if !client.isLoggedIn { show(Self.notNow ? .hidden : .loggedOut); return }
            if card == nil && !Self.notNow { show(.unclaimed); note.text = error.localizedDescription; claim.isEnabled = false }
        }
    }

    func apply(_ c: LightningAddress.Card) {
        card = c
        limits.text = LightningAddress.limitsLine(c.limits)
        if c.address != nil { show(.claimed); return }
        if Self.notNow { show(.hidden); return }
        let was = phase
        show(.unclaimed)
        // Coming to the offer: the suggested name in the field, checked. Already on it: keep what is typed.
        guard was != .unclaimed else { return }
        if (field.text ?? "").isEmpty && !c.suggest.isEmpty { field.text = c.suggest }
        typed()
    }

    private func show(_ p: Phase) {
        phase = p
        isHidden = p == .hidden || p == .loading
        let unclaimed = p == .unclaimed, claimed = p == .claimed, out = p == .loggedOut
        nameRow.isHidden = !unclaimed; offerRow.isHidden = !unclaimed; outRow.isHidden = !out
        about.isHidden = claimed
        address.isHidden = !claimed; status.isHidden = !claimed; shareRow.isHidden = !claimed; toggle.isHidden = !claimed
        code.superview?.isHidden = !claimed
        limits.isHidden = out || limits.text == nil
        if out {
            heading.text = "Get a Lightning Address"
            about.text = "Log in to get a Lightning Address: yourname@openagents.com, paid straight to this wallet."
            note.text = nil
        } else if unclaimed {
            heading.text = "Get a Lightning Address"
            about.text = "Anyone can pay you at this name. Payments go straight to this wallet, never through ours."
        } else if claimed, let c = card {
            heading.text = "Your Lightning Address"
            address.text = c.address
            status.text = LightningAddress.status(c)
            status.textColor = c.state == "active" && (c.mode != "spark" || c.sparkReceiving) ? Look.ink : Look.warn
            code.image = c.address.flatMap { ReceiveViewController.qr("lightning:" + $0) }
            let on = c.state == "active"
            toggle.configuration?.title = on ? "Turn off" : "Turn on"
            toggle.isHidden = !(c.state == "active" || c.state == "off")
            copy.isEnabled = true; share.isEnabled = true
        }
        changed?()
    }

    // MARK: typing and claiming

    func textFieldShouldReturn(_ textField: UITextField) -> Bool { textField.resignFirstResponder(); return true }

    /// The name as typed changed: ask whether it's free after a short pause.
    private func typed() {
        checking?.cancel(); checked = nil
        let name = LightningAddress.typed(field.text ?? "")
        claim.isEnabled = false
        guard !name.isEmpty else { note.text = "Choose a name: 3 to 32 letters, digits, dots, dashes or underscores."; note.textColor = Look.dim; changed?(); return }
        note.text = "Checking " + name + "@openagents.com…"; note.textColor = Look.dim
        checking = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled, let self else { return }
            await self.check(name)
        }
    }

    func check(_ name: String) async {
        do {
            let a = try await client.available(name)
            guard LightningAddress.typed(field.text ?? "") == name else { return }
            checked = a
            if a.available {
                note.text = a.name + "@openagents.com is free."; note.textColor = Look.good
            } else {
                note.text = a.reason.isEmpty ? "That name is taken." : a.reason; note.textColor = Look.warn
            }
            claim.isEnabled = a.available && !busy
        } catch {
            note.text = error.localizedDescription; note.textColor = Look.warn
        }
        changed?()
    }

    func claimIt() {
        guard let a = checked, a.available, !busy else { return }
        field.resignFirstResponder()
        busy = true; claim.isEnabled = false; claim.configuration?.showsActivityIndicator = true
        note.text = "Claiming " + a.name + "@openagents.com…"; note.textColor = Look.dim
        Task { @MainActor in
            defer { busy = false; claim.configuration?.showsActivityIndicator = false }
            do {
                let c = try await client.claim(a.name, sign: sign)
                note.text = nil
                apply(c)
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                UIAccessibility.post(notification: .announcement, argument: "Your Lightning Address is " + (c.address ?? a.name))
            } catch {
                note.text = error.localizedDescription; note.textColor = Look.warn
                claim.isEnabled = true
                changed?()
            }
        }
    }

    private func turn() {
        guard let c = card, !busy else { return }
        let on = c.state != "active"
        busy = true; toggle.isEnabled = false
        Task { @MainActor in
            defer { busy = false; toggle.isEnabled = true }
            do { note.text = nil; apply(try await client.turn(on: on)) }
            catch { note.text = error.localizedDescription; note.textColor = Look.warn; changed?() }
        }
    }
}

/// The login sheet opened from the card: when it closes, the card asks again.
final class LoginSheet: UINavigationController {
    var closed: (() -> Void)?
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if isBeingDismissed { closed?() }
    }
}
