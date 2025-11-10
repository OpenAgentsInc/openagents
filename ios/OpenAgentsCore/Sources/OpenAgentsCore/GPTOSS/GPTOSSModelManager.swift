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
            // If installed is detected, Hub/MLX will load from cache; otherwise it will download.
            if let detected = await detectInstalled(), detected.installed {
                print("[GPTOSS] Local snapshot detected; loading by id from cache")
            }
            let t0 = Date()
            print("[GPTOSS] loadModel begin id=\(config.modelID)")
            let model = try await MLXLMCommon.loadModel(id: config.modelID)
            self.chat = ChatSession(model)
            state = .ready
            let dt = Date().timeIntervalSince(t0)
            print(String(format: "[GPTOSS] loadModel ready in %.2fs", dt))
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

    /// Stream a response token-by-token and invoke the handler for each delta.
    public func stream(prompt: String, onToken: @escaping (String) async -> Void) async throws {
        guard let chat = chat else { throw GPTOSSError.modelNotLoaded }
        do {
            print("[GPTOSS] stream start prompt_len=\(prompt.count)")
            var lastLog = Date(timeIntervalSince1970: 0)
            var totalChars = 0
            var step = 0
            for try await delta in chat.streamResponse(to: prompt) {
                step += 1
                totalChars += delta.count
                let now = Date()
                if now.timeIntervalSince(lastLog) > 0.5 {
                    print("[GPTOSS] stream progress steps=\(step) chars=\(totalChars)")
                    lastLog = now
                }
                await onToken(delta)
            }
            print("[GPTOSS] stream complete steps=\(step) chars=\(totalChars)")
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
        let link = "https://huggingface.co/\(config.modelID)"
        print("[GPTOSS] Starting download from \(link)")
        print("[GPTOSS] Files: \(files.joined(separator: ", "))")
        let env = ProcessInfo.processInfo.environment
        if let off = env["HF_HUB_OFFLINE"], !off.isEmpty { print("[GPTOSS] HF_HUB_OFFLINE=\(off)") }
        if let off = env["TRANSFORMERS_OFFLINE"], !off.isEmpty { print("[GPTOSS] TRANSFORMERS_OFFLINE=\(off)") }

        let modelDir = try await Hub.snapshot(from: repo, matching: files) { p in
            // Heuristic: if totalUnitCount is tiny, treat it as "file count" and estimate bytes from percent
            let fallbackTotalBytes: Int64 = Int64(12.1 * 1_073_741_824.0) // ~12.1 GiB
            let looksLikeFileCount = p.totalUnitCount > 0 && p.totalUnitCount < (128 * 1024 * 1024)
            let totalBytes = looksLikeFileCount ? fallbackTotalBytes : p.totalUnitCount
            let completedBytes = (looksLikeFileCount || p.completedUnitCount == 0)
                ? Int64(Double(totalBytes) * p.fractionCompleted)
                : p.completedUnitCount

            let prog = DownloadProgress(
                fractionCompleted: p.fractionCompleted,
                bytesDownloaded: completedBytes,
                totalBytes: totalBytes
            )
            progressHandler(prog)

            let pct = Int((p.fractionCompleted * 100).rounded())
            func fmt(_ b: Int64) -> String {
                if b >= 1_073_741_824 { return String(format: "%.1f GB", Double(b)/1_073_741_824.0) }
                if b >= 1_048_576 { return String(format: "%.1f MB", Double(b)/1_048_576.0) }
                if b >= 1024 { return String(format: "%.1f KB", Double(b)/1024.0) }
                return "\(b) B"
            }
            print("[GPTOSS] Download progress: \(pct)% (~\(fmt(completedBytes)) / \(fmt(totalBytes)))")
        }
        // Optional: verify by attempting a load
        print("[GPTOSS] Download complete at \(modelDir.path)")
        do { try await loadModel() } catch {
            print("[GPTOSS] Load after download failed (will defer to on‑demand): \(error)")
        }
    }

    // MARK: - Detect installed (best-effort)

    /// Try to detect if the model is already installed in the default Hub snapshot location.
    /// Returns (installed, totalBytes) if found.
    public func detectInstalled() async -> (installed: Bool, totalBytes: Int64)? {
        let modelId = config.modelID
        let fm = FileManager.default
        if let docs = try? fm.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: false) {
            let dir = docs.appendingPathComponent("huggingface/models/\(modelId)")
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: dir.path, isDirectory: &isDir), isDir.boolValue {
                // Recursively sum relevant files
                var total: Int64 = 0
                let enumerator = fm.enumerator(at: dir, includingPropertiesForKeys: [.isRegularFileKey, .fileSizeKey], options: [.skipsHiddenFiles])
                while let item = enumerator?.nextObject() as? URL {
                    let name = item.lastPathComponent
                    if name.hasSuffix(".safetensors") || ["config.json","tokenizer.json","tokenizer_config.json","generation_config.json"].contains(name) {
                        if let vals = try? item.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey]), vals.isRegularFile == true,
                           let size = vals.fileSize.map(Int64.init) {
                            total += size
                        }
                    }
                }
                if total > 0 {
                    print("[GPTOSS] Detected installed model at \(dir.path); total bytes ~\(total)")
                    return (true, total)
                }
            }
        }
        return nil
    }
}
#endif
