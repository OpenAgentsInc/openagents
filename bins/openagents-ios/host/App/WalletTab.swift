// The Wallet tab: bitcoin on mainnet through Breez's Spark SDK. Rust runs
// the wallet and decides every state and line of text; this view lays them
// out, collects typed values, and adds the clipboard, the camera, and the
// browser. The seed lives in Keychain (DeviceKey); the recovery words reach
// this view only in a direct reply, and only while their sheet is open.
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Rust's Wallet screen (`wallet::Screen`): `failed`, or `ready` with the
/// summary. `ready` shows from launch: while the wallet starts it carries
/// the last balance read (or `balance_unknown`) and a `status`.
/// Rust's amount format (`amounts::AmountsView`): BIP 177 integer base units
/// (`₿12,345`) or legacy BTC (`0.00012345 BTC`), saved and applied app-wide.
struct AmountsState: Decodable, Equatable {
    struct Choice: Decodable, Equatable, Identifiable {
        let id: String
        let label: String
        let selected: Bool
    }
    let format: String
    let unit: String
    let decimal: Bool
    let choices: [Choice]

    static let standard = AmountsState(
        format: "bip177", unit: "₿", decimal: false,
        choices: [Choice(id: "bip177", label: "₿ bitcoin (BIP 177)", selected: true),
                  Choice(id: "btc", label: "BTC (legacy)", selected: false)])
}

struct WalletState: Decodable, Equatable {
    struct Trust: Decodable, Equatable {
        let acknowledged: Bool
        let title: String
        /// The note in one plain paragraph, shown first.
        let summary: String?
        let lines: [String]
    }
    struct Code: Decodable, Equatable {
        let text: String
        let uri: String
        let qr: ComputersQR?
        let caption: String
    }
    struct Receive: Decodable, Equatable {
        let lightning: Code?
        let lightning_busy: Bool
        let lightning_error: String?
        let spark: Code?
        let bitcoin: Code?
        let nostr: Code?
        let publish: Publish?
    }
    struct Publish: Decodable, Equatable {
        let on: Bool
        let busy: Bool
        let detail: String
        let message: String?
    }
    struct Person: Decodable, Equatable, Identifiable {
        let name: String
        let detail: String
        let input: String
        var id: String { input }
    }
    struct Quote: Decodable, Equatable {
        let id: UInt64
        let kind: String
        /// What is paid, in plain words: "Payment request", "Address".
        let to: String?
        let destination: String
        let amount: String
        let fee: String
        let total: String
        let note: String?
        let comment: String?
        let speeds: [Speed]?
    }
    struct Speed: Decodable, Equatable, Identifiable {
        let id: String
        let label: String
        let fee: String
        let chosen: Bool
    }
    struct Refund: Decodable, Equatable {
        let txid: String
        let vout: UInt32
        let busy: Bool
        let speeds: [Speed]
        let review: String?
        let message: String?
    }
    struct Backup: Decodable, Equatable {
        let title: String
        let detail: String
        let saved_at: UInt64?
        let can_export: Bool
        let error: String?
    }
    struct Payment: Decodable, Equatable, Identifiable {
        let id: String
        let title: String
        let amount: String
        let fee: String?
        let method: String
        let status: String
        let at: UInt64
    }
    struct Send: Decodable, Equatable {
        let state: String
        let message: String?
        let quote: Quote?
        let result: Payment?
        /// While an amount is needed: who is paid, their description, and
        /// the longest comment they take (no comment field when absent).
        let recipient: String?
        let description: String?
        let comment_max: UInt16?
        /// After a payment: what the recipient said.
        let recipient_message: String?
        /// The person an npub resolved to, and where their address came from.
        let person: String?
        let person_source: String?
        /// A Lightning address just paid that could be saved as a contact.
        let save_suggestion: String?
    }
    struct Provider: Decodable, Equatable, Identifiable {
        let id: String
        let label: String
        let detail: String
    }
    struct Buy: Decodable, Equatable {
        let busy: Bool
        let error: String?
        let providers: [Provider]
    }
    struct Deposit: Decodable, Equatable, Identifiable {
        let txid: String
        let vout: UInt32
        let amount: String
        let status: String
        let actionable: Bool?
        var id: String { "\(txid):\(vout)" }
    }
    /// Shown until the person writes down this wallet's recovery words.
    struct BackupCard: Decodable, Equatable {
        let title: String
        let detail: String
        let action: String
    }
    /// Everything but Receive, Send, and recent activity.
    struct Advanced: Decodable, Equatable {
        let open: Bool
        let note: String?
    }
    struct Claim: Decodable, Equatable {
        let txid: String
        let vout: UInt32
        let busy: Bool
        let quote: String?
        let message: String?
    }

    let state: String
    let message: String?
    let network: String?
    let balance: String?
    /// The balance in the other format, shown under Advanced.
    let balance_alternate: String?
    /// "12,345 bitcoin", for VoiceOver.
    let balance_spoken: String?
    let empty: Bool?
    let synced_at: UInt64?
    let refreshing: Bool?
    let error: String?
    let balance_unknown: Bool?
    let status: String?
    let warning: String?
    let trust: Trust?
    let receive: Receive?
    let send: Send?
    let payments: [Payment]?
    let can_show_words: Bool?
    let buy: Buy?
    let deposits: [Deposit]?
    let claim: Claim?
    let refund: Refund?
    let backup: Backup?
    let people: [Person]?
    /// The balance is old: the main screen says when it was read.
    let stale: Bool?
    /// The newest few payments, for Recent activity.
    let recent: [Payment]?
    let more_payments: Bool?
    let backup_card: BackupCard?
    let advanced: Advanced?
}


