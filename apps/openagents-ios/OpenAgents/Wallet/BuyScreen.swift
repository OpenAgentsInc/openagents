import UIKit

/// Buy bitcoin with dollars through MoonPay or Cash App, Breez's two
/// providers. The SDK makes the provider's page for this wallet;
/// verse/wallet.coax checks the amount, passes the page and moves the
/// purchase (verse_wallet_buy_step), so one answer opens one page. After
/// the page opens, the screen shows what the SDK reports coming in.
final class BuyViewController: WalletForm {
    private let host: SparkHost
    private let amount = Look.field("Amount in sats", "buy.amount")
    private let worth = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    private let moonpay = Look.button("MoonPay", "buy.moonpay", primary: true)
    private let moonpayTerms = Look.label("", .preferredFont(forTextStyle: .footnote), Look.dim)
    private let cashApp = Look.button("Cash App", "buy.cashapp", primary: true)
    private let cashAppTerms = Look.label("Pay from your Cash App balance or debit card. The bitcoin comes over Lightning and is in your balance as soon as Cash App pays.",
                                          .preferredFont(forTextStyle: .footnote), Look.dim)
    private let status = Look.label("", Look.title(20))
    private let detail = Look.label("", .preferredFont(forTextStyle: .body), Look.dim)
    private var flow = BuyState.idle
    private var provider = SparkHost.Provider.moonpay
    private var asked: UInt64 = 0
    private var rate: Double?
    private var claimMax: UInt64?
    /// Deposits already waiting when the page opened, so only a new one counts.
    private var before: Set<String> = []
    private var watching: Task<Void, Never>?
    #if DEBUG
    /// UI tests only (OPENAGENTS_BUY_KEEP_PAGE): pages kept instead of opened.
    private var kept = 0
    #endif

