// The Wallet tab: bitcoin on mainnet through Breez's Spark SDK. Rust runs
// the wallet and decides every state and line of text; this view lays them
// out, collects typed values, and adds the clipboard, the camera, and the
// browser. The seed lives in Keychain (DeviceKey); the recovery words reach
// this view only in a direct reply, and only while their sheet is open.
import SwiftUI
import UIKit

/// Rust's Wallet screen (`wallet::Screen`): `failed`, or `ready` with the
/// summary. `ready` shows from launch: while the wallet starts it carries
/// the last balance read (or `balance_unknown`) and a `status`.
struct WalletState: Decodable, Equatable {
    struct Trust: Decodable, Equatable {
        let acknowledged: Bool
        let title: String
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
    }
    struct Quote: Decodable, Equatable {
        let id: UInt64
        let kind: String
        let destination: String
        let amount: String
        let fee: String
        let total: String
        let note: String?
        let comment: String?
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
        var id: String { "\(txid):\(vout)" }
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
    let balance_btc: String?
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
}

struct WalletTab: View {
    enum Section: String, CaseIterable { case receive = "Receive", send = "Send", buy = "Buy" }
    enum Method: String, CaseIterable { case lightning = "Lightning", spark = "Spark", bitcoin = "Bitcoin" }

    @ObservedObject var bridge: MobileBridge
    @State private var section = Section.receive
    @State private var method = Method.lightning
    @State private var invoiceAmount = ""
    @State private var payInput = ""
    @State private var payAmount = ""
    @State private var payComment = ""
    @State private var buyAmount = ""
    @State private var scanning = false
    @State private var copied: String?
    @State private var confirmWords = false
    @State private var words: [String]?
    @State private var restoring = false
    @State private var showTrust = false

    private var wallet: WalletState? { bridge.packet?.wallet }
    private var loading: Bool { bridge.packet?.wallet_loading == true }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(alignment: .firstTextBaseline) {
                    Text("Wallet").font(.largeTitle.bold()).foregroundStyle(.white)
                    Spacer()
                    Button { showTrust = true } label: {
                        Image(systemName: "info.circle").font(.title2).foregroundStyle(.white)
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
        .scrollDismissesKeyboard(.interactively)
        .dismissesKeyboard()
        .background(Color.black.ignoresSafeArea())
        .refreshable { bridge.refreshWallet() }
        .onAppear {
            bridge.openWallet()
            if let value = AppTabLaunch.wallet("--wallet-section"), let chosen = Section(rawValue: value.capitalized) { section = chosen }
            if let value = AppTabLaunch.wallet("--wallet-method"), let chosen = Method(rawValue: value.capitalized) { method = chosen }
            if AppTabLaunch.wallet("--wallet-info") != nil { showTrust = true }
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
                payInput = send
                payAmount = AppTabLaunch.wallet("--wallet-amount") ?? ""
                bridge.wallet("wallet_quote", ["input": send, "amount": payAmount])
            }
            if let invoice { invoiceAmount = invoice; bridge.wallet("wallet_invoice", ["amount": invoice]) }
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
            WordsSheet(words: words ?? []) { words = nil }
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

    @ViewBuilder private func ready(_ wallet: WalletState) -> some View {
        balance(wallet)
        if let warning = wallet.warning {
            Text(warning).font(.footnote).foregroundStyle(.white)
                .padding(12).background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 12))
        }
        Picker("Action", selection: $section) {
            ForEach(Section.allCases, id: \.self) { Text($0.rawValue).tag($0) }
        }
        .pickerStyle(.segmented)
        .accessibilityIdentifier("wallet-section")
        switch section {
        case .receive: receive(wallet)
        case .send: send(wallet)
        case .buy: buy(wallet)
        }
        if let deposits = wallet.deposits, !deposits.isEmpty {
            depositsView(deposits, claim: wallet.claim)
        }
        history(wallet.payments ?? [])
        recovery(wallet)
    }

    private func balance(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text((wallet.network ?? "Bitcoin · Spark").uppercased())
                .font(.caption.weight(.semibold)).foregroundStyle(.gray)
            let unknown = wallet.balance_unknown == true
            Text(unknown ? "000,000 sats" : wallet.balance ?? "")
                .font(.system(size: 44, weight: .semibold, design: .rounded))
                .foregroundStyle(.white)
                .minimumScaleFactor(0.5).lineLimit(1)
                .redacted(reason: unknown ? .placeholder : [])
                .accessibilityIdentifier("wallet-balance")
            Text(unknown ? "0.00000000 BTC" : wallet.balance_btc ?? "")
                .font(.callout.monospacedDigit()).foregroundStyle(.gray)
                .redacted(reason: unknown ? .placeholder : [])
            HStack(spacing: 8) {
                if let status = wallet.status {
                    ProgressView()
                    Text(status).foregroundStyle(.gray)
                } else if wallet.refreshing == true {
                    ProgressView()
                    Text("Refreshing…").foregroundStyle(.gray)
                } else {
                    Text(updated(wallet.synced_at)).foregroundStyle(.gray)
                    Spacer()
                    Button("Refresh", systemImage: "arrow.clockwise") { bridge.refreshWallet() }
                        .labelStyle(.iconOnly)
                }
            }
            .font(.footnote)
            .padding(.top, 4)
            if let error = wallet.error {
                Text(error).font(.footnote).foregroundStyle(.white)
            }
            if wallet.empty == true {
                Text("No bitcoin yet. Receive some below, or buy it with dollars.")
                    .font(.callout).foregroundStyle(.gray)
            }
        }
    }

