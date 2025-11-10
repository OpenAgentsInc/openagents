# OpenAgents Embeddings System - Implementation Plan

**Status:** Proposed
**Date:** 2025-11-10
**Owner:** OpenAgents Core Team
**Related Docs:**
- [Embeddings Research](../research/20251110-embeddings.md)
- [SearchKit Spec v0.2.2](../engine/spec-v0.2.2.md)
- ADR-0006 (Foundation Models)

---

## 1. Executive Summary

We're implementing a **reusable embeddings primitive** for OpenAgents using the **BGE-small-en-v1.5** model from Hugging Face via MLX Swift. The macOS desktop app will download and run the model locally, providing embedding generation and semantic search capabilities to all clients (iOS app, agents, orchestration) via the existing JSON-RPC bridge.

### Key Design Principles

1. **Reusable & Swappable**: Abstract interface allows model switching (local MLX, cloud APIs, Apple frameworks)
2. **macOS-Hosted**: All model loading and inference on macOS; iOS/agents consume via bridge
3. **Actor-Based**: Thread-safe service following existing patterns (`SessionUpdateHub`, `TinyvexDbLayer`)
4. **Storage-Ready**: SQLite vector storage with L2-normalized Float32 arrays
5. **ACP-Aligned**: JSON-RPC methods follow ACP conventions (snake_case, typed responses)

### Primary Use Cases

- **SearchKit**: Semantic code search, hybrid retrieval (FTS5 + embeddings)
- **Orchestration**: Context-aware file selection for agent exploration
- **Clustering**: Group similar conversations, files, or code blocks
- **RAG**: Retrieval-augmented generation for on-device LLM
- **Future**: Duplicate detection, recommendation, anomaly detection

---

## 2. Architecture Overview

### 2.1 Component Structure

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── Embeddings/                          # NEW MODULE
│   ├── EmbeddingProvider.swift          # Protocol for swappable providers
│   ├── MLXEmbeddingProvider.swift       # MLX-based implementation
│   ├── EmbeddingService.swift           # Main service actor (macOS-only)
│   ├── VectorStore.swift                # SQLite vector storage actor
│   ├── EmbeddingTypes.swift             # Request/response types
│   └── SimilaritySearch.swift           # Cosine search + utilities
│
├── DesktopBridge/
│   ├── DesktopWebSocketServer+Embeddings.swift  # NEW: JSON-RPC methods
│   └── DesktopWebSocketServer+Local.swift       # EXTEND: local mappings
│
├── Tinyvex/
│   └── DbLayer.swift                    # EXTEND: vector table schema
```

### 2.2 Service Layer Design

```
┌─────────────────────────────────────────────────────────────────┐
│                        macOS Application                         │
├─────────────────────────────────────────────────────────────────┤
│  LocalJsonRpcClient                                             │
│    ↓                                                             │
│  DesktopWebSocketServer                                         │
│    ├── EmbeddingService (actor)                                 │
│    │   ├── provider: EmbeddingProvider (MLX/Cloud/Apple)        │
│    │   └── vectorStore: VectorStore (actor)                     │
│    │       └── db: TinyvexDbLayer                               │
│    └── SessionUpdateHub, HistoryApi, etc.                       │
└─────────────────────────────────────────────────────────────────┘
                            ↓ JSON-RPC over WebSocket
┌─────────────────────────────────────────────────────────────────┐
│                      iOS App / Agents                            │
└─────────────────────────────────────────────────────────────────┘
```

**Rationale:**
- `EmbeddingService` coordinates provider and storage
- `EmbeddingProvider` protocol allows model swapping without changing service
- `VectorStore` isolates persistence, enables future ANN indexing
- JSON-RPC exposure follows existing bridge patterns

---

## 3. Model Selection & Management

### 3.1 Primary Model: BGE-small-en-v1.5 (MLX 6-bit)

**Model ID:** `mlx-community/bge-small-en-v1.5-6bit`

**Characteristics:**
- **Dimensions:** 384
- **Quality:** Strong retrieval performance (MTEB benchmark)
- **Size:** ~45 MB (6-bit quantized)
- **Speed:** Fast on Apple Silicon (M1+)
- **Languages:** English (optimized)

**Storage Footprint:**
- 384 dimensions × 4 bytes (Float32) = 1,536 bytes per vector
- 100,000 vectors ≈ 146 MB
- L2-normalized → cosine similarity = dot product (fast with Accelerate)

### 3.2 Model Download & Caching

**Hugging Face Hub Integration:**
- MLX Swift Examples handles download automatically via `Embedder.from(modelID:)`
- Cache location: `~/.cache/huggingface/hub/models--mlx-community--bge-small-en-v1.5-6bit/`
- First launch: ~30-60 second download (one-time)
- Subsequent launches: instant load from cache

**Availability Check:**
```swift
public enum ModelAvailability {
    case available
    case downloading(progress: Double)
    case unavailable(reason: String)
}
```

### 3.3 Provider Swapping Strategy

**Protocol Definition:**
```swift
public protocol EmbeddingProvider: Actor {
    var modelID: String { get }
    var dimensions: Int { get }
    var availability: ModelAvailability { get }

    func loadModel() async throws
    func embed(_ text: String) async throws -> [Float]
    func embedBatch(_ texts: [String]) async throws -> [[Float]]
}
```

**Planned Providers:**
1. **MLXEmbeddingProvider** (Phase 1): BGE-small via MLX Swift
2. **AppleNLEmbeddingProvider** (Phase 2): NaturalLanguage framework fallback
3. **CloudEmbeddingProvider** (Phase 3): OpenAI/Anthropic API (with policy gate)
4. **CustomMLXProvider** (Phase 3): User-supplied Hugging Face model ID

**Swapping Mechanism:**
```swift
// Configuration in orchestration_configs or new embeddings_config table
public struct EmbeddingConfig: Codable {
    public var providerType: ProviderType  // .mlx, .appleNL, .cloud, .custom
    public var modelID: String             // e.g., "mlx-community/bge-small-en-v1.5-6bit"
    public var cloudEndpoint: String?      // For cloud provider
    public var customModelPath: String?    // For local custom models
}
```

---

## 4. Implementation Details

### 4.1 Core Types (`EmbeddingTypes.swift`)

```swift
import Foundation

// MARK: - Request/Response Types

public struct EmbedRequest: Codable, Sendable {
    public var texts: [String]
    public var normalize: Bool = true          // L2 normalize output
    public var modelID: String?                // Override default model
}

public struct EmbedResponse: Codable, Sendable {
    public var embeddings: [[Float]]           // One vector per input text
    public var dimensions: Int
    public var modelID: String
    public var processingTimeMs: Double
}

public struct SemanticSearchRequest: Codable, Sendable {
    public var query: String
    public var collection: String              // e.g., "files", "conversations"
    public var limit: Int = 10
    public var filters: [String: String]?      // Optional metadata filters
    public var minSimilarity: Float?           // Threshold (0.0-1.0)
}

