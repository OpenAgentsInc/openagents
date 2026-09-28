// The Wallet tab: test bitcoin on Mutinynet signet. Rust runs the wallet and
// decides every state and line of text; this view lays them out and adds the
// clipboard and the faucet link.
import SwiftUI
import UIKit

/// Rust's Wallet screen (`wallet::Screen`): `closed`, `loading`, `failed`, or
/// `ready` with the summary fields.
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
        case "ready": if let wallet { ready(wallet) }
        case "failed":
            VStack(alignment: .leading, spacing: 16) {
                Text(wallet?.message ?? "The wallet could not start.").foregroundStyle(.white)
                Button("Try again") { bridge.refreshWallet() }.buttonStyle(.bordered)
            }
        default:
            HStack(spacing: 12) {
                ProgressView()
                Text(wallet?.message ?? "Opening the wallet…").foregroundStyle(.gray)
            }
            .accessibilityElement(children: .combine)
        }
    }

    private func ready(_ wallet: WalletState) -> some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 6) {
                Text((wallet.network ?? "Test network").uppercased() + " · TEST COINS")
                    .font(.caption.weight(.semibold)).foregroundStyle(.gray)
                Text(wallet.balance ?? "")
                    .font(.system(size: 44, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
                    .minimumScaleFactor(0.5).lineLimit(1)
                    .accessibilityIdentifier("wallet-balance")
                Text(wallet.balance_btc ?? "").font(.callout.monospacedDigit()).foregroundStyle(.gray)
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
                if let qr = wallet.qr {
                    InvitationQR(qr: qr)
                        .frame(width: 200, height: 200)
                        .frame(maxWidth: .infinity)
                        .accessibilityLabel("Receive address QR code")
                }
                Text(wallet.address ?? "")
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(.white)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("wallet-address")
                HStack(spacing: 12) {
                    Button(copied ? "Copied" : "Copy address", systemImage: copied ? "checkmark" : "doc.on.doc") {
                        UIPasteboard.general.string = wallet.address
                        copied = true
                        Task { try? await Task.sleep(for: .seconds(2)); copied = false }
                    }
                    .buttonStyle(.bordered)
                    if let faucet = wallet.faucet, let url = URL(string: faucet) {
                        Link(destination: url) { Label("Get test coins", systemImage: "drop") }
                            .buttonStyle(.bordered)
                    }
                }
            }
            HStack(spacing: 8) {
                if wallet.refreshing == true {
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
