// Firm reason: 6 (cross-Apple secret persistence: iOS Keychain com.openagents.app.auth vs macOS 0600 file ~/.openagents/macos/, plus offline transcript cache).
import Foundation
import Security

/// One computer this device holds a signed NIP-HOST grant for.
/// Matches the existing Keychain JSON schema (`own-computer-paired-v1`) so existing iOS installs keep their pairings.
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

protocol SecretBlobStore {
    func load() -> Data?
    @discardableResult func save(_ data: Data) -> Bool
    func clear()
}

#if os(iOS)
struct MobileKeychainItem: SecretBlobStore {
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
#endif

/// Owner-only 0600 file store used on macOS (avoids login Keychain prompts across ad-hoc rebuilds).
struct PrivateFileSecretStore: SecretBlobStore {
    let url: URL

    func load() -> Data? {
        try? Data(contentsOf: url)
    }

    @discardableResult
    func save(_ data: Data) -> Bool {
        let fm = FileManager.default
        let dir = url.deletingLastPathComponent()
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let tmp = url.appendingPathExtension("tmp")
        try? fm.removeItem(at: tmp)
        guard fm.createFile(atPath: tmp.path, contents: data, attributes: [.posixPermissions: 0o600]) else { return false }
        if rename(tmp.path, url.path) != 0 {
            try? fm.removeItem(at: tmp)
            return false
        }
        return true
    }

    func clear() {
        try? FileManager.default.removeItem(at: url)
    }
}

/// Offline snapshot cache for instant launch rendering before the Tailscale link finishes syncing.
struct CachedHostSnapshot: Codable {
    var hostID: String
    var workspaceName: String
    var sessions: [ChatSessionSummary]
    var lastSessionID: String?
    var lastItems: [TranscriptItem]
    var savedAt: Int64
}

/// Holds the device's persistent secp256k1 identity, paired computers, Tailscale book, preferences, and offline cache.
final class MobileDeviceStore {
    private let keyItem: SecretBlobStore
    private let pairedItem: SecretBlobStore
    private let pairedFileURL: URL?
    private let defaults: UserDefaults
    let tailnetBook: AppTailnetBook
    private let cacheURL: URL
    private let cacheQueue = DispatchQueue(label: "com.openagents.cache.io", qos: .utility)
    private var cacheWriteWorkItem: DispatchWorkItem?
    private var pendingCacheSnapshots: [String: CachedHostSnapshot] = [:]
    private let cacheLock = NSLock()
    /// Surfaces a non-fatal recovery warning if a corrupt persistence file was backed up instead of wiped (#14007).
    var lastPersistenceWarning: String?

    init(
        service: String = "com.openagents.app.auth",
        defaults: UserDefaults = .standard,
        tailnetBookURL: URL? = nil,
        cacheURL: URL? = nil,
        pairedFileURL: URL? = nil
    ) {
        self.defaults = defaults
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory

        #if os(iOS)
        self.keyItem = MobileKeychainItem(service: service, account: "own-computer-key-v1")
        if let customPaired = pairedFileURL {
            self.pairedItem = PrivateFileSecretStore(url: customPaired)
            self.pairedFileURL = customPaired
        } else {
            self.pairedItem = MobileKeychainItem(service: service, account: "own-computer-paired-v1")
            self.pairedFileURL = nil
        }
        self.tailnetBook = AppTailnetBook(url: tailnetBookURL ?? support.appendingPathComponent("OpenAgents/tailnet-computers-v1.json"))
        self.cacheURL = cacheURL ?? support.appendingPathComponent("OpenAgents/transcript-cache-v1.json")
        #else
        let home = FileManager.default.homeDirectoryForCurrentUser
        let dir = home.appendingPathComponent(".openagents/macos", isDirectory: true)
        let pURL = pairedFileURL ?? dir.appendingPathComponent("own-computer-paired-v1.json")
        self.keyItem = PrivateFileSecretStore(url: dir.appendingPathComponent("own-computer-key-v1"))
        self.pairedItem = PrivateFileSecretStore(url: pURL)
        self.pairedFileURL = pURL
        self.tailnetBook = AppTailnetBook(url: tailnetBookURL ?? dir.appendingPathComponent("tailnet-computers-v1.json"))
        self.cacheURL = cacheURL ?? dir.appendingPathComponent("transcript-cache-v1.json")
        #endif
    }

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

    var localClientLabel: String {
        #if os(iOS)
        return "OpenAgents iOS"
        #else
        return Host.current().localizedName ?? "OpenAgents macOS"
        #endif
    }

