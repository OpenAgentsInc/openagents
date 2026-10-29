import UIKit
import AVFoundation
import CoreImage.CIFilterBuiltins

// The wallet's screens (UIKit, per docs/lightning/coax-integration.md):
// the balance and activity, sending, receiving and a payment's details.
// SparkHost does the work; verse/wallet.coax decides each send.

enum Look {
    static let ink = UIColor(white: 1, alpha: 0.92)
    static let dim = UIColor(white: 1, alpha: 0.55)
    static let line = UIColor(white: 1, alpha: 0.28)
    static let good = UIColor(red: 0.45, green: 0.86, blue: 0.55, alpha: 1)
    static let warn = UIColor(red: 1, green: 0.78, blue: 0.35, alpha: 1)
    static func title(_ size: CGFloat) -> UIFont { UIFont(name: "Michroma", size: size) ?? .systemFont(ofSize: size, weight: .semibold) }
    static func label(_ text: String = "", _ font: UIFont = .preferredFont(forTextStyle: .body), _ color: UIColor = ink) -> UILabel {
        let l = UILabel(); l.text = text; l.font = font; l.textColor = color; l.numberOfLines = 0
        l.adjustsFontForContentSizeCategory = true
        return l
    }
    /// aethyr's HUD slot as a UIKit button: glass, a thin rim, white text.
    static func button(_ title: String, _ id: String, primary: Bool = false) -> UIButton {
        var c = UIButton.Configuration.filled()
        c.title = title
        c.baseBackgroundColor = UIColor(white: 1, alpha: primary ? 0.16 : 0.07)
        c.baseForegroundColor = ink
        c.cornerStyle = .medium
        c.contentInsets = NSDirectionalEdgeInsets(top: 14, leading: 18, bottom: 14, trailing: 18)
        c.background.strokeColor = primary ? UIColor(white: 1, alpha: 0.6) : line
        c.background.strokeWidth = 1
        let b = UIButton(configuration: c)
        b.accessibilityIdentifier = id
        return b
    }
    static func field(_ placeholder: String, _ id: String) -> UITextField {
        let f = UITextField()
        f.attributedPlaceholder = NSAttributedString(string: placeholder, attributes: [.foregroundColor: dim])
        f.textColor = ink; f.borderStyle = .none; f.font = .preferredFont(forTextStyle: .body)
        f.backgroundColor = UIColor(white: 1, alpha: 0.07); f.layer.cornerRadius = 8
        f.layer.borderColor = line.cgColor; f.layer.borderWidth = 1
        f.leftView = UIView(frame: CGRect(x: 0, y: 0, width: 12, height: 1)); f.leftViewMode = .always
        f.heightAnchor.constraint(greaterThanOrEqualToConstant: 48).isActive = true
        f.accessibilityIdentifier = id
        return f
    }
    static func stack(_ views: [UIView], spacing: CGFloat = 14) -> UIStackView {
        let s = UIStackView(arrangedSubviews: views); s.axis = .vertical; s.spacing = spacing
        return s
    }
    static func sheet(_ vc: UIViewController) {
        vc.view.backgroundColor = .black
        vc.overrideUserInterfaceStyle = .dark
    }
}

/// A screen of stacked rows that scrolls, for the forms below.
class WalletForm: UIViewController {
    let scroll = UIScrollView()
    let column = Look.stack([])
    override func viewDidLoad() {
        super.viewDidLoad()
        Look.sheet(self)
        scroll.translatesAutoresizingMaskIntoConstraints = false
        column.translatesAutoresizingMaskIntoConstraints = false
        scroll.keyboardDismissMode = .interactive
        view.addSubview(scroll); scroll.addSubview(column)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.topAnchor), scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            column.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 20),
            column.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            column.leadingAnchor.constraint(equalTo: scroll.frameLayoutGuide.leadingAnchor, constant: 16),
            column.trailingAnchor.constraint(equalTo: scroll.frameLayoutGuide.trailingAnchor, constant: -16),
        ])
    }
    func fail(_ error: Error) { fail(error.localizedDescription) }
    func fail(_ message: String) {
        let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        present(alert, animated: true)
    }
    /// Runs `work`, with `button` showing it is busy until it ends.
    func busy(_ button: UIButton, _ work: @escaping @MainActor () async -> Void) {
        button.configuration?.showsActivityIndicator = true; button.isEnabled = false
        Task { @MainActor in
            await work()
            button.configuration?.showsActivityIndicator = false; button.isEnabled = true
        }
    }
}