public struct SemanticSearchResult: Codable, Sendable {
    public var id: String                      // e.g., file path, conversation ID
    public var score: Float                    // Cosine similarity (0.0-1.0)
    public var metadata: [String: String]?     // Optional context
    public var text: String?                   // Original text (optional)
}

public struct SemanticSearchResponse: Codable, Sendable {
    public var results: [SemanticSearchResult]
    public var processingTimeMs: Double
    public var modelID: String
}

// MARK: - Storage Types

public struct StoredEmbedding: Codable, Sendable {
    public var id: String                      // Primary key
    public var collection: String              // Logical grouping
    public var embedding: [Float]              // L2-normalized vector
    public var dimensions: Int
    public var modelID: String
    public var metadata: [String: String]?
    public var text: String?                   // Optional source text
    public var createdAt: Date
    public var updatedAt: Date
}

// MARK: - Error Types

public enum EmbeddingError: Error, LocalizedError {
    case modelNotLoaded
    case modelUnavailable(reason: String)
    case downloadFailed(underlying: Error)
    case invalidDimensions(expected: Int, got: Int)
    case normalizationFailed
    case storageError(underlying: Error)
    case providerNotSupported(ProviderType)

    public var errorDescription: String? {
        switch self {
        case .modelNotLoaded:
            return "Embedding model is not loaded. Call loadModel() first."
        case .modelUnavailable(let reason):
            return "Embedding model unavailable: \(reason)"
        case .downloadFailed(let error):
            return "Model download failed: \(error.localizedDescription)"
        case .invalidDimensions(let expected, let got):
            return "Dimension mismatch: expected \(expected), got \(got)"
        case .normalizationFailed:
            return "Failed to L2-normalize vector"
        case .storageError(let error):
            return "Storage error: \(error.localizedDescription)"
        case .providerNotSupported(let type):
            return "Provider not supported: \(type)"
        }
    }
}

// MARK: - Configuration

public enum ProviderType: String, Codable {
    case mlx
    case appleNL
    case cloud
    case custom
}

public struct EmbeddingConfig: Codable, Sendable {
    public var providerType: ProviderType = .mlx
    public var modelID: String = "mlx-community/bge-small-en-v1.5-6bit"
    public var cloudEndpoint: String?
    public var customModelPath: String?

    public static let `default` = EmbeddingConfig()
}
```

### 4.2 Provider Protocol (`EmbeddingProvider.swift`)

```swift
import Foundation

public protocol EmbeddingProvider: Actor {
    /// Unique identifier for this model (e.g., Hugging Face model ID)
    var modelID: String { get }

    /// Vector dimensions produced by this model
    var dimensions: Int { get }

    /// Current availability status
    var availability: ModelAvailability { get }

    /// Load the model (download if necessary, then load into memory)
    func loadModel() async throws

    /// Generate embedding for a single text (L2-normalized if requested)
    func embed(_ text: String, normalize: Bool) async throws -> [Float]

    /// Generate embeddings for multiple texts (batched for efficiency)
    func embedBatch(_ texts: [String], normalize: Bool) async throws -> [[Float]]

    /// Unload model from memory (optional, for resource management)
    func unloadModel() async
}

public enum ModelAvailability {
    case available
    case downloading(progress: Double)
    case unavailable(reason: String)
}

// MARK: - Utilities

extension EmbeddingProvider {
    /// Default: single embed delegates to batch
    public func embed(_ text: String, normalize: Bool = true) async throws -> [Float] {
        let batch = try await embedBatch([text], normalize: normalize)
        guard let first = batch.first else {
            throw EmbeddingError.normalizationFailed
        }
        return first
    }
}
```

### 4.3 MLX Implementation (`MLXEmbeddingProvider.swift`)

```swift
#if os(macOS)
import Foundation
import MLXEmbedders
import Accelerate

public actor MLXEmbeddingProvider: EmbeddingProvider {
    public let modelID: String
    public let dimensions: Int
    private var embedder: Embedder?

    public var availability: ModelAvailability {
        if embedder != nil {
            return .available
        }
        // TODO: Check download progress if in-flight
        return .unavailable(reason: "Model not loaded")
    }

    public init(modelID: String = "mlx-community/bge-small-en-v1.5-6bit",
                dimensions: Int = 384) {
        self.modelID = modelID
        self.dimensions = dimensions
    }

    public func loadModel() async throws {
        guard embedder == nil else { return }  // Already loaded

        do {
            // MLX handles download automatically from Hugging Face Hub
            embedder = try await Embedder.from(modelID: modelID)
            print("[MLXEmbeddingProvider] Loaded model: \(modelID)")
        } catch {
            throw EmbeddingError.downloadFailed(underlying: error)
        }
    }

    public func embedBatch(_ texts: [String], normalize: Bool = true) async throws -> [[Float]] {
        guard let embedder = embedder else {
            throw EmbeddingError.modelNotLoaded
        }

        var results: [[Float]] = []
        results.reserveCapacity(texts.count)

        for text in texts {
            let vector = try await embedder.encode(text)

            // Validate dimensions
            guard vector.count == dimensions else {
                throw EmbeddingError.invalidDimensions(expected: dimensions, got: vector.count)
            }

            // Normalize if requested
            if normalize {
                let normalized = l2Normalize(vector)
                results.append(normalized)
            } else {
                results.append(vector)
            }
        }

        return results
    }

    public func unloadModel() async {
        embedder = nil
        print("[MLXEmbeddingProvider] Unloaded model")
    }

    // MARK: - Private Helpers

    private func l2Normalize(_ vector: [Float]) -> [Float] {
        var normalized = vector
        var norm: Float = 0.0

        // Compute L2 norm using Accelerate
        vDSP_svesq(vector, 1, &norm, vDSP_Length(vector.count))
        norm = sqrtf(norm)

        guard norm > 1e-12 else {
            // Zero vector, return as-is
            return vector
        }

        // Divide by norm
        var normReciprocal = 1.0 / norm
        vDSP_vsmul(vector, 1, &normReciprocal, &normalized, 1, vDSP_Length(vector.count))

        return normalized
    }
}
#endif
```

### 4.4 Storage Schema Extension (`DbLayer.swift`)

```swift
// MARK: - Schema Migration (add to TinyvexDbLayer)

private func ensureVectorSchemaV2() throws {
    let createEmbeddingsTable = """
    CREATE TABLE IF NOT EXISTS embeddings (
        id TEXT NOT NULL,
        collection TEXT NOT NULL,
        embedding_blob BLOB NOT NULL,
        dimensions INTEGER NOT NULL,
        model_id TEXT NOT NULL,
        metadata_json TEXT NULL,
        text TEXT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (id, collection)
    );
    """

    let createIndexes = """
    CREATE INDEX IF NOT EXISTS idx_embeddings_collection ON embeddings(collection);
    CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings(model_id);
    CREATE INDEX IF NOT EXISTS idx_embeddings_updated ON embeddings(updated_at);
    """

    try execute(createEmbeddingsTable, params: [])
    try execute(createIndexes, params: [])
}

// MARK: - Vector Storage Methods