struct WalletTab: View {
    /// What the main screen shows under the two buttons.
    enum Mode: String { case home, receive, send }
    /// The other ways to receive, under Advanced.
    enum Method: String, CaseIterable { case lightning = "Lightning", spark = "Spark", bitcoin = "Bitcoin", nostr = "Nostr" }

    @ObservedObject var bridge: MobileBridge
    @State private var mode = Mode.home
    @State private var method = Method.spark
    @State private var invoiceAmount = ""
    @State private var payInput = ""
    @State private var payAmount = ""
    @State private var payComment = ""
    @State private var contactName = ""
    @State private var buyAmount = ""
    @State private var scanning = false
    @State private var copied: String?
    @State private var confirmWords = false
    @State private var words: [String]?
    @State private var restoring = false
    @State private var showTrust = false
    @State private var showHistory = false
    @State private var refundAddress = ""
    @State private var refundSpeed = "medium"
    @State private var exportFile: ExitFile?
    @State private var exportError: String?
    /// Simulator checks open Advanced without saving the choice.
    @State private var forceAdvanced = false

    private var wallet: WalletState? { bridge.packet?.wallet }
    private var amounts: AmountsState { bridge.packet?.amounts ?? .standard }
    private var amountKeyboard: UIKeyboardType { amounts.decimal ? .decimalPad : .numberPad }
    private var loading: Bool { bridge.packet?.wallet_loading == true }
    private var advancedOpen: Bool { forceAdvanced || wallet?.advanced?.open == true }

