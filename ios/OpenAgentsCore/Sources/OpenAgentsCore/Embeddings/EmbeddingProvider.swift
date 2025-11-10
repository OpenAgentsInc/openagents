import Foundation

/// Protocol for swappable embedding providers
///
/// Implementations can use different models (MLX, CoreML, cloud APIs, Apple NL) while
/// providing a consistent interface for embedding generation.
///
/// All providers are actors for thread-safe access to model state.
public protocol EmbeddingProvider: Actor {
    /// Unique identifier for this model (e.g., Hugging Face model ID)
    var modelID: String { get }

    /// Vector dimensions produced by this model
    var dimensions: Int { get }

    /// Current availability status
    var availability: ModelAvailability { get }

    /// Load the model (download if necessary, then load into memory)
    ///
    /// This is an idempotent operation - calling multiple times should be safe.
    /// If the model is already loaded, this should return immediately.
    ///
    /// - Throws: `EmbeddingError.downloadFailed` if model download fails
    /// - Throws: `EmbeddingError.modelUnavailable` if model cannot be loaded
    func loadModel() async throws

    /// Generate embedding for a single text
    ///
    /// - Parameters:
    ///   - text: Input text to embed
    ///   - normalize: Whether to L2-normalize the output vector (recommended for cosine similarity)
    /// - Returns: Embedding vector of size `dimensions`
    /// - Throws: `EmbeddingError.modelNotLoaded` if model hasn't been loaded
    /// - Throws: `EmbeddingError.normalizationFailed` if normalization fails
    /// - Throws: `EmbeddingError.invalidDimensions` if output size doesn't match expected
    func embed(_ text: String, normalize: Bool) async throws -> [Float]

    /// Generate embeddings for multiple texts (batched for efficiency)
    ///
    /// Batching can significantly improve throughput for large datasets.
    ///
    /// - Parameters:
    ///   - texts: Input texts to embed
    ///   - normalize: Whether to L2-normalize the output vectors
    /// - Returns: Array of embedding vectors, one per input text
    /// - Throws: `EmbeddingError.modelNotLoaded` if model hasn't been loaded
    /// - Throws: `EmbeddingError.normalizationFailed` if normalization fails
    /// - Throws: `EmbeddingError.invalidDimensions` if output size doesn't match expected
    func embedBatch(_ texts: [String], normalize: Bool) async throws -> [[Float]]

    /// Unload model from memory (optional, for resource management)
    ///
    /// Implementations can use this to free memory when the model is idle.
    /// After calling this, `loadModel()` must be called again before embedding.
    func unloadModel() async
}

// MARK: - Default Implementations

extension EmbeddingProvider {
    /// Default implementation: single embed delegates to batch
    ///
    /// This provides a convenient default so implementations only need to implement `embedBatch`.
    public func embed(_ text: String, normalize: Bool = true) async throws -> [Float] {
        let batch = try await embedBatch([text], normalize: normalize)
        guard let first = batch.first else {
            throw EmbeddingError.normalizationFailed
        }
        return first
    }
}

// MARK: - Helper Extensions

extension EmbeddingProvider {
    /// Check if model is available and ready to use
    public var isAvailable: Bool {
        if case .available = availability {
            return true
        }
        return false
    }

    /// Check if model is currently downloading
    public var isDownloading: Bool {
        if case .downloading = availability {
            return true
        }
        return false
    }
}
