# Bitcoin Seed Backup & Recovery UI

**Phase:** 2 - Payments
**Component:** iOS App, macOS App
**Priority:** P1 (Critical for user safety)
**Estimated Effort:** 3-5 days
**Dependencies:** Issue #010, #013 (Wallets use SeedManager)

## Summary

Implement secure **seed phrase backup and recovery** UI for iOS and macOS, ensuring users can safely back up their 12-word BIP39 mnemonic and recover wallets after device loss or reinstall. This is **critical for user safety** - without proper backup, users risk losing Bitcoin.

## Motivation

The Spark SDK wallet is **self-custodial**: users control their private keys via a BIP39 mnemonic seed phrase. If the seed is lost, **funds are unrecoverable**.

Users must be able to:
1. **View their seed phrase** (once, securely, with warnings)
2. **Back up the seed** (write down, screenshot with warnings, iCloud Keychain optional)
3. **Verify backup** (enter seed words to confirm)
4. **Recover wallet** (import seed on new device)
5. **Understand risks** (clear warnings about seed exposure)

Good UX prevents:
- ❌ Users losing funds due to device failure
- ❌ Users sharing seed phrases insecurely (screenshots in cloud, email, messages)
- ❌ Users importing seeds into malicious wallets
- ❌ Users not backing up at all ("I'll do it later")

## Acceptance Criteria

### Seed Display (iOS/macOS)

- [ ] **Show 12-word seed phrase** in grid layout (read-only)
- [ ] **Require authentication** before displaying (Face ID, Touch ID, password)
- [ ] **Blur by default** (tap to reveal each word)
- [ ] **Warning banner**: "Never share your seed phrase. Anyone with these words can steal your Bitcoin."
- [ ] **Copy to clipboard** button (with warning)
- [ ] **Screenshot detection** (warn user if screenshot taken)

### Backup Flow (First-Time Setup)

- [ ] **Prompt backup** immediately after wallet creation
- [ ] **Explain importance**: "Write down these 12 words. You'll need them to recover your wallet."
- [ ] **Show seed words** (numbered 1-12)
- [ ] **Verification step**: User enters 3-5 random words to confirm backup
- [ ] **Skip option**: "I'll back up later" (with warning, re-prompt on next launch)

### Recovery Flow

- [ ] **Import seed screen** (12 text fields, auto-capitalization, word suggestions)
- [ ] **BIP39 word validation** (only accept valid BIP39 words)
- [ ] **Checksum validation** (reject invalid mnemonics)
- [ ] **Network selection**: Mainnet/testnet (critical - wrong network = wrong addresses)
- [ ] **Overwrite warning**: "This will replace your current wallet. Make sure you have backed up your old seed."

### iCloud Keychain Backup (Optional)

- [ ] **Offer iCloud Keychain sync** (encrypted, Apple-managed)
- [ ] **Explain trade-offs**: Convenience vs trusting Apple/iCloud
- [ ] **User choice**: Enable/disable iCloud sync
- [ ] **Sync status indicator**: "Seed backed up to iCloud Keychain"

### Paper Backup Guidance

- [ ] **Paper backup template**: Printable PDF with numbered lines
- [ ] **Best practices**: "Store in fireproof safe, split across locations, laminate, use metal backup"
- [ ] **Link to resources**: Metal backup products (Cryptosteel, etc.)

### Settings UI

- [ ] **View seed** (Settings → Wallet → View Seed Phrase, requires auth)
- [ ] **Backup status**: "Backed up" or "Not backed up" indicator
- [ ] **Re-verify backup**: Option to re-run verification
- [ ] **Change backup method**: Switch between paper, iCloud, both

### Testing

- [ ] **Unit tests** for seed generation, import, validation
- [ ] **UI tests** for backup flow, recovery flow
- [ ] **Test invalid seeds** (wrong words, bad checksum)
- [ ] **Test iCloud Keychain** sync (enable, disable, device switch)

## Technical Design

### Architecture

```
┌──────────────────────────────────────────────────────┐
│              Wallet Creation Flow                    │
│                                                      │
│  1. Generate seed (SeedManager)                     │
│  2. Save to Keychain                                │
│  3. Display backup prompt (BackupFlowView)          │
│  4. User writes down seed                           │
│  5. Verify backup (random word quiz)                │
│  6. Mark as backed up (UserDefaults flag)           │
│                                                      │
└──────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────┐
│              Wallet Recovery Flow                    │
│                                                      │
│  1. User enters 12 words (RecoveryView)             │
│  2. Validate BIP39 words + checksum                 │
│  3. Save to Keychain (SeedManager)                  │
│  4. Initialize wallet with recovered seed           │
│  5. Sync wallet state                               │
│                                                      │
└──────────────────────────────────────────────────────┘
```

### Seed Display View

