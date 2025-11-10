import Foundation

public struct GPTOSSConfig: Codable, Sendable {
    public var modelID: String
    public var temperature: Double
    public var topP: Double
    public var maxTokens: Int?
    public var idleTimeoutSeconds: TimeInterval

    public static let `default` = GPTOSSConfig(
        modelID: "mlx-community/gpt-oss-20b-MXFP4-Q8",
        temperature: 0.7,
        topP: 0.9,
        maxTokens: nil,
        idleTimeoutSeconds: 600
    )
}

public struct GPTOSSGenerationOptions: Sendable {
    public var temperature: Double
    public var topP: Double
    public var maxTokens: Int?

    public init(temperature: Double = 0.7, topP: Double = 0.9, maxTokens: Int? = nil) {
        self.temperature = temperature
        self.topP = topP
        self.maxTokens = maxTokens
    }
}

public enum GPTOSSModelState: Equatable, Sendable {
    case notLoaded
    case loading
    case ready
    case error(String)
}

public enum GPTOSSError: Error, LocalizedError {
    case modelNotLoaded
    case loadingFailed(underlying: Error)
    case generationFailed(underlying: Error)
    case insufficientMemory(available: UInt64, required: UInt64)
    case unsupportedPlatform
    case serverUnavailable
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .modelNotLoaded:
            return "GPTOSS model is not loaded. Please load the model first."
        case .loadingFailed(let e):
            return "Model loading failed: \(e.localizedDescription)"
        case .generationFailed(let e):
            return "Text generation failed: \(e.localizedDescription)"
        case .insufficientMemory(let avail, let req):
            let a = Double(avail) / 1_000_000_000
            let r = Double(req) / 1_000_000_000
            return String(format: "Insufficient memory: %.1f GB available, %.1f GB required", a, r)
        case .unsupportedPlatform:
            return "GPTOSS 20B is only supported on macOS with Apple Silicon"
        case .serverUnavailable:
            return "Server reference unavailable for delegation"
        case .cancelled:
            return "Generation cancelled by user"
        }
    }
}