// MARK: - The wallet

/// The balance, Send and Receive, deposits on the way, and activity.
final class WalletViewController: UITableViewController {
    private let host: SparkHost
    private var balance: UInt64 = 0
    private var rate: Double?
    private var payments: [SparkHost.Activity] = []
    private var deposits: [SparkHost.Deposit] = []
    private var address: String?
    private let balanceButton = UIButton(type: .system)
    private let dollars = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    private var watching: Task<Void, Never>?
    var closed: (() -> Void)?

    init(host: SparkHost) {
        self.host = host
        super.init(style: .insetGrouped)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Wallet"
        Look.sheet(self)
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "row")
        tableView.accessibilityIdentifier = "wallet.activity"
        balanceButton.titleLabel?.font = Look.title(30)
        balanceButton.setTitleColor(Look.ink, for: .normal)
        balanceButton.accessibilityIdentifier = "wallet.total"
        balanceButton.accessibilityHint = "Switches between sats and BTC."
        balanceButton.addAction(UIAction { [weak self] _ in Money.inBTC.toggle(); self?.render() }, for: .touchUpInside)
        dollars.textAlignment = .center
        let send = Look.button("Send", "wallet.send", primary: true)
        let receive = Look.button("Receive", "wallet.receive", primary: true)
        send.addAction(UIAction { [weak self] _ in self?.open(SendViewController(host: self!.host)) }, for: .touchUpInside)
        receive.addAction(UIAction { [weak self] _ in self?.open(ReceiveViewController(host: self!.host)) }, for: .touchUpInside)
        let buttons = UIStackView(arrangedSubviews: [receive, send]); buttons.spacing = 12; buttons.distribution = .fillEqually
        let header = Look.stack([balanceButton, dollars, buttons], spacing: 10)
        header.setCustomSpacing(22, after: dollars)
        header.layoutMargins = UIEdgeInsets(top: 24, left: 20, bottom: 12, right: 20); header.isLayoutMarginsRelativeArrangement = true
        header.frame.size = header.systemLayoutSizeFitting(CGSize(width: view.bounds.width, height: 0), withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        tableView.tableHeaderView = header
        refreshControl = UIRefreshControl()
        refreshControl?.addAction(UIAction { [weak self] _ in self?.reload() }, for: .valueChanged)
        render(); reload()
        watching = Task { [weak self, host] in
            for await change in await host.changes() {
                guard let self else { return }
                if case .received(let a) = change { self.announce("Received " + Money.text(a.amount)) }
                self.reload()
            }
        }
    }
    deinit { watching?.cancel() }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if navigationController?.isBeingDismissed ?? isBeingDismissed { closed?() }
    }

    private func open(_ vc: UIViewController) {
        navigationController?.pushViewController(vc, animated: true)
    }
    private func announce(_ text: String) {
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        UIAccessibility.post(notification: .announcement, argument: text)
    }

    private func reload() {
        Task { @MainActor in
            do {
                async let b = host.spendable()
                async let p = host.activity()
                async let d = host.deposits()
                balance = try await b; payments = try await p; deposits = (try? await d) ?? []
            } catch {
                NSLog("OpenAgents wallet: %@", String(describing: error))
            }
            rate = await host.dollarsPerBitcoin()
            address = await host.lightningAddress()
            refreshControl?.endRefreshing()
            render()
        }
    }

    private func render() {
        balanceButton.setTitle(Money.text(balance), for: .normal)
        balanceButton.accessibilityValue = Money.text(balance)
        dollars.text = [Money.dollars(balance, rate), address].compactMap { $0 }.joined(separator: "\n")
        tableView.reloadData()
    }

    // Sections: deposits on the way (if any), then activity.
    override func numberOfSections(in tableView: UITableView) -> Int { 2 }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        section == 0 ? deposits.count : max(payments.count, 1)
    }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        section == 0 ? (deposits.isEmpty ? nil : "Arriving on-chain") : "Activity"
    }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "row", for: indexPath)
        var c = UIListContentConfiguration.valueCell()
        c.secondaryTextProperties.color = Look.dim
        cell.accessoryType = .none; cell.selectionStyle = .default
        if indexPath.section == 0 {
            let d = deposits[indexPath.row]
            c.text = "+" + Money.text(d.amount)
            c.secondaryText = d.problem ?? (d.mature ? "Being added to your balance" : "Waiting for confirmations")
            c = c.subtitle()
            cell.selectionStyle = d.problem == nil ? .none : .default
        } else if payments.isEmpty {
            c.text = "No payments yet"; c.textProperties.color = Look.dim
            cell.selectionStyle = .none
        } else {
            let p = payments[indexPath.row]
            // Direction is shown by sign and word, not colour alone.
            c.text = (p.incoming ? "+" : "−") + Money.text(p.amount)
            c.textProperties.color = p.incoming ? Look.good : Look.ink
            let when = RelativeDateTimeFormatter().localizedString(for: p.date, relativeTo: Date())
            let state = p.status == .pending ? "Pending" : p.status == .failed ? "Failed" : (p.incoming ? "Received" : "Sent")
            c.secondaryText = [state, p.note ?? p.rail, when].compactMap { $0 }.joined(separator: " · ")
            c = c.subtitle()
            cell.accessoryType = .disclosureIndicator
            cell.accessibilityIdentifier = "wallet.payment.\(indexPath.row)"
        }
        cell.contentConfiguration = c
        cell.backgroundColor = UIColor(white: 1, alpha: 0.06)
        return cell
    }
    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if indexPath.section == 0 {
            let d = deposits[indexPath.row]
            guard d.problem != nil else { return }
            let ask = UIAlertController(title: "Claim this deposit now?", message: "\(d.problem ?? "")\n\nClaiming now pays the network's current fee, taken from the deposit.", preferredStyle: .alert)
            ask.addAction(UIAlertAction(title: "Not now", style: .cancel))
            ask.addAction(UIAlertAction(title: "Claim", style: .default) { [weak self] _ in
                Task { @MainActor in
                    do { try await self?.host.claim(d); self?.reload() } catch { self?.fail(error) }
                }
            })
            present(ask, animated: true)
        } else if !payments.isEmpty {
            open(PaymentViewController(payment: payments[indexPath.row], rate: rate))
        }
    }
    private func fail(_ error: Error) {
        let alert = UIAlertController(title: nil, message: error.localizedDescription, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default)); present(alert, animated: true)
    }
}

