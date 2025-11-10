#if os(macOS)
import Foundation

/// Agent provider for llama.cpp CLI with GGUF models
///
/// Uses llama-cli to run local GGUF models (e.g., GPT-OSS 20B).
/// Significantly faster than MLX for inference on Apple Silicon.
///
/// Model Path Configuration (priority order):
/// 1. AgentContext metadata["model_path"]
/// 2. Environment variable OPENAGENTS_LLAMA_MODEL
/// 3. Default: ~/.openagents/models/gpt-oss-20b-MXFP4.gguf
///
/// Installation:
/// ```bash
/// brew install llama.cpp
/// # Or build from source: https://github.com/ggml-org/llama.cpp
/// ```
public final class LlamaAgentProvider: CLIAgentProvider, @unchecked Sendable {
    // MARK: - Configuration

    private static let defaultModelPath = "~/.openagents/models/gpt-oss-20b-MXFP4.gguf"

    /// Conversation history by session (for context reconstruction on resume)
    private var conversationHistory: [String: [ChatMessage]] = [:]

    private struct ChatMessage {
        let role: String  // "system", "user", "assistant"
        let content: String
    }

    // MARK: - Initialization

    public init() {
        super.init(
            id: .llama_cpp,
            displayName: "Llama.cpp",
            binaryName: "llama-cli",
            envOverride: "OPENAGENTS_LLAMA_CLI",
            capabilities: AgentCapabilities(
                executionMode: .cli,
                streamingMode: .text,  // Plain text streaming
                supportsResume: true,  // Via conversation history
                supportsWorkingDirectory: true,
                requiresExternalBinary: true,
                supportsMCP: false
            )
        )
    }

    // MARK: - CLI Arguments

    public override func buildStartArguments(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext
    ) -> [String] {
        var args: [String] = []

        // 1. Model path (required)
        let modelPath = resolveModelPath(from: context)
        let expandedPath = (modelPath as NSString).expandingTildeInPath
        args += ["-m", expandedPath]

        OpenAgentsLog.orchestration.debug("[Llama] Using model: \(expandedPath, privacy: .private)")

        // 2. GPU acceleration (Metal on Apple Silicon)
        // Use fewer layers for M2 Pro to avoid OOM; M3 Max/Ultra can handle more
        args += ["-ngl", "20"]  // Offload layers to GPU (reduced for M2 Pro)

        // 3. Context window
        args += ["-c", "8192"]  // 8K context (adjust based on model)

        // 4. Sampling parameters
        let temperature = context.metadata["temperature"] as? String ?? "0.7"
        let topP = context.metadata["top_p"] as? String ?? "0.9"
        let maxTokens = context.metadata["max_tokens"] as? String ?? "2000"

        args += [
            "--temp", temperature,
            "--top-p", topP,
            "-n", maxTokens,  // Max new tokens
        ]

        // 5. System prompt (optional)
        if let systemPrompt = context.metadata["system_prompt"] as? String {
            args += ["--system-prompt", systemPrompt]
        }

        // 6. Output formatting
        args += [
            "--no-display-prompt",  // Don't echo prompt
            "--simple-io",          // Clean output
            "--log-disable",        // Suppress debug logs
        ]

        // 7. User prompt
        args += ["-p", prompt]

        // Store initial message in history
        var history: [ChatMessage] = []
        if let systemPrompt = context.metadata["system_prompt"] as? String {
            history.append(ChatMessage(role: "system", content: systemPrompt))
        }
        history.append(ChatMessage(role: "user", content: prompt))
        conversationHistory[sessionId.value] = history

        OpenAgentsLog.orchestration.debug("[Llama] Start arguments: \(args.joined(separator: " "), privacy: .private)")

        return args
    }

    public override func buildResumeArguments(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext
    ) -> [String] {
        var args: [String] = []

        // 1. Model path
        let modelPath = resolveModelPath(from: context)
        let expandedPath = (modelPath as NSString).expandingTildeInPath
        args += ["-m", expandedPath]

        // 2. GPU acceleration
        args += ["-ngl", "20"]  // Reduced for M2 Pro

        // 3. Context window
        args += ["-c", "8192"]

        // 4. Sampling parameters
        let temperature = context.metadata["temperature"] as? String ?? "0.7"
        let topP = context.metadata["top_p"] as? String ?? "0.9"
        let maxTokens = context.metadata["max_tokens"] as? String ?? "2000"

        args += [
            "--temp", temperature,
            "--top-p", topP,
            "-n", maxTokens,
        ]

        // 5. Output formatting
        args += [
            "--no-display-prompt",
            "--simple-io",
            "--log-disable",
        ]

        // 6. Rebuild conversation context
        // llama-cli doesn't have built-in session management, so we reconstruct
        // the conversation by concatenating history into the prompt
        var history = conversationHistory[sessionId.value] ?? []

        // Append new user message
        history.append(ChatMessage(role: "user", content: prompt))
        conversationHistory[sessionId.value] = history

        // Build conversation prompt
        let conversationPrompt = buildConversationPrompt(history: history)
        args += ["-p", conversationPrompt]

        OpenAgentsLog.orchestration.debug("[Llama] Resume with \(history.count) messages in context")

        return args
    }

    // MARK: - Output Processing

    public override func processStdoutLine(
        _ line: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // llama-cli outputs plain text token-by-token
        // Each line may contain multiple tokens or partial output

        // Don't filter empty lines here - they may be intentional formatting
        let text = line.trimmingCharacters(in: .newlines)  // Keep spaces

        OpenAgentsLog.orchestration.debug("[Llama] token: \(text, privacy: .public)")

        // Stream as agent message chunk
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text + "\n")))
        await updateHub.sendSessionUpdate(
            sessionId: sessionId,
            update: .agentMessageChunk(chunk)
        )

        // Accumulate assistant response in history (would need to buffer complete response)
        // For now, we'll handle this when the process completes
    }

    // MARK: - Cleanup

    public override func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {
        // Keep conversation history for resume
        // Only clear on explicit session delete

        await super.cancel(sessionId: sessionId, handle: handle)
    }

    // MARK: - Helpers

    private func resolveModelPath(from context: AgentContext) -> String {
        // Priority 1: Context metadata
        if let contextPath = context.metadata["model_path"] as? String {
            return contextPath
        }

        // Priority 2: Environment variable
        if let envPath = ProcessInfo.processInfo.environment["OPENAGENTS_LLAMA_MODEL"],
           !envPath.isEmpty {
            return envPath
        }

        // Priority 3: Default location
        return Self.defaultModelPath
    }

    private func buildConversationPrompt(history: [ChatMessage]) -> String {
        // Build a conversation-style prompt from history
        // Format depends on model's chat template, but we'll use a generic format
        var prompt = ""

        for message in history {
            switch message.role {
            case "system":
                prompt += "System: \(message.content)\n\n"
            case "user":
                prompt += "User: \(message.content)\n\n"
            case "assistant":
                prompt += "Assistant: \(message.content)\n\n"
            default:
                break
            }
        }

        prompt += "Assistant:"  // Prompt for next response

        return prompt
    }

    /// Public method to clear session history (for explicit session deletion)
    public func clearHistory(sessionId: ACPSessionId) {
        conversationHistory.removeValue(forKey: sessionId.value)
    }
}
#endif