    var body: some View {
        ScrollViewReader { scroller in
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(alignment: .firstTextBaseline) {
                    Text("Wallet").font(.paper(.largeTitle, weight: .bold)).foregroundStyle(.white)
                    Spacer()
                    Button { showTrust = true } label: {
                        Image(systemName: "info.circle").font(.paper(.title2)).foregroundStyle(.white)
                    }
                    .accessibilityLabel("About this wallet")
                    .accessibilityIdentifier("wallet-info")
                    .disabled(wallet?.trust == nil)
                }
                content
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
        }
        .task {
            // Simulator checks: open a refund review, or the backup.
            let refund = AppTabLaunch.wallet("--wallet-refund")
            let backup = AppTabLaunch.wallet("--wallet-backup")
            guard refund != nil || backup != nil else { return }
            forceAdvanced = true
            while !Task.isCancelled, wallet?.status != nil || wallet == nil {
                try? await Task.sleep(for: .seconds(1))
            }
            if backup != nil { scroller.scrollTo("wallet-backup", anchor: .bottom); return }
            guard let refund, let deposit = wallet?.deposits?.first else { return }
            refundAddress = refund
            bridge.wallet("wallet_refund_start", ["txid": deposit.txid, "vout": deposit.vout])
            try? await Task.sleep(for: .seconds(2))
            if AppTabLaunch.wallet("--wallet-refund-review") != nil {
                bridge.wallet("wallet_refund_review", ["txid": deposit.txid, "vout": deposit.vout,
                                                       "address": refund, "speed": refundSpeed])
                try? await Task.sleep(for: .seconds(2))
            }
            scroller.scrollTo("wallet-deposits", anchor: .top)
        }
        .task {
            // Simulator checks: open Advanced and show its end.
            guard AppTabLaunch.wallet("--wallet-advanced") != nil else { return }
            forceAdvanced = true
            try? await Task.sleep(for: .seconds(1))
            scroller.scrollTo("wallet-advanced", anchor: .top)
        }
        }
        .scrollDismissesKeyboard(.interactively)
        .dismissesKeyboard()
        .background(Color.black.ignoresSafeArea())
        // Pull down to read the wallet again.
        .refreshable { bridge.refreshWallet() }
        .onAppear {
            bridge.openWallet()
            switch AppTabLaunch.wallet("--wallet-section") {
            case "receive": mode = .receive
            case "send": mode = .send
            case "buy": forceAdvanced = true
            default: break
            }
            if let value = AppTabLaunch.wallet("--wallet-method"), let chosen = Method(rawValue: value.capitalized) {
                method = chosen; forceAdvanced = true
            }
            if AppTabLaunch.wallet("--wallet-info") != nil { showTrust = true }
            if let format = AppTabLaunch.wallet("--amount-format") { bridge.wallet("amount_format", ["format": format]) }
        }
        .task {
            // Simulator checks: act once the wallet runs.
            let send = AppTabLaunch.wallet("--wallet-send")
            let invoice = AppTabLaunch.wallet("--wallet-invoice")
            guard send != nil || invoice != nil else { return }
            while !Task.isCancelled, wallet?.status != nil || wallet == nil {
                try? await Task.sleep(for: .seconds(1))
            }
            if let send {
                mode = .send
                payInput = send
                payAmount = AppTabLaunch.wallet("--wallet-amount") ?? ""
                bridge.wallet("wallet_quote", ["input": send, "amount": payAmount])
            }
            if let invoice { mode = .receive; invoiceAmount = invoice; bridge.wallet("wallet_invoice", ["amount": invoice]) }
        }
        // Starts, syncs, quotes, and payments finish in the background;
        // poll quickly while one runs, and slowly otherwise to pick up
        // payments that arrive.
        .task(id: loading) {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(loading ? 1 : 10))
                if !Task.isCancelled && !bridge.busy { bridge.snapshot() }
            }
        }
        .alert("Show your recovery words?", isPresented: $confirmWords) {
            Button("Show words", role: .destructive) {
                bridge.walletWords { words = $0 }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Anyone who sees these words can take your bitcoin. Make sure no one is watching and nothing is recording your screen.")
        }
        .sheet(isPresented: Binding(get: { words != nil }, set: { if !$0 { words = nil } })) {
            WordsSheet(words: words ?? [], saved: {
                bridge.wallet("wallet_words_saved")
                words = nil
            }) { words = nil }
        }
        .sheet(isPresented: $showTrust) {
            if let trust = wallet?.trust {
                TrustSheet(trust: trust) {
                    if !trust.acknowledged { bridge.wallet("wallet_acknowledge") }
                    showTrust = false
                }
                .presentationDetents([.medium, .large])
            }
        }
        .sheet(isPresented: $showHistory) {
            HistorySheet(payments: wallet?.payments ?? []) { showHistory = false }
        }
        .sheet(isPresented: $restoring) {
            RestoreSheet(bridge: bridge, hasBalance: (wallet?.empty == false)) { restoring = false }
        }
    }

    @ViewBuilder private var content: some View {
        switch wallet?.state {
        case "failed":
            VStack(alignment: .leading, spacing: 16) {
                Text(wallet?.message ?? "The wallet could not start.").foregroundStyle(.white)
                Button("Try again") { bridge.refreshWallet() }.buttonStyle(.bordered)
                Button("Restore from recovery words") { restoring = true }.buttonStyle(.bordered)
            }
        default:
            ready(wallet ?? WalletState.opening)
        }
    }

    /// The main screen: the balance, Receive and Send, recent activity, the
    /// backup card while it's needed, and everything else under Advanced.
    @ViewBuilder private func ready(_ wallet: WalletState) -> some View {
        balance(wallet)
        if let warning = wallet.warning {
            Text(warning).font(.paper(.footnote)).foregroundStyle(.white)
                .padding(12).background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 12))
        }
        if let card = wallet.backup_card { backupCard(card) }
        primaryButtons(wallet)
        switch activeMode(wallet) {
        case .receive: receive(wallet)
        case .send: send(wallet)
        case .home: EmptyView()
        }
        recent(wallet)
        advanced(wallet)
    }

    /// A payment in progress keeps the Send panel open.
    private func activeMode(_ wallet: WalletState) -> Mode {
        if let state = wallet.send?.state, state != "idle", state != "failed" || mode == .send { return .send }
        return mode
    }

    // MARK: Balance

    private func balance(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            let unknown = wallet.balance_unknown == true
            Text(unknown ? "₿000,000" : wallet.balance ?? "")
                .font(.paper(52, weight: .semibold))
                .foregroundStyle(.white)
                .minimumScaleFactor(0.5).lineLimit(1)
                .redacted(reason: unknown ? .placeholder : [])
                .accessibilityLabel(unknown ? "Balance not read yet" : wallet.balance_spoken ?? wallet.balance ?? "")
                .accessibilityIdentifier("wallet-balance")
            // Quiet: only while starting, or when the balance is old or failed to update.
            if let status = wallet.status {
                HStack(spacing: 8) { ProgressView(); Text(status) }
                    .font(.paper(.footnote)).foregroundStyle(.gray)
            } else if let error = wallet.error {
                Text(error).font(.paper(.footnote)).foregroundStyle(.gray)
            } else if wallet.stale == true, wallet.refreshing != true {
                Text(updated(wallet.synced_at)).font(.paper(.footnote)).foregroundStyle(.gray)
            }
            if wallet.empty == true {
                Text("No bitcoin yet. Tap Receive to get some.")
                    .font(.paper(.callout)).foregroundStyle(.gray)
            }
        }
    }

    private func backupCard(_ card: WalletState.BackupCard) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(card.title).font(.paper(.headline)).foregroundStyle(.white)
            Text(card.detail).font(.paper(.footnote)).foregroundStyle(.gray)
            Button(card.action) { confirmWords = true }
                .buttonStyle(.bordered).tint(.white)
                .disabled(wallet?.can_show_words != true)
                .accessibilityIdentifier("wallet-backup-card")
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 14))
    }

    private func primaryButtons(_ wallet: WalletState) -> some View {
        HStack(spacing: 12) {
            bigButton("Receive", systemImage: "arrow.down", on: activeMode(wallet) == .receive) {
                bridge.wallet("wallet_send_reset")
                if mode == .receive { mode = .home; return }
                mode = .receive
            }
            .accessibilityIdentifier("wallet-receive")
            bigButton("Send", systemImage: "arrow.up", on: activeMode(wallet) == .send) {
                if mode == .send { bridge.wallet("wallet_send_reset") }
                mode = mode == .send ? .home : .send
            }
            .accessibilityIdentifier("wallet-send")
        }
    }

    /// White, or dimmed while the other button's panel is open.
    private func bigButton(_ title: String, systemImage: String, on: Bool, action: @escaping () -> Void) -> some View {
        let dimmed = !on && activeMode(wallet ?? WalletState.opening) != .home
        return Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.paper(.headline))
                .frame(maxWidth: .infinity, minHeight: 52)
                .foregroundStyle(dimmed ? .white : .black)
                .background(dimmed ? Color(white: 0.2) : Color.white, in: RoundedRectangle(cornerRadius: 14))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    // MARK: Receive

    @ViewBuilder private func receive(_ wallet: WalletState) -> some View {
        let receive = wallet.receive
        VStack(alignment: .leading, spacing: 12) {
            if let code = receive?.lightning { codeView(code) } else if receive?.lightning_busy == true || wallet.status != nil {
                placeholderCode()
            }
            if let error = receive?.lightning_error { Text(error).font(.paper(.footnote)).foregroundStyle(.white) }
            HStack {
                TextField("Amount in \(amounts.unit) (optional)", text: $invoiceAmount)
                    .keyboardType(amountKeyboard)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("wallet-invoice-amount")
                Button(receive?.lightning_busy == true ? "Making…" : (receive?.lightning == nil ? "Create" : "Update")) {
                    bridge.wallet("wallet_invoice", ["amount": invoiceAmount])
                }
                .buttonStyle(.bordered).tint(.white)
                .disabled(receive?.lightning_busy == true || wallet.status != nil)
                .accessibilityIdentifier("wallet-new-invoice")
            }
            Text("Other ways to receive are under Advanced.").font(.paper(.footnote)).foregroundStyle(.gray)
        }
        // One request for any amount, ready to scan, once the wallet runs.
        .task(id: wallet.status == nil) {
            if wallet.status == nil, receive?.lightning == nil, receive?.lightning_busy != true, receive?.lightning_error == nil {
                bridge.wallet("wallet_invoice", ["amount": ""])
            }
        }
    }

    private func codeView(_ code: WalletState.Code, size: CGFloat = 240) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if let qr = code.qr {
                InvitationQR(qr: qr)
                    .frame(width: size, height: size)
                    .frame(maxWidth: .infinity)
                    .accessibilityLabel("QR code")
            }
            Text(code.caption).font(.paper(.footnote)).foregroundStyle(.gray)
                .frame(maxWidth: .infinity)
            Text(code.text)
                .font(.paper(.footnote))
                .foregroundStyle(.white)
                .lineLimit(2).truncationMode(.middle)
                .textSelection(.enabled)
                .accessibilityIdentifier("wallet-receive-code")
            HStack(spacing: 12) {
                Button(copied == code.text ? "Copied" : "Copy", systemImage: copied == code.text ? "checkmark" : "doc.on.doc") {
                    UIPasteboard.general.string = code.text
                    copied = code.text
                    Task { try? await Task.sleep(for: .seconds(2)); copied = nil }
                }
                .buttonStyle(.borderedProminent).foregroundStyle(.black)
                ShareLink(item: code.text) { Label("Share", systemImage: "square.and.arrow.up") }
                    .buttonStyle(.bordered)
            }
            .tint(.white)
            .frame(maxWidth: .infinity)
        }
    }

    private func placeholderCode() -> some View {
        RoundedRectangle(cornerRadius: 12)
            .fill(Color.white.opacity(0.08))
            .frame(width: 240, height: 240)
            .frame(maxWidth: .infinity)
            .accessibilityHidden(true)
    }

    // MARK: Send

    @ViewBuilder private func send(_ wallet: WalletState) -> some View {
        let send = wallet.send
        VStack(alignment: .leading, spacing: 12) {
            switch send?.state {
            case "quoted", "paying":
                if let quote = send?.quote { confirm(quote, paying: send?.state == "paying", message: send?.message) }
            case "sent":
                VStack(alignment: .leading, spacing: 8) {
                    Text(send?.message ?? "Sent.").font(.paper(.headline)).foregroundStyle(.white)
                    if let result = send?.result { paymentRow(result, method: false) }
                    if let said = send?.recipient_message {
                        Text(said).font(.paper(.footnote)).foregroundStyle(.white)
                            .textSelection(.enabled)
                            .accessibilityIdentifier("wallet-recipient-message")
                    }
                    if let address = send?.save_suggestion {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Save \(address) for next time?").font(.paper(.footnote)).foregroundStyle(.gray)
                            HStack {
                                TextField("Name", text: $contactName)
                                    .textFieldStyle(.roundedBorder)
                                    .accessibilityIdentifier("wallet-contact-name")
                                Button("Save") {
                                    bridge.wallet("wallet_save_contact", ["name": contactName, "address": address])
                                    contactName = ""
                                }
                                .buttonStyle(.bordered).tint(.white)
                                .disabled(contactName.trimmingCharacters(in: .whitespaces).isEmpty)
                            }
                        }
                    }
                    Button("Done") {
                        payInput = ""; payAmount = ""; payComment = ""; mode = .home
                        bridge.wallet("wallet_send_reset")
                    }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                }
            default:
                if scanning {
                    InlineQRScanner(prompt: "Point the camera at a payment QR code.",
                                    accept: QRInvitation.payment) { scanned in
                        scanning = false
                        payInput = scanned
                        bridge.wallet("wallet_quote", ["input": scanned, "amount": payAmount, "comment": payComment])
                    }
                    Button("Type instead") { scanning = false }
                } else {
                    TextField("Paste or scan", text: $payInput, axis: .vertical)
                        .lineLimit(1...4)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("wallet-send-input")
                    HStack(spacing: 12) {
                        Button("Paste", systemImage: "doc.on.clipboard") {
                            payInput = UIPasteboard.general.string ?? ""
                            if !payInput.isEmpty { review() }
                        }
                        Button("Scan", systemImage: "qrcode.viewfinder") { scanning = true }
                    }
                    .buttonStyle(.bordered).tint(.white)
                    let needsAmount = send?.state == "needs_amount"
                    if needsAmount, let recipient = send?.recipient {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(recipient).font(.paper(.callout)).foregroundStyle(.white)
                            if let description = send?.description {
                                Text(description).font(.paper(.footnote)).foregroundStyle(.gray)
                            }
                        }
                        .accessibilityIdentifier("wallet-recipient")
                    }
                    if needsAmount || !payAmount.isEmpty {
                        TextField("Amount in \(amounts.unit)", text: $payAmount)
                            .keyboardType(amountKeyboard)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("wallet-send-amount")
                    }
                    if needsAmount, let most = send?.comment_max {
                        TextField("Note (optional, up to \(most) characters)", text: $payComment)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("wallet-send-comment")
                    }
                    if let message = send?.message, send?.state != "idle" {
                        Text(message).font(.paper(.footnote)).foregroundStyle(.white)
                    }
                    Button(send?.state == "quoting" ? "Preparing…" : "Continue") { review() }
                        .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                        .disabled(payInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                  || send?.state == "quoting" || wallet.status != nil)
                        .accessibilityIdentifier("wallet-review")
                }
            }
        }
    }

    /// Rust reads what was pasted or scanned and says what it is.
    private func review() {
        bridge.wallet("wallet_quote", ["input": payInput, "amount": payAmount, "comment": payComment])
    }

    private func confirm(_ quote: WalletState.Quote, paying: Bool, message: String?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Send \(quote.amount)?").font(.paper(.title3, weight: .semibold)).foregroundStyle(.white)
            if let person = wallet?.send?.person {
                row("To", person)
            }
            row(wallet?.send?.person == nil ? "To" : (quote.to ?? quote.kind), quote.destination)
            if let source = wallet?.send?.person_source {
                Text(source).font(.paper(.caption)).foregroundStyle(.gray)
            }
            if let note = quote.note { row("For", note) }
            if let comment = quote.comment { row("Note", comment) }
            if let speeds = quote.speeds, !speeds.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("How fast").foregroundStyle(.gray).font(.paper(.callout))
                    ForEach(speeds) { speed in
                        speedButton(speed, disabled: paying) {
                            bridge.wallet("wallet_speed", ["quote": quote.id, "speed": speed.id])
                        }
                    }
                }
                .accessibilityIdentifier("wallet-speeds")
            }
            row("Amount", quote.amount)
            row("Fee", quote.fee)
            Divider().overlay(Color.white.opacity(0.3))
            row("Total", quote.total, weight: .semibold)
            if let message { Text(message).font(.paper(.footnote)).foregroundStyle(.gray) }
            Text("Payments can't be undone.").font(.paper(.footnote)).foregroundStyle(.gray)
            HStack {
                Button(paying ? "Sending…" : "Send \(quote.total)") {
                    bridge.wallet("wallet_pay", ["quote": quote.id])
                }
                .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                .disabled(paying)
                .accessibilityIdentifier("wallet-confirm")
                Spacer()
                Button("Cancel") { bridge.wallet("wallet_send_reset") }.disabled(paying).tint(.white)
            }
        }
        .padding(14)
        .background(Color(white: 0.1), in: RoundedRectangle(cornerRadius: 14))
    }

    private func speedButton(_ speed: WalletState.Speed, disabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Image(systemName: speed.chosen ? "largecircle.fill.circle" : "circle")
                Text(speed.label).multilineTextAlignment(.leading)
                Spacer()
                Text(speed.fee)
            }
            .font(.paper(.footnote))
            .foregroundStyle(.white)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .accessibilityAddTraits(speed.chosen ? .isSelected : [])
    }

    private func row(_ label: String, _ value: String,
                     weight: Font.Weight = .regular) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.gray)
            Spacer()
            Text(value).foregroundStyle(.white).multilineTextAlignment(.trailing)
                .font(.paper(.callout, weight: weight))
        }
        .font(.paper(.callout, weight: weight))
    }

    // MARK: Recent activity

    private func recent(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Recent activity").font(.paper(.headline)).foregroundStyle(.white)
                Spacer()
                if wallet.more_payments == true {
                    Button("See all") { showHistory = true }.tint(.white)
                        .accessibilityIdentifier("wallet-see-all")
                }
            }
            let recent = wallet.recent ?? []
            if recent.isEmpty {
                Text("Payments you send and receive appear here.").font(.paper(.footnote)).foregroundStyle(.gray)
            }
            ForEach(recent) { paymentRow($0, method: false) }
        }
    }

    private func paymentRow(_ payment: WalletState.Payment, method: Bool) -> some View {
        WalletPaymentRow(payment: payment, method: method)
    }

    // MARK: Advanced

    @ViewBuilder private func advanced(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 24) {
            Button {
                if forceAdvanced { forceAdvanced = false }
                bridge.wallet("wallet_advanced", ["open": !advancedOpen])
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Advanced").font(.paper(.headline))
                        if !advancedOpen, let note = wallet.advanced?.note {
                            Text(note).font(.paper(.footnote)).foregroundStyle(.gray)
                        }
                    }
                    Spacer()
                    Image(systemName: advancedOpen ? "chevron.up" : "chevron.down")
                }
                .foregroundStyle(.white)
                .padding(.vertical, 12)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("wallet-advanced")
            .id("wallet-advanced")
            if advancedOpen {
                details(wallet)
                otherWays(wallet)
                buy(wallet)
                if let deposits = wallet.deposits, !deposits.isEmpty {
                    depositsView(deposits, claim: wallet.claim, refund: wallet.refund)
                }
                people(wallet)
                if let spend = bridge.packet?.spend {
                    AgentPaymentsSection(spend: spend, bridge: bridge)
                }
                amountSetting()
                recovery(wallet)
                if let backup = wallet.backup { backupView(backup) }
            }
        }
        .padding(.top, 8)
    }

    /// The balance in the other unit, the network, and when it was read.
    private func details(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Balance").font(.paper(.headline)).foregroundStyle(.white)
            if let alternate = wallet.balance_alternate, !alternate.isEmpty { row("Also", alternate) }
            row("Network", wallet.network ?? "Bitcoin · Spark")
            HStack {
                Text(wallet.refreshing == true ? "Refreshing…" : updated(wallet.synced_at))
                    .font(.paper(.footnote)).foregroundStyle(.gray)
                Spacer()
                Button("Refresh", systemImage: "arrow.clockwise") { bridge.refreshWallet() }
                    .font(.paper(.footnote)).tint(.white)
                    .disabled(wallet.refreshing == true)
                    .accessibilityIdentifier("wallet-refresh")
            }
        }
    }

    // MARK: Other ways to receive

    @ViewBuilder private func otherWays(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Other ways to receive").font(.paper(.headline)).foregroundStyle(.white)
            Picker("Method", selection: $method) {
                ForEach(Method.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("wallet-method")
            let receive = wallet.receive
            switch method {
            case .lightning:
                if let code = receive?.lightning { codeView(code, size: 200) } else {
                    Text("Tap Receive above to make a Lightning request.").font(.paper(.footnote)).foregroundStyle(.gray)
                }
            case .spark:
                if let code = receive?.spark { codeView(code, size: 200) } else { placeholderCode() }
            case .bitcoin:
                if let code = receive?.bitcoin { codeView(code, size: 200) } else { placeholderCode() }
            case .nostr:
                if let code = receive?.nostr { codeView(code, size: 200) } else { placeholderCode() }
                if let publish = receive?.publish {
                    VStack(alignment: .leading, spacing: 6) {
                        Toggle(isOn: Binding(get: { publish.on }, set: { bridge.wallet("wallet_publish", ["on": $0]) })) {
                            Text("Publish my Spark address").foregroundStyle(.white)
                        }
                        .tint(.white)
                        .disabled(publish.busy || wallet.status != nil)
                        .accessibilityIdentifier("wallet-publish")
                        Text(publish.detail).font(.paper(.footnote)).foregroundStyle(.gray)
                        if publish.busy { ProgressView() }
                        if let message = publish.message { Text(message).font(.paper(.footnote)).foregroundStyle(.white) }
                    }
                }
            }
        }
    }

    // MARK: Buy

    @ViewBuilder private func buy(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Buy bitcoin").font(.paper(.headline)).foregroundStyle(.white)
            Text("Pay with dollars. The provider's page opens in your browser.")
                .font(.paper(.footnote)).foregroundStyle(.gray)
            TextField("Amount in \(amounts.unit)", text: $buyAmount)
                .keyboardType(amountKeyboard)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("wallet-buy-amount")
            ForEach(wallet.buy?.providers ?? []) { provider in
                Button {
                    bridge.wallet("wallet_buy", ["provider": provider.id, "amount": buyAmount])
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(provider.label).font(.paper(.headline))
                        Text(provider.detail).font(.paper(.footnote)).foregroundStyle(.gray)
                    }
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(Color(white: 0.1), in: RoundedRectangle(cornerRadius: 12))
                }
                .foregroundStyle(.white)
                .disabled(wallet.buy?.busy == true || wallet.status != nil)
            }
            if wallet.buy?.busy == true { HStack { ProgressView(); Text("Opening…").foregroundStyle(.gray) } }
            if let error = wallet.buy?.error { Text(error).font(.paper(.footnote)).foregroundStyle(.white) }
        }
    }

    // MARK: People

    @ViewBuilder private func people(_ wallet: WalletState) -> some View {
        if let people = wallet.people, !people.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("People").font(.paper(.headline)).foregroundStyle(.white)
                ForEach(people) { person in
                    Button {
                        payInput = person.input
                        mode = .send
                        bridge.wallet("wallet_quote", ["input": person.input, "amount": payAmount, "comment": payComment])
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(person.name).font(.paper(.callout)).foregroundStyle(.white)
                                Text(person.detail).font(.paper(.caption2)).foregroundStyle(.gray).lineLimit(1)
                            }
                            Spacer()
                            Text("Pay").font(.paper(.footnote)).foregroundStyle(.white)
                        }
                        .padding(.horizontal, 12).padding(.vertical, 8)
                        .background(Color(white: 0.1), in: RoundedRectangle(cornerRadius: 10))
                    }
                    .buttonStyle(.plain)
                }
            }
            .accessibilityIdentifier("wallet-people")
        }
    }

    // MARK: Amount format

    private func amountSetting() -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Show amounts as").font(.paper(.headline)).foregroundStyle(.white)
            Picker("Show amounts as", selection: Binding(
                get: { amounts.format },
                set: { chosen in
                    // What was typed was in the old format; start over.
                    invoiceAmount = ""; payAmount = ""; buyAmount = ""
                    bridge.wallet("amount_format", ["format": chosen])
                })) {
                ForEach(amounts.choices) { Text($0.label).tag($0.id) }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("amount-format")
            Text("Applies everywhere in the app. Stored amounts don't change.")
                .font(.paper(.footnote)).foregroundStyle(.gray)
        }
    }

    // MARK: Deposits, recovery, exit backup

    private func depositsView(_ deposits: [WalletState.Deposit], claim: WalletState.Claim?, refund: WalletState.Refund?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Bitcoin deposits").font(.paper(.headline)).foregroundStyle(.white)
                .id("wallet-deposits")
            ForEach(deposits) { deposit in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(deposit.amount).foregroundStyle(.white)
                        Spacer()
                        if deposit.actionable != false {
                            Button("Claim") {
                                bridge.wallet("wallet_claim_quote", ["txid": deposit.txid, "vout": deposit.vout])
                            }
                            .buttonStyle(.bordered).tint(.white)
                            .disabled(claim?.busy == true)
                            Button("Refund") {
                                refundAddress = ""
                                refundSpeed = "medium"
                                bridge.wallet("wallet_refund_start", ["txid": deposit.txid, "vout": deposit.vout])
                            }
                            .buttonStyle(.bordered).tint(.white)
                            .disabled(refund?.busy == true)
                            .accessibilityIdentifier("wallet-refund")
                        }
                    }
                    Text(deposit.status).font(.paper(.footnote)).foregroundStyle(.gray)
                    if let claim, claim.txid == deposit.txid, claim.vout == deposit.vout {
                        if claim.busy { ProgressView() }
                        if let quote = claim.quote {
                            Text(quote).font(.paper(.footnote)).foregroundStyle(.white)
                            HStack {
                                Button("Claim at this fee") {
                                    bridge.wallet("wallet_claim", ["txid": claim.txid, "vout": claim.vout])
                                }
                                .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                                Button("Cancel") { bridge.wallet("wallet_claim_reset") }.tint(.white)
                            }
                        }
                        if let message = claim.message {
                            Text(message).font(.paper(.footnote)).foregroundStyle(.white)
                        }
                    }
                    if let refund, refund.txid == deposit.txid, refund.vout == deposit.vout {
                        refundView(refund)
                    }
                }
                .padding(12)
                .background(Color(white: 0.08), in: RoundedRectangle(cornerRadius: 12))
            }
        }
    }

    @ViewBuilder private func refundView(_ refund: WalletState.Refund) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Refund on-chain").font(.paper(.subheadline, weight: .semibold)).foregroundStyle(.white)
            if refund.busy { ProgressView() }
            if !refund.speeds.isEmpty && refund.review == nil {
                TextField("Bitcoin address to refund to", text: $refundAddress)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("wallet-refund-address")
                ForEach(refund.speeds) { speed in
                    let chosen = WalletState.Speed(id: speed.id, label: speed.label, fee: speed.fee, chosen: speed.id == refundSpeed)
                    speedButton(chosen, disabled: false) { refundSpeed = speed.id }
                }
                HStack {
                    Button("Review refund") {
                        bridge.wallet("wallet_refund_review", ["txid": refund.txid, "vout": refund.vout,
                                                               "address": refundAddress, "speed": refundSpeed])
                    }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                    .disabled(refundAddress.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Button("Cancel") { bridge.wallet("wallet_refund_reset") }.tint(.white)
                }
            }
            if let review = refund.review {
                Text(review).font(.paper(.footnote)).foregroundStyle(.white)
                HStack {
                    Button("Refund") {
                        bridge.wallet("wallet_refund", ["txid": refund.txid, "vout": refund.vout])
                    }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                    .disabled(refund.busy)
                    .accessibilityIdentifier("wallet-refund-confirm")
                    Button("Cancel") { bridge.wallet("wallet_refund_reset") }.tint(.white)
                }
            }
            if let message = refund.message {
                Text(message).font(.paper(.footnote)).foregroundStyle(.white)
                if refund.review == nil && refund.speeds.isEmpty && !refund.busy {
                    Button("Close") { bridge.wallet("wallet_refund_reset") }.tint(.white)
                }
            }
        }
        .padding(.top, 4)
    }

    private func backupView(_ backup: WalletState.Backup) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(backup.title).font(.paper(.headline)).foregroundStyle(.white)
            Text(backup.detail).font(.paper(.footnote)).foregroundStyle(.gray)
            Text(backup.saved_at.map { "Saved on this phone " + Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named)) } ?? "Not saved yet. It saves after the wallet syncs.")
                .font(.paper(.footnote)).foregroundStyle(.white)
            if let error = backup.error ?? exportError { Text(error).font(.paper(.footnote)).foregroundStyle(.white) }
            Button("Export to Files", systemImage: "square.and.arrow.down") {
                exportError = nil
                bridge.walletExitExport { result in
                    switch result {
                    case .success(let (name, text)): exportFile = ExitFile(name: name, text: text)
                    case .failure(let error): exportError = error.message
                    }
                }
            }
            .buttonStyle(.bordered).tint(.white)
            .disabled(!backup.can_export)
            .accessibilityIdentifier("wallet-export-exit")
        }
        .id("wallet-backup")
        .fileExporter(isPresented: Binding(get: { exportFile != nil }, set: { if !$0 { exportFile = nil } }),
                      document: exportFile, contentType: .json,
                      defaultFilename: exportFile?.name ?? "openagents-spark-exit.json") { result in
            if case .failure(let error) = result { exportError = error.localizedDescription }
            exportFile = nil
        }
    }

    private func recovery(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Recovery").font(.paper(.headline)).foregroundStyle(.white)
            Text("Your recovery words restore this wallet on another phone or a computer. Write them down and keep them offline.")
                .font(.paper(.footnote)).foregroundStyle(.gray)
            HStack(spacing: 12) {
                Button("Show recovery words") { confirmWords = true }
                    .disabled(wallet.can_show_words != true)
                Button("Restore") { restoring = true }
            }
            .buttonStyle(.bordered).tint(.white)
        }
    }

    private func updated(_ seconds: UInt64?) -> String {
        guard let seconds else { return "Not updated yet" }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        return "Updated " + date.formatted(.relative(presentation: .named))
    }
}

