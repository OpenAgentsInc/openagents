// A computer's ask for the owner's wallet (`openagents wallet link`). Rust
// reads the ask from the computer and derives the code the computer shows;
// this sheet names the computer and the code, asks for Face ID or the
// passcode, and sends the owner's tap. Rust seals the wallet to the
// computer's one-time key; the seed never passes through Swift.
import LocalAuthentication
import SwiftUI

/// Rust's `wallet_link::View`.
struct WalletLinkState: Decodable, Equatable {
    struct Sheet: Decodable, Equatable, Identifiable {
        let id: String
        let host: String
        let computer: String
        let code: String
    }
    let sheet: Sheet?
    let busy: Bool
    let notice: String?
}

/// The approval sheet for the oldest waiting ask.
struct WalletLinkSheet: View {
    let sheet: WalletLinkState.Sheet
    let busy: Bool
    @ObservedObject var bridge: MobileBridge
    @State private var failure: String?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(spacing: 6) {
                        Text(sheet.code)
                            .font(.paper(40, weight: .semibold))
                            .accessibilityIdentifier("wallet-link-code")
                        Text("Shown on \(sheet.computer)")
                            .font(.paper(.subheadline))
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                } footer: {
                    Text("Approve only if \(sheet.computer) shows the code \(sheet.code). It will hold your wallet and can spend from it, like this phone.")
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
                            if busy { ProgressView() } else { Text("Approve").font(.paper(.body, weight: .bold)) }
                            Spacer()
                        }
                    }
                    .disabled(busy)
                    .accessibilityIdentifier("wallet-link-approve")
                    Button(role: .destructive) {
                        bridge.wallet("wallet_link_deny", ["host": sheet.host, "id": sheet.id])
                    } label: {
                        HStack { Spacer(); Text("Deny"); Spacer() }
                    }
                    .disabled(busy)
                    .accessibilityIdentifier("wallet-link-deny")
                }
            }
            .navigationTitle("Use your wallet on \(sheet.computer)?")
            .navigationBarTitleDisplayMode(.inline)
        }
        .interactiveDismissDisabled()
        .preferredColorScheme(.dark)
    }

    /// Sending the wallet to a computer always asks for Face ID or the
    /// passcode first.
    private func approve() {
        failure = nil
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            failure = "Set a passcode on this phone to use your wallet on a computer."
            return
        }
        let host = sheet.host
        let id = sheet.id
        context.evaluatePolicy(.deviceOwnerAuthentication,
                               localizedReason: "Use your wallet on \(sheet.computer)") { success, _ in
            DispatchQueue.main.async {
                if success {
                    bridge.wallet("wallet_link_approve", ["host": host, "id": id])
                } else {
                    failure = "Not approved. Your wallet stayed on this phone."
                }
            }
        }
    }
}