private extension UIListContentConfiguration {
    func subtitle() -> UIListContentConfiguration {
        var s = UIListContentConfiguration.subtitleCell()
        s.text = text; s.secondaryText = secondaryText
        s.textProperties = textProperties; s.secondaryTextProperties = secondaryTextProperties
        s.textProperties.font = .monospacedDigitSystemFont(ofSize: 17, weight: .medium)
        return s
    }
}

// MARK: - A payment

final class PaymentViewController: UITableViewController {
    private let rows: [(String, String, Bool)]
    private let explorer: URL?
    init(payment p: SparkHost.Activity, rate: Double?) {
        var rows: [(String, String, Bool)] = [
            ("Amount", (p.incoming ? "+" : "−") + Money.text(p.amount) + (Money.dollars(p.amount, rate).map { "  " + $0 } ?? ""), false),
            ("Status", p.status == .pending ? "Pending" : p.status == .failed ? "Failed. No money left your wallet." : (p.incoming ? "Received" : "Sent"), false),
            ("Date", DateFormatter.localizedString(from: p.date, dateStyle: .medium, timeStyle: .short), false),
            ("Network", p.rail, false),
        ]
        if !p.incoming || p.fee > 0 { rows.insert(("Fee", Money.text(p.fee), false), at: 1) }
        if let note = p.note, !note.isEmpty { rows.append(("Note", note, false)) }
        if let r = p.reference { rows.append((p.txid != nil ? "Transaction" : "Request", r, true)) }
        self.rows = rows
        explorer = p.txid.flatMap { URL(string: "https://mempool.space/tx/\($0)") }
        super.init(style: .insetGrouped)
    }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Payment"; Look.sheet(self)
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "row")
    }
    override func numberOfSections(in tableView: UITableView) -> Int { explorer == nil ? 1 : 2 }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { section == 0 ? rows.count : 1 }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "row", for: indexPath)
        var c = UIListContentConfiguration.subtitleCell()
        c.textProperties.color = Look.dim; c.textProperties.font = .preferredFont(forTextStyle: .footnote)
        c.secondaryTextProperties.color = Look.ink; c.secondaryTextProperties.font = .preferredFont(forTextStyle: .body)
        if indexPath.section == 0 {
            let (name, value, copyable) = rows[indexPath.row]
            c.text = name; c.secondaryText = value
            if copyable { c.secondaryTextProperties.font = .monospacedSystemFont(ofSize: 13, weight: .regular) }
            cell.selectionStyle = copyable ? .default : .none
            cell.accessoryType = .none
        } else {
            c.text = nil; c.secondaryText = "View on mempool.space"
            cell.accessoryType = .disclosureIndicator
        }
        cell.contentConfiguration = c
        cell.backgroundColor = UIColor(white: 1, alpha: 0.06)
        return cell
    }
    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if indexPath.section == 1, let explorer { UIApplication.shared.open(explorer); return }
        let (name, value, copyable) = rows[indexPath.row]
        guard copyable else { return }
        UIPasteboard.general.string = value
        UIAccessibility.post(notification: .announcement, argument: "\(name) copied")
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }
}

