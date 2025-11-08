# macOS: MLX/Swift-MLX Integration

**Phase:** 3 - Backends
**Component:** macOS App
**Priority:** P1 (High - Backend diversity)
**Estimated Effort:** 3-4 weeks

## Summary

Integrate MLX (Apple's ML framework) and Swift-MLX bindings to enable custom model execution on macOS, expanding beyond Foundation Models to support GGUF models, fine-tuned models, and specialized workloads.

## Motivation

Foundation Models are powerful but limited:
- ❌ No custom models (fine-tunes, domain-specific)
- ❌ No control over model selection
- ❌ No quantization control (4-bit, 8-bit)

MLX enables:
- ✅ Custom model loading (GGUF, SafeTensors)
- ✅ Quantization (reduce memory, increase speed)
- ✅ Metal acceleration (GPU)
- ✅ Model zoo (LLaMA, Mistral, Phi, etc.)

## Acceptance Criteria

### MLX Integration
- [ ] Add `mlx-swift` as SwiftPM dependency
- [ ] Model loader (GGUF, SafeTensors formats)
- [ ] Tokenizer integration (sentencepiece, tiktoken)
- [ ] Inference engine (generate text)
- [ ] KV cache management (context reuse)
- [ ] Quantization support (4-bit, 8-bit, FP16)

### Model Management
- [ ] Download models from Hugging Face
- [ ] Local model storage (`~/Library/Application Support/OpenAgents/models/`)
- [ ] Model registry (installed models, metadata)
- [ ] Model deletion/cleanup (disk space management)
- [ ] Model verification (checksum, integrity)

### Backend Implementation
- [ ] `MLXBackend` conforming to `ModelBackend` protocol
- [ ] Session management (model loading, unloading)
- [ ] Streaming generation (token-by-token)
- [ ] Temperature, top-p, top-k sampling
- [ ] Stop sequences
- [ ] Error handling (OOM, model not found)

### Configuration
- [ ] Default model selection
- [ ] Model parameters (context length, quantization)
- [ ] GPU memory limit
- [ ] Model auto-download (enabled/disabled)

## Technical Design

```swift
// MLXBackend.swift

import Foundation
import MLX

class MLXBackend: ModelBackend {
    private let modelRegistry: ModelRegistry
    private var loadedModels: [String: LoadedModel] = [:]

    struct LoadedModel {
        let model: MLXModel
        let tokenizer: Tokenizer
        let config: ModelConfig
    }

    func generate(
        prompt: String,
        modelName: String,
        options: GenerationOptions
    ) async throws -> String {
        // Load model if not loaded
        if loadedModels[modelName] == nil {
            try await loadModel(name: modelName)
        }

        guard let loaded = loadedModels[modelName] else {
            throw MLXError.modelNotLoaded(modelName)
        }

        // Tokenize
        let tokens = loaded.tokenizer.encode(prompt)

        // Generate
        let output = try await loaded.model.generate(
            tokens: tokens,
            maxTokens: options.maxTokens,
            temperature: options.temperature,
            topP: options.topP
        )

        // Decode
        return loaded.tokenizer.decode(output)
    }

    private func loadModel(name: String) async throws {
        guard let modelPath = modelRegistry.path(for: name) else {
            throw MLXError.modelNotFound(name)
        }

        // Load model and tokenizer
        let model = try MLXModel.load(from: modelPath)
        let tokenizer = try Tokenizer.load(from: modelPath)

        loadedModels[name] = LoadedModel(
            model: model,
            tokenizer: tokenizer,
            config: try ModelConfig.load(from: modelPath)
        )
    }
}

// ModelRegistry.swift

class ModelRegistry {
    private let modelsDir: URL

    struct ModelInfo: Codable {
        let name: String
        let path: String
        let format: ModelFormat
        let size: Int64
        let quantization: Quantization?

        enum ModelFormat: String, Codable {
            case gguf, safetensors
        }

        enum Quantization: String, Codable {
            case q4_0, q4_1, q8_0, fp16, fp32
        }
    }

    func downloadModel(
        repo: String,  // e.g., "mlx-community/Llama-3.2-3B-Instruct-4bit"
        name: String
    ) async throws {
        let url = "https://huggingface.co/\(repo)/resolve/main/model.gguf"
        // Download and save to modelsDir
    }

    func installedModels() -> [ModelInfo] {
        // Scan modelsDir for models
        []
    }
}
```

## Dependencies

### Swift Packages
- **mlx-swift**: https://github.com/ml-explore/mlx-swift
  - Version: Latest stable

### System Frameworks
- **Metal**: GPU acceleration
- **Accelerate**: CPU BLAS operations

### OpenAgents
- **Issue #007**: macOS Worker (ModelBackend protocol)
- **Issue #020**: Model Router (backend selection)

## Apple Compliance

**ASRG 2.5.2 (No Downloaded Code)**
- ⚠️  **Model downloads**: Models are **data** (weights), not executable code
- ✅ MLX interprets model weights (doesn't execute arbitrary code)
- ✅ Similar to downloading ML models for on-device Core ML

**Privacy**: Models run on-device (no cloud API calls)

## Testing

- [ ] Load GGUF model
- [ ] Generate text (temperature, top-p)
- [ ] KV cache reuse
- [ ] Quantized models (4-bit, 8-bit)
- [ ] Download model from Hugging Face
- [ ] Model registry

## Success Metrics

- [ ] Load and run 3+ models (LLaMA, Mistral, Phi)
- [ ] Inference speed competitive with llama.cpp
- [ ] Memory usage within limits (4GB for 3B model)
- [ ] Streaming generation works

## Future Enhancements

- Fine-tuning support (LoRA)
- Multi-modal models (vision + text)
- Quantization-aware training
- Model merging/blending
