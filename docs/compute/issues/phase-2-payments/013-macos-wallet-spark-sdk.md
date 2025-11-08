# macOS Lightning Wallet with Spark SDK

**Phase:** 2 - Payments
**Component:** macOS App (OpenAgentsMac)
**Priority:** P0 (Critical - Required for compute provider payments)
**Estimated Effort:** 1-2 weeks
**Dependencies:** Issue #010 (iOS Wallet with Spark SDK - shares core implementation)

## Summary

Implement a Bitcoin/Lightning wallet for macOS using the Breez Spark SDK, sharing the core `SparkWalletManager` from iOS while providing macOS-specific UI and desktop-optimized features. The macOS wallet is critical for compute providers to receive payments for completed jobs.

## Motivation

macOS devices are the **compute providers** in the OpenAgents marketplace. They need a Lightning wallet to:

1. **Receive payments** for completed compute jobs (via BOLT11 invoices or Spark addresses)
2. **Hold balances** from multiple buyers over time
3. **Withdraw funds** to on-chain Bitcoin or external Lightning wallets
4. **Monitor transactions** while running in background (menu bar app)
5. **Always be available** for payments (desktop uptime >> mobile uptime)

Unlike iOS (coordination + wallet for buyers), macOS wallet is **provider-focused** and **always-on**.

## Breez Spark SDK Overview

**Spark is NOT Lightning** - it's a Layer 2 protocol using **statechain technology** with threshold signatures (FROST):

- **Self-custodial**: Users hold one key, Spark Operators collectively hold another
- **Nodeless**: No Lightning node management required
- **Pre-signed exits**: Timelocked transactions ensure users can always exit to L1
- **BOLT11 support**: Complete invoice generation/parsing (send/receive)
- **Offline receive**: Can receive payments while offline (address-based)
- **Cross-platform**: iOS 13.0+, macOS 15.0+, **same Swift SDK**

### Why Spark vs Manual Lightning

- ✅ **Production-ready**: Breez maintains the SDK, node infrastructure, LSP
- ✅ **No BOLT11 implementation**: SDK handles all Lightning protocol details
- ✅ **No channel management**: SDK manages liquidity automatically
- ✅ **Better UX**: Instant sends, offline receives, automatic backups
- ✅ **Reduced effort**: 1-2 weeks vs 4-5 weeks for manual implementation

## Acceptance Criteria

### Core Wallet Functionality

- [ ] **Initialize Spark SDK** on macOS with mnemonic from Keychain
- [ ] **Share SparkWalletManager** from iOS implementation (via OpenAgentsCore)
- [ ] **Connect to Breez Spark backend** (mainnet/testnet configurable)
- [ ] **Sync wallet state** on launch and periodically
- [ ] **Display current balance** in sats/BTC (live updates)
- [ ] **Generate receive addresses** (Spark addresses, BOLT11 invoices, BTC addresses)
- [ ] **Send payments** to BOLT11 invoices, Lightning addresses, BTC addresses
- [ ] **View transaction history** with timestamps, amounts, status
- [ ] **Handle payment events** (incoming, outgoing, confirmed, failed)
- [ ] **Graceful disconnection** on app quit (cleanup listeners)

### macOS-Specific UI

- [ ] **Menu bar integration** (status item with icon, balance, quick actions)
- [ ] **Native macOS window** (NSWindow with SwiftUI content)
- [ ] **Wallet dashboard** (balance card, recent transactions, quick send/receive)
- [ ] **Transaction list** (sortable, filterable by type/status/date)
- [ ] **Send payment sheet** (paste invoice, amount entry, confirm)
- [ ] **Receive payment sheet** (generate address, display QR code, copy)
- [ ] **Settings panel** (network selection, API key, backup/restore)
- [ ] **Notifications** (payment received, payment sent, errors)

### Desktop-Optimized Features

