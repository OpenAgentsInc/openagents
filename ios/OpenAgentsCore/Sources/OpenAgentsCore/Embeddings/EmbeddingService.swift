#if os(macOS)
import Foundation

/// High-level service coordinating embedding provider and vector storage
public actor EmbeddingService {
    private let db: TinyvexDbLayer
    private var provider: any EmbeddingProvider
    private var store: VectorStore
    private let config: EmbeddingConfig

    public init(db: TinyvexDbLayer, config: EmbeddingConfig = .default) async throws {
        self.db = db
        self.config = config

        switch config.providerType {
        case .mlx:
            self.provider = MLXEmbeddingProvider(modelID: config.modelID, dimensions: 384)
        case .appleNL, .cloud, .custom:
            throw EmbeddingError.providerNotSupported(config.providerType)
        }

        try await provider.loadModel()
        let dims = await provider.dimensions
        let mid = await provider.modelID
        self.store = VectorStore(db: db, dimensions: dims, modelID: mid)
    }

    // MARK: - Generation
    public func generateEmbeddings(_ request: EmbedRequest) async throws -> EmbedResponse {
        let start = Date()
        let vectors = try await provider.embedBatch(request.texts, normalize: request.normalize)
        let elapsed = Date().timeIntervalSince(start) * 1000.0
        return EmbedResponse(
            embeddings: vectors,
            dimensions: await provider.dimensions,
            modelID: await provider.modelID,
            processingTimeMs: elapsed
        )
    }

    // MARK: - Storage
    public func storeEmbedding(
        id: String,
        collection: String,
        text: String,
        metadata: [String: String]? = nil
    ) async throws {
        let vector = try await provider.embed(text, normalize: true)
        try await store.store(
            id: id,
            collection: collection,
            embedding: vector,
            metadata: metadata,
            text: text
        )
    }

    public func storeBatch(
        items: [(id: String, text: String, metadata: [String: String]?)] ,
        collection: String
    ) async throws {
        let texts = items.map { $0.text }
        let vectors = try await provider.embedBatch(texts, normalize: true)
        let toStore = zip(items, vectors).map { tup in
            (id: tup.0.id, embedding: tup.1, metadata: tup.0.metadata, text: tup.0.text)
        }
        try await store.storeBatch(items: toStore, collection: collection)
    }

    // MARK: - Search
    public func semanticSearch(_ request: SemanticSearchRequest) async throws -> SemanticSearchResponse {
        let start = Date()
        let q = try await provider.embed(request.query, normalize: true)
        let results = try await store.search(
            query: q,
            collection: request.collection,
            limit: request.limit,
            minSimilarity: request.minSimilarity
        )
        let elapsed = Date().timeIntervalSince(start) * 1000.0
        return SemanticSearchResponse(results: results, processingTimeMs: elapsed, modelID: await provider.modelID)
    }
}
#endif
