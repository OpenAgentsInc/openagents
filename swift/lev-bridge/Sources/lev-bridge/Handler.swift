import Foundation
import FoundationModels

/// Answers one request. Every decision builds its own session, so no
/// question can see another question's text. That isolation is structural
/// here; the Rust side proves it with the planted-secret probe.
func handle(_ request: Request) async -> Response {
    switch request.op {
    case "availability":
        return availability(request)
    case "decide":
        return await decide(request)
    case "generate":
        return await generate(request)
    case "adapter_compat":
        return adapterCompat(request)
    case "adapter_load":
        return adapterLoad(request)
    default:
        return Response.failure(
            id: request.id, code: "invalid_request",
            message: "unknown op '\(request.op)'")
    }
}

private func model(for request: Request) throws -> SystemLanguageModel {
    let guardrails: SystemLanguageModel.Guardrails =
        request.guardrails == "permissive_content_transformations"
        ? .permissiveContentTransformations : .default
    // An adapted model replaces the use case rather than adding to it: the
    // runtime takes an adapter or a use case, not both.
    if let path = request.adapterPath {
        let adapter = try SystemLanguageModel.Adapter(fileURL: URL(fileURLWithPath: path))
        return SystemLanguageModel(adapter: adapter, guardrails: guardrails)
    }
    let useCase: SystemLanguageModel.UseCase =
        request.useCase == "content_tagging" ? .contentTagging : .general
    return SystemLanguageModel(useCase: useCase, guardrails: guardrails)
}

/// Asks the running base which adapter identifiers it will accept.
///
/// This is the only way to learn the live base model signature from outside:
/// the identifiers come back as `fmadapter-<name>-<signature prefix>`, so a
/// package can be checked against the device before it is attached.
private func adapterCompat(_ request: Request) -> Response {
    let name = request.adapterName ?? "lev"
    var response = Response(id: request.id, ok: true)
    response.compatibleAdapters = SystemLanguageModel.Adapter.compatibleAdapterIdentifiers(name: name)
    return response
}

/// Loads a package and reports what the runtime made of it.
private func adapterLoad(_ request: Request) -> Response {
    guard let path = request.adapterPath else {
        return Response.failure(
            id: request.id, code: "invalid_request", message: "adapter_load needs an adapterPath")
    }
    do {
        let adapter = try SystemLanguageModel.Adapter(fileURL: URL(fileURLWithPath: path))
        var response = Response(id: request.id, ok: true)
        response.adapterMetadata = adapter.creatorDefinedMetadata.mapValues { "\($0)" }
        return response
    } catch {
        return adapterFailure(request.id, error)
    }
}

/// Maps the runtime's adapter asset errors onto stable codes.
private func adapterFailure(_ id: String, _ error: Error) -> Response {
    if let asset = error as? SystemLanguageModel.Adapter.AssetError {
        let code: String
        switch asset {
        case .invalidAsset: code = "adapter_invalid"
        case .invalidAdapterName: code = "adapter_not_found"
        case .compatibleAdapterNotFound: code = "adapter_incompatible"
        @unknown default: code = "adapter_invalid"
        }
        return Response.failure(id: id, code: code, message: "\(asset)")
    }
    return failure(id, error)
}

private func availability(_ request: Request) -> Response {
    var response = Response(id: request.id, ok: true)
    let resolved: SystemLanguageModel
    do {
        resolved = try model(for: request)
    } catch {
        return adapterFailure(request.id, error)
    }
    switch resolved.availability {
    case .available:
        response.availability = AvailabilityBody(status: "available", reason: nil)
    case .unavailable(let reason):
        let label: String
        switch reason {
        case .appleIntelligenceNotEnabled: label = "apple_intelligence_not_enabled"
        case .deviceNotEligible: label = "device_not_eligible"
        case .modelNotReady: label = "model_not_ready"
        @unknown default: label = "unknown"
        }
        response.availability = AvailabilityBody(status: "unavailable", reason: label)
    @unknown default:
        response.availability = AvailabilityBody(status: "unknown", reason: nil)
    }
    return response
}