    private func backupCorruptBlob(_ raw: Data, label: String, fileURL: URL?) {
        let ts = Int64(Date().timeIntervalSince1970)
        if let fileURL {
            let backup = fileURL.deletingLastPathComponent()
                .appendingPathComponent("\(fileURL.lastPathComponent).corrupt.\(ts)")
            try? raw.write(to: backup, options: .atomic)
            lastPersistenceWarning = "Recovered from corrupt \(label); backup saved to \(backup.lastPathComponent)."
        } else {
            let backup = cacheURL.deletingLastPathComponent()
                .appendingPathComponent("\(label).corrupt.\(ts).json")
            try? FileManager.default.createDirectory(at: backup.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? raw.write(to: backup, options: .atomic)
            lastPersistenceWarning = "Recovered from corrupt \(label); backup saved to \(backup.lastPathComponent)."
        }
    }

    var pairedComputers: [PairedComputer] {
        get {
            guard let data = pairedItem.load(), !data.isEmpty else {
                return []
            }
            do {
                return try JSONDecoder().decode([PairedComputer].self, from: data)
            } catch {
                backupCorruptBlob(data, label: "own-computer-paired-v1.json", fileURL: pairedFileURL)
                return []
            }
        }
        set {
            // Protect against accidentally overwriting a corrupt-but-unrecovered file until backed up (#14007).
            if let existing = pairedItem.load(), !existing.isEmpty {
                if (try? JSONDecoder().decode([PairedComputer].self, from: existing)) == nil {
                    backupCorruptBlob(existing, label: "own-computer-paired-v1.json", fileURL: pairedFileURL)
                }
            }
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

    /// Resets saved pairings, discovered endpoints, and offline cache for isolated UI test launches.
    func resetForTests() {
        pairedItem.clear()
        tailnetBook.computers = []
        lastActiveHost = nil
        try? FileManager.default.removeItem(at: cacheURL)
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

    // MARK: - Offline transcript cache (debounced background I/O + eviction on forget; #14007)

    func loadCache(for hostID: String) -> CachedHostSnapshot? {
        cacheLock.lock()
        if let inMem = pendingCacheSnapshots[hostID] {
            cacheLock.unlock()
            return inMem
        }
        cacheLock.unlock()
        guard let data = try? Data(contentsOf: cacheURL),
              let all = try? JSONDecoder().decode([String: CachedHostSnapshot].self, from: data) else {
            return nil
        }
        return all[hostID]
    }

    func saveCache(_ snapshot: CachedHostSnapshot) {
        cacheLock.lock()
        pendingCacheSnapshots[snapshot.hostID] = snapshot
        cacheWriteWorkItem?.cancel()
        let url = cacheURL
        let work = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.cacheLock.lock()
            let dirty = self.pendingCacheSnapshots
            self.cacheLock.unlock()
            guard !dirty.isEmpty else { return }
            var all: [String: CachedHostSnapshot] = [:]
            if let data = try? Data(contentsOf: url),
               let decoded = try? JSONDecoder().decode([String: CachedHostSnapshot].self, from: data) {
                all = decoded
            }
            for (k, v) in dirty { all[k] = v }
            guard let encoded = try? JSONEncoder().encode(all) else { return }
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? encoded.write(to: url, options: .atomic)
        }
        cacheWriteWorkItem = work
        cacheLock.unlock()
        cacheQueue.asyncAfter(deadline: .now() + 0.4, execute: work)
    }

    func evictCache(for hostID: String) {
        cacheLock.lock()
        pendingCacheSnapshots.removeValue(forKey: hostID)
        cacheLock.unlock()
        let url = cacheURL
        cacheQueue.async {
            guard let data = try? Data(contentsOf: url),
                  var all = try? JSONDecoder().decode([String: CachedHostSnapshot].self, from: data),
                  all.removeValue(forKey: hostID) != nil,
                  let encoded = try? JSONEncoder().encode(all) else { return }
            try? encoded.write(to: url, options: .atomic)
        }
    }

    // MARK: - Durable Disk-Backed Offline Prompt Outbox (#14019)

    private var outboxURL: URL {
        cacheURL.deletingLastPathComponent().appendingPathComponent("offline-outbox-v1.json")
    }

    func loadPendingOutbox() -> [PendingOutboxMessage] {
        guard let data = try? Data(contentsOf: outboxURL),
              let decoded = try? JSONDecoder().decode([PendingOutboxMessage].self, from: data) else {
            return []
        }
        return decoded
    }

    func savePendingOutbox(_ items: [PendingOutboxMessage]) {
        guard let encoded = try? JSONEncoder().encode(items) else { return }
        try? FileManager.default.createDirectory(at: outboxURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? encoded.write(to: outboxURL, options: .atomic)
    }
}
