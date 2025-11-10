# Issue #001: Implement MLX Embedding Provider

**Component:** `component:embeddings`
**Priority:** `priority:p0`
**Status:** `status:proposed`
**Effort:** 3-5 days
**Assignee:** TBD
**Created:** 2025-11-10

## Description

Implement the `MLXEmbeddingProvider` actor that uses Apple's MLX Swift framework to generate embeddings using the BGE-small-en-v1.5 model from Hugging Face.

This is the foundational component for all semantic search functionality in OpenAgents.

## Goals

1. Implement `EmbeddingProvider` protocol with MLX backend
2. Support automatic model downloading from Hugging Face Hub
3. Implement L2 normalization using Accelerate framework
4. Support batch embedding for efficiency
5. Handle model availability and error states

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── Embeddings/
    ├── EmbeddingProvider.swift          # Protocol definition
    ├── EmbeddingTypes.swift             # Request/response types
    └── MLXEmbeddingProvider.swift       # MLX implementation
```

### Key Components

1. **Protocol**: `EmbeddingProvider`
   - `loadModel() async throws`
   - `embed(_ text: String, normalize: Bool) async throws -> [Float]`
   - `embedBatch(_ texts: [String], normalize: Bool) async throws -> [[Float]]`
   - `var availability: ModelAvailability { get }`

2. **MLX Provider**: `MLXEmbeddingProvider`
   - Model ID: `mlx-community/bge-small-en-v1.5-6bit`
   - Dimensions: 384
   - Uses `Accelerate` for L2 normalization
   - Caches model in `~/.cache/huggingface/`

3. **Types**: Request/response envelopes
   - `EmbedRequest`
   - `EmbedResponse`
   - `ModelAvailability`
   - `EmbeddingError`

## Acceptance Criteria

- [ ] `EmbeddingProvider` protocol defined with async methods
- [ ] `MLXEmbeddingProvider` actor implemented (macOS-only)
- [ ] Model downloads automatically on first use
- [ ] L2 normalization produces unit vectors (norm = 1.0 ±1e-6)
- [ ] Batch embedding works for 1-100 texts
- [ ] Error handling for model unavailable, download failure, dimension mismatch
- [ ] Unit tests:
  - `testMLXProviderLoadsModel()`
  - `testEmbeddingNormalization()`
  - `testBatchEmbedding()`
  - `testErrorHandling()`
- [ ] Documentation with code examples

## Dependencies

**Packages:**
- Add `mlx-swift-examples` to `Package.swift`
- Link `MLXEmbedders` library

**Frameworks:**
- `Accelerate` (for vDSP operations)
- `Foundation` (for async/await)

## References

- [Embeddings Implementation Plan](../../../plans/embeddings-implementation-plan.md) § 4.1-4.3
- [BGE-small Model Card](https://huggingface.co/mlx-community/bge-small-en-v1.5-6bit)
- [MLX Swift Examples](https://github.com/ml-explore/mlx-swift-examples)

## Example Usage

```swift
let provider = MLXEmbeddingProvider()
try await provider.loadModel()

let embedding = try await provider.embed("Hello world", normalize: true)
print("Dimensions: \(embedding.count)")  // 384

let norm = sqrt(embedding.reduce(0.0) { $0 + $1 * $1 })
print("Norm: \(norm)")  // ≈1.0
```

## Notes

- macOS-only implementation (wrap in `#if os(macOS)`)
- First run downloads ~45 MB model (30-60 seconds)
- Subsequent runs load from cache (~1 second)
- Model runs on Apple Silicon GPU (fast) or CPU (slower)

## Testing

Run unit tests:
```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/EmbeddingsTests
```

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Documentation complete
- [ ] Merged to main branch