    // MARK: Receive

    @ViewBuilder private func receive(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Picker("Method", selection: $method) {
                ForEach(Method.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            let receive = wallet.receive
            switch method {
            case .lightning:
                HStack {
                    TextField("Amount in sats (optional)", text: $invoiceAmount)
                        .keyboardType(.numberPad)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("wallet-invoice-amount")
                    Button(receive?.lightning_busy == true ? "Making…" : "New invoice") {
                        bridge.wallet("wallet_invoice", ["amount": invoiceAmount])
                    }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                    .disabled(receive?.lightning_busy == true || wallet.status != nil)
                }
                if let error = receive?.lightning_error { Text(error).font(.footnote).foregroundStyle(.white) }
                if let code = receive?.lightning { codeView(code) } else {
                    Text("Make an invoice for someone to pay over Lightning. Leave the amount empty to let them choose.")
                        .font(.footnote).foregroundStyle(.gray)
                }
            case .spark:
                if let code = receive?.spark { codeView(code) } else { placeholderCode() }
            case .bitcoin:
                if let code = receive?.bitcoin { codeView(code) } else { placeholderCode() }
            }
        }
    }

    private func codeView(_ code: WalletState.Code) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if let qr = code.qr {
                InvitationQR(qr: qr)
                    .frame(width: 220, height: 220)
                    .frame(maxWidth: .infinity)
                    .accessibilityLabel("QR code")
            }
            Text(code.caption).font(.footnote).foregroundStyle(.gray)
            Text(code.text)
                .font(.system(.footnote, design: .monospaced))
                .foregroundStyle(.white)
                .lineLimit(3).truncationMode(.middle)
                .textSelection(.enabled)
                .accessibilityIdentifier("wallet-receive-code")
            HStack(spacing: 12) {
                Button(copied == code.text ? "Copied" : "Copy", systemImage: copied == code.text ? "checkmark" : "doc.on.doc") {
                    UIPasteboard.general.string = code.text
                    copied = code.text
                    Task { try? await Task.sleep(for: .seconds(2)); copied = nil }
                }
                .buttonStyle(.bordered)
                ShareLink(item: code.text) { Label("Share", systemImage: "square.and.arrow.up") }
                    .buttonStyle(.bordered)
            }
            .tint(.white)
        }
    }

    private func placeholderCode() -> some View {
        VStack(alignment: .leading, spacing: 10) {
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.white.opacity(0.08))
                .frame(width: 220, height: 220)
                .frame(maxWidth: .infinity)
                .accessibilityHidden(true)
            Text("spark1000000000000000000000000000000000000000")
                .font(.system(.footnote, design: .monospaced))
                .redacted(reason: .placeholder)
        }
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
                    Text(send?.message ?? "Sent.").font(.headline).foregroundStyle(.white)
                    if let result = send?.result { paymentRow(result) }
                    if let said = send?.recipient_message {
                        Text(said).font(.footnote).foregroundStyle(.white)
                            .textSelection(.enabled)
                            .accessibilityIdentifier("wallet-recipient-message")
                    }
                    Button("Done") { payInput = ""; payAmount = ""; payComment = ""; bridge.wallet("wallet_send_reset") }
                        .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                }
            default:
                if scanning {
                    InlineQRScanner(prompt: "Point the camera at a Lightning invoice, Lightning address, or Bitcoin QR code.") { scanned in
                        scanning = false
                        payInput = scanned
                        bridge.wallet("wallet_quote", ["input": scanned, "amount": payAmount, "comment": payComment])
                    }
                    Button("Type instead") { scanning = false }
                } else {
                    TextField("Invoice, Lightning address, LNURL, or Bitcoin address", text: $payInput, axis: .vertical)
                        .lineLimit(1...4)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("wallet-send-input")
                    HStack(spacing: 12) {
                        Button("Paste", systemImage: "doc.on.clipboard") {
                            payInput = UIPasteboard.general.string ?? ""
                        }
                        Button("Scan", systemImage: "qrcode.viewfinder") { scanning = true }
                    }
                    .buttonStyle(.bordered).tint(.white)
                    if send?.state == "needs_amount", let recipient = send?.recipient {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(recipient).font(.system(.callout, design: .monospaced)).foregroundStyle(.white)
                            if let description = send?.description {
                                Text(description).font(.footnote).foregroundStyle(.gray)
                            }
                        }
                        .accessibilityIdentifier("wallet-recipient")
                    }
                    TextField("Amount in sats, if the request has none", text: $payAmount)
                        .keyboardType(.numberPad)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("wallet-send-amount")
                    if send?.state == "needs_amount", let most = send?.comment_max {
                        TextField("Comment (optional, up to \(most) characters)", text: $payComment)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("wallet-send-comment")
                    }
                    if let message = send?.message, send?.state != "idle" {
                        Text(message).font(.footnote).foregroundStyle(.white)
                    }
                    Button(send?.state == "quoting" ? "Preparing…" : "Review payment") {
                        bridge.wallet("wallet_quote", ["input": payInput, "amount": payAmount, "comment": payComment])
                    }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                    .disabled(payInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                              || send?.state == "quoting" || wallet.status != nil)
                    .accessibilityIdentifier("wallet-review")
                }
            }
        }
    }

    private func confirm(_ quote: WalletState.Quote, paying: Bool, message: String?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Confirm payment").font(.headline).foregroundStyle(.white)
            row("To", "\(quote.kind)\n\(quote.destination)")
            if let note = quote.note { row("For", note) }
            if let comment = quote.comment { row("Comment", comment) }
            row("Amount", quote.amount)
            row("Fee", quote.fee)
            Divider().overlay(Color.white.opacity(0.3))
            row("Total", quote.total).fontWeight(.semibold)
            if let message { Text(message).font(.footnote).foregroundStyle(.gray) }
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

    private func row(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.gray)
            Spacer()
            Text(value).foregroundStyle(.white).multilineTextAlignment(.trailing)
                .font(.system(.callout, design: label == "To" ? .monospaced : .default))
        }
        .font(.callout)
    }

    // MARK: Buy

    @ViewBuilder private func buy(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Buy bitcoin with dollars. The provider's page opens in your browser.")
                .font(.footnote).foregroundStyle(.gray)
            TextField("Amount in sats", text: $buyAmount)
                .keyboardType(.numberPad)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("wallet-buy-amount")
            ForEach(wallet.buy?.providers ?? []) { provider in
                Button {
                    bridge.wallet("wallet_buy", ["provider": provider.id, "amount": buyAmount])
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(provider.label).font(.headline)
                        Text(provider.detail).font(.footnote).foregroundStyle(.gray)
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
            if let error = wallet.buy?.error { Text(error).font(.footnote).foregroundStyle(.white) }
        }
    }

    // MARK: Deposits, history, recovery

    private func depositsView(_ deposits: [WalletState.Deposit], claim: WalletState.Claim?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Bitcoin deposits").font(.headline).foregroundStyle(.white)
            ForEach(deposits) { deposit in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(deposit.amount).foregroundStyle(.white)
                        Spacer()
                        Button("Claim") {
                            bridge.wallet("wallet_claim_quote", ["txid": deposit.txid, "vout": deposit.vout])
                        }
                        .buttonStyle(.bordered).tint(.white)
                        .disabled(claim?.busy == true)
                    }
                    Text(deposit.status).font(.footnote).foregroundStyle(.gray)
                    if let claim, claim.txid == deposit.txid, claim.vout == deposit.vout {
                        if claim.busy { ProgressView() }
                        if let quote = claim.quote {
                            Text(quote).font(.footnote).foregroundStyle(.white)
                            HStack {
                                Button("Claim at this fee") {
                                    bridge.wallet("wallet_claim", ["txid": claim.txid, "vout": claim.vout])
                                }
                                .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                                Button("Cancel") { bridge.wallet("wallet_claim_reset") }.tint(.white)
                            }
                        }
                        if let message = claim.message {
                            Text(message).font(.footnote).foregroundStyle(.white)
                        }
                    }
                }
                .padding(12)
                .background(Color(white: 0.08), in: RoundedRectangle(cornerRadius: 12))
            }
        }
    }

    private func history(_ payments: [WalletState.Payment]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("History").font(.headline).foregroundStyle(.white)
            if payments.isEmpty {
                Text("Payments you send and receive appear here.").font(.footnote).foregroundStyle(.gray)
            }
            ForEach(payments) { paymentRow($0) }
        }
    }

    private func paymentRow(_ payment: WalletState.Payment) -> some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 2) {
                Text(payment.title).foregroundStyle(.white)
                Text([payment.method, payment.status == "completed" ? nil : payment.status.capitalized,
                      Date(timeIntervalSince1970: TimeInterval(payment.at)).formatted(.relative(presentation: .named))]
                    .compactMap { $0 }.joined(separator: " · "))
                    .font(.caption).foregroundStyle(.gray)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text(payment.amount).font(.callout.monospacedDigit()).foregroundStyle(.white)
                if let fee = payment.fee { Text(fee).font(.caption).foregroundStyle(.gray) }
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func recovery(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Recovery").font(.headline).foregroundStyle(.white)
            Text("Your recovery words restore this wallet on another phone. Write them down and keep them offline.")
                .font(.footnote).foregroundStyle(.gray)
            HStack(spacing: 12) {
                Button("Show recovery words") { confirmWords = true }
                    .disabled(wallet.can_show_words != true)
                Button("Restore") { restoring = true }
            }
            .buttonStyle(.bordered).tint(.white)
        }
    }

    private func updated(_ seconds: UInt64?) -> String {
        guard let seconds else { return "Not synced yet" }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        return "Updated " + date.formatted(.relative(presentation: .named))
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
                    ForEach(trust.lines, id: \.self) { Text($0).font(.callout).foregroundStyle(.white) }
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
    let done: () -> Void
    @State private var copied = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("Write these \(words.count) words down in order and keep them somewhere safe and offline. Anyone with them can take your bitcoin.")
                        .font(.callout).foregroundStyle(.white)
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 12) {
                        ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                            HStack(spacing: 8) {
                                Text("\(index + 1).").foregroundStyle(.gray).monospacedDigit()
                                Text(word).foregroundStyle(.white)
                            }
                            .font(.system(.body, design: .monospaced))
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
                        .font(.footnote).foregroundStyle(.gray)
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
                    .font(.callout).foregroundStyle(.white)
                if hasBalance {
                    Text("This wallet holds bitcoin. Write down its recovery words before you replace it, or you lose that bitcoin.")
                        .font(.footnote).foregroundStyle(.white)
                        .padding(10).background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 10))
                }
                TextField("Recovery words", text: $words, axis: .vertical)
                    .lineLimit(3...8)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .privacySensitive()
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("wallet-restore-words")
                if let error { Text(error).font(.footnote).foregroundStyle(.white) }
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

