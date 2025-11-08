# Breez API Key Management & SDK Configuration

**Phase:** 2 - Payments
**Component:** OpenAgentsCore (Shared)
**Priority:** P1 (Required before wallet initialization)
**Estimated Effort:** 2-3 days
**Dependencies:** None (foundational)

## Summary

Implement secure Breez API key management and SDK configuration infrastructure shared between iOS and macOS. Provides environment-based config (mainnet/testnet), secure key storage, and initialization patterns for the Spark SDK.

## Motivation

The Breez Spark SDK requires an **API key** to connect to Breez's backend infrastructure:

- **Why needed**: Authenticates OpenAgents with Breez Spark Operators
- **Free tier**: Available for development/testing (rate limits apply)
- **Production**: Paid tiers for higher volume
- **Security**: API key must be protected (not hardcoded, not in source control)

Proper configuration management ensures:
1. **Secure key storage** (Keychain on iOS/macOS, environment variables for development)
2. **Environment switching** (mainnet vs testnet without code changes)
3. **Testability** (mock configurations for unit tests)
4. **Ease of deployment** (single config point for production keys)

## Acceptance Criteria

### API Key Management

- [ ] **Secure storage** of Breez API key in Keychain (iOS/macOS)
- [ ] **Fallback to environment variable** for development (`BREEZ_API_KEY`)
- [ ] **No hardcoded keys** in source code
- [ ] **Key validation** on initialization (check format, non-empty)
- [ ] **Error handling** for missing/invalid keys
- [ ] **Key rotation support** (update key without reinstall)

### SDK Configuration

- [ ] **Network selection** (mainnet/testnet/signet) via Config
- [ ] **Storage directory** configuration (app-specific, sandboxed)
- [ ] **Logging configuration** (debug/production levels)
- [ ] **Default config factory** (sensible defaults for each network)
- [ ] **Custom overrides** (advanced users can tweak settings)

### Testing Support

- [ ] **Mock configuration** for unit tests (no real Breez backend)
- [ ] **Testnet config** for integration tests
- [ ] **In-memory storage** option for ephemeral tests

### Documentation

- [ ] **README** for obtaining Breez API key
- [ ] **Setup guide** for development environment
- [ ] **Production deployment** checklist
- [ ] **Troubleshooting** common config issues

## Technical Design

### Architecture Overview

```
┌──────────────────────────────────────────────────┐
│              iOS / macOS App                     │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │    SparkConfigManager (Singleton)          │ │
│  │                                            │ │
│  │  - Loads API key (Keychain or env)        │ │
│  │  - Provides Config for SDK                │ │
│  │  - Handles network switching              │ │
│  └──────────────┬─────────────────────────────┘ │
│                 │                                │
└─────────────────┼────────────────────────────────┘
                  │
       ┌──────────▼──────────┐
       │  OpenAgentsCore     │
       │                     │
       │  SparkConfig        │  (Codable struct)
       │  - network          │
       │  - apiKey           │
       │  - storageDir       │
       │  - loggingLevel     │
       └──────────┬──────────┘
                  │
         ┌────────▼────────┐
         │  BreezSdkSpark  │
         │   (SDK init)    │
         └─────────────────┘
```

### SparkConfig (Shared Type)

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkConfig.swift

import Foundation

/// Configuration for Breez Spark SDK
public struct SparkConfig: Codable, Equatable {
    /// Network to connect to
    public let network: Network

    /// Breez API key (required)
    public let apiKey: String

    /// Storage directory for SDK data
    public let storageDirectory: String

    /// Logging level
    public let loggingLevel: LogLevel

    /// Custom node config (optional, for advanced users)
    public let nodeConfig: NodeConfig?

    public init(
        network: Network,
        apiKey: String,
        storageDirectory: String,
        loggingLevel: LogLevel = .info,
        nodeConfig: NodeConfig? = nil
    ) {
        self.network = network
        self.apiKey = apiKey
        self.storageDirectory = storageDirectory
        self.loggingLevel = loggingLevel
        self.nodeConfig = nodeConfig
    }

    /// Validate configuration
    public func validate() throws {
        guard !apiKey.isEmpty else {
            throw ConfigError.missingAPIKey
        }

        guard apiKey.count >= 32 else {
            throw ConfigError.invalidAPIKey("API key too short")
        }

        guard FileManager.default.fileExists(atPath: storageDirectory) else {
            throw ConfigError.invalidStorageDirectory("Directory does not exist: \(storageDirectory)")
        }
    }
}

// MARK: - Network

public enum Network: String, Codable, CaseIterable {
    case mainnet = "mainnet"
    case testnet = "testnet"
    case signet = "signet"

