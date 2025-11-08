# iOS Lightning Wallet UI with Spark SDK Integration

**Phase:** 2 - Payments
**Component:** iOS App + OpenAgentsCore
**Priority:** P0 (Critical - Enables buyer payments)
**Estimated Effort:** 2-3 weeks (reduced from 4-5 weeks with manual Lightning)

## Summary

Implement a self-custodial Lightning wallet on iOS using Breez Spark SDK as the core payment engine. Build SwiftUI-based UI for wallet management, payment sending/receiving, and transaction history. Integrate with the compute marketplace for seamless job payments.

## Motivation

Buyers need to pay for compute jobs using Lightning. Providers need to receive payments. Spark SDK provides:

- ✅ **Self-custodial**: Users control keys (Apple App Store compatible)
- ✅ **Nodeless**: No Lightning node management required
- ✅ **Cross-platform**: Same SDK for iOS + macOS
- ✅ **Production-ready**: Backed by Breez/Lightspark
- ✅ **Complete BOLT11 support**: No manual implementation needed
- ✅ **Lightning + On-chain**: Automatic fallback, offline receive

**vs. Manual Implementation:**
- ❌ Building from scratch: 4-5 weeks, complex, error-prone
- ✅ Using Spark SDK: 2-3 weeks, tested, maintained by Breez

## Acceptance Criteria

### Spark SDK Integration
- [ ] Add `BreezSdkSpark` Swift package dependency
- [ ] Obtain and securely store Breez API key
- [ ] Implement `SparkWalletManager` actor for SDK lifecycle
- [ ] Implement `SeedManager` for secure mnemonic storage (Keychain)
- [ ] Configure storage directory (app sandbox compliant)
- [ ] Initialize SDK on app launch
- [ ] Graceful shutdown on app termination

### Wallet Creation & Recovery
- [ ] Generate 12-word BIP39 mnemonic
- [ ] Display mnemonic with warnings ("WRITE THIS DOWN")
- [ ] Confirmation quiz before enabling wallet
- [ ] Import existing wallet from mnemonic
- [ ] Restore transaction history after import
- [ ] Encrypted iCloud backup (opt-in, password-protected)

### Payment Sending
- [ ] Parse BOLT11 invoices (via Spark SDK)
- [ ] Support Lightning addresses (user@domain.com)
- [ ] Support LNURL-pay
- [ ] Preview payment with fee breakdown
- [ ] Confirm payment dialog
- [ ] Execute payment via Spark SDK
- [ ] Track payment status (pending, succeeded, failed)
- [ ] Handle payment errors with retry options

### Payment Receiving
- [ ] Generate BOLT11 invoices (via Spark SDK)
- [ ] Amount input (or amountless invoice)
- [ ] Description input
- [ ] Display invoice as text + QR code
- [ ] Share invoice via system share sheet
- [ ] Wait for payment notification
- [ ] Display payment received confirmation

### Balance & Transaction History
- [ ] Display current balance (sats + fiat conversion)
- [ ] List all transactions (sent/received)
- [ ] Filter by type (sent, received, pending, failed)
- [ ] Transaction details view (amount, fee, timestamp, status)
- [ ] Export transaction history (CSV/JSON)
- [ ] Pull-to-refresh for latest transactions

### Event Handling
- [ ] Listen for Spark SDK events (PaymentSucceeded, PaymentFailed, Synced)
- [ ] Update UI on balance changes
- [ ] Show notifications for received payments
- [ ] Handle background sync events
- [ ] Recover from SDK connection errors

### Settings
- [ ] View seed phrase (with biometric auth required)
- [ ] Change wallet password (for cloud backup)
- [ ] Network selection (mainnet/testnet)
- [ ] Fee preferences
- [ ] Currency selection (USD, EUR, BTC)
- [ ] Delete wallet (with warnings)

## Technical Design

### Architecture