extension WalletState {
    /// The screen before Rust's first packet arrives.
    static let opening = WalletState(
        state: "ready", message: nil, network: "Bitcoin · Spark", balance: nil, balance_btc: nil,
        empty: nil, synced_at: nil, refreshing: true, error: nil, balance_unknown: true,
        status: "Opening the wallet…", warning: nil, trust: nil, receive: nil, send: nil,
        payments: nil, can_show_words: false, buy: nil, deposits: nil, claim: nil)
}

/// Dismiss the keyboard from a Done bar just above it and from a tap
/// anywhere outside a text field. The tap does not stop the touch, so a
/// button tapped while the keyboard is up still acts.
private struct DismissesKeyboard: ViewModifier {
    @State private var keyboard = false

    func body(content: Content) -> some View {
        content
            .background(OutsideTap().frame(width: 0, height: 0))
            .safeAreaInset(edge: .bottom, spacing: 0) {
                if keyboard {
                    HStack {
                        Spacer()
                        Button("Done") { OutsideTap.dismiss() }
                            .fontWeight(.semibold).tint(.white)
                            .accessibilityIdentifier("keyboard-done")
                    }
                    .padding(.horizontal, 20).padding(.vertical, 10)
                    .background(Color(white: 0.1))
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
                keyboard = true
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
                keyboard = false
            }
    }
}

extension View {
    fileprivate func dismissesKeyboard() -> some View { modifier(DismissesKeyboard()) }
}

/// Watches taps on its window while it is on screen and ends editing when one
/// lands outside a text field.
private struct OutsideTap: UIViewRepresentable {
    static func dismiss() {
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }

    func makeUIView(context: Context) -> Watcher { Watcher() }
    func updateUIView(_ view: Watcher, context: Context) {}

    final class Watcher: UIView, UIGestureRecognizerDelegate {
        private lazy var tap: UITapGestureRecognizer = {
            let tap = UITapGestureRecognizer(target: self, action: #selector(tapped))
            tap.cancelsTouchesInView = false
            tap.delegate = self
            return tap
        }()

        override func didMoveToWindow() {
            super.didMoveToWindow()
            tap.view?.removeGestureRecognizer(tap)
            window?.addGestureRecognizer(tap)
        }

        @objc private func tapped() { OutsideTap.dismiss() }

        func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
            var view = touch.view
            while let current = view {
                if current is UITextField || current is UITextView { return false }
                view = current.superview
            }
            return true
        }

        func gestureRecognizer(_ recognizer: UIGestureRecognizer,
                               shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
    }
}
