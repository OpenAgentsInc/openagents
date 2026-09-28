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
    let refund: Refund?
    let backup: Backup?
    let people: [Person]?
}

struct WalletTab: View {
    enum Section: String, CaseIterable { case receive = "Receive", send = "Send", buy = "Buy" }
    enum Method: String, CaseIterable { case lightning = "Lightning", spark = "Spark", bitcoin = "Bitcoin", nostr = "Nostr" }

    @ObservedObject var bridge: MobileBridge
    @State private var section = Section.receive
    @State private var method = Method.lightning
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
    @State private var refundAddress = ""
    @State private var refundSpeed = "medium"
    @State private var exportFile: ExitFile?
    @State private var exportError: String?

    private var wallet: WalletState? { bridge.packet?.wallet }
    private var loading: Bool { bridge.packet?.wallet_loading == true }

    var body: some View {
        ScrollViewReader { scroller in
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
        .task {
            // Simulator checks: open a refund review, or the backup.
            let refund = AppTabLaunch.wallet("--wallet-refund")
            let backup = AppTabLaunch.wallet("--wallet-backup")
            guard refund != nil || backup != nil else { return }
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
            depositsView(deposits, claim: wallet.claim, refund: wallet.refund)
        }
        history(wallet.payments ?? [])
        if let spend = bridge.packet?.spend {
            AgentPaymentsSection(spend: spend, bridge: bridge)
        }
        recovery(wallet)
        if let backup = wallet.backup { backupView(backup) }
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
            case .nostr:
                if let code = receive?.nostr { codeView(code) } else { placeholderCode() }
                if let publish = receive?.publish {
                    VStack(alignment: .leading, spacing: 6) {
                        Toggle(isOn: Binding(get: { publish.on }, set: { bridge.wallet("wallet_publish", ["on": $0]) })) {
                            Text("Publish my Spark address").foregroundStyle(.white)
                        }
                        .tint(.white)
                        .disabled(publish.busy || wallet.status != nil)
                        .accessibilityIdentifier("wallet-publish")
                        Text(publish.detail).font(.footnote).foregroundStyle(.gray)
                        if publish.busy { ProgressView() }
                        if let message = publish.message { Text(message).font(.footnote).foregroundStyle(.white) }
                    }
                }
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
                    if let address = send?.save_suggestion {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Save \(address) as a contact?").font(.footnote).foregroundStyle(.gray)
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
                    Button("Done") { payInput = ""; payAmount = ""; payComment = ""; bridge.wallet("wallet_send_reset") }
                        .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                }
            default:
                if scanning {
                    InlineQRScanner(prompt: "Point the camera at a Lightning invoice, Lightning address, LNURL, Spark, Bitcoin, or Nostr (npub) QR code.",
                                    accept: QRInvitation.payment) { scanned in
                        scanning = false
                        payInput = scanned
                        bridge.wallet("wallet_quote", ["input": scanned, "amount": payAmount, "comment": payComment])
                    }
                    Button("Type instead") { scanning = false }
                } else {
                    if let people = wallet.people, !people.isEmpty, send?.state == "idle" || send?.state == nil {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 8) {
                                ForEach(people) { person in
                                    Button {
                                        payInput = person.input
                                        bridge.wallet("wallet_quote", ["input": person.input, "amount": payAmount, "comment": payComment])
                                    } label: {
                                        VStack(alignment: .leading, spacing: 2) {
                                            Text(person.name).font(.callout).foregroundStyle(.white)
                                            Text(person.detail).font(.caption2).foregroundStyle(.gray).lineLimit(1)
                                        }
                                        .padding(.horizontal, 12).padding(.vertical, 8)
                                        .background(Color(white: 0.1), in: RoundedRectangle(cornerRadius: 10))
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        .accessibilityIdentifier("wallet-people")
                    }
                    TextField("Invoice, Lightning address, npub, LNURL, or Bitcoin address", text: $payInput, axis: .vertical)
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
            if let person = wallet?.send?.person {
                row("Person", person)
            }
            row("To", "\(quote.kind)\n\(quote.destination)")
            if let source = wallet?.send?.person_source {
                Text(source).font(.caption).foregroundStyle(.gray)
            }
            if let note = quote.note { row("For", note) }
            if let comment = quote.comment { row("Comment", comment) }
            row("Amount", quote.amount)
            if let speeds = quote.speeds, !speeds.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Speed").foregroundStyle(.gray).font(.callout)
                    ForEach(speeds) { speed in
                        speedButton(speed, disabled: paying) {
                            bridge.wallet("wallet_speed", ["quote": quote.id, "speed": speed.id])
                        }
                    }
                }
                .accessibilityIdentifier("wallet-speeds")
            }
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

    private func speedButton(_ speed: WalletState.Speed, disabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Image(systemName: speed.chosen ? "largecircle.fill.circle" : "circle")
                Text(speed.label).multilineTextAlignment(.leading)
                Spacer()
                Text(speed.fee).monospacedDigit()
            }
            .font(.footnote)
            .foregroundStyle(.white)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .accessibilityAddTraits(speed.chosen ? .isSelected : [])
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

    private func depositsView(_ deposits: [WalletState.Deposit], claim: WalletState.Claim?, refund: WalletState.Refund?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Bitcoin deposits").font(.headline).foregroundStyle(.white)
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
            Text("Refund on-chain").font(.subheadline.weight(.semibold)).foregroundStyle(.white)
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
                Text(review).font(.footnote).foregroundStyle(.white)
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
                Text(message).font(.footnote).foregroundStyle(.white)
                if refund.review == nil && refund.speeds.isEmpty && !refund.busy {
                    Button("Close") { bridge.wallet("wallet_refund_reset") }.tint(.white)
                }
            }
        }
        .padding(.top, 4)
    }

    private func backupView(_ backup: WalletState.Backup) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(backup.title).font(.headline).foregroundStyle(.white)
            Text(backup.detail).font(.footnote).foregroundStyle(.gray)
            Text(backup.saved_at.map { "Saved on this phone " + Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named)) } ?? "Not saved yet. It saves after the wallet syncs.")
                .font(.footnote).foregroundStyle(.white)
            if let error = backup.error ?? exportError { Text(error).font(.footnote).foregroundStyle(.white) }
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
        state: "ready", message: nil, network: "Bitcoin · Spark", balance: nil, balance_btc: nil,
        empty: nil, synced_at: nil, refreshing: true, error: nil, balance_unknown: true,
        status: "Opening the wallet…", warning: nil, trust: nil, receive: nil, send: nil,
        payments: nil, can_show_words: false, buy: nil, deposits: nil, claim: nil, refund: nil, backup: nil, people: nil)
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
