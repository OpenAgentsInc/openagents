# macOS: Model Router

**Phase:** 3 - Backends
**Component:** macOS App
**Priority:** P1 (High - Intelligent backend selection)
**Estimated Effort:** 2-3 weeks

## Summary

Implement a model router that intelligently selects the best backend (Foundation Models, MLX, Ollama) for each job based on requirements, cost, latency, and availability.

## Motivation

Multiple backends require intelligent routing:
- **Foundation Models**: Fast, private, limited
- **MLX**: Custom models, flexible, higher memory
- **Ollama**: Easy model management, HTTP API

Router optimizes for:
- ✅ **Latency**: Foundation Models for fast tasks
- ✅ **Cost**: Free on-device vs cloud fallback
- ✅ **Quality**: Best model for task
- ✅ **Availability**: Fallback when primary unavailable

## Acceptance Criteria

### Backend Abstraction
- [ ] `ModelBackend` protocol:
  - `generate(prompt:options:) async throws -> String`
  - `availability() -> BackendAvailability`
  - `capabilities() -> BackendCapabilities`
- [ ] Implementations:
  - `FoundationModelsBackend`
  - `MLXBackend`
  - `OllamaBackend`

### Routing Logic
- [ ] Route based on job kind
- [ ] Route based on model hint (param: "model")
- [ ] Route based on backend availability
- [ ] Fallback chain (primary → secondary → tertiary)
- [ ] Cost-aware routing (prefer free/on-device)

### Configuration
- [ ] Backend priority per job kind
- [ ] Fallback chains per job kind
- [ ] Backend enable/disable
- [ ] Model mapping (job kind → model name per backend)

## Technical Design

```swift
// ModelRouter.swift

class ModelRouter {
    private let config: RouterConfig
    private let backends: [BackendType: ModelBackend]

    enum BackendType {
        case foundationModels, mlx, ollama
    }

    struct RouterConfig {
        var fallbackChains: [JobKind: [BackendType]] = [
            .textSummarization: [.foundationModels, .mlx, .ollama],
            .codeGeneration: [.mlx, .ollama, .foundationModels],
            .qaRag: [.foundationModels, .mlx]
        ]

        var modelMappings: [JobKind: [BackendType: String]] = [
            .codeGeneration: [
                .mlx: "codellama-7b-q4",
                .ollama: "codellama:7b"
            ]
        ]
    }

    func route(job: DVMJobRequest) async throws -> (BackendType, ModelBackend) {
        let chain = config.fallbackChains[job.kind] ?? [.foundationModels]

        for backendType in chain {
            guard let backend = backends[backendType] else { continue }

            // Check availability
            let availability = await backend.availability()
            guard availability == .available else { continue }

            // Check capabilities
            let caps = backend.capabilities()
            guard caps.supports(jobKind: job.kind) else { continue }

            return (backendType, backend)
        }

        throw RouterError.noAvailableBackend(job.kind)
    }

    func execute(job: DVMJobRequest) async throws -> JobResult {
        let (backendType, backend) = try await route(job: job)

        let modelName = config.modelMappings[job.kind]?[backendType]
        let prompt = buildPrompt(from: job)

        let output = try await backend.generate(
            prompt: prompt,
            modelName: modelName,
            options: GenerationOptions(from: job.params)
        )

        return JobResult(
            output: output,
            metadata: [
                "backend": "\(backendType)",
                "model": modelName ?? "default"
            ],
            duration: 0  // TODO: track
        )
    }
}

protocol ModelBackend {
    func generate(
        prompt: String,
        modelName: String?,
        options: GenerationOptions
    ) async throws -> String

    func availability() async -> BackendAvailability
    func capabilities() -> BackendCapabilities
}

enum BackendAvailability {
    case available
    case unavailable(reason: String)
    case limited(reason: String)  // Available but degraded
}

struct BackendCapabilities {
    let supportedJobKinds: Set<JobKind>
    let maxContextLength: Int
    let supportsStreaming: Bool
    let supportsTools: Bool
}

struct GenerationOptions {
    var maxTokens: Int = 1000
    var temperature: Double = 0.7
    var topP: Double = 0.9
    var stopSequences: [String] = []

    init(from params: [String: String]?) {
        if let params = params {
            maxTokens = Int(params["max_tokens"] ?? "1000") ?? 1000
            temperature = Double(params["temperature"] ?? "0.7") ?? 0.7
            topP = Double(params["top_p"] ?? "0.9") ?? 0.9
        }
    }
}
```

## Dependencies

- **Issue #007**: macOS Worker
- **Issue #017**: MLX Integration
- **Issue #018**: Ollama Integration

## Testing

- [ ] Route to Foundation Models (job kind: summarization)
- [ ] Fallback to MLX when FM unavailable
- [ ] Model mapping per job kind
- [ ] All backends implement protocol
- [ ] Cost-based routing

## Success Metrics

- [ ] 95%+ jobs routed to optimal backend
- [ ] Fallback works when primary unavailable
- [ ] Avg latency improves vs single backend

## Future Enhancements

- Load balancing across multiple backends
- A/B testing (route % to different models)
- Quality-based routing (use best model for hard jobs)
- Cost optimization (minimize cloud API usage)