// MARK: - Sending

/// Paste or scan a request, pick an amount, review, send.
final class SendViewController: WalletForm {
    private let host: SparkHost
    private let input = Look.field("Invoice, address or Lightning address", "send.input")
    private let amount = Look.field("Amount in sats", "send.amount")
    private let comment = Look.field("Note to the recipient (optional)", "send.comment")
    private let about = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    private let onward = Look.button("Continue", "send.continue", primary: true)
    private var destination: SparkHost.Destination?
    private var parsedText = ""

    init(host: SparkHost) { self.host = host; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Send"
        input.autocapitalizationType = .none; input.autocorrectionType = .no; input.returnKeyType = .next; input.clearButtonMode = .always
        input.addAction(UIAction { [weak self] _ in self?.destination = nil; self?.layout() }, for: .editingChanged)
        input.addAction(UIAction { [weak self] _ in self?.parse() }, for: .editingDidEndOnExit)
        amount.keyboardType = .numberPad
        amount.addAction(UIAction { [weak self] _ in self?.typed() }, for: .editingChanged)
        let paste = Look.button("Paste", "send.paste")
        paste.addAction(UIAction { [weak self] _ in
            guard let self, let text = UIPasteboard.general.string else { return }
            self.input.text = text; self.parse()
        }, for: .touchUpInside)
        let scan = Look.button("Scan", "send.scan")
        scan.addAction(UIAction { [weak self] _ in
            let scanner = ScannerViewController { code in self?.input.text = code; self?.parse() }
            self?.present(scanner, animated: true)
        }, for: .touchUpInside)
        let tools = UIStackView(arrangedSubviews: [paste, scan]); tools.spacing = 12; tools.distribution = .fillEqually
        onward.addAction(UIAction { [weak self] _ in self?.proceed() }, for: .touchUpInside)
        about.accessibilityIdentifier = "send.about"
        [input, tools, about, amount, comment, onward].forEach(column.addArrangedSubview)
        column.setCustomSpacing(24, after: comment)
        layout()
    }

    private func layout() {
        var needsAmount = false, commentLength = 0
        switch destination {
        case .invoice(let a, _, _)?: needsAmount = a == nil
        case .lnurl(_, _, _, let c)?: needsAmount = true; commentLength = c
        case .bitcoin(_, let a)?, .spark(_, let a)?: needsAmount = a == nil
        case .withdraw?: needsAmount = true
        default: break
        }
        amount.isHidden = !needsAmount
        comment.isHidden = commentLength == 0
        onward.isHidden = destination == nil
        if case .withdraw? = destination { onward.configuration?.title = "Withdraw" } else { onward.configuration?.title = "Continue" }
        if destination == nil, input.text?.isEmpty ?? true { about.text = "Paste or scan a Lightning invoice, Lightning address, bitcoin address or Spark address." }
        typed()
    }

    private func typed() {
        // The keypad goes through verse_wallet_digit; anything else is cut.
        if let text = amount.text, !text.isEmpty, Money.typed(text) == nil { amount.text = String(text.dropLast()) }
    }