```
iOS App
├── Views/Wallet/
│   ├── WalletHomeView          // Balance + actions
│   ├── SendPaymentView         // Send flow
│   ├── ReceivePaymentView      // Receive flow
│   ├── TransactionListView     // History
│   ├── TransactionDetailView   // Details
│   ├── SeedBackupView          // Backup mnemonic
│   └── WalletSettingsView      // Settings
│
├── ViewModels/
│   ├── WalletViewModel         // UI state
│   └── TransactionViewModel    // History state
│
└── OpenAgentsCore/Lightning/
    ├── SparkWalletManager      // SDK wrapper (actor)
    ├── SeedManager             // Keychain storage
    ├── SparkConfig             // Configuration
    └── SparkEventHandler       // Event listener
```

### Core Implementation

#### SparkWalletManager (Actor)

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkWalletManager.swift

import Foundation
import BreezSdkSpark

/// Manages Breez Spark SDK lifecycle and payment operations
@MainActor
public class SparkWalletManager: ObservableObject {
    @Published public private(set) var isConnected = false
    @Published public private(set) var balanceSats: UInt64 = 0
    @Published public private(set) var isSyncing = false
    @Published public private(set) var transactions: [Payment] = []

    private var sdk: BreezSdk?
    private var eventListenerId: String?
    private let seedManager: SeedManager
    private let config: SparkConfig

    public init(seedManager: SeedManager = SeedManager(), config: SparkConfig = .default) {
        self.seedManager = seedManager
        self.config = config
    }

    // MARK: - Initialization

    /// Initialize wallet with existing or new mnemonic
    public func initialize() async throws {
        // Load or generate mnemonic
        let mnemonic: String
        if let existing = try await seedManager.loadMnemonic() {
            mnemonic = existing
        } else {
            throw WalletError.mnemonicNotFound
        }

        try await connect(mnemonic: mnemonic)
    }

    /// Connect to Spark network
    private func connect(mnemonic: String) async throws {
        let seed = Seed.mnemonic(mnemonic: mnemonic, passphrase: nil)

        var sdkConfig = defaultConfig(network: config.network)
        sdkConfig.apiKey = config.apiKey

        let connectRequest = ConnectRequest(
            config: sdkConfig,
            seed: seed,
            storageDir: config.storageDirectory
        )

        self.sdk = try await BreezSdkSpark.connect(request: connectRequest)

        // Register event listener
        let listener = WalletEventListener(manager: self)
        self.eventListenerId = await sdk?.addEventListener(listener: listener)

        self.isConnected = true

        // Initial sync
        try await refresh()
    }

    /// Disconnect and cleanup
    public func disconnect() async throws {
        guard let sdk = sdk else { return }

        if let listenerId = eventListenerId {
            await sdk.removeEventListener(id: listenerId)
            eventListenerId = nil
        }

        try await sdk.disconnect()
        self.sdk = nil
        self.isConnected = false
    }

    // MARK: - Balance & Info