- [ ] **Background operation** (wallet syncs while app in background)
- [ ] **Menu bar-only mode** (hide main window, all features via menu)
- [ ] **System notifications** for incoming payments
- [ ] **Quick Actions** (Command-N for new receive, Command-S for send)
- [ ] **Export transaction CSV** for accounting
- [ ] **Copy/paste optimization** (detect invoice in pasteboard, auto-fill)

### Integration with Worker

- [ ] **Worker can query balance** before accepting bids (ensure liquidity)
- [ ] **Worker can generate invoices** for job payments
- [ ] **Worker receives payment notifications** (update job status to "paid")
- [ ] **Automatic reconciliation** (match invoices to job IDs via metadata)

### Security & Privacy

- [ ] **Mnemonic stored in Keychain** (kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
- [ ] **No mnemonic in logs** or error messages
- [ ] **API key stored securely** (Keychain or environment variable)
- [ ] **Transaction privacy** (Spark provides privacy via address reuse limits)
- [ ] **No custody by OpenAgents** (user holds keys)

### Testing

- [ ] **Unit tests** for WalletManager (shared from iOS)
- [ ] **macOS UI tests** for wallet windows and menu bar
- [ ] **Integration tests** with testnet Spark backend
- [ ] **Test invoice generation** and parsing (BOLT11, Spark addresses)
- [ ] **Test payment sending** (mock SDK, verify call patterns)
- [ ] **Test event handling** (incoming payment triggers notification)
- [ ] **Test error scenarios** (network failure, insufficient balance)

### Apple Compliance

- [ ] **Organization Developer Account** required (ASRG 3.1.5(i) for crypto wallets)
- [ ] **No background mining** (wallet is coordination, not compute)
- [ ] **Privacy manifest** if SDK uses sensitive APIs
- [ ] **Keychain entitlements** configured correctly

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    macOS App                            │
│                                                         │
│  ┌─────────────────┐         ┌─────────────────┐      │
│  │  WalletWindow   │         │  Menu Bar App   │      │
│  │   (SwiftUI)     │         │  (StatusItem)   │      │
│  └────────┬────────┘         └────────┬────────┘      │
│           │                           │                │
│           └───────────┬───────────────┘                │
│                       │                                │
│              ┌────────▼────────┐                       │
│              │ WalletCoordinator│ (macOS-specific)     │
│              └────────┬────────┘                       │
└───────────────────────┼─────────────────────────────────┘
                        │
         ┌──────────────▼──────────────┐
         │   OpenAgentsCore (Shared)   │
         │                             │
         │  ┌──────────────────────┐  │
         │  │ SparkWalletManager   │  │ (from iOS #010)
         │  │      (Actor)         │  │
         │  └──────────┬───────────┘  │
         │             │               │
         │  ┌──────────▼───────────┐  │
         │  │    SeedManager       │  │
         │  │  (Keychain storage)  │  │
         │  └──────────────────────┘  │
         └─────────────┬───────────────┘
                       │
              ┌────────▼────────┐
              │  BreezSdkSpark  │ (Swift package)
              └────────┬────────┘
                       │
              ┌────────▼────────┐
              │  Spark Operators │ (Breez backend)
              │  (mainnet/testnet)│
              └──────────────────┘
```

### Shared Core (from iOS #010)

The **SparkWalletManager** is implemented in OpenAgentsCore and shared between iOS and macOS:

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkWalletManager.swift

import Foundation
import BreezSdkSpark

@MainActor
public class SparkWalletManager: ObservableObject {
    // MARK: - Published State
    @Published public private(set) var isConnected = false
    @Published public private(set) var balanceSats: UInt64 = 0
    @Published public private(set) var pendingReceiveSats: UInt64 = 0
    @Published public private(set) var pendingSendSats: UInt64 = 0
    @Published public private(set) var transactions: [Transaction] = []
    @Published public private(set) var lastSyncTimestamp: Date?

    // MARK: - Private State
    private var sdk: BreezSdk?
    private var eventListenerId: String?
    private let seedManager: SeedManager
    private let apiKey: String
    private let network: Network
    private let storageDirectory: String

    // MARK: - Initialization
    public init(
        apiKey: String,
        network: Network = .mainnet,
        storageDirectory: String
    ) {
        self.apiKey = apiKey
        self.network = network
        self.storageDirectory = storageDirectory
        self.seedManager = SeedManager()
    }

    // MARK: - Public API

    /// Initialize SDK and connect to Spark backend
    public func initialize() async throws {
        // Load mnemonic from Keychain
        let mnemonic: String
        if await seedManager.hasMnemonic() {
            mnemonic = try await seedManager.loadMnemonic()
        } else {
            // Generate new mnemonic
            mnemonic = try await seedManager.generateAndSaveMnemonic()
        }

        // Configure SDK
        let seed = Seed.mnemonic(mnemonic: mnemonic, passphrase: nil)
        var config = defaultConfig(network: network)
        config.apiKey = apiKey

        // Connect
        self.sdk = try await connect(request: ConnectRequest(
            config: config,
            seed: seed,
            storageDir: storageDirectory
        ))

        // Register event listener
        let listener = WalletEventListener(manager: self)
        self.eventListenerId = await sdk?.addEventListener(listener: listener)

        self.isConnected = true

        // Sync state
        try await sync()
    }

    /// Sync wallet state with Spark backend
    public func sync() async throws {
        guard let sdk = sdk else { throw WalletError.notInitialized }

        let info = try await sdk.getInfo()
        self.balanceSats = info.balanceSats
        self.pendingReceiveSats = info.pendingReceiveSats
        self.pendingSendSats = info.pendingSendSats

        self.transactions = try await sdk.listPayments(request: ListPaymentsRequest(
            filters: [],
            fromTimestamp: nil,
            toTimestamp: nil,
            limit: 100,
            offset: 0
        ))

        self.lastSyncTimestamp = Date()
    }

    /// Send payment to BOLT11 invoice
    public func sendPayment(invoice: String) async throws -> Payment {
        guard let sdk = sdk else { throw WalletError.notInitialized }

        // Prepare payment (validate invoice, check balance)
        let prepareResponse = try await sdk.prepareSendPayment(
            request: PrepareSendPaymentRequest(
                paymentRequest: invoice,
                amount: nil  // Use invoice amount
            ))

        // Validate
        guard balanceSats >= prepareResponse.feesSat + prepareResponse.amountSat else {
            throw WalletError.insufficientBalance
        }

        // Send
        let sendResponse = try await sdk.sendPayment(
            request: SendPaymentRequest(
                prepareResponse: prepareResponse,
                options: nil
            ))

        // Update state
        try await sync()

        return sendResponse.payment
    }

    /// Generate receive address (Spark address)
    public func generateReceiveAddress() async throws -> String {
        guard let sdk = sdk else { throw WalletError.notInitialized }

        let response = try await sdk.receivePayment(
            request: ReceivePaymentRequest(
                amountSats: nil,  // Open amount
                description: "OpenAgents compute payment"
            ))

        return response.destination  // Spark address
    }

    /// Generate BOLT11 invoice for specific amount
    public func generateInvoice(amountSats: UInt64, description: String) async throws -> String {
        guard let sdk = sdk else { throw WalletError.notInitialized }

        let response = try await sdk.receivePayment(
            request: ReceivePaymentRequest(
                amountSats: amountSats,
                description: description
            ))

        // Extract BOLT11 invoice from response
        return response.invoiceDetails?.bolt11Invoice ?? response.destination
    }

    /// Disconnect from Spark backend
    public func disconnect() async throws {
        guard let sdk = sdk else { return }

        if let listenerId = eventListenerId {
            await sdk.removeEventListener(id: listenerId)
        }

        try await sdk.disconnect()

        self.sdk = nil
        self.isConnected = false
    }
}

// MARK: - Event Listener

public class WalletEventListener: EventListener {
    weak var manager: SparkWalletManager?

    init(manager: SparkWalletManager) {
        self.manager = manager
    }

    public func onEvent(event: BreezEvent) {
        Task { @MainActor in
            switch event {
            case .paymentSucceeded(let details):
                // Refresh balance and transactions
                try? await manager?.sync()

            case .paymentFailed(let details):
                // Handle failure (log, notify user)
                print("Payment failed: \(details.error)")

            case .synced:
                try? await manager?.sync()

            default:
                break
            }
        }
    }
}

// MARK: - Errors

public enum WalletError: LocalizedError {
    case notInitialized
    case insufficientBalance
    case invalidInvoice
    case paymentFailed(String)

    public var errorDescription: String? {
        switch self {
        case .notInitialized: return "Wallet not initialized"
        case .insufficientBalance: return "Insufficient balance"
        case .invalidInvoice: return "Invalid invoice"
        case .paymentFailed(let msg): return "Payment failed: \(msg)"
        }
    }
}
```

### macOS-Specific Coordinator

```swift
// OpenAgentsMac/Wallet/WalletCoordinator.swift

import AppKit
import SwiftUI
import OpenAgentsCore

@MainActor
class WalletCoordinator: ObservableObject {
    // Shared wallet manager
    let walletManager: SparkWalletManager

    // macOS-specific state
    @Published var showMainWindow = true
    @Published var menuBarOnly = false

    // Status item (menu bar)
    private var statusItem: NSStatusItem?

    init() {
        // Get API key from environment or Keychain
        let apiKey = ProcessInfo.processInfo.environment["BREEZ_API_KEY"] ?? ""

        // Storage directory
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let walletDir = appSupport.appendingPathComponent("OpenAgents/wallet")
        try? FileManager.default.createDirectory(at: walletDir, withIntermediateDirectories: true)

        self.walletManager = SparkWalletManager(
            apiKey: apiKey,
            network: .mainnet,
            storageDirectory: walletDir.path
        )
    }

    func start() async {
        // Initialize wallet
        do {
            try await walletManager.initialize()

            // Setup menu bar
            setupMenuBar()

            // Start periodic sync (every 30 seconds)
            startPeriodicSync()

        } catch {
            print("Failed to initialize wallet: \(error)")
        }
    }

    func setupMenuBar() {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem?.button {
            button.image = NSImage(systemSymbolName: "bitcoinsign.circle", accessibilityDescription: "Bitcoin Wallet")
            button.action = #selector(menuBarClicked)
            button.target = self
        }

        updateMenuBarTitle()
    }

    @objc func menuBarClicked() {
        let menu = NSMenu()

        // Balance
        let balanceItem = NSMenuItem(
            title: "Balance: \(formatSats(walletManager.balanceSats)) sats",
            action: nil,
            keyEquivalent: ""
        )
        balanceItem.isEnabled = false
        menu.addItem(balanceItem)

        menu.addItem(NSMenuItem.separator())

        // Quick actions
        menu.addItem(NSMenuItem(title: "Receive Payment...", action: #selector(showReceive), keyEquivalent: ""))
        menu.addItem(NSMenuItem(title: "Send Payment...", action: #selector(showSend), keyEquivalent: ""))

        menu.addItem(NSMenuItem.separator())

        // Window toggle
        let windowTitle = showMainWindow ? "Hide Wallet Window" : "Show Wallet Window"
        menu.addItem(NSMenuItem(title: windowTitle, action: #selector(toggleWindow), keyEquivalent: ""))

        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))

        statusItem?.menu = menu
        statusItem?.button?.performClick(nil)
        statusItem?.menu = nil
    }

    @objc func showReceive() {
        // Show receive sheet
        NotificationCenter.default.post(name: .showReceiveSheet, object: nil)
    }

    @objc func showSend() {
        // Show send sheet
        NotificationCenter.default.post(name: .showSendSheet, object: nil)
    }

    @objc func toggleWindow() {
        showMainWindow.toggle()
    }

    func updateMenuBarTitle() {
        Task { @MainActor in
            if let button = statusItem?.button {
                button.title = "\(formatSats(walletManager.balanceSats))"
            }
        }
    }

    func startPeriodicSync() {
        Task {
            while true {
                try? await Task.sleep(nanoseconds: 30_000_000_000)  // 30 seconds
                try? await walletManager.sync()
                updateMenuBarTitle()
            }
        }
    }

    func formatSats(_ sats: UInt64) -> String {
        if sats >= 100_000_000 {
            let btc = Double(sats) / 100_000_000.0
            return String(format: "%.3f BTC", btc)
        } else if sats >= 1000 {
            let k = Double(sats) / 1000.0
            return String(format: "%.1fk", k)
        } else {
            return "\(sats)"
        }
    }
}

extension Notification.Name {
    static let showReceiveSheet = Notification.Name("showReceiveSheet")
    static let showSendSheet = Notification.Name("showSendSheet")
}
```

### macOS Main Window

```swift
// OpenAgentsMac/Wallet/WalletWindow.swift

import SwiftUI
import OpenAgentsCore

struct WalletWindow: View {
    @EnvironmentObject var coordinator: WalletCoordinator
    @StateObject var walletManager: SparkWalletManager

    @State private var showingSendSheet = false
    @State private var showingReceiveSheet = false
    @State private var showingSettings = false

    var body: some View {
        NavigationSplitView {
            // Sidebar
            List {
                Section("Wallet") {
                    NavigationLink("Dashboard") {
                        DashboardView()
                    }
                    NavigationLink("Transactions") {
                        TransactionListView()
                    }
                }

                Section("Actions") {
                    Button(action: { showingReceiveSheet = true }) {
                        Label("Receive", systemImage: "arrow.down.circle")
                    }
                    Button(action: { showingSendSheet = true }) {
                        Label("Send", systemImage: "arrow.up.circle")
                    }
                }
            }
            .navigationTitle("Bitcoin Wallet")
            .toolbar {
                ToolbarItem {
                    Button(action: { showingSettings = true }) {
                        Image(systemName: "gear")
                    }
                }
            }
        } detail: {
            DashboardView()
        }
        .frame(minWidth: 800, minHeight: 600)
        .sheet(isPresented: $showingSendSheet) {
            SendPaymentSheet()
                .environmentObject(coordinator)
        }
        .sheet(isPresented: $showingReceiveSheet) {
            ReceivePaymentSheet()
                .environmentObject(coordinator)
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView()
        }
        .onReceive(NotificationCenter.default.publisher(for: .showReceiveSheet)) { _ in
            showingReceiveSheet = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .showSendSheet)) { _ in
            showingSendSheet = true
        }
    }
}

struct DashboardView: View {
    @EnvironmentObject var coordinator: WalletCoordinator

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                // Balance card
                BalanceCard(walletManager: coordinator.walletManager)
                    .padding()

                // Recent transactions
                VStack(alignment: .leading, spacing: 12) {
                    Text("Recent Activity")
                        .font(.headline)
                        .padding(.horizontal)

                    ForEach(coordinator.walletManager.transactions.prefix(10)) { tx in
                        TransactionRow(transaction: tx)
                            .padding(.horizontal)
                    }
                }
            }
            .padding()
        }
        .navigationTitle("Dashboard")
    }
}

struct BalanceCard: View {
    @ObservedObject var walletManager: SparkWalletManager

    var body: some View {
        VStack(spacing: 16) {
            HStack {
                Image(systemName: "bitcoinsign.circle.fill")
                    .font(.largeTitle)
                    .foregroundStyle(.orange)

                Spacer()

                if let lastSync = walletManager.lastSyncTimestamp {
                    Text("Last sync: \(lastSync, style: .relative)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Available Balance")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Text("\(walletManager.balanceSats) sats")
                    .font(.system(size: 36, weight: .bold, design: .rounded))

                Text("≈ \(formatBTC(walletManager.balanceSats)) BTC")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            if walletManager.pendingReceiveSats > 0 {
                HStack {
                    Image(systemName: "arrow.down.circle.fill")
                        .foregroundStyle(.green)
                    Text("Pending receive: \(walletManager.pendingReceiveSats) sats")
                        .font(.caption)
                }
            }
        }
        .padding()
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    func formatBTC(_ sats: UInt64) -> String {
        let btc = Double(sats) / 100_000_000.0
        return String(format: "%.8f", btc)
    }
}

struct TransactionRow: View {
    let transaction: Transaction

    var body: some View {
        HStack {
            Image(systemName: transaction.paymentType == .received ? "arrow.down.circle.fill" : "arrow.up.circle.fill")
                .foregroundStyle(transaction.paymentType == .received ? .green : .orange)

            VStack(alignment: .leading, spacing: 4) {
                Text(transaction.description ?? "Payment")
                    .font(.body)

                Text(transaction.timestamp, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Text("\(transaction.paymentType == .received ? "+" : "-")\(transaction.amountSats) sats")
                .font(.body.monospacedDigit())
                .foregroundStyle(transaction.paymentType == .received ? .green : .primary)
        }
        .padding()
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
    }
}
```

### Send/Receive Sheets

```swift
// OpenAgentsMac/Wallet/SendPaymentSheet.swift

import SwiftUI
import OpenAgentsCore

struct SendPaymentSheet: View {
    @EnvironmentObject var coordinator: WalletCoordinator
    @Environment(\.dismiss) var dismiss

    @State private var invoice = ""
    @State private var sending = false
    @State private var error: String?
    @State private var success = false

    var body: some View {
        VStack(spacing: 24) {
            Text("Send Payment")
                .font(.title)

            TextField("Paste BOLT11 invoice or Lightning address", text: $invoice)
                .textFieldStyle(.roundedBorder)
                .font(.body.monospaced())

            if let error = error {
                Text(error)
                    .foregroundStyle(.red)
                    .font(.caption)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)

                Button("Send") {
                    Task {
                        await sendPayment()
                    }
                }
                .disabled(invoice.isEmpty || sending)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding()
        .frame(width: 500, height: 250)
    }

    func sendPayment() async {
        sending = true
        error = nil

        do {
            let payment = try await coordinator.walletManager.sendPayment(invoice: invoice)
            success = true

            // Show success notification
            let notification = NSUserNotification()
            notification.title = "Payment Sent"
            notification.informativeText = "\(payment.amountSats) sats"
            NSUserNotificationCenter.default.deliver(notification)

            dismiss()
        } catch {
            self.error = error.localizedDescription
        }

        sending = false
    }
}

// OpenAgentsMac/Wallet/ReceivePaymentSheet.swift

struct ReceivePaymentSheet: View {
    @EnvironmentObject var coordinator: WalletCoordinator
    @Environment(\.dismiss) var dismiss

    @State private var amountSats: String = ""
    @State private var description: String = "OpenAgents compute payment"
    @State private var invoice: String?
    @State private var generating = false
    @State private var error: String?

    var body: some View {
        VStack(spacing: 24) {
            Text("Receive Payment")
                .font(.title)

            if let invoice = invoice {
                // Show generated invoice
                VStack(spacing: 16) {
                    // QR code would go here
                    Text("Invoice generated!")
                        .font(.headline)

                    Text(invoice)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                        .padding()
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))

                    Button("Copy to Clipboard") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(invoice, forType: .string)
                    }
                }
            } else {
                // Input form
                VStack(spacing: 16) {
                    TextField("Amount (sats, leave empty for open amount)", text: $amountSats)
                        .textFieldStyle(.roundedBorder)

                    TextField("Description", text: $description)
                        .textFieldStyle(.roundedBorder)
                }

                if let error = error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.caption)
                }

                HStack {
                    Button("Cancel") {
                        dismiss()
                    }
                    .keyboardShortcut(.cancelAction)

                    Button("Generate Invoice") {
                        Task {
                            await generateInvoice()
                        }
                    }
                    .disabled(generating)
                    .keyboardShortcut(.defaultAction)
                }
            }
        }
        .padding()
        .frame(width: 500, height: 350)
    }

    func generateInvoice() async {
        generating = true
        error = nil

        do {
            let amount = amountSats.isEmpty ? nil : UInt64(amountSats)

            let generatedInvoice: String
            if let amount = amount {
                generatedInvoice = try await coordinator.walletManager.generateInvoice(
                    amountSats: amount,
                    description: description
                )
            } else {
                generatedInvoice = try await coordinator.walletManager.generateReceiveAddress()
            }

            self.invoice = generatedInvoice
        } catch {
            self.error = error.localizedDescription
        }

        generating = false
    }
}
```

## Integration with Worker

The worker needs to interact with the wallet to receive payments:

```swift
// OpenAgentsMac/Worker/WorkerPaymentHandler.swift

import Foundation
import OpenAgentsCore

actor WorkerPaymentHandler {
    let walletManager: SparkWalletManager
    let nostrClient: NostrClient

    // Map job IDs to invoice IDs
    private var jobInvoiceMap: [String: String] = [:]

    func handleJobAccepted(job: NostrJob) async throws -> String {
        // Generate invoice for job payment
        let invoice = try await walletManager.generateInvoice(
            amountSats: job.amount,
            description: "OpenAgents job \(job.id.prefix(8))"
        )

        // Store mapping
        jobInvoiceMap[job.id] = invoice

        // Publish NIP-90 feedback event with invoice
        let feedbackEvent = NostrEvent(
            kind: 7000,  // Job feedback
            content: "Invoice: \(invoice)",
            tags: [
                ["e", job.id],  // Reference original job
                ["status", "payment-required"],
                ["amount", "\(job.amount)"]
            ]
        )

        try await nostrClient.publish(event: feedbackEvent)

        return invoice
    }

    func checkPaymentReceived(jobId: String) async throws -> Bool {
        guard let invoice = jobInvoiceMap[jobId] else {
            return false
        }

        // Check if invoice is paid
        // (Spark SDK provides payment status via events)
        let transactions = walletManager.transactions
        return transactions.contains { tx in
            tx.details?.invoiceId == invoice && tx.status == .complete
        }
    }
}
```

## Dependencies

### SwiftPM Package

```swift
// Package.swift (for OpenAgentsCore)

dependencies: [
    .package(url: "https://github.com/breez/breez-sdk-spark-swift", from: "0.1.0")
],
targets: [
    .target(
        name: "OpenAgentsCore",
        dependencies: [
            .product(name: "BreezSdkSpark", package: "breez-sdk-spark-swift")
        ]
    )
]
```

### OpenAgents Issues

- **Issue #002**: Secp256k1 & Cryptography (BIP39 mnemonic generation)
- **Issue #010**: iOS Wallet with Spark SDK (shares SparkWalletManager)
- **Issue #007**: macOS Foundation Models Worker (receives payments)

### External

- Breez Spark SDK: https://sdk-doc-spark.breez.technology/
- Breez API Key: Required (free tier available)

## Testing

### Unit Tests

```swift
// OpenAgentsCoreTests/Lightning/SparkWalletManagerTests.swift

import XCTest
@testable import OpenAgentsCore

class SparkWalletManagerTests: XCTestCase {
    // (Same tests as iOS #010 - shared implementation)

    func testInitializeWithExistingMnemonic() async throws {
        let manager = createTestManager()

        try await manager.initialize()

        XCTAssertTrue(manager.isConnected)
        XCTAssertNotNil(manager.lastSyncTimestamp)
    }

    func testGenerateInvoice() async throws {
        let manager = createTestManager()
        try await manager.initialize()

        let invoice = try await manager.generateInvoice(
            amountSats: 1000,
            description: "Test payment"
        )

        XCTAssertTrue(invoice.hasPrefix("lnbc"))
    }
}
```

### macOS UI Tests

```swift
// OpenAgentsMacTests/Wallet/WalletWindowTests.swift

import XCTest

class WalletWindowTests: XCTestCase {
    func testMenuBarAppears() throws {
        let app = XCUIApplication()
        app.launch()

        // Check menu bar item exists
        let menuBar = app.menuBars
        XCTAssertTrue(menuBar.statusItems["Bitcoin Wallet"].exists)
    }

    func testOpenReceiveSheet() throws {
        let app = XCUIApplication()
        app.launch()

        // Click menu bar
        app.menuBars.statusItems["Bitcoin Wallet"].click()

        // Click "Receive Payment..."
        app.menuItems["Receive Payment..."].click()

        // Verify sheet appears
        XCTAssertTrue(app.sheets["Receive Payment"].exists)
    }
}
```

### Integration Tests

```swift
func testWorkerGeneratesInvoiceForJob() async throws {
    let coordinator = WalletCoordinator()
    await coordinator.start()

    let handler = WorkerPaymentHandler(
        walletManager: coordinator.walletManager,
        nostrClient: mockNostrClient
    )

    let job = NostrJob(id: "test-job", amount: 500, kind: .summarization)

    let invoice = try await handler.handleJobAccepted(job: job)

    XCTAssertTrue(invoice.hasPrefix("lnbc"))
    XCTAssertTrue(invoice.contains("500"))  // Amount encoded
}
```

## Success Metrics

- [ ] macOS app launches with wallet connected to Spark backend
- [ ] Menu bar shows current balance (updated every 30 seconds)
- [ ] User can generate receive invoices via menu bar or main window
- [ ] User can send payments by pasting BOLT11 invoice
- [ ] Transaction history displays all sent/received payments
- [ ] Worker can generate invoices for accepted jobs
- [ ] Worker receives notification when payment received
- [ ] Settings allow network switching (mainnet/testnet)
- [ ] No mnemonic leaks in logs or error messages
- [ ] Menu bar-only mode works (can hide main window entirely)

## Apple Compliance

### ASRG 3.1.5(i) - Cryptocurrency Wallets

✅ **Allowed**: "Apps may facilitate transactions or transmissions of cryptocurrency on an approved exchange, provided they are offered by the exchange itself."

**Mitigation**:
- Requires **Organization Developer Account** (not Individual)
- App Store submission may require additional review
- Clearly state "self-custodial" in app description
- No custody of user funds by OpenAgents

### ASRG 2.4.2 - Background Processing

✅ **Compliant**: Wallet sync is not "mining" or "compute work"
- Wallet syncs periodically (low CPU usage)
- No background job execution (worker is separate concern)
- macOS allows menu bar apps to run continuously

### Privacy

✅ **Privacy manifest** (if SDK uses network APIs):
- Declare Breez API usage
- Keys stored locally (Keychain)
- No data collection by OpenAgents

## Future Enhancements

- [ ] **Advanced Lightning features**: Channel management, watchtower
- [ ] **Multi-wallet support**: Separate wallets for personal/business
- [ ] **Fiat conversion**: Display balance in USD/EUR
- [ ] **CSV export**: Transaction history for accounting
- [ ] **Hardware wallet support**: Trezor/Ledger integration
- [ ] **Submarine swaps**: BTC on-chain ↔ Lightning

## Notes

- **Same core as iOS**: SparkWalletManager is shared, only UI differs
- **Menu bar optimized**: macOS users expect menu bar apps for utilities
- **Always-on ready**: Desktop uptime allows 24/7 payment receipt
- **Notification system**: macOS notifications for incoming payments
- **Worker integration**: Wallet is tightly coupled with worker for payment reconciliation

## Reference

- **Breez Spark SDK**: https://sdk-doc-spark.breez.technology/
- **Swift Bindings**: https://github.com/breez/breez-sdk-spark-swift
- **Issue #010**: iOS Wallet with Spark SDK (shared core)
- **Issue #007**: macOS Foundation Models Worker (payment recipient)
- **ASRG 3.1.5(i)**: https://developer.apple.com/app-store/review/guidelines/#cryptocurrency