    private func parse() {
        let text = input.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !text.isEmpty else { return }
        input.resignFirstResponder()
        busy(onward) { [self] in
            do {
                let d = try await host.parse(text)
                parsedText = text
                destination = d
                switch d {
                case .invoice(let a, let note, let expires):
                    about.text = ["Lightning invoice", a.map { "for " + Money.text($0) }, note.map { "“\($0)”" }].compactMap { $0 }.joined(separator: " ")
                    if expires < Date() { about.text = "This invoice has expired. Ask for a new one."; destination = nil }
                case .lnurl(let name, let low, let high, _):
                    about.text = "Pay \(name), between \(Money.text(low)) and \(Money.text(high))."
                case .bitcoin(let address, let a):
                    about.text = "Bitcoin address \(address)" + (a.map { " for " + Money.text($0) } ?? "") + ". On-chain payments take ten minutes or more and cost a network fee."
                case .spark(let address, let a):
                    about.text = "Spark address \(address.prefix(16))…" + (a.map { " for " + Money.text($0) } ?? "")
                case .withdraw(let note, let low, let high):
                    about.text = "Withdraw into this wallet: \(note). Between \(Money.text(low)) and \(Money.text(high))."
                case .unsupported(let why):
                    about.text = why; destination = nil
                }
            } catch {
                NSLog("OpenAgents wallet: parse (%ld characters): %@", text.count, String(describing: error))
                destination = nil
                about.text = "That isn't a payment request OpenAgents recognises. Check it and try again."
            }
            layout()
        }
    }

    private func proceed() {
        guard let destination else { return parse() }
        let chosen = amount.isHidden ? nil : Money.typed(amount.text ?? "")
        if !amount.isHidden, (chosen ?? 0) == 0 { return fail("Enter an amount in sats.") }
        if case .withdraw(_, let low, let high) = destination {
            guard verse_wallet_amount_ok(Int64(chosen!), Int64(low), Int64(high), 2_100_000_000_000_000) else {
                return fail("Choose an amount between \(Money.text(low)) and \(Money.text(high)).")
            }
            return busy(onward) { [self] in
                do { try await host.withdraw(amount: chosen!); navigationController?.pushViewController(ResultViewController(.received(chosen!)), animated: true) }
                catch { fail(error) }
            }
        }
        if case .lnurl(_, let low, let high, _) = destination,
           !verse_wallet_amount_ok(Int64(chosen!), Int64(low), Int64(high), 2_100_000_000_000_000) {
            return fail("Choose an amount between \(Money.text(low)) and \(Money.text(high)).")
        }
        let note = comment.isHidden ? nil : comment.text
        busy(onward) { [self] in
            do {
                let quote = try await host.quote(text: parsedText, amount: chosen, comment: note)
                let spendable = try await host.spendable()
                navigationController?.pushViewController(ReviewViewController(host: host, quote: quote, spendable: spendable, comment: note), animated: true)
            } catch {
                fail(error)
            }
        }
    }
}