/// One payment: Sent or Received, when, and the amount. `method` adds how it
/// traveled, for the full history.
private struct WalletPaymentRow: View {
    let payment: WalletState.Payment
    let method: Bool

    var body: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 2) {
                Text(payment.title).foregroundStyle(.white)
                Text([method ? payment.method : nil, payment.status == "completed" ? nil : payment.status.capitalized,
                      Date(timeIntervalSince1970: TimeInterval(payment.at)).formatted(.relative(presentation: .named))]
                    .compactMap { $0 }.joined(separator: " · "))
                    .font(.paper(.caption)).foregroundStyle(.gray)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text(payment.amount).font(.paper(.callout)).foregroundStyle(.white)
                if let fee = payment.fee { Text(fee).font(.paper(.caption)).foregroundStyle(.gray) }
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// Every payment the wallet lists, from See all.
private struct HistorySheet: View {
    let payments: [WalletState.Payment]
    let done: () -> Void

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(payments) { WalletPaymentRow(payment: $0, method: true) }
                }
                .padding(20)
            }
            .background(Color.black.ignoresSafeArea())
            .navigationTitle("Activity")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: done) } }
        }
        .preferredColorScheme(.dark)
    }
}


/// The trust note, opened from the info button.
private struct TrustSheet: View {
    let trust: WalletState.Trust
    let done: () -> Void

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if let summary = trust.summary {
                        Text(summary).font(.paper(.body)).foregroundStyle(.white)
                        Text("Details").font(.paper(.headline)).foregroundStyle(.white).padding(.top, 8)
                    }
                    ForEach(trust.lines, id: \.self) { Text($0).font(.paper(.footnote)).foregroundStyle(.gray) }
                }
                .padding(20)
            }
            .background(Color.black.ignoresSafeArea())
            .navigationTitle(trust.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: done) } }
        }
        .preferredColorScheme(.dark)
    }
}