    public var displayName: String {
        switch self {
        case .mainnet: return "Bitcoin Mainnet"
        case .testnet: return "Bitcoin Testnet"
        case .signet: return "Bitcoin Signet"
        }
    }
}

// MARK: - LogLevel

public enum LogLevel: String, Codable {
    case trace = "trace"
    case debug = "debug"
    case info = "info"
    case warn = "warn"
    case error = "error"
}

// MARK: - NodeConfig (Advanced)

public struct NodeConfig: Codable, Equatable {
    public let maxInboundLiquiditySats: UInt64?
    public let maxOutboundLiquiditySats: UInt64?
    public let channelFeePPM: UInt64?

    public init(
        maxInboundLiquiditySats: UInt64? = nil,
        maxOutboundLiquiditySats: UInt64? = nil,
        channelFeePPM: UInt64? = nil
    ) {
        self.maxInboundLiquiditySats = maxInboundLiquiditySats
        self.maxOutboundLiquiditySats = maxOutboundLiquiditySats
        self.channelFeePPM = channelFeePPM
    }
}

// MARK: - ConfigError

public enum ConfigError: LocalizedError {
    case missingAPIKey
    case invalidAPIKey(String)
    case invalidStorageDirectory(String)
    case networkMismatch

    public var errorDescription: String? {
        switch self {
        case .missingAPIKey:
            return "Breez API key is required. Set BREEZ_API_KEY environment variable or store in Keychain."
        case .invalidAPIKey(let reason):
            return "Invalid API key: \(reason)"
        case .invalidStorageDirectory(let reason):
            return "Invalid storage directory: \(reason)"
        case .networkMismatch:
            return "Network configuration does not match stored wallet data"
        }
    }
}
```

### SparkConfigManager (Singleton)

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkConfigManager.swift

import Foundation

/// Manages Breez Spark SDK configuration across app lifecycle
@MainActor
public class SparkConfigManager: ObservableObject {
    public static let shared = SparkConfigManager()

    // MARK: - Published State
    @Published public private(set) var currentConfig: SparkConfig?
    @Published public private(set) var currentNetwork: Network = .mainnet

    // MARK: - Private State
    private let apiKeyManager: APIKeyManager
    private let defaults = UserDefaults.standard

    // UserDefaults keys
    private let networkKey = "com.openagents.spark.network"

    private init() {
        self.apiKeyManager = APIKeyManager()

        // Load saved network preference
        if let savedNetwork = defaults.string(forKey: networkKey),
           let network = Network(rawValue: savedNetwork) {
            self.currentNetwork = network
        }
    }

    // MARK: - Public API

    /// Load or create configuration for current network
    public func loadConfig() async throws -> SparkConfig {
        // Get API key
        let apiKey = try await apiKeyManager.getAPIKey()

        // Get storage directory
        let storageDir = try createStorageDirectory(for: currentNetwork)

        // Create config
        let config = SparkConfig(
            network: currentNetwork,
            apiKey: apiKey,
            storageDirectory: storageDir,
            loggingLevel: isDebugBuild() ? .debug : .info
        )

        // Validate
        try config.validate()

        self.currentConfig = config
        return config
    }

    /// Switch to different network (requires wallet re-initialization)
    public func switchNetwork(to network: Network) async throws {
        guard network != currentNetwork else { return }

        // Save preference
        defaults.set(network.rawValue, forKey: networkKey)
        self.currentNetwork = network

        // Reload config
        _ = try await loadConfig()
    }

    /// Update API key (e.g., for rotation)
    public func updateAPIKey(_ newKey: String) async throws {
        try await apiKeyManager.saveAPIKey(newKey)

        // Reload config
        _ = try await loadConfig()
    }

    /// Get storage directory for network
    private func createStorageDirectory(for network: Network) throws -> String {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let baseDir = appSupport.appendingPathComponent("OpenAgents/spark")

        let networkDir = baseDir.appendingPathComponent(network.rawValue)

        try FileManager.default.createDirectory(at: networkDir, withIntermediateDirectories: true)

        return networkDir.path
    }

    private func isDebugBuild() -> Bool {
        #if DEBUG
        return true
        #else
        return false
        #endif
    }
}
```

### APIKeyManager (Keychain + Environment)

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/APIKeyManager.swift

import Foundation
#if canImport(Security)
import Security
#endif