/// The last step before money leaves: recipient, amount, fee and total,
/// with anything unusual called out. verse/wallet.coax decides whether
/// the confirmed quote may be paid.
final class ReviewViewController: WalletForm {
    private let host: SparkHost
    private var quote: SparkHost.Quote
    private let spendable: UInt64
    private var flow = verse_wallet_step(verse_wallet_start(), PayEvent.quote)
    private let key = UUID().uuidString.lowercased()
    private var speed = 1
    private let fee = Look.label()
    private let total = Look.label("", Look.title(22))
    private let totalDollars = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    private let warning = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.warn)
    private let send = Look.button("Send", "send.confirm", primary: true)
    private var rate: Double?

    init(host: SparkHost, quote: SparkHost.Quote, spendable: UInt64, comment: String?) {
        self.host = host; self.quote = quote; self.spendable = spendable
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Review"
        func row(_ name: String, _ value: UIView) -> UIView {
            let n = Look.label(name, .preferredFont(forTextStyle: .footnote), Look.dim)
            return Look.stack([n, value], spacing: 4)
        }
        let to = Look.label(quote.recipient, .monospacedSystemFont(ofSize: 15, weight: .regular))
        to.accessibilityIdentifier = "send.recipient"
        let amount = Look.label(Money.text(quote.amount))
        column.addArrangedSubview(row("To", to))
        column.addArrangedSubview(row("Amount", amount))
        if quote.speeds.count == 3 {
            // On-chain: a speed, each with its own fee.
            let speeds = UISegmentedControl(items: ["Fast", "Normal", "Slow"])
            speeds.selectedSegmentIndex = speed
            speeds.accessibilityIdentifier = "send.speed"
            speeds.addAction(UIAction { [weak self, weak speeds] _ in
                guard let self, let speeds else { return }
                self.speed = speeds.selectedSegmentIndex; self.quote.fee = self.quote.speeds[self.speed]; self.render()
            }, for: .valueChanged)
            column.addArrangedSubview(row("Speed", speeds))
        }
        column.addArrangedSubview(row("Network fee", fee))
        column.addArrangedSubview(row("Total", Look.stack([total, totalDollars], spacing: 2)))
        column.addArrangedSubview(warning)
        send.addAction(UIAction { [weak self] _ in self?.confirm() }, for: .touchUpInside)
        column.setCustomSpacing(28, after: warning)
        column.addArrangedSubview(send)
        render()
        Task { @MainActor in rate = await host.dollarsPerBitcoin(); render() }
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if isMovingFromParent { flow = verse_wallet_step(flow, PayEvent.cancel) }
    }

    private func render() {
        fee.text = quote.fee == 0 ? "No fee" : Money.text(quote.fee)
        let sum = quote.amount + quote.fee
        total.text = Money.text(sum)
        totalDollars.text = Money.dollars(sum, rate)
        total.accessibilityIdentifier = "send.total"
        var notes: [String] = []
        if verse_wallet_fee_high(Int64(quote.amount), Int64(quote.fee)) { notes.append("The fee is more than a tenth of the amount.") }
        if let expires = quote.expires, verse_wallet_expiring(Int64(Date().timeIntervalSince1970), Int64(expires.timeIntervalSince1970)) {
            notes.append("This request expires in under a minute.")
        }
        if quote.speeds.count == 3 { notes.append(["Fast: about 10 minutes.", "Normal: about 30 minutes.", "Slow: an hour or more."][speed]) }
        if sum > spendable { notes.append("Your wallet has \(Money.text(spendable)); this needs \(Money.text(sum)).") }
        warning.text = notes.joined(separator: "\n")
    }

    private func confirm() {
        let now = Int64(Date().timeIntervalSince1970)
        let expires = quote.expires.map { Int64($0.timeIntervalSince1970) } ?? Int64.max
        let verdict = verse_wallet_may_send(flow.state, Int64(quote.amount), Int64(quote.fee), Int64(quote.fee), Int64(spendable), now, expires)
        switch verdict {
        case Verdict.send: break
        case Verdict.expired:
            flow = verse_wallet_step(flow, PayEvent.expire)
            return fail("This quote has expired. Go back and start again.")
        case Verdict.overLimit: return fail("There isn't enough in your wallet for this payment and its fee.")
        case Verdict.badAmount: return fail("This amount can't be sent.")
        default: return
        }
        busy(send) { [self] in
            guard await OwnerCheck.confirm("Send \(Money.text(quote.amount))") else { return }
            flow = verse_wallet_step(flow, PayEvent.confirm)
            guard flow.state == Pay.sending else { return }
            let outcome = await host.send(key: key, speed: speed)
            switch outcome {
            case .sent: flow = verse_wallet_step(flow, PayEvent.sent)
            case .notSent: flow = verse_wallet_step(flow, PayEvent.failed)
            case .waiting: flow = verse_wallet_step(flow, PayEvent.timedOut)
            }
            let result: ResultViewController.Result
            switch outcome {
            case .sent(let a): result = .sent(a.amount, a.fee)
            case .notSent(let why): result = .notSent(why)
            case .waiting(let id): result = .waiting(id, quote.speeds.count == 3)
            }
            navigationController?.pushViewController(ResultViewController(result, host: host), animated: true)
        }
    }
}

/// A send's or withdrawal's end: what happened and whether money moved.
final class ResultViewController: WalletForm {
    enum Result { case sent(UInt64, UInt64), notSent(String), waiting(String?, Bool), received(UInt64) }
    private var result: Result
    private let host: SparkHost?
    private let headline = Look.label("", Look.title(22))
    private let detail = Look.label("", .preferredFont(forTextStyle: .body), Look.dim)
    private var watching: Task<Void, Never>?

