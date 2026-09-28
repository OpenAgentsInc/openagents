// Agents' payment requests (agent spending, phase 1): an agent on one of the
// owner's computers asks, and the owner approves each payment here. Rust
// reads the requests, checks them against the computer's grant and the
// ledger, and decodes the payee and amount from the invoice itself; this
// view shows the approval sheet, asks for Face ID or the passcode above the
// threshold Rust names, and sends the owner's tap. Nothing pays without it.
import LocalAuthentication
import SwiftUI

/// Rust's `spend::View`.
struct SpendState: Decodable, Equatable {
    struct Sheet: Decodable, Equatable, Identifiable {
        let request: String
        let host: String
        let computer: String
        let task: String?
        let title: String?
        let purpose: String
        let payee: String
        let payee_new: Bool
        let description: String?
        let note: String?
        let resource: String?
        let amount: String
        let amount_msat: UInt64
        let fee: String
        let fee_ceiling: String
        let remaining: String
        let expires_at: UInt64
        let authenticate: Bool
        let ready: Bool
        let can_trust: Bool
        var id: String { request }
    }
    struct Trusted: Decodable, Equatable, Identifiable {
        let payee: String
        let label: String
        let limit: String
        var id: String { payee }
    }
    struct Computer: Decodable, Equatable, Identifiable {
        let host: String
        let computer: String
        let blocked: Bool
        let remaining: String
        let automatic: String?
        let trusted: [Trusted]
        var id: String { host }
    }
    struct Entry: Decodable, Equatable, Identifiable {
        let request: String
        let computer: String
        let title: String?
        let purpose: String
        let amount: String
        let fee: String?
        let state: String
        let auto: Bool
        let detail: String?
        let at: UInt64
        var id: String { request }
    }
    let sheet: Sheet?
    let waiting: Int
    let busy: Bool
    let notice: String?
    let computers: [Computer]
    let history: [Entry]
}

/// The approval sheet for the oldest waiting request.
struct SpendApprovalSheet: View {
    let sheet: SpendState.Sheet
    let waiting: Int
    let busy: Bool
    @ObservedObject var bridge: MobileBridge
    @State private var failure: String?
    @State private var confirmingBlock = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(spacing: 6) {
                        Text(sheet.amount)
                            .font(.system(size: 40, weight: .semibold, design: .rounded))
                            .accessibilityIdentifier("spend-amount")
                        Text("Fee \(sheet.fee)")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                }
                Section("Asked by") {
                    row("Computer", sheet.computer)
                    if let title = sheet.title { row("Task", title) }
                    row("Purpose", sheet.purpose)
                    if let resource = sheet.resource { row("For", resource) }
                    if let note = sheet.note { row("Agent's note", note) }
                }
                Section {
                    row("Payee", sheet.payee)
                    if sheet.payee_new {
                        Text("You haven't paid this payee from this computer before.")
                            .font(.footnote)
                            .foregroundStyle(.orange)
                    }
                    if let description = sheet.description { row("Invoice says", description) }
                } header: {
                    Text("Paid to")
                } footer: {
                    Text(sheet.can_trust
                         ? "Read from the invoice itself, not from the agent's description. Trusting the payee lets this computer pay it small amounts without asking, within daily limits."
                         : "Read from the invoice itself, not from the agent's description.")
                }
                Section {
                    row("Fee ceiling", sheet.fee_ceiling)
                    row("This computer has", sheet.remaining)
                    if waiting > 0 { row("Also waiting", "\(waiting) more") }
                }
                if let failure {
                    Section { Text(failure).foregroundStyle(.red) }
                }
                Section {
                    Button {
                        approve()
                    } label: {
                        HStack {
                            Spacer()
                            if busy { ProgressView() } else { Text("Approve and pay \(sheet.amount)").bold() }
                            Spacer()
                        }
                    }
                    .disabled(!sheet.ready || busy)
                    .accessibilityIdentifier("spend-approve")
                    if sheet.can_trust {
                        Button {
                            approveAndTrust()
                        } label: {
                            HStack { Spacer(); Text("Approve and trust this payee"); Spacer() }
                        }
                        .disabled(!sheet.ready || busy)
                        .accessibilityIdentifier("spend-approve-trust")
                    }
                    Button(role: .destructive) {
                        bridge.spend("spend_deny", ["request": sheet.request])
                    } label: {
                        HStack { Spacer(); Text("Deny"); Spacer() }
                    }
                    .disabled(busy)
                    .accessibilityIdentifier("spend-deny")
                }
                Section {
                    Button("Stop payment requests from \(sheet.computer)", role: .destructive) {
                        confirmingBlock = true
                    }
                    .disabled(busy)
                }
            }
            .navigationTitle("Payment request")
            .navigationBarTitleDisplayMode(.inline)
            .confirmationDialog("Stop \(sheet.computer)'s payment requests?", isPresented: $confirmingBlock,
                                titleVisibility: .visible) {
                Button("Stop requests", role: .destructive) {
                    bridge.spend("spend_block", ["host": sheet.host])
                }
            } message: {
                Text("Its waiting requests are refused. You can allow it again from the Wallet tab.")
            }
        }
        .interactiveDismissDisabled()
        .preferredColorScheme(.dark)
    }

    private func row(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(value).multilineTextAlignment(.trailing).textSelection(.enabled)
        }
    }

    /// Trusting a payee widens what pays without a tap, so it always asks
    /// for Face ID or the passcode first.
    private func approveAndTrust() {
        failure = nil
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            failure = "Set a passcode on this phone to trust a payee."
            return
        }
        let request = sheet.request
        context.evaluatePolicy(.deviceOwnerAuthentication,
                               localizedReason: "Pay \(sheet.amount) and trust this payee") { success, _ in
            DispatchQueue.main.async {
                if success {
                    bridge.spend("spend_approve_trust", ["request": request])
                } else {
                    failure = "Not approved. Nothing was paid."
                }
            }
        }
    }

    /// Face ID or the passcode first when Rust asks for it; then the tap.
    private func approve() {
        failure = nil
        guard sheet.authenticate else {
            bridge.spend("spend_approve", ["request": sheet.request])
            return
        }
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            failure = "Set a passcode on this phone to approve larger payments."
            return
        }
        let request = sheet.request
        context.evaluatePolicy(.deviceOwnerAuthentication,
                               localizedReason: "Pay \(sheet.amount) for \(sheet.computer)") { success, _ in
            DispatchQueue.main.async {
                if success {
                    bridge.spend("spend_approve", ["request": request])
                } else {
                    failure = "Not approved. Nothing was paid."
                }
            }
        }
    }
}

