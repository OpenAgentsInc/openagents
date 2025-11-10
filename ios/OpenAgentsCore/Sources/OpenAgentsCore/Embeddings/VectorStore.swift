import Foundation
import Accelerate

/// Actor encapsulating vector storage and similarity search
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

        let rows = try db.fetchEmbeddings(collection: collection)

        var results: [(id: String, score: Float, metadata: [String: String]?)] = []
        results.reserveCapacity(rows.count)

        for row in rows {
            guard row.dimensions == dimensions else { continue }
            let score = cosine(query, row.embedding)
            if let th = minSimilarity, score < th { continue }
            results.append((id: row.id, score: score, metadata: row.metadata))
        }

        results.sort { $0.score > $1.score }
        let top = results.prefix(max(0, limit))

        return top.map { item in
            SemanticSearchResult(id: item.id, score: item.score, metadata: item.metadata, text: nil)
        }
    }

    // MARK: - Utilities

    /// Cosine similarity via dot product (assumes vectors are L2-normalized)
    private func cosine(_ a: [Float], _ b: [Float]) -> Float {
        var out: Float = 0.0
        vDSP_dotpr(a, 1, b, 1, &out, vDSP_Length(a.count))
        return out
    }
}