    init(_ result: Result, host: SparkHost? = nil) { self.result = result; self.host = host; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    deinit { watching?.cancel() }

    override func viewDidLoad() {
        super.viewDidLoad()
        navigationItem.hidesBackButton = true
        headline.accessibilityIdentifier = "send.result"
        let done = Look.button("Done", "send.done", primary: true)
        done.addAction(UIAction { [weak self] _ in self?.navigationController?.popToRootViewController(animated: true) }, for: .touchUpInside)
        [headline, detail, done].forEach(column.addArrangedSubview)
        column.setCustomSpacing(28, after: detail)
        render()
        if case .waiting(let id?, _) = result, let host {
            // An unknown outcome waits for the SDK to say; it is never shown
            // as a failure on a guess.
            watching = Task { [weak self] in
                for await change in await host.changes() {
                    switch change {
                    case .settled(let a) where a.id == id: self?.result = .sent(a.amount, a.fee)
                    case .failed(let a) where a.id == id: self?.result = .notSent("The payment failed. No money left your wallet.")
                    default: continue
                    }
                    self?.render()
                }
            }
        }
    }

    private func render() {
        switch result {
        case .sent(let amount, let fee):
            headline.text = "Sent " + Money.text(amount)
            detail.text = fee == 0 ? "No fee." : "Fee " + Money.text(fee) + "."
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        case .received(let amount):
            headline.text = "Withdrawing " + Money.text(amount)
            detail.text = "It shows in your activity when it arrives."
        case .notSent(let why):
            headline.text = "Not sent"
            detail.text = why + "\nYour money is safe in your wallet."
        case .waiting(_, let onchain):
            headline.text = "On its way"
            detail.text = onchain
                ? "The transaction is with the bitcoin network. It confirms in the next blocks; you can close this."
                : "The payment hasn't settled yet. Your activity shows when it does; don't send it again."
        }
        UIAccessibility.post(notification: .screenChanged, argument: headline)
    }
}

// MARK: - Receiving

/// A request to be paid: one QR for Lightning and on-chain together by
/// default (BIP21), or one of them alone, with an optional amount and note.
final class ReceiveViewController: WalletForm {
    private let host: SparkHost
    private let kinds = UISegmentedControl(items: ["Any", "Lightning", "On-chain", "Spark"])
    private let amount = Look.field("Amount in sats (optional)", "receive.amount")
    private let note = Look.field("Note (optional)", "receive.note")
    private let create = Look.button("Create request", "receive.create", primary: true)
    private let code = UIImageView()
    private let text = Look.label("", .monospacedSystemFont(ofSize: 12, weight: .regular), Look.dim)
    private let fee = Look.label("", .preferredFont(forTextStyle: .footnote), Look.dim)
    private let status = Look.label("", .preferredFont(forTextStyle: .headline))
    private let actions = UIStackView()
    private var request: SparkHost.Request?
    private var watching: Task<Void, Never>?

