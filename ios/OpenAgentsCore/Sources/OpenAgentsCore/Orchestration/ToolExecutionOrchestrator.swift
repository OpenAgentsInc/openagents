import Foundation

/// Manages execution of AgentOps and streaming of tool call updates.
actor ToolExecutionOrchestrator {
    private let toolExecutor: ToolExecutor
    private let stream: ACPUpdateStreamHandler
    private let planner: PlanningReducer

    init(toolExecutor: ToolExecutor, stream: @escaping ACPUpdateStreamHandler, planner: PlanningReducer) {
        self.toolExecutor = toolExecutor
        self.stream = stream
        self.planner = planner
    }

    func execute(_ ops: [AgentOp]) async throws -> [AgentOp: any Encodable] {
        var results: [AgentOp: any Encodable] = [:]
        for op in ops {
            // Start
            await streamToolCall(op, status: .started)
            do {
                let result = try await toolExecutor.execute(op)
                results[op] = result
                // Completed
                await streamToolCallUpdate(op, status: .completed, output: result, error: nil)
            } catch {
                // Error
                await streamToolCallUpdate(op, status: .error, output: nil, error: error.localizedDescription)
                throw error
            }
        }
        return results
    }

    // MARK: - Streaming helpers

    private func streamToolCall(_ op: AgentOp, status: ACPToolCallUpdateWire.Status) async {
        let call = ACPToolCallWire(
            call_id: op.opId.uuidString,
            name: op.toolName,
            arguments: nil
        )
        await stream(.toolCall(call))
        await planner.updateEntry(opId: op.opId.uuidString, to: .in_progress)
    }

    private func streamToolCallUpdate(
        _ op: AgentOp,
        status: ACPToolCallUpdateWire.Status,
        output: (any Encodable)?,
        error: String?
    ) async {
        let outputEnc: AnyEncodable? = output.map { AnyEncodable($0) }
        let upd = ACPToolCallUpdateWire(
            call_id: op.opId.uuidString,
            status: status,
            output: outputEnc,
            error: error,
            _meta: nil
        )
        await stream(.toolCallUpdate(upd))
        if status == .completed {
            await planner.updateEntry(opId: op.opId.uuidString, to: .completed)
        } else if status == .error {
            await planner.updateEntry(opId: op.opId.uuidString, to: .completed, error: error)
        }
    }

    // Progress streaming (from ToolExecutor callback)
    func streamProgress(_ op: AgentOp, fraction: Double, note: String?) async {
        var meta: [String: AnyEncodable] = ["progress": AnyEncodable(fraction)]
        if let note = note { meta["note"] = AnyEncodable(note) }
        let upd = ACPToolCallUpdateWire(
            call_id: op.opId.uuidString,
            status: .started,
            output: nil,
            error: nil,
            _meta: meta
        )
        await stream(.toolCallUpdate(upd))
    }
}