/// The recovery words, numbered. They exist only while this sheet is open.
private struct WordsSheet: View {
    let words: [String]
    /// The person wrote them down: the Back up card goes away.
    let saved: () -> Void
    let done: () -> Void
    @State private var copied = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("Write these \(words.count) words down in order and keep them somewhere safe and offline. Anyone with them can take your bitcoin.")
                        .font(.paper(.callout)).foregroundStyle(.white)
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 12) {
                        ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                            HStack(spacing: 8) {
                                Text("\(index + 1).").foregroundStyle(.gray)
                                Text(word).foregroundStyle(.white)
                            }
                            .font(.paper(.body))
                        }
                    }
                    .privacySensitive()
                    Button(copied ? "Copied" : "Copy words", systemImage: copied ? "checkmark" : "doc.on.doc") {
                        // One line of words separated by spaces, the form a
                        // restore takes. This device's pasteboard only, for a
                        // minute.
                        KeyCopy.copy(words.joined(separator: " "), secret: true)
                        copied = true
                        Task { try? await Task.sleep(for: .seconds(2)); copied = false }
                    }
                    .buttonStyle(.bordered).tint(.white)
                    .accessibilityIdentifier("wallet-copy-words")
                    Text("The copy stays on this phone and is cleared after a minute. Paste it somewhere offline, not into a message or a notes app that syncs.")
                        .font(.paper(.footnote)).foregroundStyle(.gray)
                    Button("I wrote them down", action: saved)
                        .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                        .frame(maxWidth: .infinity)
                        .padding(.top, 8)
                        .accessibilityIdentifier("wallet-words-saved")
                }
                .padding(20)
            }
            .background(Color.black.ignoresSafeArea())
            .navigationTitle("Recovery words")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: done) } }
        }
        .preferredColorScheme(.dark)
    }
}