/// Manages secure storage and retrieval of Breez API key
actor APIKeyManager {
    private let keychainKey = "com.openagents.breez.apikey"
    private let envVarKey = "BREEZ_API_KEY"

    /// Get API key (Keychain first, then environment variable)
    func getAPIKey() async throws -> String {
        // Try Keychain first
        if let keychainKey = try? loadFromKeychain() {
            return keychainKey
        }

        // Fallback to environment variable
        if let envKey = ProcessInfo.processInfo.environment[envVarKey], !envKey.isEmpty {
            return envKey
        }

        throw ConfigError.missingAPIKey
    }

    /// Save API key to Keychain
    func saveAPIKey(_ apiKey: String) async throws {
        let data = apiKey.data(using: .utf8)!

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        ]

        // Delete existing
        SecItemDelete(query as CFDictionary)

        // Add new
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw ConfigError.invalidAPIKey("Failed to save to Keychain: \(status)")
        }
    }

    /// Load API key from Keychain
    private func loadFromKeychain() throws -> String {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey,
            kSecReturnData as String: true
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess else {
            throw ConfigError.missingAPIKey
        }

        guard let data = result as? Data,
              let apiKey = String(data: data, encoding: .utf8) else {
            throw ConfigError.invalidAPIKey("Could not decode Keychain data")
        }

        return apiKey
    }

    /// Delete API key from Keychain
    func deleteAPIKey() async throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: keychainKey
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw ConfigError.invalidAPIKey("Failed to delete from Keychain: \(status)")
        }
    }
}
```

### Usage in SparkWalletManager

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Lightning/SparkWalletManager.swift

@MainActor
public class SparkWalletManager: ObservableObject {
    // ...

    public func initialize() async throws {
        // Load config from manager
        let config = try await SparkConfigManager.shared.loadConfig()

        // Load mnemonic
        let mnemonic: String
        if await seedManager.hasMnemonic() {
            mnemonic = try await seedManager.loadMnemonic()
        } else {
            mnemonic = try await seedManager.generateAndSaveMnemonic()
        }

        // Create SDK config from SparkConfig
        let seed = Seed.mnemonic(mnemonic: mnemonic, passphrase: nil)
        var sdkConfig = defaultConfig(network: config.network == .mainnet ? .mainnet : .testnet)
        sdkConfig.apiKey = config.apiKey

        // Connect
        self.sdk = try await connect(request: ConnectRequest(
            config: sdkConfig,
            seed: seed,
            storageDir: config.storageDirectory
        ))

        // ...
    }
}
```

### Settings UI (iOS/macOS)

```swift
// Shared SwiftUI view for settings

import SwiftUI
import OpenAgentsCore

struct SparkSettingsView: View {
    @StateObject private var configManager = SparkConfigManager.shared

    @State private var apiKey = ""
    @State private var showingAPIKeyField = false
    @State private var saving = false

    var body: some View {
        Form {
            Section("Network") {
                Picker("Network", selection: $configManager.currentNetwork) {
                    ForEach(Network.allCases, id: \.self) { network in
                        Text(network.displayName).tag(network)
                    }
                }
                .onChange(of: configManager.currentNetwork) { newNetwork in
                    Task {
                        try? await configManager.switchNetwork(to: newNetwork)
                    }
                }

                Text("⚠️ Switching networks requires wallet re-initialization")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("API Key") {
                if showingAPIKeyField {
                    SecureField("Breez API Key", text: $apiKey)

                    Button("Save") {
                        Task {
                            saving = true
                            try? await configManager.updateAPIKey(apiKey)
                            showingAPIKeyField = false
                            apiKey = ""
                            saving = false
                        }
                    }
                    .disabled(apiKey.isEmpty || saving)
                } else {
                    Button("Update API Key") {
                        showingAPIKeyField = true
                    }
                }

                Link("Get API Key from Breez", destination: URL(string: "https://breez.technology/api-key")!)
                    .font(.caption)
            }

            Section("Storage") {
                if let config = configManager.currentConfig {
                    LabeledContent("Storage Directory") {
                        Text(config.storageDirectory)
                            .font(.caption.monospaced())
                            .textSelection(.enabled)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Spark SDK Settings")
    }
}
```

## Documentation

### README: Obtaining Breez API Key

Create `docs/compute/BREEZ_API_KEY.md`:

