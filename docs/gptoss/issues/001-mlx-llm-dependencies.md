# Issue #1: Add MLXLLM Dependencies to Package.swift

**Phase:** 1 (Foundation)
**Priority:** P0 (Blocking)
**Estimated Effort:** 2-4 hours
**Dependencies:** None
**Related Issues:** #1467 (Embeddings), #1468 (Embeddings Audit)

---

## Summary

Add MLX Swift LLM dependencies (`MLXLLM`, `MLXLMCommon`, `Tokenizers`) to `Package.swift` to enable local language model integration following the same pattern used for embeddings (`MLXEmbedders`).

## Context

OpenAgents currently uses `mlx-swift-examples` for the embeddings system (`MLXEmbedders` product). We need to extend this to include LLM products for GPTOSS 20B integration.

**Current Package.swift** (`ios/OpenAgentsCore/Package.swift`):
```swift
dependencies: [
    .package(url: "https://github.com/ml-explore/mlx-swift-examples.git", from: "2.29.0"),
],
targets: [
    .target(
        name: "OpenAgentsCore",
        dependencies: [
            .product(name: "MLXEmbedders", package: "mlx-swift-examples"),  // Existing
        ]
    ),
]
```

## Acceptance Criteria

- [ ] `MLXLLM` product added to OpenAgentsCore target dependencies
- [ ] `MLXLMCommon` product added to OpenAgentsCore target dependencies
- [ ] `Tokenizers` product added to OpenAgentsCore target dependencies (explicit, not transitive)
- [ ] Project builds successfully on macOS with `xcodebuild -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk macosx`
- [ ] No new warnings or errors introduced
- [ ] Package resolution completes in <30 seconds
- [ ] Dependencies download and cache correctly

## Technical Details

### Package.swift Changes

**File:** `ios/OpenAgentsCore/Package.swift`

**Add to target dependencies** (around line 26):

```swift
.target(
    name: "OpenAgentsCore",
    dependencies: [
        .target(name: "OpenAgentsNostr"),
        .product(name: "MLXEmbedders", package: "mlx-swift-examples"),  // Existing
        .product(name: "MLXLLM", package: "mlx-swift-examples"),        // NEW
        .product(name: "MLXLMCommon", package: "mlx-swift-examples"),   // NEW
        .product(name: "Tokenizers", package: "mlx-swift-examples"),    // NEW (explicit)
    ]
),
```

### Verify Imports

Create a test file to verify imports work:

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/_Imports.swift`

```swift
// Verification file to ensure MLX LLM dependencies are accessible
// Delete this file after GPTOSSAgentProvider is implemented

#if os(macOS)
import MLX
import MLXLLM
import MLXLMCommon
import MLXNN
import Tokenizers

// Compile-time check that types are available
func _verifyMLXLLMImports() {
    // This function should compile but never be called
    let _: ModelConfiguration? = nil
    let _: GenerationOptions? = nil
    let _: LLM? = nil
}
#endif
```

### Package Products Available

From `mlx-swift-examples` (v2.29.0+):

- **MLXLLM**: Core LLM inference (model loading, generation)
- **MLXLMCommon**: Common types and utilities (ModelConfiguration, GenerationOptions, ChatSession)
- **Tokenizers**: Text tokenization (AutoTokenizer, encode/decode)
- **MLX**: Core MLX framework (arrays, ops) - transitive dependency
- **MLXNN**: Neural network layers - transitive dependency

**Note:** `Tokenizers` may be transitively available via `MLXLLM`, but we make it explicit for clarity.

## Testing Steps

1. **Clean build:**
   ```bash
   cd ios
   xcodebuild clean -workspace OpenAgents.xcworkspace -scheme OpenAgents
   ```

2. **Resolve packages:**
   ```bash
   xcodebuild -resolvePackageDependencies -workspace OpenAgents.xcworkspace -scheme OpenAgents
   ```

   Expected output: `Package resolution successful`

3. **Build macOS target:**
   ```bash
   xcodebuild build -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk macosx -configuration Debug
   ```

   Expected output: `BUILD SUCCEEDED`

4. **Build iOS target** (should still work):
   ```bash
   xcodebuild build -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk iphonesimulator -configuration Debug
   ```

   Expected output: `BUILD SUCCEEDED`

5. **Verify imports** (open in Xcode):
   - Open `OpenAgents.xcworkspace`
   - Navigate to `_Imports.swift`
   - Cmd+B to build
   - No errors or warnings

## Rollback Plan

If build fails or dependencies conflict:

1. Revert `Package.swift` changes
2. Clean derived data: `rm -rf ~/Library/Developer/Xcode/DerivedData/OpenAgents-*`
3. Resolve packages again
4. Investigate error messages:
   - Dependency conflicts → Check mlx-swift-examples version
   - Missing products → Verify mlx-swift-examples provides those products
   - Platform issues → Ensure macOS-only code uses `#if os(macOS)`

## References

- **MLX Swift Examples Repo:** https://github.com/ml-explore/mlx-swift-examples
- **Package Index:** https://swiftpackageindex.com/ml-explore/mlx-swift-examples
- **Embeddings Implementation:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift` (working example)
- **GPTOSS Research:** `docs/gptoss/research.md`
- **Integration Spec:** `docs/gptoss/gptoss-integration-spec.md` (Section 5.1)

## Notes

- This is a **pure dependency addition** - no functional code changes
- The `#if os(macOS)` guard will be used in actual provider implementation
- MLX dependencies are **macOS-only** but should not break iOS builds (they're simply not imported)
- After this issue, we can begin implementing GPTOSSAgentProvider (#2)

---

## Implementation Log — 2025‑11‑10

1) Added MLX LLM + Tokenizers dependencies
- Updated `ios/OpenAgentsCore/Package.swift` to include:
  - `.product(name: "MLXLLM", package: "mlx-swift-examples")`
  - `.product(name: "MLXLMCommon", package: "mlx-swift-examples")`
  - `.package(url: "https://github.com/huggingface/swift-transformers.git", from: "1.0.0")`
  - `.product(name: "Tokenizers", package: "swift-transformers")`

2) Verified macOS‑only imports compile
- Added `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/_Imports.swift` with `#if os(macOS)` and imports for MLX, MLXLLM, MLXLMCommon, MLXNN, Tokenizers.

3) Build validation
- Built `OpenAgentsCore` (macOS) with `xcodebuild`. Initially, `Tokenizers` was pointed at `mlx-swift-examples`; corrected to `swift-transformers` to resolve the product.

4) Commit & push
- Commit: "SPM: add MLXLLM, MLXLMCommon (mlx-swift-examples) and Tokenizers (swift-transformers) to OpenAgentsCore; add macOS-only import verification"

5) Continued with Issue #2 scaffolding
- Added GPTOSS Phase 1 scaffolding (ModelManager/Provider/Types) and extended `ACPSessionModeId` with `.gptoss_20b` to accelerate integration.

## Definition of Done

- [ ] Package.swift updated with 3 new product dependencies
- [ ] `xcodebuild` succeeds on both macOS and iOS targets
- [ ] `_Imports.swift` verification file compiles
- [ ] No new warnings in build log
- [ ] Changes committed with message: "Add MLXLLM dependencies to Package.swift"
- [ ] Ready for Issue #2 (GPTOSSAgentProvider implementation)