public func storeEmbedding(
    id: String,
    collection: String,
    embedding: [Float],
    dimensions: Int,
    modelID: String,
    metadata: [String: String]?,
    text: String?
) throws {
    let metadataJSON = metadata.map { try? JSONEncoder().encode($0) }
        .flatMap { String(data: $0, encoding: .utf8) }

    // Serialize Float array to Data
    let embeddingData = embedding.withUnsafeBytes { Data($0) }

    let now = Int64(Date().timeIntervalSince1970 * 1000)

    let sql = """
    INSERT OR REPLACE INTO embeddings
    (id, collection, embedding_blob, dimensions, model_id, metadata_json, text, created_at, updated_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """

    try execute(sql, params: [
        id, collection, embeddingData, dimensions, modelID,
        metadataJSON ?? "", text ?? "", now, now
    ])
}

public func fetchEmbeddings(collection: String) throws -> [(id: String, embedding: [Float], metadata: [String: String]?)] {
    let sql = "SELECT id, embedding_blob, dimensions, metadata_json FROM embeddings WHERE collection = ?"
    let rows = try queryAll(sql, params: [collection])

    return rows.compactMap { row in
        guard let id = row["id"] as? String,
              let embeddingData = row["embedding_blob"] as? Data,
              let dimensions = row["dimensions"] as? Int else {
            return nil
        }

        // Deserialize Data to [Float]
        let embedding = embeddingData.withUnsafeBytes {
            Array(UnsafeBufferPointer<Float>(
                start: $0.baseAddress!.assumingMemoryBound(to: Float.self),
                count: dimensions
            ))
        }

        let metadata = (row["metadata_json"] as? String)
            .flatMap { $0.data(using: .utf8) }
            .flatMap { try? JSONDecoder().decode([String: String].self, from: $0) }

        return (id: id, embedding: embedding, metadata: metadata)
    }
}
```

### 4.5 Vector Store Actor (`VectorStore.swift`)

```swift
import Foundation
import Accelerate

public actor VectorStore {
    private let db: TinyvexDbLayer
    private let dimensions: Int
    private let modelID: String

    public init(db: TinyvexDbLayer, dimensions: Int, modelID: String) {
        self.db = db
        self.dimensions = dimensions
        self.modelID = modelID
    }

    // MARK: - Storage

    public func store(
        id: String,
        collection: String,
        embedding: [Float],
        metadata: [String: String]? = nil,
        text: String? = nil
    ) async throws {
        guard embedding.count == dimensions else {
            throw EmbeddingError.invalidDimensions(expected: dimensions, got: embedding.count)
        }

        try db.storeEmbedding(
            id: id,
            collection: collection,
            embedding: embedding,
            dimensions: dimensions,
            modelID: modelID,
            metadata: metadata,
            text: text
        )
    }

    public func storeBatch(
        items: [(id: String, embedding: [Float], metadata: [String: String]?, text: String?)],
        collection: String
    ) async throws {
        for item in items {
            try await store(
                id: item.id,
                collection: collection,
                embedding: item.embedding,
                metadata: item.metadata,
                text: item.text
            )
        }
    }

    // MARK: - Search

    public func search(
        query: [Float],
        collection: String,
        limit: Int = 10,
        minSimilarity: Float? = nil
    ) async throws -> [SemanticSearchResult] {
        guard query.count == dimensions else {
            throw EmbeddingError.invalidDimensions(expected: dimensions, got: query.count)
        }

        // Fetch all embeddings in collection (brute-force)
        let rows = try db.fetchEmbeddings(collection: collection)

        // Compute cosine similarities (dot product since vectors are normalized)
        var results: [(id: String, score: Float, metadata: [String: String]?)] = []

        for row in rows {
            let similarity = cosineSimilarity(query, row.embedding)

            // Apply threshold if specified
            if let minSim = minSimilarity, similarity < minSim {
                continue
            }

            results.append((id: row.id, score: similarity, metadata: row.metadata))
        }

        // Sort by score descending, take top-K
        results.sort { $0.score > $1.score }
        let topK = results.prefix(limit)

        return topK.map { result in
            SemanticSearchResult(
                id: result.id,
                score: result.score,
                metadata: result.metadata,
                text: nil  // Optional: fetch from DB if needed
            )
        }
    }

    // MARK: - Utilities

    private func cosineSimilarity(_ a: [Float], _ b: [Float]) -> Float {
        var result: Float = 0.0
        vDSP_dotpr(a, 1, b, 1, &result, vDSP_Length(a.count))
        return result
    }
}
```

### 4.6 Main Service (`EmbeddingService.swift`)

```swift
#if os(macOS)
import Foundation

public actor EmbeddingService {
    private let db: TinyvexDbLayer
    private var provider: EmbeddingProvider
    private var vectorStore: VectorStore?
    private let config: EmbeddingConfig

    public init(db: TinyvexDbLayer, config: EmbeddingConfig = .default) async throws {
        self.db = db
        self.config = config

        // Initialize provider based on config
        switch config.providerType {
        case .mlx:
            self.provider = MLXEmbeddingProvider(
                modelID: config.modelID,
                dimensions: 384  // BGE-small
            )
        case .appleNL, .cloud, .custom:
            throw EmbeddingError.providerNotSupported(config.providerType)
        }

        // Load model
        try await provider.loadModel()

        // Initialize vector store
        self.vectorStore = VectorStore(
            db: db,
            dimensions: provider.dimensions,
            modelID: provider.modelID
        )
    }

    // MARK: - Embedding Generation

    public func generateEmbeddings(_ request: EmbedRequest) async throws -> EmbedResponse {
        let start = Date()

        let embeddings = try await provider.embedBatch(
            request.texts,
            normalize: request.normalize
        )

        let elapsed = Date().timeIntervalSince(start) * 1000  // ms

        return EmbedResponse(
            embeddings: embeddings,
            dimensions: provider.dimensions,
            modelID: provider.modelID,
            processingTimeMs: elapsed
        )
    }

    // MARK: - Storage & Search

    public func storeEmbedding(
        id: String,
        collection: String,
        text: String,
        metadata: [String: String]? = nil
    ) async throws {
        guard let store = vectorStore else {
            throw EmbeddingError.storageError(underlying: NSError(domain: "VectorStore", code: -1))
        }

        // Generate embedding
        let embedding = try await provider.embed(text, normalize: true)

        // Store
        try await store.store(
            id: id,
            collection: collection,
            embedding: embedding,
            metadata: metadata,
            text: text
        )
    }

    public func semanticSearch(_ request: SemanticSearchRequest) async throws -> SemanticSearchResponse {
        guard let store = vectorStore else {
            throw EmbeddingError.storageError(underlying: NSError(domain: "VectorStore", code: -1))
        }

        let start = Date()

        // Embed query
        let queryEmbedding = try await provider.embed(request.query, normalize: true)

        // Search
        let results = try await store.search(
            query: queryEmbedding,
            collection: request.collection,
            limit: request.limit,
            minSimilarity: request.minSimilarity
        )

        let elapsed = Date().timeIntervalSince(start) * 1000  // ms

        return SemanticSearchResponse(
            results: results,
            processingTimeMs: elapsed,
            modelID: provider.modelID
        )
    }

    // MARK: - Batch Operations

    public func storeBatch(
        items: [(id: String, text: String, metadata: [String: String]?)],
        collection: String
    ) async throws {
        guard let store = vectorStore else {
            throw EmbeddingError.storageError(underlying: NSError(domain: "VectorStore", code: -1))
        }

        // Generate embeddings in batch
        let texts = items.map { $0.text }
        let embeddings = try await provider.embedBatch(texts, normalize: true)

        // Store in batch
        let storeItems = zip(items, embeddings).map { item, embedding in
            (id: item.id, embedding: embedding, metadata: item.metadata, text: item.text)
        }

        try await store.storeBatch(items: storeItems, collection: collection)
    }
}
#endif
```

### 4.7 Bridge Methods (`DesktopWebSocketServer+Embeddings.swift`)

```swift
#if os(macOS)
import Foundation