    /// Refresh balance and info from SDK
    public func refresh() async throws {
        guard let sdk = sdk else { throw WalletError.notConnected }

        isSyncing = true
        defer { isSyncing = false }

        let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: true))
        self.balanceSats = info.balanceSats

        // Load recent transactions
        try await loadTransactions()
    }

    private func loadTransactions(limit: UInt32 = 100) async throws {
        guard let sdk = sdk else { return }

        let payments = try await sdk.listPayments(
            request: ListPaymentsRequest(limit: limit)
        )

        self.transactions = payments
    }

    // MARK: - Send Payment

    public struct PaymentPreview {
        public let invoice: String
        public let amountSats: UInt64?
        public let description: String?
        public let feeSats: UInt64
        public let totalSats: UInt64
        public let expiresAt: Date?
    }

    /// Preview payment (shows fees before sending)
    public func previewPayment(invoice: String, amount: UInt64? = nil) async throws -> PaymentPreview {
        guard let sdk = sdk else { throw WalletError.notConnected }

        let prepareResponse = try await sdk.prepareSendPayment(
            request: PrepareSendPaymentRequest(
                paymentRequest: invoice,
                amount: amount
            ))

        // Extract details based on payment method
        let (amountSats, feeSats, description) = extractPaymentDetails(prepareResponse)

        return PaymentPreview(
            invoice: invoice,
            amountSats: amountSats,
            description: description,
            feeSats: feeSats,
            totalSats: (amountSats ?? 0) + feeSats,
            expiresAt: nil  // TODO: Extract from invoice
        )
    }

    /// Send payment (after user confirms preview)
    public func sendPayment(invoice: String, amount: UInt64? = nil) async throws -> Payment {
        guard let sdk = sdk else { throw WalletError.notConnected }

        // Prepare payment
        let prepareResponse = try await sdk.prepareSendPayment(
            request: PrepareSendPaymentRequest(
                paymentRequest: invoice,
                amount: amount
            ))

        // Execute payment
        let sendResponse = try await sdk.sendPayment(
            request: SendPaymentRequest(
                prepareResponse: prepareResponse,
                options: nil
            ))

        // Update balance
        try await refresh()

        return sendResponse.payment
    }

    // MARK: - Receive Payment

    public struct Invoice {
        public let bolt11: String
        public let amountSats: UInt64?
        public let description: String
        public let feeSats: UInt64
        public let expiresAt: Date
    }

    /// Generate BOLT11 invoice
    public func createInvoice(
        amountSats: UInt64?,
        description: String
    ) async throws -> Invoice {
        guard let sdk = sdk else { throw WalletError.notConnected }

        let response = try await sdk.receivePayment(
            request: ReceivePaymentRequest(
                paymentMethod: .bolt11Invoice(
                    description: description,
                    amountSats: amountSats
                )
            ))

        return Invoice(
            bolt11: response.paymentRequest,
            amountSats: amountSats,
            description: description,
            feeSats: response.fee,
            expiresAt: Date().addingTimeInterval(3600)  // 1 hour default
        )
    }

    /// Wait for payment on invoice
    public func waitForPayment(invoice: String) async throws -> Payment {
        guard let sdk = sdk else { throw WalletError.notConnected }

        let response = try await sdk.waitForPayment(
            request: WaitForPaymentRequest(
                identifier: .paymentRequest(invoice)
            ))

        // Update balance after payment received
        try await refresh()

        return response.payment
    }

    // MARK: - Helpers

    private func extractPaymentDetails(_ prepareResponse: PrepareSendPaymentResponse) -> (UInt64?, UInt64, String?) {
        switch prepareResponse.paymentMethod {
        case .bolt11Invoice(let details, _, let lightningFee):
            return (details.amountSats, lightningFee, details.description)
        case .sparkInvoice(let details, let fee, _):
            return (details.amountSats.map { UInt64($0) }, fee, details.description)
        case .sparkAddress(let details, let fee, _):
            return (UInt64(details.amountSats), fee, nil)
        case .bitcoinAddress(let details, let fee, _):
            return (UInt64(details.amountSats), fee, nil)
        }
    }
}

// MARK: - Event Listener

class WalletEventListener: EventListener {
    weak var manager: SparkWalletManager?

    init(manager: SparkWalletManager) {
        self.manager = manager
    }

    func onEvent(event: SdkEvent) async {
        guard let manager = manager else { return }

        switch event {
        case .synced:
            try? await manager.refresh()

        case .paymentSucceeded(let payment):
            print("✅ Payment succeeded: \(payment.id)")
            try? await manager.refresh()
            // TODO: Show notification

        case .paymentFailed(let error):
            print("❌ Payment failed: \(error)")
            // TODO: Show error to user

        case .claimDepositsSucceeded(let claimed):
            print("💰 Claimed deposits: \(claimed)")
            try? await manager.refresh()

        default:
            break
        }
    }
}

// MARK: - Errors

public enum WalletError: Error, LocalizedError {
    case notConnected
    case mnemonicNotFound
    case invalidInvoice
    case paymentFailed(String)
    case insufficientBalance

