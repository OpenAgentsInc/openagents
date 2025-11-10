import Foundation

// MARK: - Request/Response Types

/// Request to generate embeddings for one or more texts
public struct EmbedRequest: Codable, Sendable {
    /// Text inputs to embed
    public var texts: [String]

    /// Whether to L2-normalize output vectors (recommended for cosine similarity)
    public var normalize: Bool

    /// Optional model ID override (default uses service's configured model)
    public var modelID: String?

    public init(texts: [String], normalize: Bool = true, modelID: String? = nil) {
        self.texts = texts
        self.normalize = normalize
        self.modelID = modelID
    }
}

/// Response containing generated embeddings
public struct EmbedResponse: Codable, Sendable {
    /// Generated embedding vectors (one per input text)
    public var embeddings: [[Float]]

    /// Dimension of each embedding vector
    public var dimensions: Int

    /// Model ID used for generation
    public var modelID: String

    /// Processing time in milliseconds
    public var processingTimeMs: Double

    public init(embeddings: [[Float]], dimensions: Int, modelID: String, processingTimeMs: Double) {
        self.embeddings = embeddings
        self.dimensions = dimensions
        self.modelID = modelID
        self.processingTimeMs = processingTimeMs
    }
}

/// Request to search for semantically similar items
public struct SemanticSearchRequest: Codable, Sendable {
    /// Natural language query
    public var query: String

    /// Collection to search within (e.g., "files", "conversations")
    public var collection: String

    /// Maximum number of results to return
    public var limit: Int

    /// Optional metadata filters (exact match)
    public var filters: [String: String]?

    /// Minimum similarity threshold (0.0-1.0, cosine similarity)
    public var minSimilarity: Float?

    public init(
        query: String,
        collection: String,
        limit: Int = 10,
        filters: [String: String]? = nil,
        minSimilarity: Float? = nil
    ) {
        self.query = query
        self.collection = collection
        self.limit = limit
        self.filters = filters
        self.minSimilarity = minSimilarity
    }
}

/// A single search result with similarity score
public struct SemanticSearchResult: Codable, Sendable {
    /// Unique identifier (e.g., file path, conversation ID)
    public var id: String

    /// Cosine similarity score (0.0-1.0, higher is more similar)
    public var score: Float

    /// Optional metadata associated with this result
    public var metadata: [String: String]?

    /// Optional original text that was embedded
    public var text: String?

    public init(id: String, score: Float, metadata: [String: String]? = nil, text: String? = nil) {
        self.id = id
        self.score = score
        self.metadata = metadata
        self.text = text
    }
}

/// Response containing search results
public struct SemanticSearchResponse: Codable, Sendable {
    /// Search results ordered by descending similarity
    public var results: [SemanticSearchResult]

    /// Processing time in milliseconds
    public var processingTimeMs: Double

    /// Model ID used for query embedding
    public var modelID: String

    public init(results: [SemanticSearchResult], processingTimeMs: Double, modelID: String) {
        self.results = results
        self.processingTimeMs = processingTimeMs
        self.modelID = modelID
    }
}

// MARK: - Storage Types

/// A stored embedding with metadata
public struct StoredEmbedding: Codable, Sendable {
    /// Unique identifier
    public var id: String

    /// Logical collection (namespace)
    public var collection: String

    /// L2-normalized embedding vector
    public var embedding: [Float]

    /// Vector dimensions
    public var dimensions: Int

    /// Model ID used to generate this embedding
    public var modelID: String

    /// Optional metadata (key-value pairs)
    public var metadata: [String: String]?

    /// Optional source text
    public var text: String?

    /// Creation timestamp
    public var createdAt: Date

    /// Last update timestamp
    public var updatedAt: Date

    public init(
        id: String,
        collection: String,
        embedding: [Float],
        dimensions: Int,
        modelID: String,
        metadata: [String: String]? = nil,
        text: String? = nil,
        createdAt: Date = Date(),
        updatedAt: Date = Date()
    ) {
        self.id = id
        self.collection = collection
        self.embedding = embedding
        self.dimensions = dimensions
        self.modelID = modelID
        self.metadata = metadata
        self.text = text
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}

// MARK: - Error Types

/// Errors that can occur during embedding operations
public enum EmbeddingError: Error, LocalizedError {
    /// Model has not been loaded yet
    case modelNotLoaded

    /// Model is unavailable (with reason)
    case modelUnavailable(reason: String)

    /// Model download failed
    case downloadFailed(underlying: Error)

    /// Vector dimensions don't match expected size
    case invalidDimensions(expected: Int, got: Int)

    /// Failed to normalize vector (e.g., zero vector)
    case normalizationFailed

    /// Storage operation failed
    case storageError(underlying: Error)

    /// Provider type not supported
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
            return "Failed to L2-normalize vector (may be zero vector)"
        case .storageError(let error):
            return "Storage error: \(error.localizedDescription)"
        case .providerNotSupported(let type):
            return "Provider not supported: \(type.rawValue)"
        }
    }
}

// MARK: - Configuration

/// Embedding provider types
public enum ProviderType: String, Codable, Sendable {
    /// MLX-based local embeddings
    case mlx

    /// Apple NaturalLanguage framework
    case appleNL

    /// Cloud API (OpenAI, Anthropic, etc.)
    case cloud

    /// Custom user-supplied provider
    case custom
}

/// Configuration for embedding service
public struct EmbeddingConfig: Codable, Sendable {
    /// Provider type to use
    public var providerType: ProviderType

    /// Model identifier (e.g., Hugging Face model ID)
    public var modelID: String

    /// Cloud endpoint URL (for cloud provider)
    public var cloudEndpoint: String?

    /// Custom model path (for custom provider)
    public var customModelPath: String?

    public init(
        providerType: ProviderType = .mlx,
        modelID: String = "mlx-community/bge-small-en-v1.5-6bit",
        cloudEndpoint: String? = nil,
        customModelPath: String? = nil
    ) {
        self.providerType = providerType
        self.modelID = modelID
        self.cloudEndpoint = cloudEndpoint
        self.customModelPath = customModelPath
    }

    /// Default configuration (MLX with BGE-small)
    public static let `default` = EmbeddingConfig()
}

// MARK: - Model Availability

/// Model availability status
public enum ModelAvailability: Sendable {
    /// Model is loaded and ready to use
    case available

    /// Model is currently downloading (with progress 0.0-1.0)
    case downloading(progress: Double)

    /// Model is unavailable (with reason)
    case unavailable(reason: String)
}

extension ModelAvailability: Equatable {
    public static func == (lhs: ModelAvailability, rhs: ModelAvailability) -> Bool {
        switch (lhs, rhs) {
        case (.available, .available):
            return true
        case (.downloading(let p1), .downloading(let p2)):
            return abs(p1 - p2) < 0.001
        case (.unavailable(let r1), .unavailable(let r2)):
            return r1 == r2
        default:
            return false
        }
    }
}
