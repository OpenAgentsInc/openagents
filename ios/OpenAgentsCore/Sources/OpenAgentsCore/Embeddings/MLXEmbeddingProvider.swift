#if os(macOS)
import Foundation
import MLX
import MLXEmbedders
import MLXNN
import Tokenizers

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
    private var container: ModelContainer?

    // MARK: - Availability

    public var availability: ModelAvailability {
        if container != nil {
            return .available
        }
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
        guard container == nil else {
            // Already loaded
            return
        }

        do {
            print("[MLXEmbeddingProvider] Loading model: \(modelID)")

            // Create model configuration
            let configuration = ModelConfiguration(id: modelID)

            // Load model container (downloads if needed, caches in ~/.cache/huggingface/hub/)
            container = try await loadModelContainer(configuration: configuration)

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
        guard let container = container else {
            throw EmbeddingError.modelNotLoaded
        }

        guard !texts.isEmpty else {
            return []
        }

        // Perform embedding within the model container for thread safety
        return try await container.perform { model, tokenizer, pooler in
            // Track which texts failed to tokenize
            var skipped: [Int] = []

            // Tokenize all texts
            let encoded = texts.enumerated().compactMap { index, text -> (Int, [Int])? in
                let tokens = tokenizer.encode(text: text, addSpecialTokens: true)
                guard !tokens.isEmpty else {
                    skipped.append(index)
                    return nil
                }
                return (index, tokens)
            }

            guard !encoded.isEmpty else {
                print("[MLXEmbeddingProvider] All texts failed to tokenize")
                return []
            }

            // Determine padding token
            guard let padToken = tokenizer.eosTokenId else {
                throw EmbeddingError.modelUnavailable(reason: "Could not determine padding token")
            }

            // Find maximum sequence length
            let maxLength = encoded.map { $0.1.count }.max() ?? 0

            // Pad all sequences to same length and stack into batch
            let paddedTokens = encoded.map { _, tokens in
                tokens + Array(repeating: padToken, count: maxLength - tokens.count)
            }

            let inputIds = MLX.stacked(paddedTokens.map { MLXArray($0) })

            // Create attention mask (1 for real tokens, 0 for padding)
            let mask = (inputIds .!= padToken)

            // Create token type IDs (all zeros for single-sequence tasks)
            let tokenTypeIds = MLXArray.zeros(like: inputIds)

            // Run model forward pass
            let outputs = model(
                inputIds,
                positionIds: nil,
                tokenTypeIds: tokenTypeIds,
                attentionMask: mask
            )

            // Apply pooling (pooling includes normalization if requested)
            let pooled = pooler(
                outputs,
                mask: mask,
                normalize: normalize,
                applyLayerNorm: false
            )

            // Evaluate the computation
            pooled.eval()

            // Extract vectors from pooled output
            let vectors = try Self.extractVectors(from: pooled, expectedCount: encoded.count)

            // Validate dimensions
            for vector in vectors {
                guard vector.count == self.dimensions else {
                    throw EmbeddingError.invalidDimensions(
                        expected: self.dimensions,
                        got: vector.count
                    )
                }
            }

            // If some texts were skipped, we need to insert empty vectors at their indices
            if skipped.isEmpty {
                return vectors
            } else {
                // Build result array with placeholders for skipped indices
                var result: [[Float]] = Array(repeating: [], count: texts.count)
                var vectorIndex = 0
                for (originalIndex, _) in encoded {
                    result[originalIndex] = vectors[vectorIndex]
                    vectorIndex += 1
                }
                // Skipped indices remain as empty arrays
                return result.filter { !$0.isEmpty }
            }
        }
    }

    // MARK: - Unloading

    /// Unload model from memory
    ///
    /// Frees memory by releasing the model. After calling this, `loadModel()`
    /// must be called again before generating embeddings.
    public func unloadModel() async {
        container = nil
        print("[MLXEmbeddingProvider] Model unloaded")
    }

    // MARK: - Private Helpers

    /// Extract Float vectors from MLXArray
    ///
    /// Handles both 2D (batch, dim) and 3D (batch, seq, dim) shapes.
    /// For 3D shapes, applies mean pooling over the sequence dimension.
    ///
    /// - Parameters:
    ///   - array: Pooled output from the model
    ///   - expectedCount: Number of vectors we expect
    /// - Returns: Array of Float vectors
    /// - Throws: `EmbeddingError` if shape is unexpected or count mismatches
    private static func extractVectors(from array: MLXArray, expectedCount: Int) throws -> [[Float]] {
        let shape = array.shape

        switch shape.count {
        case 2:
            // Shape: (batch_size, embedding_dim)
            // This is the expected shape after proper pooling
            let vectors = array.map { $0.asArray(Float.self) }
            guard vectors.count == expectedCount else {
                throw EmbeddingError.modelUnavailable(
                    reason: "Vector count mismatch: expected \(expectedCount), got \(vectors.count)"
                )
            }
            return vectors

        case 3:
            // Shape: (batch_size, seq_length, embedding_dim)
            // Pooling returned sequence embeddings; fall back to mean over tokens
            print("[MLXEmbeddingProvider] Warning: Pooling returned 3D tensor, applying mean over sequence")
            let reduced = MLX.mean(array, axis: 1)
            reduced.eval()
            let vectors = reduced.map { $0.asArray(Float.self) }
            guard vectors.count == expectedCount else {
                throw EmbeddingError.modelUnavailable(
                    reason: "Vector count mismatch after reduction: expected \(expectedCount), got \(vectors.count)"
                )
            }
            return vectors

        default:
            throw EmbeddingError.modelUnavailable(
                reason: "Unsupported pooling output shape: \(shape)"
            )
        }
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
        container != nil
    }
}

#endif // os(macOS)