```swift
// Shared/Views/SeedPhraseView.swift

import SwiftUI
import LocalAuthentication

struct SeedPhraseView: View {
    @State private var seedWords: [String] = []
    @State private var isRevealed = false
    @State private var isLoading = true
    @State private var error: String?

    let seedManager = SeedManager()

    var body: some View {
        VStack(spacing: 24) {
            // Warning banner
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Keep Your Seed Phrase Secret")
                        .font(.headline)
                    Text("Anyone with these words can steal your Bitcoin. Never share them.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding()
            .background(.orange.opacity(0.1), in: RoundedRectangle(cornerRadius: 12))

            if isLoading {
                ProgressView("Loading seed phrase...")
            } else if let error = error {
                Text(error)
                    .foregroundStyle(.red)
            } else {
                // Seed words grid
                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 16) {
                    ForEach(Array(seedWords.enumerated()), id: \.offset) { index, word in
                        SeedWordCard(number: index + 1, word: word, isRevealed: isRevealed)
                    }
                }
                .padding()

                // Reveal toggle
                Button(isRevealed ? "Hide Words" : "Tap to Reveal") {
                    isRevealed.toggle()
                }
                .buttonStyle(.bordered)

                // Copy button
                Button(action: copySeedToClipboard) {
                    Label("Copy to Clipboard", systemImage: "doc.on.doc")
                }
                .buttonStyle(.borderedProminent)
                .tint(.orange)

                Text("⚠️ Copying to clipboard may expose your seed to other apps")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
        .navigationTitle("Seed Phrase")
        .task {
            await loadSeed()
        }
    }

    func loadSeed() async {
        // Require authentication
        let context = LAContext()
        var authError: NSError?

        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &authError) else {
            error = "Authentication not available"
            isLoading = false
            return
        }

        do {
            let success = try await context.evaluatePolicy(
                .deviceOwnerAuthentication,
                localizedReason: "Authenticate to view your seed phrase"
            )

            guard success else {
                error = "Authentication failed"
                isLoading = false
                return
            }

            // Load seed from Keychain
            let mnemonic = try await seedManager.loadMnemonic()
            seedWords = mnemonic.split(separator: " ").map(String.init)
            isLoading = false

        } catch {
            self.error = error.localizedDescription
            isLoading = false
        }
    }

    func copySeedToClipboard() {
        let seedPhrase = seedWords.joined(separator: " ")

        #if os(iOS)
        UIPasteboard.general.string = seedPhrase
        #elseif os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(seedPhrase, forType: .string)
        #endif

        // Show alert or toast
    }
}

struct SeedWordCard: View {
    let number: Int
    let word: String
    let isRevealed: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\(number)")
                .font(.caption)
                .foregroundStyle(.secondary)

            Text(isRevealed ? word : "••••••")
                .font(.body.monospaced())
                .fontWeight(.medium)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding()
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
    }
}
```

### Backup Flow (First-Time)

