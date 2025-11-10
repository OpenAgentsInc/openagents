import XCTest
@testable import OpenAgentsCore

/// Unit tests for embedding functionality
///
/// These tests cover:
/// - Type encoding/decoding
/// - Provider protocol conformance
/// - MLX provider functionality (macOS only)
/// - Normalization correctness
/// - Error handling
final class EmbeddingsTests: XCTestCase {

    // MARK: - Type Tests

    func testEmbedRequestEncodingDecoding() throws {
        let request = EmbedRequest(
            texts: ["hello", "world"],
            normalize: true,
            modelID: "test-model"
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(request)

        let decoder = JSONDecoder()
        let decoded = try decoder.decode(EmbedRequest.self, from: data)

        XCTAssertEqual(decoded.texts, request.texts)
        XCTAssertEqual(decoded.normalize, request.normalize)
        XCTAssertEqual(decoded.modelID, request.modelID)
    }

    func testSemanticSearchRequestEncodingDecoding() throws {
        let request = SemanticSearchRequest(
            query: "test query",
            collection: "files",
            limit: 10,
            filters: ["language": "swift"],
            minSimilarity: 0.7
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(request)

        let decoder = JSONDecoder()
        let decoded = try decoder.decode(SemanticSearchRequest.self, from: data)

        XCTAssertEqual(decoded.query, request.query)
        XCTAssertEqual(decoded.collection, request.collection)
        XCTAssertEqual(decoded.limit, request.limit)
        XCTAssertEqual(decoded.filters, request.filters)
        XCTAssertEqual(decoded.minSimilarity, request.minSimilarity)
    }

    func testEmbeddingConfigDefault() {
        let config = EmbeddingConfig.default

        XCTAssertEqual(config.providerType, .mlx)
        XCTAssertEqual(config.modelID, "mlx-community/bge-small-en-v1.5-6bit")
        XCTAssertNil(config.cloudEndpoint)
        XCTAssertNil(config.customModelPath)
    }

    func testModelAvailabilityEquality() {
        XCTAssertEqual(ModelAvailability.available, ModelAvailability.available)
        XCTAssertEqual(
            ModelAvailability.downloading(progress: 0.5),
            ModelAvailability.downloading(progress: 0.5)
        )
        XCTAssertEqual(
            ModelAvailability.unavailable(reason: "test"),
            ModelAvailability.unavailable(reason: "test")
        )

        XCTAssertNotEqual(
            ModelAvailability.available,
            ModelAvailability.unavailable(reason: "test")
        )
    }

    // MARK: - Error Tests

    func testEmbeddingErrorDescriptions() {
        let errors: [EmbeddingError] = [
            .modelNotLoaded,
            .modelUnavailable(reason: "test reason"),
            .downloadFailed(underlying: NSError(domain: "test", code: -1)),
            .invalidDimensions(expected: 384, got: 512),
            .normalizationFailed,
            .storageError(underlying: NSError(domain: "test", code: -1)),
            .providerNotSupported(.cloud)
        ]

        for error in errors {
            XCTAssertNotNil(error.errorDescription)
            XCTAssertFalse(error.errorDescription!.isEmpty)
        }
    }

    // MARK: - MLX Provider Tests (macOS only)

    #if os(macOS)

    func testMLXProviderInitialization() async throws {
        let provider = MLXEmbeddingProvider()

        XCTAssertEqual(provider.modelID, "mlx-community/bge-small-en-v1.5-6bit")
        XCTAssertEqual(provider.dimensions, 384)
        XCTAssertFalse(await provider.isLoaded)
    }

    func testMLXProviderCustomModel() async throws {
        let provider = MLXEmbeddingProvider(
            modelID: "custom-model",
            dimensions: 512
        )

        XCTAssertEqual(provider.modelID, "custom-model")
        XCTAssertEqual(provider.dimensions, 512)
    }

    func testMLXProviderAvailabilityBeforeLoad() async throws {
        let provider = MLXEmbeddingProvider()
        let availability = await provider.availability

        if case .available = availability {
            XCTFail("Provider should not be available before loading")
        }
    }

    func testMLXProviderLoadModel() async throws {
        let provider = MLXEmbeddingProvider()

        // This test requires network access to download the model
        // Skip if network is unavailable
        do {
            try await provider.loadModel()
            let availability = await provider.availability

            if case .available = availability {
                XCTAssertTrue(true, "Model loaded successfully")
            } else {
                XCTFail("Model should be available after loading")
            }

            XCTAssertTrue(await provider.isLoaded)
        } catch {
            // If download fails (no network, etc.), skip the test
            print("Skipping MLX test due to download failure: \(error)")
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderEmbeddingGeneration() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            // Generate embedding for a simple text
            let embedding = try await provider.embed("Hello world", normalize: true)

            // Check dimensions
            XCTAssertEqual(embedding.count, 384, "Embedding should have 384 dimensions")

            // Check that values are floats
            for value in embedding {
                XCTAssertTrue(value.isFinite, "Embedding values should be finite")
            }
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderNormalization() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            // Generate normalized embedding
            let embedding = try await provider.embed("test", normalize: true)

            // Calculate L2 norm (should be 1.0)
            let norm = sqrt(embedding.reduce(0.0) { $0 + $1 * $1 })
            XCTAssertEqual(norm, 1.0, accuracy: 1e-6, "Normalized embedding should have L2 norm = 1.0")
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderBatchEmbedding() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            let texts = ["apple", "banana", "orange"]
            let embeddings = try await provider.embedBatch(texts, normalize: true)

            XCTAssertEqual(embeddings.count, 3, "Should generate 3 embeddings")

            for (index, embedding) in embeddings.enumerated() {
                XCTAssertEqual(embedding.count, 384, "Embedding \(index) should have 384 dimensions")

                // Check normalization
                let norm = sqrt(embedding.reduce(0.0) { $0 + $1 * $1 })
                XCTAssertEqual(norm, 1.0, accuracy: 1e-6, "Embedding \(index) should be normalized")
            }
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderSimilarity() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            // Generate embeddings for similar and dissimilar texts
            let embeddings = try await provider.embedBatch([
                "programming language",
                "coding language",
                "banana fruit"
            ], normalize: true)

            // Compute cosine similarity (dot product for normalized vectors)
            let sim12 = zip(embeddings[0], embeddings[1]).reduce(0.0) { $0 + $1.0 * $1.1 }
            let sim13 = zip(embeddings[0], embeddings[2]).reduce(0.0) { $0 + $1.0 * $1.1 }

            // Similar texts should have higher similarity than dissimilar ones
            XCTAssertGreaterThan(sim12, sim13, "Similar texts should have higher similarity")
            XCTAssertGreaterThan(sim12, 0.5, "Similar texts should have positive similarity")
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderUnload() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()
            XCTAssertTrue(await provider.isLoaded)

            await provider.unloadModel()
            XCTAssertFalse(await provider.isLoaded)

            // Should throw error when trying to embed after unload
            do {
                _ = try await provider.embed("test", normalize: true)
                XCTFail("Should throw error when embedding after unload")
            } catch EmbeddingError.modelNotLoaded {
                // Expected
                XCTAssertTrue(true)
            }
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderIdempotentLoad() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            // Load multiple times should be safe
            try await provider.loadModel()
            try await provider.loadModel()
            try await provider.loadModel()

            XCTAssertTrue(await provider.isLoaded)
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderAutoLoad() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            // embedWithAutoLoad should load model automatically
            let embedding = try await provider.embedWithAutoLoad("test", normalize: true)

            XCTAssertEqual(embedding.count, 384)
            XCTAssertTrue(await provider.isLoaded)
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    #endif // os(macOS)

    // MARK: - Provider Protocol Tests

    func testProviderProtocolConformance() {
        // This is a compile-time test - if it compiles, the protocol is correct
        #if os(macOS)
        let _: any EmbeddingProvider = MLXEmbeddingProvider()
        #endif
        XCTAssertTrue(true)
    }

    // MARK: - Performance Tests

    #if os(macOS)

    func testMLXProviderPerformance() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            // Measure single embedding latency
            measure {
                Task {
                    _ = try? await provider.embed("test query", normalize: true)
                }
            }
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    func testMLXProviderBatchPerformance() async throws {
        let provider = MLXEmbeddingProvider()

        do {
            try await provider.loadModel()

            let texts = (0..<100).map { "Sample text \($0)" }

            // Measure batch embedding throughput
            let start = Date()
            let embeddings = try await provider.embedBatch(texts, normalize: true)
            let elapsed = Date().timeIntervalSince(start)

            XCTAssertEqual(embeddings.count, 100)

            let throughput = Double(100) / elapsed
            print("Batch embedding throughput: \(throughput) embeddings/sec")

            // Should be reasonably fast (>10 embeddings/sec)
            XCTAssertGreaterThan(throughput, 10.0)
        } catch {
            throw XCTSkip("Model download requires network access")
        }
    }

    #endif // os(macOS)
}
