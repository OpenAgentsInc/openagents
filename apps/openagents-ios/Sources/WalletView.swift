// Firm reason: 6 (iOS pure-black OLED SwiftUI Spark Bitcoin & Lightning Wallet + read-only NIP-06 Nostr identity sheet).
import CoreImage.CIFilterBuiltins
import SwiftUI
import UIKit

struct WalletView: View {
    @ObservedObject var wallet: MobileWalletController
    @Environment(\.dismiss) private var dismiss

    @State private var restoreDraft = ""
    @State private var showRestore = false
    @State private var sendDestination = ""
    @State private var sendAmountText = ""
    @State private var receiveAmountText = ""
    @State private var revealedPhrase: String?
    @State private var copiedText: String?

    var body: some View {
        NavigationStack {
            List {
                if let err = wallet.errorMessage, !err.isEmpty {
                    Section {
                        Text(err)
                            .font(.system(size: 13))
                            .foregroundStyle(OATheme.statusError)
                            .listRowBackground(OATheme.surface)
                    }
                }

                if !wallet.hasSeed {
                    onboardingSection
                } else {
                    balanceSection
                    if wallet.ready {
                        receiveSection
                        sendSection
                        historySection
                    }
                    backupAndNostrSection
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Spark Wallet")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("wallet.done")
                }
            }
        }
        .preferredColorScheme(.dark)
        .task {
            wallet.refreshKeychainAndNostr()
            if wallet.hasSeed && !wallet.ready {
                await wallet.connect()
            }
        }
    }

