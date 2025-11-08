# iOS: Nostr Identity & Key Management

**Phase:** 1 - MVP
**Component:** iOS App
**Priority:** P0 (Critical - Blocks all marketplace features)
**Estimated Effort:** 2-3 weeks

## Summary

Implement Nostr identity and key management in the iOS app, including key generation/import, Secure Enclave storage, relay configuration, and identity display. This provides the foundation for all Nostr interactions on the marketplace.

## Motivation

Every marketplace participant needs a Nostr identity:

- **Buyers**: Sign job requests, receive results
- **Providers**: Sign capability advertisements, publish job results
- **Payments**: Zap receipts tied to Nostr pubkeys
- **Reputation**: Trust anchored to long-lived identities

Without identity management, users cannot interact with the marketplace.

## Acceptance Criteria

### Key Generation & Import
- [ ] Generate new Nostr keypair (secp256k1, Secure Enclave)
- [ ] Import existing private key (nsec format)
- [ ] Export public key (npub format with QR code)
- [ ] **Never** export private key in plaintext (warn user)
- [ ] Multiple identity support (default + additional)
- [ ] Set active identity

### Key Storage
- [ ] Store private keys in Secure Enclave (or Keychain fallback)
- [ ] Biometric authentication for key access (Face ID / Touch ID)
- [ ] Accessibility: `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
- [ ] No iCloud keychain sync for Nostr keys (security)

### Identity Display
- [ ] Show npub (truncated: npub1abc...xyz with copy)
- [ ] QR code for npub (share with others)
- [ ] Identity avatar (generated from pubkey hash)
- [ ] Optional display name (local only, not published)
- [ ] NIP-05 verification status (future: link to domain)

### Relay Management
- [ ] Default relay list (3-5 popular relays)
- [ ] Add custom relay (URL validation)
- [ ] Remove relay
- [ ] Test relay connection (ping/pong)
- [ ] Relay status indicators (connected, disconnected, error)
- [ ] Per-relay stats (events sent/received, latency)

### Settings UI
- [ ] Identity list (with active indicator)
- [ ] Create/import/delete identity
- [ ] Relay list (add/remove/test)
- [ ] Security settings (biometric requirement)
- [ ] Backup/export warning UI

## Technical Design

### UI Structure

```
Settings/
├── IdentityView                 // Identity management
│   ├── IdentityListView        // List of identities
│   ├── IdentityDetailView      // Detail for one identity
│   ├── CreateIdentityView      // Create new identity
│   ├── ImportIdentityView      // Import from nsec
│   └── IdentityQRView          // QR code display
├── RelayView                    // Relay management
│   ├── RelayListView           // List of relays
│   ├── AddRelayView            // Add custom relay
│   └── RelayDetailView         // Stats for one relay
└── SecurityView                 // Security settings
```

### SwiftUI Views

```swift
// ios/OpenAgents/Views/Settings/

IdentityView.swift               // Main identity management
IdentityListView.swift           // List of identities
IdentityDetailView.swift         // Identity detail with QR
CreateIdentityView.swift         // Create/import flow
RelayView.swift                  // Relay management
RelayListView.swift              // Relay list
AddRelayView.swift               // Add relay
SecurityView.swift               // Security settings
```

### View Models

```swift
// ios/OpenAgents/ViewModels/

IdentityViewModel.swift          // Identity state management
RelayViewModel.swift             // Relay state management
```

### Key Types

```swift
// IdentityViewModel.swift

import Foundation
import OpenAgentsCore

@MainActor
class IdentityViewModel: ObservableObject {
    @Published var identities: [NostrIdentity] = []
    @Published var activeIdentityId: UUID?
    @Published var error: IdentityError?

    struct NostrIdentity: Identifiable {
        let id: UUID
        let npub: String              // Public key (BECH32)
        let displayName: String?      // Optional local name
        let createdAt: Date
        var isActive: Bool

        // Computed
        var truncatedNpub: String {
            guard npub.count > 16 else { return npub }
            return "\(npub.prefix(8))...\(npub.suffix(8))"
        }

        var avatar: Image {
            // Generate identicon from pubkey hash
        }
    }

    enum IdentityError: LocalizedError {
        case keyGenerationFailed
        case invalidNsec
        case duplicateIdentity
        case secureEnclaveUnavailable
        case biometricAuthFailed
    }

    // MARK: - Key Management

    func createIdentity(displayName: String?) async throws

    func importIdentity(nsec: String, displayName: String?) async throws

    func deleteIdentity(id: UUID) throws