extension DesktopWebSocketServer {

    // MARK: - Setup

    func registerEmbeddingMethods() {
        router.register(method: "embedding/generate") { [weak self] id, params, _ in
            await self?.handleEmbeddingGenerate(id: id, params: params)
        }

        router.register(method: "embedding/store") { [weak self] id, params, _ in
            await self?.handleEmbeddingStore(id: id, params: params)
        }

        router.register(method: "embedding/search") { [weak self] id, params, _ in
            await self?.handleEmbeddingSearch(id: id, params: params)
        }

        router.register(method: "embedding/store_batch") { [weak self] id, params, _ in
            await self?.handleEmbeddingStoreBatch(id: id, params: params)
        }
    }

    // MARK: - Handlers

    private func handleEmbeddingGenerate(id: String, params: [String: Any]) async {
        guard let service = embeddingService else {
            sendError(id: id, code: "ERR_SERVICE_UNAVAILABLE", message: "Embedding service not initialized")
            return
        }

        do {
            let requestData = try JSONSerialization.data(withJSONObject: params)
            let request = try JSONDecoder().decode(EmbedRequest.self, from: requestData)

            let response = try await service.generateEmbeddings(request)
            let responseData = try JSONEncoder().encode(response)
            let responseDict = try JSONSerialization.jsonObject(with: responseData) as! [String: Any]

            sendResponse(id: id, result: responseDict)
        } catch {
            sendError(id: id, code: "ERR_EMBEDDING_FAILED", message: error.localizedDescription)
        }
    }

    private func handleEmbeddingStore(id: String, params: [String: Any]) async {
        guard let service = embeddingService else {
            sendError(id: id, code: "ERR_SERVICE_UNAVAILABLE", message: "Embedding service not initialized")
            return
        }

        do {
            guard let itemId = params["id"] as? String,
                  let collection = params["collection"] as? String,
                  let text = params["text"] as? String else {
                throw NSError(domain: "Params", code: -1, userInfo: [NSLocalizedDescriptionKey: "Missing required params"])
            }

            let metadata = params["metadata"] as? [String: String]

            try await service.storeEmbedding(
                id: itemId,
                collection: collection,
                text: text,
                metadata: metadata
            )

            sendResponse(id: id, result: ["success": true])
        } catch {
            sendError(id: id, code: "ERR_STORAGE_FAILED", message: error.localizedDescription)
        }
    }

    private func handleEmbeddingSearch(id: String, params: [String: Any]) async {
        guard let service = embeddingService else {
            sendError(id: id, code: "ERR_SERVICE_UNAVAILABLE", message: "Embedding service not initialized")
            return
        }

        do {
            let requestData = try JSONSerialization.data(withJSONObject: params)
            let request = try JSONDecoder().decode(SemanticSearchRequest.self, from: requestData)

            let response = try await service.semanticSearch(request)
            let responseData = try JSONEncoder().encode(response)
            let responseDict = try JSONSerialization.jsonObject(with: responseData) as! [String: Any]

            sendResponse(id: id, result: responseDict)
        } catch {
            sendError(id: id, code: "ERR_SEARCH_FAILED", message: error.localizedDescription)
        }
    }

    private func handleEmbeddingStoreBatch(id: String, params: [String: Any]) async {
        guard let service = embeddingService else {
            sendError(id: id, code: "ERR_SERVICE_UNAVAILABLE", message: "Embedding service not initialized")
            return
        }

        do {
            guard let collection = params["collection"] as? String,
                  let itemsArray = params["items"] as? [[String: Any]] else {
                throw NSError(domain: "Params", code: -1, userInfo: [NSLocalizedDescriptionKey: "Missing required params"])
            }

            let items = try itemsArray.map { itemDict -> (String, String, [String: String]?) in
                guard let id = itemDict["id"] as? String,
                      let text = itemDict["text"] as? String else {
                    throw NSError(domain: "Params", code: -1)
                }
                let metadata = itemDict["metadata"] as? [String: String]
                return (id, text, metadata)
            }

            try await service.storeBatch(items: items, collection: collection)

            sendResponse(id: id, result: ["success": true, "count": items.count])
        } catch {
            sendError(id: id, code: "ERR_BATCH_FAILED", message: error.localizedDescription)
        }
    }

    // MARK: - Helpers

    private func sendResponse(id: String, result: [String: Any]) {
        JsonRpcRouter.sendResponse(id: id, result: result) { [weak self] text in
            self?.currentClient?.send(text: text)
        }
    }

    private func sendError(id: String, code: String, message: String) {
        let error = ["code": code, "message": message]
        JsonRpcRouter.sendError(id: id, error: error) { [weak self] text in
            self?.currentClient?.send(text: text)
        }
    }
}
#endif
```

### 4.8 Local RPC Mappings (`DesktopWebSocketServer+Local.swift`)

```swift
// MARK: - Local Embedding Methods (add to existing file)

extension DesktopWebSocketServer {

    public func localEmbeddingGenerate(request: EmbedRequest) async throws -> EmbedResponse {
        guard let service = embeddingService else {
            throw EmbeddingError.modelUnavailable(reason: "Service not initialized")
        }
        return try await service.generateEmbeddings(request)
    }

    public func localEmbeddingStore(
        id: String,
        collection: String,
        text: String,
        metadata: [String: String]? = nil
    ) async throws {
        guard let service = embeddingService else {
            throw EmbeddingError.modelUnavailable(reason: "Service not initialized")
        }
        try await service.storeEmbedding(
            id: id,
            collection: collection,
            text: text,
            metadata: metadata
        )
    }

    public func localEmbeddingSearch(request: SemanticSearchRequest) async throws -> SemanticSearchResponse {
        guard let service = embeddingService else {
            throw EmbeddingError.modelUnavailable(reason: "Service not initialized")
        }
        return try await service.semanticSearch(request)
    }