    @ViewBuilder
    private var onboardingSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 8) {
                Text("Self-Custodial Bitcoin & Lightning")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Text("Powered by Breez Spark mainnet. Your 12-word recovery phrase is stored exclusively in this device's Keychain and never leaves your device.")
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(.vertical, 6)
            .listRowBackground(OATheme.surface)

            Button {
                Task { await wallet.createNewWallet() }
            } label: {
                HStack {
                    if wallet.busy { ProgressView().controlSize(.small) }
                    Text("Create New Spark Wallet")
                        .font(.system(size: 15, weight: .semibold))
                }
                .frame(maxWidth: .infinity)
            }
            .disabled(wallet.busy)
            .accessibilityIdentifier("wallet.create")
            .listRowBackground(OATheme.elevated)

            Button(showRestore ? "Hide Restore Phrase" : "Restore from 12/24 Words") {
                showRestore.toggle()
            }
            .font(.system(size: 14, weight: .medium))
            .accessibilityIdentifier("wallet.restoreToggle")
            .listRowBackground(OATheme.surface)

            if showRestore {
                TextField("word1 word2 word3 …", text: $restoreDraft, axis: .vertical)
                    .font(.system(size: 13, design: .monospaced))
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .lineLimit(3...6)
                    .listRowBackground(OATheme.surface)

                Button {
                    Task { await wallet.restoreWallet(mnemonic: restoreDraft) }
                } label: {
                    Text("Restore Wallet")
                        .font(.system(size: 14, weight: .semibold))
                        .frame(maxWidth: .infinity)
                }
                .disabled(wallet.busy || restoreDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("wallet.restore")
                .listRowBackground(OATheme.elevated)
            }
        }
    }

    @ViewBuilder
    private var balanceSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 6) {
                Text(MobileWalletController.formatSats(wallet.balanceSats))
                    .font(.system(size: 28, weight: .bold, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                Text(MobileWalletController.formatBTC(wallet.balanceSats))
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(OATheme.textSecondary)
                Text(wallet.statusText)
                    .font(.system(size: 12))
                    .foregroundStyle(wallet.ready ? OATheme.statusReady : OATheme.textMuted)
            }
            .padding(.vertical, 6)
            .listRowBackground(OATheme.surface)

            Button {
                Task {
                    if wallet.ready {
                        await wallet.refreshBalanceAndHistory()
                    } else {
                        await wallet.connect()
                    }
                }
            } label: {
                HStack {
                    if wallet.busy { ProgressView().controlSize(.small) }
                    Text(wallet.ready ? "Sync Balance & Payments" : "Connect Spark Wallet")
                        .font(.system(size: 14, weight: .semibold))
                }
                .frame(maxWidth: .infinity)
            }
            .disabled(wallet.busy)
            .listRowBackground(OATheme.elevated)
        } header: {
            Text("Balance")
        }
    }

    @ViewBuilder
    private var receiveSection: some View {
        Section {
            if !wallet.sparkAddress.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Spark Address")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(OATheme.textSecondary)
                    Text(wallet.sparkAddress)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(2)
                    Button(copiedText == wallet.sparkAddress ? "Copied Spark Address" : "Copy Spark Address") {
                        copy(wallet.sparkAddress)
                    }
                    .font(.system(size: 12, weight: .semibold))
                }
                .listRowBackground(OATheme.surface)
            }

            HStack {
                TextField("Invoice amount (sats, optional)", text: $receiveAmountText)
                    .font(.system(size: 14, design: .monospaced))
                    .keyboardType(.numberPad)
                Button("Create Invoice") {
                    let amt = UInt64(receiveAmountText.trimmingCharacters(in: .whitespaces))
                    Task { await wallet.generateBolt11Invoice(amountSats: amt) }
                }
                .font(.system(size: 13, weight: .semibold))
                .disabled(wallet.busy)
            }
            .listRowBackground(OATheme.surface)

            if !wallet.bolt11Invoice.isEmpty {
                VStack(alignment: .center, spacing: 10) {
                    if let img = makeQRImage(from: wallet.bolt11Invoice) {
                        Image(uiImage: img)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 180, height: 180)
                            .padding(8)
                            .background(Color.white, in: RoundedRectangle(cornerRadius: 8))
                    }
                    Text(wallet.bolt11Invoice)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(OATheme.textSecondary)
                        .lineLimit(3)
                    Button(copiedText == wallet.bolt11Invoice ? "Copied Lightning Invoice" : "Copy Lightning Invoice") {
                        copy(wallet.bolt11Invoice)
                    }
                    .font(.system(size: 12, weight: .semibold))
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .listRowBackground(OATheme.surface)
            }
        } header: {
            Text("Receive")
        }
    }

    @ViewBuilder
    private var sendSection: some View {
        Section {
            TextField("BOLT11, Lightning address, Spark, or BTC", text: $sendDestination)
                .font(.system(size: 13, design: .monospaced))
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .listRowBackground(OATheme.surface)

            TextField("Amount (sats, if not in invoice)", text: $sendAmountText)
                .font(.system(size: 14, design: .monospaced))
                .keyboardType(.numberPad)
                .listRowBackground(OATheme.surface)

            if let quote = wallet.preparedQuote {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Quote Ready (\(quote.rail))")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(OATheme.statusReady)
                    Text("Amount: \(MobileWalletController.formatSats(quote.amountSats)) · Fee: \(MobileWalletController.formatSats(quote.feeSats))")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                    HStack(spacing: 12) {
                        Button("Confirm Send") {
                            Task { await wallet.confirmSend() }
                        }
                        .font(.system(size: 13, weight: .semibold))
                        .buttonStyle(.borderedProminent)
                        .tint(.white)
                        .foregroundStyle(Color.black)
                        .disabled(wallet.busy)

                        Button("Cancel") {
                            Task { await wallet.cancelPreparedSend() }
                        }
                        .font(.system(size: 13))
                    }
                }
                .padding(.vertical, 4)
                .listRowBackground(OATheme.surface)
            } else {
                Button("Review Payment") {
                    let amt = UInt64(sendAmountText.trimmingCharacters(in: .whitespaces))
                    Task { await wallet.prepareSend(destination: sendDestination, amountSats: amt) }
                }
                .font(.system(size: 14, weight: .semibold))
                .disabled(wallet.busy || sendDestination.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .listRowBackground(OATheme.elevated)
            }
        } header: {
            Text("Send (Daily Limit: \(MobileWalletController.formatSats(wallet.dailyRemainingSats)) left)")
        }
    }

    @ViewBuilder
    private var historySection: some View {
        Section {
            if wallet.payments.isEmpty {
                Text("No payments yet.")
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textMuted)
                    .listRowBackground(OATheme.surface)
            } else {
                ForEach(wallet.payments, id: \.id) { p in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(p.incoming ? "Received (\(p.rail))" : "Sent (\(p.rail))")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                            Text(p.status.rawValue)
                                .font(.system(size: 11))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        Spacer()
                        Text((p.incoming ? "+" : "-") + MobileWalletController.formatSats(p.amountSats))
                            .font(.system(size: 13, weight: .semibold, design: .monospaced))
                            .foregroundStyle(p.incoming ? OATheme.statusReady : OATheme.textPrimary)
                    }
                    .listRowBackground(OATheme.surface)
                }
            }
        } header: {
            Text("Recent Payments")
        }
    }

    @ViewBuilder
    private var backupAndNostrSection: some View {
        Section {
            if let phrase = revealedPhrase {
                Text(phrase)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .listRowBackground(OATheme.surface)
                Button("Hide Recovery Phrase") {
                    revealedPhrase = nil
                }
                .font(.system(size: 13))
                .listRowBackground(OATheme.surface)
                if !wallet.backupConfirmed {
                    Button("Mark Backup Confirmed") {
                        wallet.backupConfirmed = true
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .listRowBackground(OATheme.surface)
                }
            } else {
                Button("Reveal 12-Word Recovery Phrase") {
                    revealedPhrase = wallet.revealRecoveryPhrase()
                }
                .font(.system(size: 14, weight: .medium))
                .listRowBackground(OATheme.surface)
            }

            if !wallet.nostrNpub.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Nostr Identity (NIP-06 · Read-Only)")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(OATheme.textSecondary)
                    Text(wallet.nostrNpub)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(2)
                    Button(copiedText == wallet.nostrNpub ? "Copied npub" : "Copy npub") {
                        copy(wallet.nostrNpub)
                    }
                    .font(.system(size: 12, weight: .semibold))
                }
                .listRowBackground(OATheme.surface)
            }
        } header: {
            Text("Backup & Nostr Identity")
        }
    }

    private func copy(_ value: String) {
        UIPasteboard.general.string = value
        copiedText = value
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            if copiedText == value { copiedText = nil }
        }
    }

    private func makeQRImage(from text: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        let ctx = CIContext()
        guard let cg = ctx.createCGImage(scaled, from: scaled.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}
