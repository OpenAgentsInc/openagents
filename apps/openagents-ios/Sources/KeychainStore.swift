// Firm reason: 6 (iOS Keychain persistence for the phone's secp256k1 identity and paired computers, compatible with existing paired installs).
import Foundation
import Security

/// One computer this iPhone holds a signed NIP-HOST grant for.
/// Matches the existing Keychain JSON schema (`own-computer-paired-v1`) so existing installs keep their pairings.
struct PairedComputer: Codable, Equatable, Identifiable {
    var host: String
    var relay: String
    var grant: String
    var epoch: Int64
    var expires: Int64
    var name: String

    var id: String { host }

    var isExpired: Bool {
        expires <= Int64(Date().timeIntervalSince1970)
    }
}

/// Keychain storage scoped to this device only (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`).
struct MobileKeychainItem {
    let service: String
    let account: String

    init(service: String = "com.openagents.app.auth", account: String) {
        self.service = service
        self.account = account
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: false,
        ]
    }

    func load() -> Data? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess else { return nil }
        return result as? Data
    }

    @discardableResult
    func save(_ data: Data) -> Bool {
        let update = [kSecValueData as String: data]
        let status = SecItemUpdate(baseQuery as CFDictionary, update as CFDictionary)
        if status == errSecSuccess { return true }
        if status == errSecItemNotFound {
            var item = baseQuery
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            return SecItemAdd(item as CFDictionary, nil) == errSecSuccess
        }
        return false
    }

    func clear() {
        SecItemDelete(baseQuery as CFDictionary)
    }
}

/// Holds the phone's persistent secp256k1 device identity, paired computers, and per-computer Antigravity defaults.
final class MobileDeviceStore {
    private let keyItem: MobileKeychainItem
    private let pairedItem: MobileKeychainItem
    private let defaults: UserDefaults
    let tailnetBook: AppTailnetBook

    init(
        service: String = "com.openagents.app.auth",
        defaults: UserDefaults = .standard,
        tailnetBookURL: URL? = nil
    ) {
        self.keyItem = MobileKeychainItem(service: service, account: "own-computer-key-v1")
        self.pairedItem = MobileKeychainItem(service: service, account: "own-computer-paired-v1")
        self.defaults = defaults
        let url: URL
        if let tailnetBookURL {
            url = tailnetBookURL
        } else {
            let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
                ?? FileManager.default.temporaryDirectory
            url = support.appendingPathComponent("OpenAgents/tailnet-computers-v1.json")
        }
        self.tailnetBook = AppTailnetBook(url: url)
    }

    /// Returns the existing 32-byte secp256k1 secret key or creates and persists a new one.
    func loadOrCreateDeviceSecret() -> Data? {
        if let existing = keyItem.load(), existing.count == 32, MobileCore.publicKeyHex(secret: existing) != nil {
            return existing
        }
        for _ in 0..<4 {
            guard let candidate = MobileCore.randomBytes(32), MobileCore.publicKeyHex(secret: candidate) != nil else {
                continue
            }
            if keyItem.save(candidate) {
                return candidate
            }
        }
        return nil
    }

    func devicePublicKeyHex() -> String? {
        guard let secret = loadOrCreateDeviceSecret() else { return nil }
        return MobileCore.publicKeyHex(secret: secret)
    }

    var pairedComputers: [PairedComputer] {
        get {
            guard let data = pairedItem.load(),
                  let decoded = try? JSONDecoder().decode([PairedComputer].self, from: data) else {
                return []
            }
            return decoded
        }
        set {
            guard let data = try? JSONEncoder().encode(newValue) else { return }
            _ = pairedItem.save(data)
        }
    }

    func upsertPaired(_ computer: PairedComputer) {
        var list = pairedComputers.filter { $0.host != computer.host }
        list.insert(computer, at: 0)
        pairedComputers = list
    }

    func removePaired(host: String) {
        pairedComputers = pairedComputers.filter { $0.host != host }
    }

    // MARK: - Per-computer Antigravity preferences

    func selectedModel(for host: String) -> String {
        let saved = defaults.string(forKey: "oa.agy.model.\(host)") ?? ""
        return MobileCore.normalizeAntigravityModel(saved)
    }

    func setSelectedModel(_ model: String, for host: String) {
        defaults.set(MobileCore.normalizeAntigravityModel(model), forKey: "oa.agy.model.\(host)")
    }

    func selectedEffort(for host: String, model: String) -> String {
        let saved = defaults.string(forKey: "oa.agy.effort.\(host)") ?? ""
        return MobileCore.normalizeAntigravityEffort(model: model, want: saved)
    }

    func setSelectedEffort(_ effort: String, for host: String, model: String) {
        defaults.set(MobileCore.normalizeAntigravityEffort(model: model, want: effort), forKey: "oa.agy.effort.\(host)")
    }

    func selectedReviewMode(for host: String) -> ReviewMode {
        let raw = defaults.string(forKey: "oa.agy.review.\(host)") ?? ReviewMode.auto.rawValue
        return ReviewMode(rawValue: raw) ?? .auto
    }

    func setSelectedReviewMode(_ mode: ReviewMode, for host: String) {
        defaults.set(mode.rawValue, forKey: "oa.agy.review.\(host)")
    }

    var lastActiveHost: String? {
        get { defaults.string(forKey: "oa.agy.activeHost") }
        set { defaults.set(newValue, forKey: "oa.agy.activeHost") }
    }
}