    public var errorDescription: String? {
        switch self {
        case .notConnected:
            return "Wallet not connected. Please initialize wallet first."
        case .mnemonicNotFound:
            return "No wallet found. Create or import a wallet."
        case .invalidInvoice:
            return "Invalid Lightning invoice."
        case .paymentFailed(let reason):
            return "Payment failed: \(reason)"
        case .insufficientBalance:
            return "Insufficient balance."
        }
    }
}
```

#### SeedManager (Keychain Storage)

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SeedManager.swift

import Foundation
import Security

/// Manages BIP39 mnemonic storage in Keychain
actor SeedManager {
    private let keychainKey = "com.openagents.lightning.mnemonic"

    /// Generate new 12-word BIP39 mnemonic
    func generateMnemonic() async throws -> String {
        // Spark SDK can generate internally, or use separate BIP39 library
        // For now, use Spark's internal generation
        // In production: Use proper BIP39 library like swift-bip39

        // Placeholder - will integrate with BIP39 library
        fatalError("Implement BIP39 generation")
    }

    /// Save mnemonic to Keychain
    func saveMnemonic(_ mnemonic: String) async throws {
        let data = mnemonic.data(using: .utf8)!

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        ]

        // Delete existing (if any)
        SecItemDelete(query as CFDictionary)

        // Add new
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw SeedManagerError.keychainStoreFailed(status)
        }
    }

    /// Load mnemonic from Keychain
    func loadMnemonic() async throws -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey,
            kSecReturnData as String: true
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess,
              let data = result as? Data,
              let mnemonic = String(data: data, encoding: .utf8) else {
            if status == errSecItemNotFound {
                return nil
            }
            throw SeedManagerError.keychainLoadFailed(status)
        }

        return mnemonic
    }

    /// Delete mnemonic (for wallet reset)
    func deleteMnemonic() async throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw SeedManagerError.keychainDeleteFailed(status)
        }
    }
}

enum SeedManagerError: Error, LocalizedError {
    case keychainStoreFailed(OSStatus)
    case keychainLoadFailed(OSStatus)
    case keychainDeleteFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .keychainStoreFailed(let status):
            return "Failed to save seed: Keychain error \(status)"
        case .keychainLoadFailed(let status):
            return "Failed to load seed: Keychain error \(status)"
        case .keychainDeleteFailed(let status):
            return "Failed to delete seed: Keychain error \(status)"
        }
    }
}
```

#### SparkConfig

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkConfig.swift

import Foundation
import BreezSdkSpark

/// Spark SDK configuration
public struct SparkConfig {
    public var apiKey: String
    public var network: Network
    public var storageDirectory: String

    public static var `default`: SparkConfig {
        let documentsDir = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("spark-wallet")
            .path

        return SparkConfig(
            apiKey: "", // Must be set by app
            network: .mainnet,
            storageDirectory: documentsDir
        )
    }
}
```

### SwiftUI Views

```swift
// ios/OpenAgents/Views/Wallet/WalletHomeView.swift

import SwiftUI
import OpenAgentsCore

struct WalletHomeView: View {
    @EnvironmentObject var walletManager: SparkWalletManager
    @State private var showingSendView = false
    @State private var showingReceiveView = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 24) {
                    // Balance Card
                    BalanceCard(
                        balanceSats: walletManager.balanceSats,
                        isSyncing: walletManager.isSyncing
                    )

                    // Actions
                    HStack(spacing: 16) {
                        Button {
                            showingSendView = true
                        } label: {
                            Label("Send", systemImage: "arrow.up.circle.fill")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)

                        Button {
                            showingReceiveView = true
                        } label: {
                            Label("Receive", systemImage: "arrow.down.circle.fill")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)
                    }
                    .padding(.horizontal)

                    // Recent Transactions
                    TransactionListSection(transactions: walletManager.transactions)
                }
            }
            .navigationTitle("Lightning Wallet")
            .refreshable {
                try? await walletManager.refresh()
            }
            .sheet(isPresented: $showingSendView) {
                SendPaymentView()
            }
            .sheet(isPresented: $showingReceiveView) {
                ReceivePaymentView()
            }
        }
    }
}

struct BalanceCard: View {
    let balanceSats: UInt64
    let isSyncing: Bool