    public func localEmbeddingStoreBatch(
        items: [(id: String, text: String, metadata: [String: String]?)],
        collection: String
    ) async throws {
        guard let service = embeddingService else {
            throw EmbeddingError.modelUnavailable(reason: "Service not initialized")
        }
        try await service.storeBatch(items: items, collection: collection)
    }
}
```

---

## 5. Integration Points

### 5.1 SearchKit Integration (Primary Use Case)

**Hybrid Search Flow:**

```swift
// In SearchKitService (future implementation)
actor SearchKitService {
    let embeddingService: EmbeddingService
    let ftsEngine: FTS5Engine

    func hybridSearch(query: String, limit: Int) async throws -> [SearchResult] {
        // 1. Lexical search (FTS5)
        let lexicalResults = try await ftsEngine.search(query, limit: limit * 2)

        // 2. Semantic search (embeddings)
        let semanticRequest = SemanticSearchRequest(
            query: query,
            collection: "files",
            limit: limit * 2
        )
        let semanticResponse = try await embeddingService.semanticSearch(semanticRequest)

        // 3. Reciprocal Rank Fusion (RRF)
        let fused = rrfFusion(lexical: lexicalResults, semantic: semanticResponse.results)

        return Array(fused.prefix(limit))
    }
}
```

**File Indexing:**

```swift
// When indexing files for SearchKit
func indexFile(path: String, content: String) async throws {
    // 1. Store in FTS5
    try await ftsEngine.indexDocument(path: path, content: content)

    // 2. Generate and store embedding
    let summary = generateFileSummary(content)  // 1-2 sentences
    try await embeddingService.storeEmbedding(
        id: path,
        collection: "files",
        text: summary,
        metadata: ["path": path, "language": detectLanguage(path)]
    )
}
```

### 5.2 Orchestration Integration

**Explore Codebase:**

```swift
// In ExploreOrchestrator
func findRelevantFiles(goal: String) async throws -> [String] {
    let request = SemanticSearchRequest(
        query: goal,
        collection: "files",
        limit: 10,
        minSimilarity: 0.7
    )

    let response = try await embeddingService.semanticSearch(request)
    return response.results.map { $0.id }
}
```

**Foundation Models Tool:**

```swift
// Add to FMTools.swift
@Tool(description: "Find files semantically similar to a description")
struct SemanticFileSearch {
    @Parameter(description: "Natural language description of desired files")
    var query: String

    @Parameter(description: "Number of results to return")
    var limit: Int = 5
}
```

### 5.3 Conversation Clustering

**Batch Embed Conversations:**

```swift
// Nightly job or on-demand
func embedAllConversations() async throws {
    let sessions = try await tinyvex.queryRecentSessions(limit: 1000)

    let items = sessions.map { session in
        let summary = summarizeConversation(session)
        return (
            id: session.sessionId,
            text: summary,
            metadata: ["created_at": "\(session.createdAt)"]
        )
    }

    try await embeddingService.storeBatch(items: items, collection: "conversations")
}
```

**Find Similar Conversations:**

```swift
func findSimilarConversations(to sessionId: String) async throws -> [String] {
    // Fetch embedding for session
    let embedding = try await vectorStore.fetchEmbedding(id: sessionId, collection: "conversations")

    // Search for similar
    let results = try await vectorStore.search(
        query: embedding,
        collection: "conversations",
        limit: 5
    )

    return results.map { $0.id }
}
```

### 5.4 Foundation Models RAG

**Context Retrieval:**

```swift
// In FMOrchestrator
func answerQuestionWithRAG(question: String) async throws -> String {
    // 1. Retrieve relevant context via semantic search
    let searchRequest = SemanticSearchRequest(
        query: question,
        collection: "files",
        limit: 5
    )
    let searchResponse = try await embeddingService.semanticSearch(searchRequest)

    // 2. Load full content for top results
    let context = searchResponse.results.compactMap { result in
        try? loadFileContent(path: result.id)
    }.joined(separator: "\n\n---\n\n")

    // 3. Pass to Foundation Models
    let prompt = """
    Context:
    \(context)

    Question: \(question)

    Answer based on the context above.
    """

    let session = LanguageModelSession(model: SystemLanguageModel.default)
    let response = try await session.respond(to: prompt)

    return response.content
}
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

**File:** `OpenAgentsCoreTests/EmbeddingsTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

final class EmbeddingsTests: XCTestCase {

    func testMLXProviderLoadsModel() async throws {
        #if os(macOS)
        let provider = MLXEmbeddingProvider()
        try await provider.loadModel()

        XCTAssertEqual(provider.availability, .available)
        XCTAssertEqual(provider.dimensions, 384)
        #endif
    }

    func testEmbeddingNormalization() async throws {
        #if os(macOS)
        let provider = MLXEmbeddingProvider()
        try await provider.loadModel()

        let embedding = try await provider.embed("test", normalize: true)

        // Verify L2 norm = 1.0
        let norm = sqrt(embedding.reduce(0.0) { $0 + $1 * $1 })
        XCTAssertEqual(norm, 1.0, accuracy: 1e-6)
        #endif
    }

    func testBatchEmbedding() async throws {
        #if os(macOS)
        let provider = MLXEmbeddingProvider()
        try await provider.loadModel()

        let texts = ["apple", "banana", "orange"]
        let embeddings = try await provider.embedBatch(texts, normalize: true)

        XCTAssertEqual(embeddings.count, 3)
        XCTAssertEqual(embeddings[0].count, 384)
        #endif
    }

    func testVectorStorageRoundTrip() async throws {
        let tempDB = try createTempDB()
        let store = VectorStore(db: tempDB, dimensions: 384, modelID: "test")

        let embedding = Array(repeating: Float(0.5), count: 384)

        try await store.store(
            id: "test_doc",
            collection: "test",
            embedding: embedding,
            metadata: ["key": "value"]
        )

        let results = try await store.search(
            query: embedding,
            collection: "test",
            limit: 1
        )

        XCTAssertEqual(results.count, 1)
        XCTAssertEqual(results[0].id, "test_doc")
        XCTAssertEqual(results[0].score, 1.0, accuracy: 1e-6)  // Perfect match
    }

    func testCosineSimilarity() async throws {
        let a: [Float] = [1, 0, 0]
        let b: [Float] = [1, 0, 0]  // Same direction
        let c: [Float] = [0, 1, 0]  // Orthogonal

        let store = VectorStore(db: try createTempDB(), dimensions: 3, modelID: "test")

        // Note: assumes vectors are normalized
        let simAB = await store.cosineSimilarity(a, b)
        let simAC = await store.cosineSimilarity(a, c)

        XCTAssertEqual(simAB, 1.0)
        XCTAssertEqual(simAC, 0.0)
    }
}
```

### 6.2 Integration Tests