    init(host: SparkHost) { self.host = host; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    deinit { watching?.cancel() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Buy bitcoin"
        amount.keyboardType = .numberPad
        amount.addAction(UIAction { [weak self] _ in self?.typed() }, for: .editingChanged)
        worth.accessibilityIdentifier = "buy.worth"
        moonpayTerms.accessibilityIdentifier = "buy.moonpay.terms"
        status.accessibilityIdentifier = "buy.status"
        detail.accessibilityIdentifier = "buy.detail"
        moonpay.addAction(UIAction { [weak self] _ in self?.choose(.moonpay) }, for: .touchUpInside)
        cashApp.addAction(UIAction { [weak self] _ in self?.choose(.cashApp) }, for: .touchUpInside)
        [amount, worth, moonpay, moonpayTerms, cashApp, cashAppTerms, status, detail].forEach(column.addArrangedSubview)
        column.setCustomSpacing(24, after: worth)
        column.setCustomSpacing(24, after: moonpayTerms)
        column.setCustomSpacing(28, after: cashAppTerms)
        terms()
        Task { @MainActor in
            async let r = host.dollarsPerBitcoin()
            async let c = host.claimFeeMax()
            rate = await r; claimMax = await c
            terms(); typed()
        }
        watching = Task { [weak self, host] in
            for await change in await host.changes() {
                guard let self else { return }
                switch change {
                case .received(let a): self.arrived(a)
                case .updated: await self.look()
                default: break
                }
            }
        }
        NotificationCenter.default.addObserver(self, selector: #selector(back), name: UIApplication.didBecomeActiveNotification, object: nil)
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if isMovingFromParent { flow = verse_wallet_buy_step(flow, BuyEvent.done) }
    }

    @objc private func back() { Task { @MainActor in await look() } }

    private func terms() {
        var text = "Pay by card or Apple Pay. MoonPay shows its price, fee and limits before you pay. The bitcoin comes on-chain and is in your balance after 3 confirmations."
        if let claimMax { text += " Adding it costs at most \(Money.text(claimMax)) at today's network fees." }
        moonpayTerms.text = text
    }

    private func typed() {
        if let text = amount.text, !text.isEmpty, Money.typed(text) == nil { amount.text = String(text.dropLast()) }
        let sats = Money.typed(amount.text ?? "") ?? 0
        worth.text = sats == 0 ? nil : Money.dollars(sats, rate)
    }

    private func choose(_ p: SparkHost.Provider) {
        view.endEditing(true)
        let sats = Money.typed(amount.text ?? "") ?? 0
        switch verse_wallet_buy_amount(p.rawValue, Int64(clamping: sats), claimMax.map { Int64(clamping: $0) } ?? -1) {
        case BuyCheck.ok: break
        case BuyCheck.noAmount: return fail("Enter how many sats to buy.")
        case BuyCheck.noFee: return fail("We couldn't get today's network fees, so a MoonPay purchase can't start yet. Try again in a moment.")
        case BuyCheck.tooSmall: return fail("Buy more than \(Money.text(claimMax ?? 0)). Adding an on-chain purchase can cost that much at today's network fees.")
        default: return fail("That amount can't be bought.")
        }
        // A tap while a page is being asked for, on either button, starts nothing.
        let starts = verse_wallet_buy_starts(flow, BuyEvent.ask)
        flow = verse_wallet_buy_step(flow, BuyEvent.ask)
        guard starts else { return }
        provider = p; asked = sats
        busy(p == .moonpay ? moonpay : cashApp) { [self] in
            before = Set(((try? await host.deposits()) ?? []).map { "\($0.txid):\($0.vout)" })
            do {
                let page = try await host.buy(p, amount: sats)
                #if DEBUG
                // UI tests only: hold the answer so a second tap lands while the page is being asked for.
                if ProcessInfo.processInfo.environment["OPENAGENTS_BUY_KEEP_PAGE"] == "1" { try? await Task.sleep(for: .seconds(2)) }
                #endif
                let opens = verse_wallet_buy_opens(flow, BuyEvent.page)
                flow = verse_wallet_buy_step(flow, BuyEvent.page)
                guard opens else { return }
                show(p == .moonpay ? "Waiting for MoonPay" : "Waiting for Cash App",
                     p == .moonpay ? "Finish paying on MoonPay's page. The bitcoin shows here once MoonPay sends it." : "Finish paying in Cash App. The bitcoin shows here once Cash App pays.")
                openPage(page)
            } catch {
                flow = verse_wallet_buy_step(flow, BuyEvent.refused)
                show("Not started", error.localizedDescription + " No money was taken.")
            }
        }
    }

    private func openPage(_ page: URL) {
        #if DEBUG
        // UI tests only: keep the page here instead of leaving the app.
        if ProcessInfo.processInfo.environment["OPENAGENTS_BUY_KEEP_PAGE"] == "1" {
            status.accessibilityValue = page.absoluteString
            kept += 1; detail.accessibilityValue = "\(kept) pages"
            return
        }
        #endif
        UIApplication.shared.open(page)
    }

    /// A new on-chain deposit, after a MoonPay page opened.
    private func look() async {
        guard provider == .moonpay, flow == BuyState.open || flow == BuyState.arriving else { return }
        guard let d = ((try? await host.deposits()) ?? []).first(where: { !before.contains("\($0.txid):\($0.vout)") }) else { return }
        flow = verse_wallet_buy_step(flow, BuyEvent.seen)
        show(Money.text(d.amount) + " on its way",
             d.problem ?? (d.mature ? "It has 3 confirmations and is being added to your balance." : "MoonPay sent it. It's in your balance after 3 confirmations, usually about 30 minutes."))
    }

    private func arrived(_ a: SparkHost.Activity) {
        let ours = provider == .moonpay ? a.rail == "On-chain" : a.rail == "Lightning" && a.amount == asked
        guard ours else { return }
        flow = verse_wallet_buy_step(flow, BuyEvent.paid)
        guard flow == BuyState.received else { return }
        show("Received " + Money.text(a.amount), "It's in your balance.")
        status.textColor = Look.good
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    private func show(_ headline: String, _ words: String) {
        status.text = headline; status.textColor = Look.ink
        detail.text = words
        UIAccessibility.post(notification: .announcement, argument: headline)
    }
}
