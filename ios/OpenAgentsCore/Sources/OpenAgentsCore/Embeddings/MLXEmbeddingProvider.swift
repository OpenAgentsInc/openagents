#if os(macOS)
import Foundation
import MLXEmbedders
import Accelerate

/// MLX-based embedding provider using Hugging Face models
///
/// This provider uses the MLX Swift framework to run local embedding models
/// on Apple Silicon. Models are automatically downloaded from Hugging Face Hub
/// and cached locally (~/.cache/huggingface/hub/).
///
/// Default model: `mlx-community/bge-small-en-v1.5-6bit`
/// - Dimensions: 384
/// - Size: ~45 MB (6-bit quantized)
/// - Quality: Strong retrieval performance (MTEB benchmark)
/// - Speed: Fast on M1+ Apple Silicon
///
/// Example usage:
/// ```swift
/// let provider = MLXEmbeddingProvider()
/// try await provider.loadModel()
/// let embedding = try await provider.embed("Hello world", normalize: true)
/// ```
public actor MLXEmbeddingProvider: EmbeddingProvider {
    // MARK: - Properties

    public let modelID: String
    public let dimensions: Int
    private var embedder: Embedder?

    // MARK: - Availability

    public var availability: ModelAvailability {
        if embedder != nil {
            return .available
        }
        // TODO: Check download progress if in-flight
        return .unavailable(reason: "Model not loaded")
    }

    // MARK: - Initialization

    /// Initialize with a specific model
    ///
    /// - Parameters:
    ///   - modelID: Hugging Face model ID (e.g., "mlx-community/bge-small-en-v1.5-6bit")
    ///   - dimensions: Expected vector dimensions (must match model)
    public init(
        modelID: String = "mlx-community/bge-small-en-v1.5-6bit",
        dimensions: Int = 384
    ) {
        self.modelID = modelID
        self.dimensions = dimensions
    }

    // MARK: - Loading

    /// Load the model from Hugging Face Hub
    ///
    /// Downloads the model if not already cached, then loads it into memory.
    /// This operation is idempotent - calling multiple times is safe.
    ///
    /// - Throws: `EmbeddingError.downloadFailed` if download or loading fails
    public func loadModel() async throws {
        guard embedder == nil else {
            // Already loaded
            return
        }

        do {
            // MLX handles download automatically from Hugging Face Hub
            // Models are cached in ~/.cache/huggingface/hub/
            print("[MLXEmbeddingProvider] Loading model: \(modelID)")
            embedder = try await Embedder.from(modelID: modelID)
            print("[MLXEmbeddingProvider] Model loaded successfully")
        } catch {
            print("[MLXEmbeddingProvider] Failed to load model: \(error)")
            throw EmbeddingError.downloadFailed(underlying: error)
        }
    }

    // MARK: - Embedding

    /// Generate embeddings for multiple texts
    ///
    /// This is the primary method for generating embeddings. The single-text
    /// `embed()` method delegates to this.
    ///
    /// - Parameters:
    ///   - texts: Input texts to embed
    ///   - normalize: Whether to L2-normalize vectors (recommended for cosine similarity)
    /// - Returns: Array of embedding vectors
    /// - Throws: `EmbeddingError.modelNotLoaded` if model not loaded
    /// - Throws: `EmbeddingError.invalidDimensions` if vector size doesn't match expected
    public func embedBatch(_ texts: [String], normalize: Bool = true) async throws -> [[Float]] {
        guard let embedder = embedder else {
            throw EmbeddingError.modelNotLoaded
        }

        var results: [[Float]] = []
        results.reserveCapacity(texts.count)

        for text in texts {
            // Generate embedding
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

    // MARK: - Unloading

    /// Unload model from memory
    ///
    /// Frees memory by releasing the model. After calling this, `loadModel()`
    /// must be called again before generating embeddings.
    public func unloadModel() async {
        embedder = nil
        print("[MLXEmbeddingProvider] Model unloaded")
    }

    // MARK: - Private Helpers

    /// L2-normalize a vector using Accelerate framework
    ///
    /// After normalization, the vector has unit length (L2 norm = 1.0).
    /// This enables cosine similarity to be computed as a simple dot product.
    ///
    /// - Parameter vector: Input vector
    /// - Returns: Normalized vector with L2 norm = 1.0
    private func l2Normalize(_ vector: [Float]) -> [Float] {
        var normalized = vector
        var norm: Float = 0.0

        // Compute L2 norm (sum of squares, then square root)
        vDSP_svesq(vector, 1, &norm, vDSP_Length(vector.count))
        norm = sqrtf(norm)

        // Check for zero vector
        guard norm > 1e-12 else {
            // Zero vector, return as-is (can't normalize)
            print("[MLXEmbeddingProvider] Warning: Zero vector encountered, skipping normalization")
            return vector
        }

        // Divide each element by norm
        var normReciprocal = 1.0 / norm
        vDSP_vsmul(vector, 1, &normReciprocal, &normalized, 1, vDSP_Length(vector.count))

        return normalized
    }
}

// MARK: - Convenience Extensions

extension MLXEmbeddingProvider {
    /// Load model and generate a single embedding in one call
    ///
    /// Useful for one-off embeddings where model loading overhead is acceptable.
    ///
    /// - Parameters:
    ///   - text: Text to embed
    ///   - normalize: Whether to normalize the vector
    /// - Returns: Embedding vector
    public func embedWithAutoLoad(_ text: String, normalize: Bool = true) async throws -> [Float] {
        try await loadModel()
        return try await embed(text, normalize: normalize)
    }

    /// Check if the model is currently loaded
    public var isLoaded: Bool {
        embedder != nil
    }
}

#endif // os(macOS)