**File:** `OpenAgentsCoreTests/EmbeddingServiceIntegrationTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

final class EmbeddingServiceIntegrationTests: XCTestCase {

    func testServiceInitializationAndEmbedding() async throws {
        #if os(macOS)
        let tempDB = try createTempDB()
        let service = try await EmbeddingService(db: tempDB)

        let request = EmbedRequest(texts: ["Hello world"])
        let response = try await service.generateEmbeddings(request)

        XCTAssertEqual(response.embeddings.count, 1)
        XCTAssertEqual(response.dimensions, 384)
        XCTAssertEqual(response.modelID, "mlx-community/bge-small-en-v1.5-6bit")
        #endif
    }

    func testEndToEndStorageAndSearch() async throws {
        #if os(macOS)
        let tempDB = try createTempDB()
        let service = try await EmbeddingService(db: tempDB)

        // Store some documents
        try await service.storeEmbedding(
            id: "doc1",
            collection: "test",
            text: "Swift programming language",
            metadata: ["type": "doc"]
        )

        try await service.storeEmbedding(
            id: "doc2",
            collection: "test",
            text: "Python programming language",
            metadata: ["type": "doc"]
        )

        try await service.storeEmbedding(
            id: "doc3",
            collection: "test",
            text: "Cooking recipes",
            metadata: ["type": "recipe"]
        )

        // Search for programming-related docs
        let searchRequest = SemanticSearchRequest(
            query: "programming languages",
            collection: "test",
            limit: 2
        )
        let searchResponse = try await service.semanticSearch(searchRequest)

        XCTAssertEqual(searchResponse.results.count, 2)
        XCTAssertTrue(["doc1", "doc2"].contains(searchResponse.results[0].id))
        XCTAssertTrue(searchResponse.results[0].score > 0.7)  // High similarity
        #endif
    }
}
```

### 6.3 Bridge Tests

**File:** `OpenAgentsCoreTests/EmbeddingBridgeTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

final class EmbeddingBridgeTests: XCTestCase {

    func testJSONRPCEmbeddingGenerate() async throws {
        #if os(macOS)
        let tempDB = try createTempDB()
        let server = DesktopWebSocketServer()
        try await server.initializeEmbeddingService(db: tempDB)

        let request = """
        {
            "jsonrpc": "2.0",
            "id": "1",
            "method": "embedding/generate",
            "params": {
                "texts": ["test"],
                "normalize": true
            }
        }
        """

        let response = try await server.handleRequest(request)
        let responseData = response.data(using: .utf8)!
        let json = try JSONSerialization.jsonObject(with: responseData) as! [String: Any]

        XCTAssertNotNil(json["result"])
        let result = json["result"] as! [String: Any]
        XCTAssertNotNil(result["embeddings"])
        XCTAssertEqual(result["dimensions"] as! Int, 384)
        #endif
    }

    func testLocalRPCMappings() async throws {
        #if os(macOS)
        let tempDB = try createTempDB()
        let server = DesktopWebSocketServer()
        try await server.initializeEmbeddingService(db: tempDB)

        let request = EmbedRequest(texts: ["test"])
        let response = try await server.localEmbeddingGenerate(request: request)

        XCTAssertEqual(response.embeddings.count, 1)
        XCTAssertEqual(response.dimensions, 384)
        #endif
    }
}
```

### 6.4 Performance Tests

**File:** `OpenAgentsCoreTests/EmbeddingPerformanceTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

final class EmbeddingPerformanceTests: XCTestCase {

    func testSingleEmbeddingLatency() async throws {
        #if os(macOS)
        let provider = MLXEmbeddingProvider()
        try await provider.loadModel()

        measure {
            Task {
                _ = try await provider.embed("test query", normalize: true)
            }
        }
        // Target: <50ms p50 on M1+
        #endif
    }

    func testBatchEmbeddingThroughput() async throws {
        #if os(macOS)
        let provider = MLXEmbeddingProvider()
        try await provider.loadModel()

        let texts = Array(repeating: "sample text", count: 100)

        let start = Date()
        let embeddings = try await provider.embedBatch(texts, normalize: true)
        let elapsed = Date().timeIntervalSince(start)

        XCTAssertEqual(embeddings.count, 100)
        XCTAssertLessThan(elapsed, 2.0)  // <2s for 100 embeddings
        print("Throughput: \(Double(100) / elapsed) embeddings/sec")
        #endif
    }

    func testSemanticSearchLatency() async throws {
        #if os(macOS)
        let tempDB = try createTempDB()
        let service = try await EmbeddingService(db: tempDB)

        // Index 1000 documents
        let items = (0..<1000).map { i in
            (id: "doc\(i)", text: "Sample document \(i)", metadata: nil)
        }
        try await service.storeBatch(items: items, collection: "perf_test")

        // Measure search
        let searchRequest = SemanticSearchRequest(
            query: "sample query",
            collection: "perf_test",
            limit: 10
        )

        let start = Date()
        let response = try await service.semanticSearch(searchRequest)
        let elapsed = Date().timeIntervalSince(start)

        XCTAssertEqual(response.results.count, 10)
        XCTAssertLessThan(elapsed, 0.5)  // <500ms for 1k brute-force
        print("Search latency: \(elapsed * 1000)ms")
        #endif
    }
}
```

---

## 7. Dependencies

### 7.1 SwiftPM Dependency Addition

**File:** `ios/OpenAgentsCore/Package.swift`

```swift
let package = Package(
    name: "OpenAgentsCore",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [
        .library(name: "OpenAgentsCore", targets: ["OpenAgentsCore"]),
    ],
    dependencies: [
        .package(path: "/Users/christopherdavid/code/nostr-sdk-ios"),

        // NEW: MLX Swift Examples for embeddings
        .package(url: "https://github.com/ml-explore/mlx-swift-examples.git",
                 from: "0.18.0"),
    ],
    targets: [
        .target(
            name: "OpenAgentsCore",
            dependencies: [
                .target(name: "OpenAgentsNostr"),

                // NEW: Link MLXEmbedders library (macOS-only code uses #if os(macOS))
                .product(name: "MLXEmbedders", package: "mlx-swift-examples"),
            ]
        ),
        .target(
            name: "OpenAgentsNostr",
            dependencies: [
                .product(name: "NostrSDK", package: "nostr-sdk-ios")
            ]
        ),
        .testTarget(
            name: "OpenAgentsCoreTests",
            dependencies: ["OpenAgentsCore"]
        )
    ]
)
```

**Note:** MLX Swift Examples will transitively pull in:
- `mlx-swift` (core MLX framework)
- `mlx-nn` (neural network ops)
- `Tokenizers` (text tokenization)

These are all macOS-compatible and handle model downloads automatically.

### 7.2 System Requirements

**macOS:**
- macOS 13.0+ (for SwiftPM and Swift 5.9+)
- Apple Silicon (M1/M2/M3) or Intel with GPU (for MLX acceleration)
- ~200 MB disk space (model + dependencies)
- ~500 MB RAM (model loaded)

**iOS:**
- iOS 16.0+ (to match existing app target)
- No embedding computation (bridge client only)
- No additional requirements

---

## 8. Milestones & Implementation Phases

### Phase 1: Foundation (Week 1-2)

**Goals:**
- Add MLX dependency
- Implement core types and provider protocol
- Implement MLXEmbeddingProvider
- Basic unit tests