/// Restore from recovery words. Rust checks them; this view saves the seed
/// in Keychain and reopens the wallet, then forgets what was typed.
private struct RestoreSheet: View {
    @ObservedObject var bridge: MobileBridge
    let hasBalance: Bool
    let done: () -> Void
    @State private var words = ""
    @State private var error: String?
    @State private var confirmReplace = false

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Enter your 12 or 24 recovery words, separated by spaces. This replaces the wallet on this phone.")
                    .font(.paper(.callout)).foregroundStyle(.white)
                if hasBalance {
                    Text("This wallet holds bitcoin. Write down its recovery words before you replace it, or you lose that bitcoin.")
                        .font(.paper(.footnote)).foregroundStyle(.white)
                        .padding(10).background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 10))
                }
                TextField("Recovery words", text: $words, axis: .vertical)
                    .lineLimit(3...8)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .privacySensitive()
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("wallet-restore-words")
                if let error { Text(error).font(.paper(.footnote)).foregroundStyle(.white) }
                Button("Restore wallet") {
                    if hasBalance { confirmReplace = true } else { restore() }
                }
                .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                .disabled(words.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                Spacer()
            }
            .padding(20)
            .dismissesKeyboard()
            .background(Color.black.ignoresSafeArea())
            .navigationTitle("Restore")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { words = ""; done() } } }
            .alert("Replace this wallet?", isPresented: $confirmReplace) {
                Button("Replace", role: .destructive) { restore() }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Its bitcoin can only be recovered with its own recovery words.")
            }
        }
        .preferredColorScheme(.dark)
    }

    private func restore() {
        bridge.restoreWallet(words: words) { failure in
            if let failure { error = failure } else { words = ""; done() }
        }
    }
}

/// The exit backup as a document for the Files exporter. It exists only
/// while the exporter is open.
struct ExitFile: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    let name: String
    let text: String

    init(name: String, text: String) { self.name = name; self.text = text }
    init(configuration: ReadConfiguration) throws { throw CocoaError(.fileReadUnsupportedScheme) }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

extension WalletState {
    /// The screen before Rust's first packet arrives.
    static let opening = WalletState(
        state: "ready", message: nil, network: "Bitcoin · Spark", balance: nil,
        balance_alternate: nil, balance_spoken: nil,
        empty: nil, synced_at: nil, refreshing: true, error: nil, balance_unknown: true,
        status: "Opening the wallet…", warning: nil, trust: nil, receive: nil, send: nil,
        payments: nil, can_show_words: false, buy: nil, deposits: nil, claim: nil, refund: nil, backup: nil, people: nil,
        stale: nil, recent: nil, more_payments: nil, backup_card: nil, advanced: nil)
}