    var body: some View {
        VStack(spacing: 8) {
            Text("Balance")
                .font(.caption)
                .foregroundColor(.secondary)

            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text("\(balanceSats.formatted())")
                    .font(.largeTitle.bold())
                Text("sats")
                    .font(.title3)
                    .foregroundColor(.secondary)
            }

            if isSyncing {
                ProgressView()
                    .scaleEffect(0.8)
            }
        }
        .frame(maxWidth: .infinity)
        .padding()
        .background(Material.regular)
        .cornerRadius(12)
        .padding()
    }
}
```

## Dependencies

### Swift Packages
- **BreezSdkSpark**: https://github.com/breez/breez-sdk-spark-swift.git (v0.3.4+)
- **Swift-BigInt**: (dependency of Spark SDK)

### External Services
- **Breez API Key**: Request from https://breez.technology/request-api-key/ (free)

### OpenAgents Dependencies
- **Issue #010-api-key**: Breez API Key Configuration (blocker)
- **Issue #012**: Marketplace Payment Coordinator (for job payments)
- **Issue #015**: Seed Backup & Recovery UI

### System Frameworks
- **Foundation**: Core types
- **Security**: Keychain
- **SwiftUI**: UI framework
- **LocalAuthentication**: Biometric auth (for viewing seed)

## Testing Requirements

### Unit Tests
- [ ] SeedManager Keychain operations
- [ ] Payment amount calculations
- [ ] Fee extraction from prepare responses
- [ ] Event handler logic
- [ ] Error handling paths

### Integration Tests (Testnet)
- [ ] Initialize wallet with test mnemonic
- [ ] Generate invoice (testnet)
- [ ] Send payment (testnet)
- [ ] Receive payment (testnet)
- [ ] Balance updates correctly
- [ ] Event listener triggers

### UI Tests
- [ ] Wallet creation flow
- [ ] Backup seed quiz
- [ ] Send payment flow
- [ ] Receive payment flow
- [ ] Transaction list scrolling

### Manual Testing (Mainnet)
- [ ] Create wallet with real mnemonic
- [ ] Small test payment (100 sats)
- [ ] Verify fee calculations
- [ ] Check transaction history
- [ ] Recovery from mnemonic

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 3.1.5(i) - Cryptocurrency Wallets**
- ✅ **Allowed**: Self-custodial wallets permitted for Organization developers
- ⚠️  **Required**: Organization Apple Developer account ($99/year)
- ✅ User controls keys (Spark is fully self-custodial)

**ASRG 5.1.1 - Privacy**
- ✅ Privacy policy must explain:
  - Keys stored locally (Keychain)
  - No Breez access to user funds
  - API calls to Breez infrastructure (for routing, not fund custody)
  - Optional encrypted iCloud backup

**ASRG 2.4.2 - Performance**
- ✅ No background mining (Spark doesn't mine)
- ✅ Power-efficient (Spark is nodeless)

### Privacy Disclosures

**App Privacy Labels:**
- **Identifiers**: None (npub is pseudonymous)
- **Financial Info**: Not collected (self-custodial)
- **Usage Data**: API calls to Breez (for network sync)

### Crypto Export Compliance
- Declare encryption use in App Store Connect
- Standard crypto (no export license needed)

## Reference Links

### Breez Spark SDK
- **Documentation**: https://sdk-doc-spark.breez.technology/
- **Getting Started**: https://sdk-doc-spark.breez.technology/guide/getting_started.html
- **Swift Package**: https://github.com/breez/breez-sdk-spark-swift
- **API Key**: https://breez.technology/request-api-key/

### Bitcoin/Lightning
- **BIP39**: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki
- **BOLT11**: https://github.com/lightning/bolts/blob/master/11-payment-encoding.md

### OpenAgents
- **Issue #012**: Marketplace Payment Coordinator
- **Issue #013**: macOS Wallet (shares this core)
- **Issue #015**: Seed Backup & Recovery
- **Apple Terms Research**: docs/compute/apple-terms-research.md

## Success Metrics

- [ ] Wallet initialization <2 seconds
- [ ] Payment preview <1 second
- [ ] Payment execution <5 seconds average
- [ ] 100% testnet payment success rate
- [ ] >95% mainnet payment success rate
- [ ] No seed leaks in logs/crashes
- [ ] App Store approval achieved

## Notes

- **Testnet First**: All development on testnet until thoroughly tested
- **Small Amounts**: Start mainnet with <10,000 sats
- **Beta SDK**: Spark v0.3.4 is beta; expect updates
- **API Key**: Free from Breez, required for SDK
- **Self-Custody**: Users responsible for seed backup

## Future Enhancements

- Lightning Address support (user@openagents.com)
- Multi-wallet support (separate wallets for different purposes)
- Advanced fee controls
- Submarine swaps (Lightning ↔ on-chain)
- Watchtower integration
- Hardware wallet support (Ledger, Trezor)