**Deliverables:**
- ✅ `EmbeddingTypes.swift`
- ✅ `EmbeddingProvider.swift`
- ✅ `MLXEmbeddingProvider.swift`
- ✅ `EmbeddingsTests.swift` (provider tests)
- ✅ Package.swift updated

**Acceptance Criteria:**
- Can load BGE-small model
- Can generate normalized embeddings
- Unit tests pass on macOS

### Phase 2: Storage (Week 2-3)

**Goals:**
- Extend SQLite schema for vectors
- Implement VectorStore actor
- Implement cosine similarity search
- Integration tests

**Deliverables:**
- ✅ `VectorStore.swift`
- ✅ Schema migration in `DbLayer.swift`
- ✅ `EmbeddingServiceIntegrationTests.swift`

**Acceptance Criteria:**
- Can store/retrieve embeddings
- Search returns correct results by similarity
- Tests cover storage round-trip

### Phase 3: Service & Bridge (Week 3-4)

**Goals:**
- Implement EmbeddingService actor
- Add JSON-RPC methods
- Add local RPC mappings
- Bridge integration tests

**Deliverables:**
- ✅ `EmbeddingService.swift`
- ✅ `DesktopWebSocketServer+Embeddings.swift`
- ✅ `DesktopWebSocketServer+Local.swift` (extended)
- ✅ `EmbeddingBridgeTests.swift`

**Acceptance Criteria:**
- macOS app can generate embeddings via local API
- JSON-RPC methods work over WebSocket
- iOS app can call via bridge

### Phase 4: SearchKit Integration (Week 4-6)

**Goals:**
- Implement file indexing with embeddings
- Implement hybrid search (FTS5 + semantic)
- Add to orchestration tools
- End-to-end tests

**Deliverables:**
- ✅ `SearchKitService.swift` (new, uses EmbeddingService)
- ✅ Hybrid search implementation
- ✅ Integration with `ExploreOrchestrator`
- ✅ SearchKit E2E tests

**Acceptance Criteria:**
- Can index OpenAgents repo
- Hybrid search returns relevant files
- Orchestrator uses semantic search for file selection

### Phase 5: Optimization & Polish (Week 6-7)

**Goals:**
- Performance benchmarks
- Memory optimization
- Error handling polish
- Documentation

**Deliverables:**
- ✅ `EmbeddingPerformanceTests.swift`
- ✅ Performance benchmarks report
- ✅ Memory profiling results
- ✅ API documentation

**Acceptance Criteria:**
- Single embedding <50ms p50
- Batch 100 embeddings <2s
- Search 10k vectors <500ms
- All tests pass

### Phase 6: Provider Extensibility (Week 7-8, Optional)

**Goals:**
- Implement AppleNLEmbeddingProvider fallback
- Implement CloudEmbeddingProvider
- Configuration UI in macOS Settings
- Provider swapping tests

**Deliverables:**
- ✅ `AppleNLEmbeddingProvider.swift`
- ✅ `CloudEmbeddingProvider.swift`
- ✅ Settings UI for provider selection
- ✅ Provider swapping tests

**Acceptance Criteria:**
- Can switch providers without code changes
- Fallback to NaturalLanguage if MLX unavailable
- Cloud provider respects policy gates

---

## 9. Future Extensibility

### 9.1 Advanced Indexing (ANN)

**When:** After Phase 5, when brute-force becomes too slow (>100k vectors)

**Options:**
- HNSW (Hierarchical Navigable Small World) in Swift
- SQLite vector extension (e.g., sqlite-vec)
- External ANN library (FAISS, Annoy) via C interop

**Integration:**
```swift
protocol ANNIndex {
    func build(vectors: [[Float]]) async throws
    func search(query: [Float], k: Int) async throws -> [(id: String, distance: Float)]
}

// VectorStore uses ANNIndex instead of brute-force
```

### 9.2 Reranking

**When:** Phase 6+, to improve top-K quality

**Approach:**
- Use Foundation Models as a reranker (cross-encoder pattern)
- Or load a small reranking model via MLX (e.g., bge-reranker-v2-m3)

**Integration:**
```swift
func rerank(query: String, candidates: [SearchResult]) async throws -> [SearchResult] {
    let session = LanguageModelSession(...)
    let scores = try await session.scoreRelevance(query: query, documents: candidates)
    return zip(candidates, scores).sorted { $0.1 > $1.1 }.map { $0.0 }
}
```

### 9.3 Multilingual Models

**When:** User demand for non-English codebases

**Options:**
- `snowflake-arctic-embed-l-v2.0-4bit` (70+ languages)
- `Qwen3-Embedding-0.6B-4bit-DWQ` (multilingual)

**Integration:**
- Add model ID to config
- Swap provider with new model
- No code changes needed

### 9.4 Contextual Code Embeddings

**When:** Phase 7+, for deeper code understanding

**Approach:**
- Embed code with context (imports, surrounding functions)
- Use task-specific prefixes (BGE-style)
- E.g., "Represent this Swift function for retrieval: \(code)"

**Integration:**
```swift
func embedCode(code: String, context: String) async throws -> [Float] {
    let prompt = "Represent this Swift function for retrieval: \(context)\n\n\(code)"
    return try await provider.embed(prompt, normalize: true)
}
```

### 9.5 On-Device Fine-Tuning

**When:** Phase 8+, for domain-specific embeddings

**Approach:**
- Collect user interaction data (clicked files, ignored suggestions)
- Fine-tune embeddings model on-device (MLX supports training)
- Or train a small adapter layer

**Integration:**
```swift
actor EmbeddingTrainer {
    func fineTune(positiveExamples: [(query: String, doc: String)],
                  negativeExamples: [(query: String, doc: String)]) async throws {
        // Use MLX training APIs
    }
}
```

---

## 10. Open Questions & Decisions

### 10.1 Model Update Strategy

**Question:** How do we handle model updates?

**Options:**
1. **Pin to specific version** (e.g., `mlx-community/bge-small-en-v1.5-6bit@abc123`)
2. **Allow auto-update** (use `from: "0.18.0"` and let Hugging Face Hub update)
3. **User-controlled** (Settings UI with "Check for Updates" button)

**Recommendation:** Option 1 (pin) for stability, with manual update path in Phase 6.

### 10.2 Storage Format

**Question:** Float32 vs Float16 for vectors?

**Trade-offs:**
- Float32: Higher precision, 2x storage (1.5 KB per 384-d vector)
- Float16: Half storage (0.75 KB), minimal quality loss for cosine similarity

**Recommendation:** Start with Float32 (simpler), migrate to Float16 in Phase 5 if storage becomes an issue.

### 10.3 Batch Size

**Question:** What batch size for embedding generation?

**Options:**
- Single (1): Simple, but slower for large batches
- Small (8-16): Good balance for MLX
- Large (64+): Best throughput, but higher memory

**Recommendation:** Start with batch size 16, make configurable in Phase 5.

### 10.4 Index vs Embed-on-Demand

**Question:** Pre-index all files or embed on-demand?