    func setActiveIdentity(id: UUID)

    func exportNpub(id: UUID) -> String?

    // MARK: - Signing

    func sign(event: NostrEvent, identityId: UUID) async throws -> NostrEvent

    // MARK: - Persistence

    private func loadIdentities()
    private func saveIdentities()
}
```

```swift
// RelayViewModel.swift

@MainActor
class RelayViewModel: ObservableObject {
    @Published var relays: [NostrRelay] = []
    @Published var relayStats: [String: RelayStats] = [:]
    @Published var error: RelayError?

    private let relayManager: NostrRelayManager

    struct NostrRelay: Identifiable {
        let id: UUID
        let url: String
        var isEnabled: Bool
        var status: ConnectionStatus

        enum ConnectionStatus {
            case disconnected
            case connecting
            case connected
            case error(String)
        }
    }

    struct RelayStats {
        let url: String
        let eventsSent: Int
        let eventsReceived: Int
        let avgLatency: TimeInterval?
        let lastConnected: Date?
    }

    enum RelayError: LocalizedError {
        case invalidURL
        case connectionFailed(String)
        case duplicateRelay
    }

    init() {
        self.relayManager = NostrRelayManager()
        loadDefaultRelays()
    }

    // MARK: - Relay Management

    func addRelay(url: String) async throws

    func removeRelay(id: UUID)

    func toggleRelay(id: UUID, enabled: Bool) async

    func testRelay(id: UUID) async throws

    func refreshStats()

    // MARK: - Defaults

    private func loadDefaultRelays() {
        let defaults = [
            "wss://relay.damus.io",
            "wss://nos.lol",
            "wss://relay.snort.social",
            "wss://relay.nostr.band"
        ]
        // Add to relays
    }
}
```

### UI Examples

```swift
// IdentityListView.swift

struct IdentityListView: View {
    @StateObject private var viewModel = IdentityViewModel()

    var body: some View {
        List {
            ForEach(viewModel.identities) { identity in
                NavigationLink(destination: IdentityDetailView(identity: identity)) {
                    HStack {
                        identity.avatar
                            .resizable()
                            .frame(width: 40, height: 40)
                            .clipShape(Circle())

                        VStack(alignment: .leading) {
                            Text(identity.displayName ?? "Nostr Identity")
                                .font(.headline)
                            Text(identity.truncatedNpub)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }

                        Spacer()

                        if identity.isActive {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundColor(.green)
                        }
                    }
                }
                .swipeActions {
                    Button(role: .destructive) {
                        try? viewModel.deleteIdentity(id: identity.id)
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                }
            }

            Button {
                // Navigate to CreateIdentityView
            } label: {
                Label("Create New Identity", systemImage: "plus.circle")
            }
        }
        .navigationTitle("Nostr Identities")
        .toolbar {
            EditButton()
        }
    }
}
```

```swift
// IdentityDetailView.swift

struct IdentityDetailView: View {
    let identity: IdentityViewModel.NostrIdentity
    @State private var showingQR = false

    var body: some View {
        List {
            Section("Identity") {
                identity.avatar
                    .resizable()
                    .frame(width: 80, height: 80)
                    .clipShape(Circle())
                    .frame(maxWidth: .infinity)

                if let displayName = identity.displayName {
                    LabeledContent("Name", value: displayName)
                }

                LabeledContent("Public Key", value: identity.npub)
                    .textSelection(.enabled)

                Button {
                    showingQR = true
                } label: {
                    Label("Show QR Code", systemImage: "qrcode")
                }
            }

            Section("Security") {
                LabeledContent("Key Storage", value: "Secure Enclave")
                LabeledContent("Created", value: identity.createdAt.formatted())
            }
        }
        .navigationTitle(identity.displayName ?? "Identity")
        .sheet(isPresented: $showingQR) {
            IdentityQRView(npub: identity.npub)
        }
    }
}
```

```swift
// RelayListView.swift

struct RelayListView: View {
    @StateObject private var viewModel = RelayViewModel()

