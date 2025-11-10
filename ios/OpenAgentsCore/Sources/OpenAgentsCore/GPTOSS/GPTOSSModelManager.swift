// Phase 1: minimal loader + single-turn generation using ChatSession
#if os(macOS)
import Foundation
import MLXLLM
import MLXLMCommon
import Hub

public actor GPTOSSModelManager {
    private let config: GPTOSSConfig
    private var chat: ChatSession?
    private var state: GPTOSSModelState = .notLoaded

    private static let minimumMemoryBytes: UInt64 = 16_000_000_000 // 16 GB

    public init(config: GPTOSSConfig = .default) {
        self.config = config
    }

    public var currentState: GPTOSSModelState { state }
    public var isModelLoaded: Bool { if case .ready = state { return true } else { return false } }

    public func loadModel() async throws {
        guard !isModelLoaded else { return }
        try checkSystemRequirements()
        state = .loading
        do {
            let model = try await MLXLMCommon.loadModel(id: config.modelID)
            self.chat = ChatSession(model)
            state = .ready
        } catch {
            state = .error(error.localizedDescription)
            throw GPTOSSError.loadingFailed(underlying: error)
        }
    }

    public func unloadModel() async {
        chat = nil
        state = .notLoaded
    }

    public func generate(prompt: String, options: GPTOSSGenerationOptions = .init()) async throws -> String {
        guard let chat = chat else { throw GPTOSSError.modelNotLoaded }
        do {
            // Harmony compliance: ChatSession applies chat template internally
            let text = try await chat.respond(to: prompt)
            return text
        } catch {
            throw GPTOSSError.generationFailed(underlying: error)
        }
    }

    private func checkSystemRequirements() throws {
        #if !os(macOS)
        throw GPTOSSError.unsupportedPlatform
        #endif
        let mem = ProcessInfo.processInfo.physicalMemory
        guard mem >= Self.minimumMemoryBytes else {
            throw GPTOSSError.insufficientMemory(available: mem, required: Self.minimumMemoryBytes)
        }
    }

    // MARK: - Download (Hub.snapshot)

    public struct DownloadProgress: Sendable {
        public var fractionCompleted: Double
        public var bytesDownloaded: Int64
        public var totalBytes: Int64
        public var estimatedTimeRemaining: TimeInterval?
        public init(fractionCompleted: Double, bytesDownloaded: Int64, totalBytes: Int64, estimatedTimeRemaining: TimeInterval? = nil) {
            self.fractionCompleted = fractionCompleted
            self.bytesDownloaded = bytesDownloaded
            self.totalBytes = totalBytes
            self.estimatedTimeRemaining = estimatedTimeRemaining
        }
    }

    /// Download model artifacts with resumable snapshot and report progress.
    public func downloadModel(progressHandler: @escaping (DownloadProgress) -> Void) async throws {
        let repo = Hub.Repo(id: config.modelID)
        let files = ["*.safetensors", "config.json", "tokenizer.json", "tokenizer_config.json", "generation_config.json"]
        let _ = try await Hub.snapshot(from: repo, matching: files) { p in
            let prog = DownloadProgress(
                fractionCompleted: p.fractionCompleted,
                bytesDownloaded: p.completedUnitCount,
                totalBytes: p.totalUnitCount
            )
            progressHandler(prog)
        }
        // Optional: verify by attempting a load
        do { try await loadModel() } catch { /* leave to caller if load on demand */ }
    }
}
#endif
