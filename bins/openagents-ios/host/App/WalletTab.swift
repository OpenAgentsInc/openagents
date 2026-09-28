// The Wallet tab: test bitcoin on Mutinynet signet. Rust runs the wallet and
// decides every state and line of text; this view lays them out and adds the
// clipboard and the faucet link.
import SwiftUI
import UIKit

/// Rust's Wallet screen (`wallet::Screen`): `failed`, or `ready` with the
/// summary fields. `ready` shows from launch: while the wallet starts it
/// carries the last balance read (or `balance_unknown`) and a `status`.
struct WalletState: Decodable, Equatable {
    let state: String
    let message: String?
    let network: String?
    let balance: String?
    let balance_btc: String?
    let pending: String?
    let empty: Bool?
    let address: String?
    let uri: String?
    let qr: ComputersQR?
    let faucet: String?
    let synced_at: UInt64?
    let refreshing: Bool?
    let error: String?
    let balance_unknown: Bool?
    let status: String?
}

struct WalletTab: View {
    @ObservedObject var bridge: MobileBridge
    @State private var copied = false

    private var wallet: WalletState? { bridge.packet?.wallet }
    private var loading: Bool { bridge.packet?.wallet_loading == true }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                Text("Wallet").font(.largeTitle.bold()).foregroundStyle(.white)
                content
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
        }
        .background(Color.black.ignoresSafeArea())
        .refreshable { bridge.refreshWallet() }
        .onAppear { bridge.openWallet() }
        // Starts and syncs finish in the background; poll quickly while one
        // runs, and slowly otherwise to pick up the node's own syncs.
        .task(id: loading) {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(loading ? 1 : 10))
                if !Task.isCancelled && !bridge.busy { bridge.snapshot() }
            }
        }
    }

    @ViewBuilder private var content: some View {
        switch wallet?.state {
        case "failed":
            VStack(alignment: .leading, spacing: 16) {
                Text(wallet?.message ?? "The wallet could not start.").foregroundStyle(.white)
                Button("Try again") { bridge.refreshWallet() }.buttonStyle(.bordered)
            }
        default:
            // Before Rust's first packet the screen still lays out the
            // wallet, with placeholders.
            ready(wallet ?? WalletState.opening)
        }
    }

    private func ready(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 6) {
                Text((wallet.network ?? "Test network").uppercased() + " · TEST COINS")
                    .font(.caption.weight(.semibold)).foregroundStyle(.gray)
                let unknown = wallet.balance_unknown == true
                Text(unknown ? "000,000 sats" : wallet.balance ?? "")
                    .font(.system(size: 44, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
                    .minimumScaleFactor(0.5).lineLimit(1)
                    .redacted(reason: unknown ? .placeholder : [])
                    .accessibilityIdentifier("wallet-balance")
                Text(unknown ? "0.00000000 tBTC" : wallet.balance_btc ?? "")
                    .font(.callout.monospacedDigit()).foregroundStyle(.gray)
                    .redacted(reason: unknown ? .placeholder : [])
                if let pending = wallet.pending {
                    Text(pending).font(.callout).foregroundStyle(.white)
                }
            }
            if wallet.empty == true {
                Text("No test coins yet. Get free coins from the Mutinynet faucet and send them to this address.")
                    .font(.callout).foregroundStyle(.gray)
            }
            VStack(alignment: .leading, spacing: 12) {
                Text("Receive").font(.headline).foregroundStyle(.white)
                let addressKnown = !(wallet.address ?? "").isEmpty
                if let qr = wallet.qr {
                    InvitationQR(qr: qr)
                        .frame(width: 200, height: 200)
                        .frame(maxWidth: .infinity)
                        .accessibilityLabel("Receive address QR code")
                } else {
                    RoundedRectangle(cornerRadius: 12)
                        .fill(Color.white.opacity(0.08))
                        .frame(width: 200, height: 200)
                        .frame(maxWidth: .infinity)
                        .accessibilityHidden(true)
                }
                Text(addressKnown ? wallet.address ?? "" : "tb1q0000000000000000000000000000000000000")
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(.white)
                    .textSelection(.enabled)
                    .redacted(reason: addressKnown ? [] : .placeholder)
                    .accessibilityIdentifier("wallet-address")
                HStack(spacing: 12) {
                    Button(copied ? "Copied" : "Copy address", systemImage: copied ? "checkmark" : "doc.on.doc") {
                        UIPasteboard.general.string = wallet.address
                        copied = true
                        Task { try? await Task.sleep(for: .seconds(2)); copied = false }
                    }
                    .buttonStyle(.bordered)
                    .disabled(!addressKnown)
                    if let faucet = wallet.faucet, let url = URL(string: faucet) {
                        Link(destination: url) { Label("Get test coins", systemImage: "drop") }
                            .buttonStyle(.bordered)
                    }
                }
            }
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
            if let error = wallet.error {
                Text(error).font(.footnote).foregroundStyle(.white)
            }
        }
    }

    private func updated(_ seconds: UInt64?) -> String {
        guard let seconds else { return "Not synced yet" }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        return "Updated " + date.formatted(.relative(presentation: .named))
    }
}

extension WalletState {
    /// The screen before Rust's first packet arrives.
    static let opening = WalletState(
        state: "ready", message: nil, network: "Mutinynet signet", balance: nil, balance_btc: nil,
        pending: nil, empty: nil, address: nil, uri: nil, qr: nil, faucet: nil, synced_at: nil,
        refreshing: true, error: nil, balance_unknown: true, status: "Opening the wallet…")
}