/// The Wallet tab's list of agent payments and the computers that may ask.
struct AgentPaymentsSection: View {
    let spend: SpendState
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        if !spend.computers.isEmpty || !spend.history.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Text("Agent payments").font(.headline)
                if let wakes = bridge.pushStatus {
                    Text(wakes).font(.caption).foregroundStyle(.secondary)
                        .accessibilityIdentifier("spend-wakes")
                }
                if let notice = spend.notice {
                    HStack {
                        Text(notice).font(.footnote)
                        Spacer()
                        Button("OK") { bridge.spend("spend_dismiss") }.font(.footnote)
                    }
                }
                ForEach(spend.computers) { computer in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            VStack(alignment: .leading) {
                                Text(computer.computer).font(.subheadline)
                                Text(computer.remaining).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if computer.blocked {
                                Button("Allow") { bridge.spend("spend_allow", ["host": computer.host]) }
                                    .font(.footnote)
                            }
                        }
                        if let automatic = computer.automatic {
                            Text(automatic).font(.caption).foregroundStyle(.secondary)
                            ForEach(computer.trusted) { payee in
                                HStack {
                                    Text("\(payee.label) · \(payee.limit)").font(.caption.monospaced())
                                    Spacer()
                                    Button("Remove") {
                                        bridge.spend("spend_untrust", ["host": computer.host, "payee": payee.payee])
                                    }
                                    .font(.caption)
                                }
                            }
                            Button("Ask me for every payment") {
                                bridge.spend("spend_manual", ["host": computer.host])
                            }
                            .font(.caption)
                            .accessibilityIdentifier("spend-manual")
                        }
                    }
                }
                ForEach(spend.history) { entry in
                    HStack(alignment: .firstTextBaseline) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(entry.title ?? entry.purpose).font(.subheadline)
                            Text(entry.detail ?? "\(entry.computer) · \(entry.purpose)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 2) {
                            Text(entry.amount).font(.subheadline.monospacedDigit())
                            Text(entry.auto && entry.state == "paid" ? "Paid automatically" : entry.state.capitalized)
                                .font(.caption)
                                .foregroundStyle(entry.state == "paid" ? .green : .secondary)
                        }
                    }
                }
            }
            .padding()
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.white.opacity(0.06), in: RoundedRectangle(cornerRadius: 12))
        }
    }
}