```swift
// Shared/Views/BackupFlowView.swift

import SwiftUI

struct BackupFlowView: View {
    @Binding var isPresented: Bool
    @State private var currentStep: BackupStep = .introduction
    @State private var seedWords: [String] = []
    @State private var verificationWords: [(index: Int, word: String)] = []
    @State private var userAnswers: [String] = ["", "", ""]
    @State private var backupCompleted = false

    enum BackupStep {
        case introduction
        case displaySeed
        case verification
        case confirmation
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                switch currentStep {
                case .introduction:
                    IntroductionStep(nextAction: { currentStep = .displaySeed })

                case .displaySeed:
                    DisplaySeedStep(
                        seedWords: seedWords,
                        nextAction: { currentStep = .verification }
                    )

                case .verification:
                    VerificationStep(
                        verificationWords: verificationWords,
                        userAnswers: $userAnswers,
                        checkAnswers: checkVerification
                    )

                case .confirmation:
                    ConfirmationStep(dismissAction: dismiss)
                }
            }
            .navigationTitle("Back Up Wallet")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Skip for Now") {
                        // Warn user
                        // Dismiss
                        isPresented = false
                    }
                }
            }
        }
        .task {
            await loadSeed()
        }
    }

    func loadSeed() async {
        let seedManager = SeedManager()
        let mnemonic = try? await seedManager.loadMnemonic()
        seedWords = mnemonic?.split(separator: " ").map(String.init) ?? []

        // Pick 3 random words for verification
        let indices = [3, 7, 11]  // Or random
        verificationWords = indices.map { (index: $0, word: seedWords[$0]) }
    }

    func checkVerification() {
        let correct = zip(verificationWords, userAnswers).allSatisfy { verification, answer in
            verification.word.lowercased() == answer.lowercased().trimmingCharacters(in: .whitespaces)
        }

        if correct {
            // Mark as backed up
            UserDefaults.standard.set(true, forKey: "wallet.seedBackedUp")
            currentStep = .confirmation
        } else {
            // Show error, allow retry
        }
    }

    func dismiss() {
        backupCompleted = true
        isPresented = false
    }
}

struct IntroductionStep: View {
    let nextAction: () -> Void

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "checkmark.shield.fill")
                .font(.system(size: 80))
                .foregroundStyle(.green)

            Text("Back Up Your Wallet")
                .font(.title)
                .fontWeight(.bold)

            Text("You'll see 12 words. Write them down in order and keep them safe. You'll need them to recover your wallet if you lose this device.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                Label("Write on paper and store securely", systemImage: "pencil.and.paper")
                Label("Never share with anyone", systemImage: "lock.fill")
                Label("Don't take screenshots", systemImage: "camera.fill")
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding()
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))

            Spacer()

            Button("Show My Seed Phrase") {
                nextAction()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}

struct DisplaySeedStep: View {
    let seedWords: [String]
    let nextAction: () -> Void

    var body: some View {
        VStack(spacing: 24) {
            Text("Write Down These Words")
                .font(.headline)

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                ForEach(Array(seedWords.enumerated()), id: \.offset) { index, word in
                    HStack {
                        Text("\(index + 1).")
                            .foregroundStyle(.secondary)
                        Text(word)
                            .fontWeight(.medium)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
                }
            }

            Spacer()

            Button("I've Written It Down") {
                nextAction()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}

struct VerificationStep: View {
    let verificationWords: [(index: Int, word: String)]
    @Binding var userAnswers: [String]
    let checkAnswers: () -> Void

    var body: some View {
        VStack(spacing: 24) {
            Text("Verify Your Backup")
                .font(.headline)

            Text("Enter these words from your seed phrase to confirm you wrote them down correctly.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)

            ForEach(Array(verificationWords.enumerated()), id: \.offset) { idx, verification in
                VStack(alignment: .leading, spacing: 8) {
                    Text("Word #\(verification.index + 1)")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField("Enter word \(verification.index + 1)", text: $userAnswers[idx])
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }
            }

            Spacer()

            Button("Verify") {
                checkAnswers()
            }
            .buttonStyle(.borderedProminent)
            .disabled(userAnswers.contains(""))
        }
        .padding()
    }
}

struct ConfirmationStep: View {
    let dismissAction: () -> Void

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 80))
                .foregroundStyle(.green)

            Text("Backup Complete!")
                .font(.title)
                .fontWeight(.bold)

            Text("Your wallet is now backed up. Keep your seed phrase safe - you'll need it to recover your wallet.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)

            Spacer()

            Button("Done") {
                dismissAction()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}
```

### Recovery Flow

```swift
// Shared/Views/RecoveryView.swift

import SwiftUI

struct RecoveryView: View {
    @State private var seedWords: [String] = Array(repeating: "", count: 12)
    @State private var selectedNetwork: Network = .mainnet
    @State private var isRecovering = false
    @State private var error: String?
    @Environment(\.dismiss) var dismiss

    let seedManager = SeedManager()
    let bip39Words = BIP39WordList.english  // 2048 words

    var body: some View {
        NavigationStack {
            Form {
                Section("Network") {
                    Picker("Network", selection: $selectedNetwork) {
                        ForEach(Network.allCases, id: \.self) { network in
                            Text(network.displayName).tag(network)
                        }
                    }

                    Text("⚠️ Make sure you select the correct network, or your wallet addresses will be wrong.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Section("Seed Phrase (12 words)") {
                    ForEach(0..<12, id: \.self) { index in
                        HStack {
                            Text("\(index + 1).")
                                .foregroundStyle(.secondary)
                                .frame(width: 30)

                            TextField("word", text: $seedWords[index])
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.never)
                        }
                    }
                }

                if let error = error {
                    Section {
                        Text(error)
                            .foregroundStyle(.red)
                    }
                }

                Section {
                    Button("Recover Wallet") {
                        Task {
                            await recoverWallet()
                        }
                    }
                    .disabled(seedWords.contains("") || isRecovering)
                }
            }
            .navigationTitle("Recover Wallet")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
    }

    func recoverWallet() async {
        isRecovering = true
        error = nil

        // Validate BIP39 words
        let invalidWords = seedWords.filter { !bip39Words.contains($0.lowercased()) }
        if !invalidWords.isEmpty {
            error = "Invalid words: \(invalidWords.joined(separator: ", "))"
            isRecovering = false
            return
        }

        // Join and validate checksum
        let mnemonic = seedWords.joined(separator: " ")

        do {
            // Validate checksum (BIP39)
            guard try seedManager.validateMnemonic(mnemonic) else {
                error = "Invalid seed phrase (checksum failed)"
                isRecovering = false
                return
            }

            // Save to Keychain
            try await seedManager.saveMnemonic(mnemonic)

            // Mark as backed up (since user imported it)
            UserDefaults.standard.set(true, forKey: "wallet.seedBackedUp")

            // Re-initialize wallet
            // (App will call walletManager.initialize() which loads new seed)

            dismiss()

        } catch {
            self.error = error.localizedDescription
            isRecovering = false
        }
    }
}

// BIP39 word list (English)
struct BIP39WordList {
    static let english: Set<String> = [
        "abandon", "ability", "able", // ... all 2048 words
    ]
}
```