    init(host: SparkHost) { self.host = host; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    deinit { watching?.cancel() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Receive"
        kinds.selectedSegmentIndex = 0
        kinds.accessibilityIdentifier = "receive.kind"
        kinds.addAction(UIAction { [weak self] _ in self?.changed() }, for: .valueChanged)
        amount.keyboardType = .numberPad
        amount.addAction(UIAction { [weak self] _ in
            guard let self, let t = self.amount.text, !t.isEmpty, Money.typed(t) == nil else { return }
            self.amount.text = String(t.dropLast())
        }, for: .editingChanged)
        create.addAction(UIAction { [weak self] _ in self?.make() }, for: .touchUpInside)
        code.contentMode = .scaleAspectFit
        code.backgroundColor = .white; code.layer.cornerRadius = 12; code.clipsToBounds = true
        code.heightAnchor.constraint(equalTo: code.widthAnchor).isActive = true
        code.isAccessibilityElement = true; code.accessibilityLabel = "Payment request QR code"
        code.accessibilityIdentifier = "receive.qr"
        status.accessibilityIdentifier = "receive.status"
        text.accessibilityIdentifier = "receive.text"
        let copy = Look.button("Copy", "receive.copy"), share = Look.button("Share", "receive.share")
        copy.addAction(UIAction { [weak self] _ in
            guard let t = self?.request?.text else { return }
            UIPasteboard.general.string = t
            UIAccessibility.post(notification: .announcement, argument: "Copied")
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        }, for: .touchUpInside)
        share.addAction(UIAction { [weak self] _ in
            guard let self, let t = self.request?.text else { return }
            let sheet = UIActivityViewController(activityItems: [t], applicationActivities: nil)
            sheet.popoverPresentationController?.sourceView = share
            self.present(sheet, animated: true)
        }, for: .touchUpInside)
        [copy, share].forEach(actions.addArrangedSubview); actions.spacing = 12; actions.distribution = .fillEqually
        [kinds, amount, note, create, status, code, text, fee, actions].forEach(column.addArrangedSubview)
        changed()
        watching = Task { [weak self, host] in
            for await change in await host.changes() {
                guard let self, case .received(let a) = change, self.request != nil else { continue }
                self.status.text = "Received " + Money.text(a.amount)
                self.status.textColor = Look.good
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                UIAccessibility.post(notification: .announcement, argument: self.status.text)
            }
        }
    }

    private func changed() {
        request = nil
        let invoice = kinds.selectedSegmentIndex <= 1
        amount.isHidden = !invoice; note.isHidden = !invoice; create.isHidden = !invoice
        [code, text, fee, actions].forEach { $0.isHidden = true }
        status.text = nil
        // An address needs no amount: show it at once.
        if !invoice { make() }
    }

    private func make() {
        view.endEditing(true)
        let sats = Money.typed(amount.text ?? "").flatMap { $0 > 0 ? $0 : nil }
        let n = note.text ?? ""
        let kind: SparkHost.Receive
        switch kinds.selectedSegmentIndex {
        case 0: kind = .unified(amount: sats, note: n)
        case 1: kind = .lightning(amount: sats, note: n)
        case 2: kind = .bitcoin
        default: kind = .spark
        }
        let index = kinds.selectedSegmentIndex
        busy(create) { [self] in
            do {
                let r = try await host.receive(kind)
                guard index == kinds.selectedSegmentIndex else { return }
                request = r
                code.image = Self.qr(r.qr)
                text.text = r.text
                fee.text = r.fee == 0 ? "No fee to receive." : "Receiving costs " + Money.text(r.fee) + "."
                if index == 2 { fee.text = (fee.text ?? "") + " On-chain payments arrive after confirmations, less the network fee to claim them." }
                status.text = "Waiting for payment…"; status.textColor = Look.ink
                [code, text, fee, actions].forEach { $0.isHidden = false }
            } catch {
                fail(error)
            }
        }
    }

    /// A high-contrast QR, scaled without smoothing.
    static func qr(_ s: String) -> UIImage? {
        let f = CIFilter.qrCodeGenerator()
        f.message = Data(s.utf8); f.correctionLevel = "M"
        guard let out = f.outputImage else { return nil }
        let scaled = out.transformed(by: CGAffineTransform(scaleX: 12, y: 12))
        guard let cg = CIContext().createCGImage(scaled, from: scaled.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}

// MARK: - Scanning

/// The camera, reading one QR code.
final class ScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let found: (String) -> Void
    private let session = AVCaptureSession()
    private var done = false
    init(found: @escaping (String) -> Void) { self.found = found; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        let close = Look.button("Cancel", "scan.cancel")
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        close.translatesAutoresizingMaskIntoConstraints = false
        let hint = Look.label("Point the camera at a payment QR code.", .preferredFont(forTextStyle: .headline))
        hint.textAlignment = .center; hint.translatesAutoresizingMaskIntoConstraints = false
        guard let camera = AVCaptureDevice.default(for: .video), let feed = try? AVCaptureDeviceInput(device: camera), session.canAddInput(feed) else {
            hint.text = "The camera isn't available. Allow camera access in Settings, or paste the request instead."
            view.addSubview(hint); view.addSubview(close); place(hint, close)
            return
        }
        session.addInput(feed)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill; preview.frame = view.bounds
        view.layer.addSublayer(preview)
        view.addSubview(hint); view.addSubview(close); place(hint, close)
        DispatchQueue.global(qos: .userInitiated).async { [session] in session.startRunning() }
    }
    private func place(_ hint: UIView, _ close: UIView) {
        NSLayoutConstraint.activate([
            hint.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
            hint.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24), hint.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            close.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -24),
            close.centerXAnchor.constraint(equalTo: view.centerXAnchor),
        ])
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        let session = session
        DispatchQueue.global(qos: .userInitiated).async { session.stopRunning() }
    }
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard !done, let code = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
        done = true
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        dismiss(animated: true) { [found] in found(code) }
    }
}