private func session(for request: Request) throws -> LanguageModelSession {
    LanguageModelSession(model: try model(for: request), instructions: request.instructions ?? "")
}

private func options(for request: Request) -> GenerationOptions {
    let sampling = request.sampling
    let mode: GenerationOptions.SamplingMode
    switch sampling?.mode {
    case "random":
        if let top = sampling?.top {
            mode = .random(top: top, seed: sampling?.seed)
        } else if let threshold = sampling?.probabilityThreshold {
            mode = .random(probabilityThreshold: threshold, seed: sampling?.seed)
        } else {
            mode = .random(probabilityThreshold: 1.0, seed: sampling?.seed)
        }
    default:
        mode = .greedy
    }
    return GenerationOptions(
        sampling: mode,
        temperature: sampling?.temperature,
        maximumResponseTokens: request.maxTokens)
}

/// Builds the guided schema: one object whose `choice` is constrained to the
/// admitted option set, plus an optional ordered `certainty` band. The option
/// set lives in the schema, so caller text cannot add to it.
private func schema(options admitted: [String], band: [String]?) throws -> GenerationSchema {
    var properties: [DynamicGenerationSchema.Property] = [
        DynamicGenerationSchema.Property(
            name: "choice",
            description: "The single admitted option that answers the question.",
            schema: DynamicGenerationSchema(name: "Choice", description: nil, anyOf: admitted))
    ]
    if let band, !band.isEmpty {
        properties.append(
            DynamicGenerationSchema.Property(
                name: "certainty",
                description: "How certain the choice is, on the given ordered scale.",
                schema: DynamicGenerationSchema(name: "Certainty", description: nil, anyOf: band)))
    }
    let root = DynamicGenerationSchema(
        name: "Decision", description: "One typed judgment.", properties: properties)
    return try GenerationSchema(root: root, dependencies: [])
}

private func decide(_ request: Request) async -> Response {
    guard let admitted = request.options, !admitted.isEmpty else {
        return Response.failure(
            id: request.id, code: "invalid_request", message: "decide needs a non-empty options array")
    }
    let prompt = request.prompt ?? ""
    let start = Date()
    do {
        let generated = try schema(options: admitted, band: request.band)
        let response = try await session(for: request).respond(
            to: prompt, schema: generated, options: options(for: request))
        let content = response.content
        var out = Response(id: request.id, ok: true)
        out.choice = try content.value(String.self, forProperty: "choice")
        if let band = request.band, !band.isEmpty {
            out.band = try? content.value(String.self, forProperty: "certainty")
        }
        out.latencyMs = Date().timeIntervalSince(start) * 1000
        return out
    } catch {
        return adapterFailure(request.id, error)
    }
}

private func generate(_ request: Request) async -> Response {
    let prompt = request.prompt ?? ""
    let start = Date()
    do {
        let response = try await session(for: request).respond(
            to: prompt, options: options(for: request))
        var out = Response(id: request.id, ok: true)
        out.text = response.content
        out.latencyMs = Date().timeIntervalSince(start) * 1000
        return out
    } catch {
        return adapterFailure(request.id, error)
    }
}

/// Maps the runtime's typed generation errors onto stable codes the Rust
/// side turns into refusals. An unmapped error is reported as such rather
/// than flattened into a generic failure.
private func failure(_ id: String, _ error: Error) -> Response {
    if let generation = error as? LanguageModelSession.GenerationError {
        let code: String
        switch generation {
        case .exceededContextWindowSize: code = "exceeded_context_window_size"
        case .assetsUnavailable: code = "assets_unavailable"
        case .guardrailViolation: code = "guardrail_violation"
        case .unsupportedGuide: code = "unsupported_guide"
        case .unsupportedLanguageOrLocale: code = "unsupported_language_or_locale"
        case .decodingFailure: code = "decoding_failure"
        case .rateLimited: code = "rate_limited"
        case .concurrentRequests: code = "concurrent_requests"
        case .refusal: code = "refusal"
        @unknown default: code = "unknown"
        }
        return Response.failure(id: id, code: code, message: generation.localizedDescription)
    }
    return Response.failure(id: id, code: "server_error", message: "\(error)")
}