```markdown
# Breez API Key Setup

## Development

For local development, set the `BREEZ_API_KEY` environment variable:

```bash
export BREEZ_API_KEY="your-api-key-here"
```

Then run the app. The SDK will use the environment variable.

## Production

For production builds, store the API key in Keychain via the Settings UI:

1. Open OpenAgents Settings
2. Navigate to "Spark SDK Settings"
3. Click "Update API Key"
4. Paste your production API key
5. Click "Save"

The key is stored securely in Keychain and persists across app launches.

## Obtaining an API Key

1. Visit https://breez.technology/developers
2. Sign up for a Breez developer account
3. Create a new project
4. Copy the API key from the dashboard
5. Use the key in your OpenAgents configuration

### Free Tier Limits

- 1000 API calls/day
- Testnet only
- Community support

### Paid Tiers

- Unlimited API calls
- Mainnet access
- Priority support
- SLA guarantees

## Network Selection

- **Testnet**: Use for development and testing (free tier)
- **Mainnet**: Production use only (requires paid tier)
- **Signet**: Advanced testing (custom network)

⚠️ **Warning**: Switching networks clears wallet state. Back up your seed phrase before switching.
```

## Testing

### Mock Configuration

```swift
// OpenAgentsCoreTests/Lightning/MockSparkConfig.swift

import Foundation
@testable import OpenAgentsCore

extension SparkConfig {
    static func mock(
        network: Network = .testnet,
        apiKey: String = "mock-api-key-32-characters-long",
        storageDirectory: String? = nil
    ) -> SparkConfig {
        let storageDir = storageDirectory ?? NSTemporaryDirectory()

        return SparkConfig(
            network: network,
            apiKey: apiKey,
            storageDirectory: storageDir,
            loggingLevel: .debug
        )
    }
}
```

### Unit Tests

```swift
// OpenAgentsCoreTests/Lightning/SparkConfigTests.swift

import XCTest
@testable import OpenAgentsCore

class SparkConfigTests: XCTestCase {
    func testValidConfig() throws {
        let config = SparkConfig.mock()

        XCTAssertNoThrow(try config.validate())
    }

    func testMissingAPIKey() {
        let config = SparkConfig(
            network: .testnet,
            apiKey: "",
            storageDirectory: NSTemporaryDirectory()
        )

        XCTAssertThrowsError(try config.validate()) { error in
            XCTAssertEqual(error as? ConfigError, .missingAPIKey)
        }
    }

    func testInvalidStorageDirectory() {
        let config = SparkConfig(
            network: .testnet,
            apiKey: "valid-key-32-characters-long",
            storageDirectory: "/nonexistent/directory"
        )

        XCTAssertThrowsError(try config.validate())
    }
}

class APIKeyManagerTests: XCTestCase {
    func testSaveAndLoadAPIKey() async throws {
        let manager = APIKeyManager()

        let testKey = "test-api-key-32-characters-long"

        try await manager.saveAPIKey(testKey)

        let loadedKey = try await manager.getAPIKey()

        XCTAssertEqual(loadedKey, testKey)

        // Cleanup
        try await manager.deleteAPIKey()
    }

    func testFallbackToEnvironmentVariable() async throws {
        let manager = APIKeyManager()

        // Set environment variable
        setenv("BREEZ_API_KEY", "env-api-key-32-characters-long", 1)

        let key = try await manager.getAPIKey()

        XCTAssertEqual(key, "env-api-key-32-characters-long")

        // Cleanup
        unsetenv("BREEZ_API_KEY")
    }
}
```

## Dependencies

### OpenAgents Issues

- **Issue #010**: iOS Wallet (uses SparkConfigManager)
- **Issue #013**: macOS Wallet (uses SparkConfigManager)

### External

- Breez Spark SDK: https://sdk-doc-spark.breez.technology/
- Breez Developer Portal: https://breez.technology/developers

## Success Metrics

- [ ] API key can be stored in Keychain on iOS and macOS
- [ ] API key falls back to environment variable for development
- [ ] Network switching works without app reinstall
- [ ] Config validation catches missing/invalid keys
- [ ] Settings UI allows key updates
- [ ] Mock config supports unit testing without real Breez backend
- [ ] Documentation is clear and complete

## Apple Compliance

### Privacy

✅ **No sensitive data in logs**: API key is redacted in error messages
✅ **Keychain security**: Uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`

## Future Enhancements

- [ ] **Multi-environment configs**: Dev, staging, production
- [ ] **API key rotation**: Automatic rotation with Breez backend
- [ ] **Config validation UI**: Test API key before saving
- [ ] **Network diagnostics**: Check connectivity to Breez backend

## Notes

- **Singleton pattern**: `SparkConfigManager.shared` ensures single config source
- **Environment fallback**: Simplifies development (no Keychain setup required)
- **Network isolation**: Mainnet/testnet use separate storage directories
- **Validation early**: Config errors caught at initialization, not runtime

## Reference

- **Breez SDK Docs**: https://sdk-doc-spark.breez.technology/guide/getting_started.html
- **Keychain Services**: https://developer.apple.com/documentation/security/keychain_services
