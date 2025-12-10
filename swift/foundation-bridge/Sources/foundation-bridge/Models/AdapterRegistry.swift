import Foundation
import FoundationModels

/// Actor for managing loaded adapters
actor AdapterRegistry {
    private var adapters: [String: AdapterEntry] = [:]
    private var compilationCache: [String: Date] = [:]

    struct AdapterEntry {
        let id: String
        let adapter: Adapter
        let fileURL: URL?
        let name: String?
        let loadedAt: Date
        var lastUsed: Date
        let metadata: [String: Any]

        init(id: String, adapter: Adapter, fileURL: URL?, name: String?) {
            self.id = id
            self.adapter = adapter
            self.fileURL = fileURL
            self.name = name
            self.loadedAt = Date()
            self.lastUsed = Date()
            self.metadata = adapter.creatorDefinedMetadata
        }
    }

    /// Load adapter from file URL
    func loadFromFile(fileURL: URL, adapterId: String? = nil) async throws -> String {
        // Check if already loaded
        if let existing = adapters.values.first(where: { $0.fileURL == fileURL }) {
            return existing.id
        }

        // Load adapter
        let adapter = try Adapter(fileURL: fileURL)

        // Compile/optimize for device
        try await adapter.compile()

        // Generate ID if not provided
        let id = adapterId ?? UUID().uuidString

        // Store entry
        let entry = AdapterEntry(
            id: id,
            adapter: adapter,
            fileURL: fileURL,
            name: nil
        )
        adapters[id] = entry
        compilationCache[id] = Date()

        return id
    }

    /// Load adapter by name
    func loadByName(name: String, adapterId: String? = nil) async throws -> String {
        // Check if already loaded
        if let existing = adapters.values.first(where: { $0.name == name }) {
            return existing.id
        }

        // Load adapter
        let adapter = try Adapter(name: name)

        // Compile/optimize for device
        try await adapter.compile()

        // Generate ID if not provided
        let id = adapterId ?? UUID().uuidString

        // Store entry
        let entry = AdapterEntry(
            id: id,
            adapter: adapter,
            fileURL: nil,
            name: name
        )
        adapters[id] = entry
        compilationCache[id] = Date()

        return id
    }

    /// Get adapter by ID
    func getAdapter(_ id: String) -> Adapter? {
        guard var entry = adapters[id] else {
            return nil
        }

        // Update last used
        entry.lastUsed = Date()
        adapters[id] = entry

        return entry.adapter
    }

    /// Get adapter info
    func getAdapterInfo(_ id: String) -> AdapterInfo? {
        guard let entry = adapters[id] else {
            return nil
        }

        return AdapterInfo(
            id: entry.id,
            name: entry.name,
            fileURL: entry.fileURL?.path,
            loadedAt: entry.loadedAt,
            lastUsed: entry.lastUsed,
            metadata: entry.metadata
        )
    }

    /// List all loaded adapters
    func listAdapters() -> [AdapterInfo] {
        return adapters.values.map { entry in
            AdapterInfo(
                id: entry.id,
                name: entry.name,
                fileURL: entry.fileURL?.path,
                loadedAt: entry.loadedAt,
                lastUsed: entry.lastUsed,
                metadata: entry.metadata
            )
        }
    }

    /// Unload adapter
    func unloadAdapter(_ id: String) -> Bool {
        let removed = adapters.removeValue(forKey: id) != nil
        compilationCache.removeValue(forKey: id)
        return removed
    }

    /// Unload all adapters
    func unloadAllAdapters() {
        adapters.removeAll()
        compilationCache.removeAll()
    }

    /// Get compatible adapter identifiers for a name
    func getCompatibleIdentifiers(name: String) -> [String] {
        return Adapter.compatibleAdapterIdentifiers(name: name)
    }

    /// Cleanup obsolete adapters from system
    func cleanupObsoleteAdapters() throws {
        try Adapter.removeObsoleteAdapters()
    }

    /// Check if adapter is compiled
    func isCompiled(_ id: String) -> Bool {
        return compilationCache[id] != nil
    }

    /// Get compilation timestamp
    func getCompilationTime(_ id: String) -> Date? {
        return compilationCache[id]
    }

    /// Recompile adapter (if needed after update)
    func recompileAdapter(_ id: String) async throws {
        guard let entry = adapters[id] else {
            throw AdapterError.adapterNotFound
        }

        try await entry.adapter.compile()
        compilationCache[id] = Date()
    }

    /// Cleanup old adapters (LRU eviction)
    func cleanupLRU(maxAge: TimeInterval) {
        let cutoff = Date().addingTimeInterval(-maxAge)
        let oldAdapterIds = adapters.filter { $0.value.lastUsed < cutoff }.map { $0.key }

        for id in oldAdapterIds {
            adapters.removeValue(forKey: id)
            compilationCache.removeValue(forKey: id)
        }
    }
}

/// Adapter information for API responses
struct AdapterInfo: Codable {
    let id: String
    let name: String?
    let fileURL: String?
    let loadedAt: Date
    let lastUsed: Date
    let metadata: [String: AnyCodable]

    init(id: String, name: String?, fileURL: String?, loadedAt: Date, lastUsed: Date, metadata: [String: Any]) {
        self.id = id
        self.name = name
        self.fileURL = fileURL
        self.loadedAt = loadedAt
        self.lastUsed = lastUsed

        // Convert metadata to AnyCodable
        var codableMetadata: [String: AnyCodable] = [:]
        for (key, value) in metadata {
            codableMetadata[key] = AnyCodable(value)
        }
        self.metadata = codableMetadata
    }

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case fileURL = "file_url"
        case loadedAt = "loaded_at"
        case lastUsed = "last_used"
        case metadata
    }
}

/// Adapter-related errors
enum AdapterError: Error {
    case adapterNotFound
    case invalidURL
    case compilationFailed
    case incompatibleDevice
}