### iCloud Keychain Sync

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SeedManager.swift

actor SeedManager {
    private let keychainKey = "com.openagents.lightning.mnemonic"
    private let iCloudKeychainKey = "com.openagents.lightning.mnemonic.icloud"

    // ... existing methods ...

    /// Enable iCloud Keychain sync
    func enableiCloudSync() async throws {
        let mnemonic = try await loadMnemonic()

        let data = mnemonic.data(using: .utf8)!
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: iCloudKeychainKey,
            kSecValueData as String: data,
            kSecAttrSynchronizable as String: true  // iCloud sync
        ]

        SecItemDelete(query as CFDictionary)
        let status = SecItemAdd(query as CFDictionary, nil)

        guard status == errSecSuccess else {
            throw SeedManagerError.iCloudSyncFailed(status)
        }
    }

    /// Disable iCloud Keychain sync
    func disableiCloudSync() async throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: iCloudKeychainKey,
            kSecAttrSynchronizable as String: true
        ]

        SecItemDelete(query as CFDictionary)
    }
}
```

## Dependencies

### OpenAgents Issues

- **Issue #010**: iOS Wallet (uses SeedManager)
- **Issue #013**: macOS Wallet (uses SeedManager)
- **Issue #002**: Secp256k1 & Cryptography (BIP39 generation)

### External

- LocalAuthentication framework (Face ID, Touch ID)
- BIP39 word list (2048 English words)

## Testing

### Unit Tests

```swift
// OpenAgentsCoreTests/Lightning/SeedManagerTests.swift

func testValidateMnemonic() async throws {
    let seedManager = SeedManager()

    let validMnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

    XCTAssertTrue(try await seedManager.validateMnemonic(validMnemonic))

    let invalidMnemonic = "invalid words that are not bip39"

    XCTAssertFalse(try await seedManager.validateMnemonic(invalidMnemonic))
}

func testiCloudSyncEnableDisable() async throws {
    let seedManager = SeedManager()

    // Generate and save mnemonic
    _ = try await seedManager.generateAndSaveMnemonic()

    // Enable iCloud sync
    try await seedManager.enableiCloudSync()

    // Verify sync is enabled
    // (Check iCloud Keychain for item)

    // Disable sync
    try await seedManager.disableiCloudSync()

    // Verify sync is disabled
}
```

### UI Tests

```swift
func testBackupFlow() throws {
    let app = XCUIApplication()
    app.launch()

    // Complete backup flow
    app.buttons["Back Up Wallet"].tap()
    app.buttons["Show My Seed Phrase"].tap()
    app.buttons["I've Written It Down"].tap()

    // Enter verification words
    // ...

    app.buttons["Verify"].tap()

    // Verify completion screen
    XCTAssertTrue(app.staticTexts["Backup Complete!"].exists)
}
```

## Success Metrics

- [ ] 90%+ of users complete backup flow on first launch
- [ ] <1% of users lose funds due to lost seed
- [ ] Zero seeds exposed via insecure channels (screenshots shared, etc.)
- [ ] Recovery success rate >95% when users have written seed

## Apple Compliance

### Privacy

✅ **Seed phrase in Keychain**: Secure, encrypted
✅ **iCloud Keychain optional**: User choice, explained clearly
✅ **No seed in logs**: Never log seed words

### Security

✅ **Authentication required**: Face ID, Touch ID, password
✅ **Screenshot warning**: Alert user if screenshot taken

## Future Enhancements

- [ ] **Shamir Secret Sharing**: Split seed into multiple shards
- [ ] **Social recovery**: Trusted contacts can help recover (threshold scheme)
- [ ] **Hardware wallet support**: Trezor, Ledger integration
- [ ] **Encrypted cloud backup**: User-encrypted backup to iCloud Drive, Dropbox

## Notes

- **Critical UX**: Clear warnings without being alarmist
- **One-time view**: After initial backup, make it hard to re-display (requires auth)
- **Paper template**: Printable PDF helps users organize backup
- **Metal backups**: Recommend Cryptosteel, Billfodl for fire/water protection

## Reference

- **BIP39**: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki
- **BIP39 Word List**: https://github.com/bitcoin/bips/blob/master/bip-0039/english.txt
- **Apple Keychain**: https://developer.apple.com/documentation/security/keychain_services
- **LocalAuthentication**: https://developer.apple.com/documentation/localauthentication
