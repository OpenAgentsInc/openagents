import Foundation
import FoundationModels

/// Actor that manages model adapters (fine-tuned models)
actor AdapterRegistry {
    private var adapters: [String: AdapterInfo] = [:]
    private var loadedAssets: [String: Any] = [:]  // Placeholder for actual adapter assets

    /// Load an adapter from a file
    func loadFromFile(fileURL: URL, adapterId: String? = nil) async throws -> String {
        let id = adapterId ?? UUID().uuidString

        // Check file exists
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            throw AdapterError.fileNotFound(fileURL.path)
        }

        // Get file info
        let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        let fileSize = attributes[.size] as? Int64 ?? 0

        // Create adapter info
        let info = AdapterInfo(
            id: id,
            name: fileURL.lastPathComponent,
            source: .file(fileURL.path),
            loaded: Date(),
            sizeBytes: fileSize
        )

        adapters[id] = info

        return id
    }

    /// Load an adapter by name (from system or known locations)
    func loadByName(name: String, adapterId: String? = nil) async throws -> String {
        let id = adapterId ?? UUID().uuidString

        // For now, just create a placeholder entry
        // In production, this would search known adapter locations
        let info = AdapterInfo(
            id: id,
            name: name,
            source: .named(name),
            loaded: Date(),
            sizeBytes: nil
        )

        adapters[id] = info

        return id
    }

    /// Get adapter info by ID
    func getAdapterInfo(_ id: String) -> AdapterInfo? {
        return adapters[id]
    }

    /// List all loaded adapters
    func listAdapters() -> [AdapterInfo] {
        return Array(adapters.values)
    }

    /// Unload an adapter
    func unloadAdapter(_ id: String) -> Bool {
        if adapters[id] != nil {
            adapters.removeValue(forKey: id)
            loadedAssets.removeValue(forKey: id)
            return true
        }
        return false
    }

    /// Recompile an adapter
    func recompileAdapter(_ id: String) async throws {
        guard adapters[id] != nil else {
            throw AdapterError.adapterNotFound
        }
        // Placeholder - actual recompilation would go here
        compilationTimes[id] = Date()
    }

    /// Get compilation time for an adapter
    func getCompilationTime(_ id: String) -> Date? {
        return compilationTimes[id]
    }

    /// Get compatible adapter identifiers for a name
    func getCompatibleIdentifiers(name: String) -> [String] {
        // Return adapters that match the name
        return adapters.values
            .filter { $0.name.lowercased().contains(name.lowercased()) }
            .map { $0.id }
    }

    /// Cleanup obsolete adapters
    func cleanupObsoleteAdapters() async throws {
        // Placeholder - would remove old/unused adapters
    }

    private var compilationTimes: [String: Date] = [:]
}

/// Information about a loaded adapter
struct AdapterInfo: Codable {
    let id: String
    let name: String
    let source: AdapterSource
    let loaded: Date
    let sizeBytes: Int64?

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case source
        case loaded
        case sizeBytes = "size_bytes"
    }
}

/// Source of an adapter
enum AdapterSource: Codable {
    case file(String)
    case named(String)

    enum CodingKeys: String, CodingKey {
        case type
        case path
        case name
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "file":
            let path = try container.decode(String.self, forKey: .path)
            self = .file(path)
        case "named":
            let name = try container.decode(String.self, forKey: .name)
            self = .named(name)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown adapter source type: \(type)"
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .file(let path):
            try container.encode("file", forKey: .type)
            try container.encode(path, forKey: .path)
        case .named(let name):
            try container.encode("named", forKey: .type)
            try container.encode(name, forKey: .name)
        }
    }
}

/// Errors for adapter operations
enum AdapterError: Error {
    case fileNotFound(String)
    case invalidFormat
    case loadFailed(String)
    case adapterNotFound

    var localizedDescription: String {
        switch self {
        case .fileNotFound(let path):
            return "Adapter file not found: \(path)"
        case .invalidFormat:
            return "Invalid adapter format"
        case .loadFailed(let reason):
            return "Failed to load adapter: \(reason)"
        case .adapterNotFound:
            return "Adapter not found"
        }
    }
}

