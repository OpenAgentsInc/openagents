#if os(macOS)
import Foundation

/// Agent provider for OpenAI Codex CLI
public final class CodexAgentProvider: CLIAgentProvider, @unchecked Sendable {
    // MARK: - State

    /// Codex thread IDs by session (for resume)
    private var threadIdBySession: [String: String] = [:]

    // MARK: - Initialization

    public init() {
        super.init(
            id: .codex,
            displayName: "Codex",
            binaryName: "codex",
            envOverride: "OPENAGENTS_CODEX_CLI",
            capabilities: AgentCapabilities(
                executionMode: .cli,
                streamingMode: .jsonl,
                supportsResume: true,
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
        var args: [String] = ["exec", "--json"]

        // Working directory
        if let wd = context.workingDirectory?.path {
            args += ["--cd", wd]
        }

        // Check for existing thread to resume
        let sidStr = sessionId.value
        if let thread = threadIdBySession[sidStr], !thread.isEmpty {
            args += ["resume", thread]
            print("[Codex] RESUMING existing thread \(thread) for session \(sidStr)")
        } else {
            print("[Codex] STARTING NEW thread for session \(sidStr)")
        }

        // Prompt
        args.append(prompt)

        OpenAgentsLog.orchestration.debug("[Codex] Arguments: \(args.joined(separator: " "), privacy: .private)")
        return args
    }

    public override func buildResumeArguments(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext
    ) -> [String] {
        var args: [String] = ["exec", "--json"]

        // Working directory
        if let wd = context.workingDirectory?.path {
            args += ["--cd", wd]
        }

        // Resume with thread ID
        let sidStr = sessionId.value
        if let thread = threadIdBySession[sidStr], !thread.isEmpty {
            args += ["resume", thread]
        } else if let thread = handle.threadId, !thread.isEmpty {
            args += ["resume", thread]
        }

        // Prompt
        args.append(prompt)

        OpenAgentsLog.orchestration.debug("[Codex] Resume arguments: \(args.joined(separator: " "), privacy: .private)")
        return args
    }

    // MARK: - Output Processing

    public override func processStdoutLine(
        _ line: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // Parse Codex JSONL and translate to ACP SessionUpdate
        guard let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return
        }

        let type = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        OpenAgentsLog.orchestration.debug("[Codex] Event: \(type, privacy: .public)")

        // Extract thread ID for resume capability
        if type == "thread.started" || type == "session_meta" {
            let sidStr = sessionId.value
            if let threadId = obj["thread_id"] as? String {
                print("[Codex] Captured thread ID \(threadId) for session \(sidStr)")
                threadIdBySession[sidStr] = threadId
            } else if let payload = obj["payload"] as? [String: Any],
                      let threadId = payload["id"] as? String {
                print("[Codex] Captured thread ID \(threadId) for session \(sidStr)")
                threadIdBySession[sidStr] = threadId
            }
        }

        // Normalized container: many Codex lines carry an `item` or `msg` payload
        let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any])
        let itemType = ((item?["type"] as? String) ?? "").lowercased()

        // Tool calls
        if itemType == "tool_call" || type == "tool_call" {
            let toolName = (item?["tool_name"] as? String) ?? (item?["tool"] as? String) ?? "tool"
            let callId = (item?["id"] as? String) ?? (item?["call_id"] as? String) ?? UUID().uuidString
            let argsDict = (item?["arguments"] as? [String: Any]) ?? (item?["args"] as? [String: Any]) ?? [:]

            // Convert dict to AnyEncodable dict
            // Encode through JSON to ensure compatibility
            var argsAEDict: [String: AnyEncodable]? = nil
            if !argsDict.isEmpty {
                if let jsonData = try? JSONSerialization.data(withJSONObject: argsDict),
                   let decoded = try? JSONDecoder().decode([String: AnyEncodable].self, from: jsonData) {
                    argsAEDict = decoded
                }
            }

            let wire = ACPToolCallWire(
                call_id: callId,
                name: toolName,
                arguments: argsAEDict
            )
            await updateHub.sendSessionUpdate(
                sessionId: sessionId,
                update: .toolCall(wire)
            )
            return
        }

        // Tool results
        if itemType == "tool_result" || type == "tool_result" {
            let callId = (item?["call_id"] as? String) ?? (item?["id"] as? String) ?? UUID().uuidString
            let ok = (item?["ok"] as? Bool) ?? (item?["success"] as? Bool) ?? true
            let status: ACPToolCallUpdateWire.Status = ok ? .completed : .error
            let outputAny = item?["result"] ?? item?["output"]
            // Encode through JSON to ensure compatibility
            var outputAE: AnyEncodable? = nil
            if let output = outputAny {
                if let jsonData = try? JSONSerialization.data(withJSONObject: [output]),
                   let decoded = try? JSONDecoder().decode([AnyEncodable].self, from: jsonData),
                   let first = decoded.first {
                    outputAE = first
                }
            }
            let error = item?["error"] as? String

            let wire = ACPToolCallUpdateWire(
                call_id: callId,
                status: status,
                output: outputAE,
                error: error
            )
            await updateHub.sendSessionUpdate(
                sessionId: sessionId,
                update: .toolCallUpdate(wire)
            )
            return
        }

        // Plan state updates → represent as ACP plan entries (simplified)
        if itemType == "plan_state" || type == "plan_state" || type == "plan.updated" {
            if let steps = item?["steps"] as? [String] {
                let entries = steps.map { step in
                    ACPPlanEntry(content: step, priority: .medium, status: .in_progress, _meta: nil)
                }
                let plan = ACPPlan(entries: entries, _meta: nil)
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .plan(plan))
                return
            }
        }

        // User messages
        if itemType == "user_message" || type == "user_message" {
            if let itemDict = item, let text = extractText(from: itemDict), !text.isEmpty {
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                await updateHub.sendSessionUpdate(
                    sessionId: sessionId,
                    update: .userMessageChunk(chunk)
                )
            }
            return
        }

        // Agent/Assistant messages (reasoning, agent_message, etc.)
        if itemType == "agent_message" || itemType == "assistant_message" || type == "agent_message" || itemType == "agent_reasoning" {
            let sourceDict = item ?? obj
            if let text = extractText(from: sourceDict), !text.isEmpty {
                OpenAgentsLog.acp.debug("[Codex] Sending agent_message_chunk: textLen=\(text.count) text=\(text.prefix(80))...")
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                await updateHub.sendSessionUpdate(
                    sessionId: sessionId,
                    update: .agentMessageChunk(chunk)
                )
            }
            return
        }

        // Fallback: extract any text and stream as agent chunks (for events we don't explicitly handle)
        let sourceDict = item ?? obj
        if let text = extractText(from: sourceDict), !text.isEmpty {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
            await updateHub.sendSessionUpdate(
                sessionId: sessionId,
                update: .agentMessageChunk(chunk)
            )
        }
    }

    // MARK: - Parsing Helpers

    private func extractText(from dict: [String: Any]) -> String? {
        // Try various text field names
        if let text = dict["text"] as? String { return text }
        if let text = dict["content"] as? String { return text }
        if let text = dict["message"] as? String { return text }

        // Try nested content
        if let content = dict["content"] as? [String: Any],
           let text = content["text"] as? String {
            return text
        }

        return nil
    }

    // MARK: - Cleanup

    public override func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {
        // Clear thread ID
        threadIdBySession.removeValue(forKey: sessionId.value)

        // Call super to handle process termination
        await super.cancel(sessionId: sessionId, handle: handle)
    }
}
#endif
