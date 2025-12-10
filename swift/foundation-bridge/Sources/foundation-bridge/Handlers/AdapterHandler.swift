import Foundation
import FoundationModels

/// Handler for adapter management endpoints
struct AdapterHandler {
    private let adapterRegistry: AdapterRegistry

    init(adapterRegistry: AdapterRegistry) {
        self.adapterRegistry = adapterRegistry
    }

    /// POST /v1/adapters/load - Load adapter from file or name
    func loadAdapter(body: String?) async throws -> HTTPResponse {
        guard let bodyData = body?.data(using: .utf8) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request body\"}"
            )
        }

        let decoder = JSONDecoder()
        guard let request = try? decoder.decode(LoadAdapterRequest.self, from: bodyData) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request format\"}"
            )
        }

        do {
            let adapterId: String

            if let fileURL = request.fileURL {
                // Load from file
                let url = URL(fileURLWithPath: fileURL)
                adapterId = try await adapterRegistry.loadFromFile(
                    fileURL: url,
                    adapterId: request.id
                )
            } else if let name = request.name {
                // Load by name
                adapterId = try await adapterRegistry.loadByName(
                    name: name,
                    adapterId: request.id
                )
            } else {
                return HTTPResponse(
                    status: 400,
                    headers: ["Content-Type": "application/json"],
                    body: "{\"error\":\"Must provide either file_url or name\"}"
                )
            }

            // Get adapter info
            guard let adapterInfo = await adapterRegistry.getAdapterInfo(adapterId) else {
                throw AdapterError.adapterNotFound
            }

            let response = LoadAdapterResponse(
                id: adapterId,
                loaded: true,
                adapter: adapterInfo
            )

            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(response)
            let json = String(data: data, encoding: .utf8) ?? "{}"

            return HTTPResponse(
                status: 201,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        } catch {
            let errorResponse = ErrorResponse(error: "Failed to load adapter: \(error.localizedDescription)")
            let encoder = JSONEncoder()
            let data = try encoder.encode(errorResponse)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Unknown error\"}"

            return HTTPResponse(
                status: 500,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }
    }

    /// GET /v1/adapters - List all loaded adapters
    func listAdapters() async throws -> HTTPResponse {
        let adapters = await adapterRegistry.listAdapters()

        let response = ListAdaptersResponse(
            adapters: adapters,
            count: adapters.count
        )

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// GET /v1/adapters/{id} - Get adapter info
    func getAdapter(id: String) async throws -> HTTPResponse {
        guard let adapterInfo = await adapterRegistry.getAdapterInfo(id) else {
            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Adapter not found\"}"
            )
        }

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(adapterInfo)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// DELETE /v1/adapters/{id} - Unload adapter
    func unloadAdapter(id: String) async throws -> HTTPResponse {
        let unloaded = await adapterRegistry.unloadAdapter(id)

        if unloaded {
            let response = UnloadAdapterResponse(
                id: id,
                unloaded: true
            )

            let encoder = JSONEncoder()
            let data = try encoder.encode(response)
            let json = String(data: data, encoding: .utf8) ?? "{}"

            return HTTPResponse(
                status: 200,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        } else {
            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Adapter not found\"}"
            )
        }
    }

    /// POST /v1/adapters/{id}/compile - Recompile adapter
    func recompileAdapter(id: String) async throws -> HTTPResponse {
        do {
            try await adapterRegistry.recompileAdapter(id)

            let response = CompileAdapterResponse(
                id: id,
                compiled: true,
                compiledAt: await adapterRegistry.getCompilationTime(id) ?? Date()
            )

            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            let data = try encoder.encode(response)
            let json = String(data: data, encoding: .utf8) ?? "{}"

            return HTTPResponse(
                status: 200,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        } catch {
            if case AdapterError.adapterNotFound = error {
                return HTTPResponse(
                    status: 404,
                    headers: ["Content-Type": "application/json"],
                    body: "{\"error\":\"Adapter not found\"}"
                )
            }

            let errorResponse = ErrorResponse(error: "Compilation failed: \(error.localizedDescription)")
            let encoder = JSONEncoder()
            let data = try encoder.encode(errorResponse)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Unknown error\"}"

            return HTTPResponse(
                status: 500,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }
    }

    /// GET /v1/adapters/compatible/{name} - Get compatible adapter identifiers
    func getCompatibleIdentifiers(name: String) async throws -> HTTPResponse {
        let identifiers = await adapterRegistry.getCompatibleIdentifiers(name: name)

        let response = CompatibleAdaptersResponse(
            name: name,
            compatibleIdentifiers: identifiers,
            count: identifiers.count
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// POST /v1/adapters/cleanup - Remove obsolete adapters
    func cleanupObsoleteAdapters() async throws -> HTTPResponse {
        do {
            try await adapterRegistry.cleanupObsoleteAdapters()

            let response = CleanupResponse(
                cleaned: true,
                message: "Obsolete adapters removed from system"
            )

            let encoder = JSONEncoder()
            let data = try encoder.encode(response)
            let json = String(data: data, encoding: .utf8) ?? "{}"

            return HTTPResponse(
                status: 200,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        } catch {
            let errorResponse = ErrorResponse(error: "Cleanup failed: \(error.localizedDescription)")
            let encoder = JSONEncoder()
            let data = try encoder.encode(errorResponse)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Unknown error\"}"

            return HTTPResponse(
                status: 500,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }
    }
}

// MARK: - Request/Response Types

struct LoadAdapterRequest: Codable {
    let id: String?
    let fileURL: String?
    let name: String?

    enum CodingKeys: String, CodingKey {
        case id
        case fileURL = "file_url"
        case name
    }
}

struct LoadAdapterResponse: Codable {
    let id: String
    let loaded: Bool
    let adapter: AdapterInfo

    enum CodingKeys: String, CodingKey {
        case id
        case loaded
        case adapter
    }
}

struct ListAdaptersResponse: Codable {
    let adapters: [AdapterInfo]
    let count: Int
}

struct UnloadAdapterResponse: Codable {
    let id: String
    let unloaded: Bool
}

struct CompileAdapterResponse: Codable {
    let id: String
    let compiled: Bool
    let compiledAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case compiled
        case compiledAt = "compiled_at"
    }
}

struct CompatibleAdaptersResponse: Codable {
    let name: String
    let compatibleIdentifiers: [String]
    let count: Int

    enum CodingKeys: String, CodingKey {
        case name
        case compatibleIdentifiers = "compatible_identifiers"
        case count
    }
}

struct CleanupResponse: Codable {
    let cleaned: Bool
    let message: String
}