    var body: some View {
        List {
            ForEach(viewModel.relays) { relay in
                NavigationLink(destination: RelayDetailView(relay: relay)) {
                    HStack {
                        Circle()
                            .fill(statusColor(for: relay.status))
                            .frame(width: 10, height: 10)

                        VStack(alignment: .leading) {
                            Text(relay.url)
                                .font(.body)
                            if let stats = viewModel.relayStats[relay.url] {
                                Text("\(stats.eventsSent) sent • \(stats.eventsReceived) received")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                        }

                        Spacer()

                        Toggle("", isOn: binding(for: relay))
                            .labelsHidden()
                    }
                }
            }
            .onDelete(perform: deleteRelays)

            Button {
                // Navigate to AddRelayView
            } label: {
                Label("Add Relay", systemImage: "plus.circle")
            }
        }
        .navigationTitle("Relays")
        .refreshable {
            viewModel.refreshStats()
        }
    }

    private func statusColor(for status: RelayViewModel.NostrRelay.ConnectionStatus) -> Color {
        switch status {
        case .connected: return .green
        case .connecting: return .yellow
        case .disconnected: return .gray
        case .error: return .red
        }
    }

    private func binding(for relay: RelayViewModel.NostrRelay) -> Binding<Bool> {
        Binding(
            get: { relay.isEnabled },
            set: { enabled in
                Task {
                    await viewModel.toggleRelay(id: relay.id, enabled: enabled)
                }
            }
        )
    }

    private func deleteRelays(at offsets: IndexSet) {
        for index in offsets {
            viewModel.removeRelay(id: viewModel.relays[index].id)
        }
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (NostrEvent, relay management)
- **Issue #002**: Secp256k1 & Cryptography (key generation, Secure Enclave)

### System Frameworks
- **SwiftUI**: UI framework
- **Security**: Secure Enclave, Keychain
- **LocalAuthentication**: Biometric auth
- **CoreImage**: QR code generation

## Testing Requirements

### Unit Tests
- [ ] Identity creation and storage
- [ ] Import from valid nsec
- [ ] Reject invalid nsec
- [ ] Duplicate identity prevention
- [ ] Active identity switching
- [ ] Relay URL validation
- [ ] Relay connection testing

### UI Tests
- [ ] Create identity flow
- [ ] Import identity flow
- [ ] Delete identity confirmation
- [ ] QR code display
- [ ] Relay add/remove
- [ ] Biometric prompt (on real device)

### Integration Tests
- [ ] Sign event with Secure Enclave key
- [ ] Connect to real relay
- [ ] Publish test event
- [ ] Verify event on relay

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 5.1.1 (Privacy - Biometric Data)**
- ✅ **Compliant**: LocalAuthentication doesn't access biometric data (only auth result)
- ✅ Privacy policy mentions biometric auth for key access

**ASRG 5.1.2 (Data Use)**
- ⚠️  **Disclosure**: Nostr public keys are broadcast to relays (public by design)
- ✅ User creates identity explicitly (opt-in)

**ASRG 2.5.6 (Security)**
- ✅ Secure Enclave for private key storage
- ✅ Keys never leave device
- ✅ No plaintext key export (warn user strongly if implemented)

### Privacy Labels

Required privacy disclosures:
- **User Content** (Linked to User): Nostr events published to relays
- **Identifiers** (Not Linked): Public key (npub) - pseudonymous

## Reference Links

### Specifications
- **NIP-01**: https://github.com/nostr-protocol/nips/blob/master/01.md
- **NIP-19** (BECH32): https://github.com/nostr-protocol/nips/blob/master/19.md
- **NIP-05** (Verification): https://github.com/nostr-protocol/nips/blob/master/05.md

### Apple Documentation
- **Secure Enclave**: https://developer.apple.com/documentation/security/certificate_key_and_trust_services/keys/storing_keys_in_the_secure_enclave
- **LocalAuthentication**: https://developer.apple.com/documentation/localauthentication

### OpenAgents
- **Issue #001**: Nostr Client Library
- **Issue #002**: Secp256k1 & Cryptography
- **ADR-0006**: Foundation Models (AUP enforcement for marketplace)

## Success Metrics

- [ ] Users can create/import identities
- [ ] Biometric auth works on real devices
- [ ] Secure Enclave storage confirmed
- [ ] Relay connections successful
- [ ] QR code generation working
- [ ] UI tests pass on iOS 16.0+
- [ ] Published in TestFlight build

## Notes

- **Security First**: Never compromise on key storage security
- **UX**: Make identity creation smooth (don't overwhelm with crypto concepts)
- **Defaults**: Provide good default relays (popular, reliable)
- **Backup**: Strong warnings about nsec backup (write down, secure location)
- **Multiple Identities**: Support but don't require (most users = 1 identity)

## Future Enhancements (Post-MVP)

- NIP-05 verification (link npub to domain)
- NIP-46 remote signing (use phone as signer for desktop)
- Contact list (NIP-02)
- Profile metadata (NIP-01 kind:0)
- Identity backup to iCloud (encrypted with user password)
- Identity recovery flow (seed phrase)