**Recommendation:**
- **Pre-index** for known collections (files, conversations)
- **On-demand** for ad-hoc queries or rare operations
- Background indexing with progress reporting (like current ACP model)

### 10.5 Collection Strategy

**Question:** How to organize collections?

**Options:**
1. **By type:** `files`, `conversations`, `code_blocks`, `docs`
2. **By workspace:** `workspace:openagents:files`, `workspace:myproject:files`
3. **Hybrid:** Both (use metadata for workspace filtering)

**Recommendation:** Option 3 (hybrid) for maximum flexibility.

---

## 11. Risk Mitigation

### 11.1 Model Download Failure

**Risk:** Model download fails (network, disk space, permissions)

**Mitigation:**
- Graceful degradation: fall back to FTS5-only search
- Retry logic with exponential backoff
- Clear error messages to user
- Offline mode detection

### 11.2 Memory Pressure

**Risk:** Large models + many vectors = memory issues

**Mitigation:**
- Monitor memory usage (os_signpost)
- Unload model when idle (configurable timeout)
- Paginated search (stream results instead of loading all)
- Warn user if approaching limits

### 11.3 Performance Regression

**Risk:** Embedding generation slows down agent operations

**Mitigation:**
- Background embedding (don't block UI)
- Batch embedding (amortize overhead)
- Cache embeddings (don't re-embed same text)
- Performance tests in CI (fail if p95 > threshold)

### 11.4 Model Compatibility

**Risk:** MLX updates break our code

**Mitigation:**
- Pin MLX version in Package.swift
- Comprehensive integration tests
- Monitor MLX release notes
- Test new versions before upgrading

---

## 12. Success Metrics

### 12.1 Performance

- **Latency:** Embedding generation <50ms p50, <100ms p95 (single text)
- **Throughput:** >50 embeddings/sec (batch mode)
- **Search:** <500ms p95 for 10k vectors (brute-force)
- **Memory:** <500 MB RAM for model + index

### 12.2 Quality

- **Relevance:** SearchKit hybrid search improves over FTS5-only (user study or A/B test)
- **Recall:** Semantic search finds relevant files missed by lexical search
- **Precision:** Top-5 results have >80% relevance (manual evaluation)

### 12.3 Reliability

- **Availability:** Model loads successfully >99% of launches
- **Errors:** <1% embedding generation failures (excl. network)
- **Crash-free:** No crashes attributed to embedding service

### 12.4 Adoption

- **SearchKit usage:** >50% of Explore Codebase flows use semantic search
- **API usage:** >100 embedding API calls/week from agents
- **User feedback:** Positive sentiment on semantic search quality

---

## 13. Documentation Plan

### 13.1 Code Documentation

- Inline comments for complex algorithms (L2 norm, cosine similarity)
- DocC documentation for public APIs
- README in `Embeddings/` module

### 13.2 Architecture Decision Record

**ADR-0008: Embeddings System Architecture**

Topics:
- Why MLX over CoreML or NaturalLanguage
- Why BGE-small over alternatives
- Why actor-based service
- Why SQLite blob storage
- Why provider protocol for swapping

### 13.3 User-Facing Documentation

**Location:** `docs/embeddings/`

Files:
- `overview.md` - What are embeddings, why they're useful
- `models.md` - Supported models, how to swap
- `api.md` - JSON-RPC API reference
- `troubleshooting.md` - Common issues, solutions

### 13.4 Developer Guide

**Location:** `docs/embeddings/developer-guide.md`

Topics:
- Adding a new provider
- Custom model integration
- Performance tuning
- Testing new models

---

## 14. Appendix: Example Usage

### 14.1 From macOS App (Local)

```swift
// In a macOS View or ViewModel
import OpenAgentsCore

actor ExampleUsage {
    let server: DesktopWebSocketServer

    func generateEmbeddings() async throws {
        let request = EmbedRequest(texts: ["Hello world", "Goodbye world"])
        let response = try await server.localEmbeddingGenerate(request: request)

        print("Embeddings: \(response.embeddings.count)")
        print("Dimensions: \(response.dimensions)")
        print("Model: \(response.modelID)")
    }

    func searchFiles(query: String) async throws {
        let request = SemanticSearchRequest(
            query: query,
            collection: "files",
            limit: 10
        )
        let response = try await server.localEmbeddingSearch(request: request)

        for result in response.results {
            print("\(result.id): \(result.score)")
        }
    }
}
```

### 14.2 From iOS App (Bridge)

```swift
// In iOS View or ViewModel
import OpenAgentsCore

actor ExampleUsageIOS {
    let client: MobileWebSocketClient

    func searchFiles(query: String) async throws {
        let params: [String: Any] = [
            "query": query,
            "collection": "files",
            "limit": 10
        ]

        let response = try await client.sendRequest(
            method: "embedding/search",
            params: params
        )

        // Parse response
        guard let results = response["results"] as? [[String: Any]] else { return }

        for result in results {
            let id = result["id"] as? String ?? ""
            let score = result["score"] as? Float ?? 0.0
            print("\(id): \(score)")
        }
    }
}
```

### 14.3 From Agent (JSON-RPC)

```json
// Generate embeddings
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "embedding/generate",
  "params": {
    "texts": ["def factorial(n):", "class HttpClient {"],
    "normalize": true
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "embeddings": [[0.12, -0.34, ...], [0.45, 0.67, ...]],
    "dimensions": 384,
    "modelID": "mlx-community/bge-small-en-v1.5-6bit",
    "processingTimeMs": 42.5
  }
}

// Semantic search
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "embedding/search",
  "params": {
    "query": "authentication logic",
    "collection": "files",
    "limit": 5,
    "minSimilarity": 0.7
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": "2",
  "result": {
    "results": [
      {
        "id": "src/auth/middleware.swift",
        "score": 0.89,
        "metadata": {"language": "swift"}
      },
      {
        "id": "src/auth/jwt.swift",
        "score": 0.85,
        "metadata": {"language": "swift"}
      }
    ],
    "processingTimeMs": 127.3,
    "modelID": "mlx-community/bge-small-en-v1.5-6bit"
  }
}
```

---

## 15. Summary

This implementation plan provides a comprehensive blueprint for adding embeddings support to OpenAgents using BGE-small via MLX Swift. The design follows existing architectural patterns (actor-based services, JSON-RPC bridge, SQLite storage) and prioritizes:

1. **Reusability:** Protocol-based providers allow model swapping
2. **Performance:** Actor isolation, batching, Accelerate framework
3. **Integration:** Seamless with SearchKit, orchestration, Foundation Models
4. **Testing:** Comprehensive unit, integration, and performance tests
5. **Extensibility:** Clear path to ANN, reranking, multilingual, fine-tuning

The phased approach (8 weeks) balances speed of delivery with quality, starting with a working MVP (Phases 1-3) and progressively adding SearchKit integration, optimization, and extensibility.

**Next Steps:**
1. Review and approve this plan
2. Create GitHub issue from this document
3. Begin Phase 1 implementation
4. Iterate based on learnings and feedback
