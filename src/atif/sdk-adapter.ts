/**
 * SDK → ATIF Adapter
 *
 * Converts Claude Agent SDK streaming events to ATIF v1.4 steps in real-time.
 * ATIF is the ONE standard format - this adapter bridges the SDK to it.
 *
 * Architecture:
 * - SDK produces streaming tool_use/tool_result blocks
 * - Adapter converts them to ATIF Step objects
 * - Emits via emitATIFStep() for HUD display
 *
 * Usage:
 * ```typescript
 * const adapter = createSDKToATIFAdapter(runId, sessionId);
 *
 * for await (const message of query({...})) {
 *   adapter.processSDKMessage(message);
 * }
 * ```
 */

import { emitATIFStep } from "./hud-emitter.js";
import type { Step, ToolCall } from "./schema.js";

// ============================================================================
// Types
// ============================================================================

/**
 * SDK streaming event (from @anthropic-ai/claude-agent-sdk)
 */
interface SDKStreamEvent {
  type: "stream_event";
  event: {
    type: string;
    content_block?: {
      id: string;
      type: string;
      name?: string;
    };
    delta?: {
      type: string;
      text?: string;
      partial_json?: string;
    };
    index?: number;
  };
}

/**
 * SDK tool result message (from SDK query output)
 */
interface SDKToolResultMessage {
  type?: string;
  tool_results?: Array<{
    tool_use_id: string;
    content: unknown;
  }>;
}

/**
 * Pending tool call waiting for result
 */
interface PendingToolCall {
  tool_call_id: string;
  function_name: string;
  arguments: unknown;
  started_at: string;
  index: number; // Content block index for matching stop events
}

// ============================================================================
// SDK → ATIF Adapter
// ============================================================================

export class SDKToATIFAdapter {
  private runId: string;
  private sessionId: string;
  private stepCounter = 1;
  private pendingToolCalls = new Map<string, PendingToolCall>();
  private toolInputBuffers = new Map<string, string>();
  private indexToToolId = new Map<number, string>(); // Map content block index to tool_call_id

  constructor(runId: string, sessionId: string) {
    this.runId = runId;
    this.sessionId = sessionId;
  }

  /**
   * Process an SDK message and convert to ATIF steps
   */
  processSDKMessage(message: unknown): void {
    // Handle streaming events
    if (this.isSDKStreamEvent(message)) {
      this.handleStreamEvent(message);
      return;
    }

    // Handle tool result messages
    if (this.isSDKToolResultMessage(message)) {
      this.handleToolResults(message);
      return;
    }
  }

  /**
   * Handle SDK stream_event messages
   */
  private handleStreamEvent(event: SDKStreamEvent): void {
    const eventData = event.event as any;
    const { type, content_block, delta } = event.event;
    const index: number | undefined = eventData?.index;

    // Tool use started - track by both id and index
    if (type === "content_block_start" && content_block?.type === "tool_use") {
      const toolCallId = content_block.id;
      const functionName = content_block.name ?? "unknown";

      this.pendingToolCalls.set(toolCallId, {
        tool_call_id: toolCallId,
        function_name: functionName,
        arguments: null,
        started_at: new Date().toISOString(),
        index: index ?? -1,
      });

      // Map index to tool ID for stop event matching
      if (index !== undefined) {
        this.indexToToolId.set(index, toolCallId);
      }

      this.toolInputBuffers.set(toolCallId, "");
    }

    // Tool input streaming - use index to find the tool since content_block might not have id in deltas
    if (type === "content_block_delta" && delta?.type === "input_json_delta") {
      // Try content_block.id first, then fall back to index lookup
      let toolCallId = content_block?.id;
      if (!toolCallId && index !== undefined) {
        toolCallId = this.indexToToolId.get(index);
      }
      if (!toolCallId) return;

      const chunk = delta.partial_json ?? delta.text ?? "";
      const currentBuffer = this.toolInputBuffers.get(toolCallId) ?? "";
      this.toolInputBuffers.set(toolCallId, currentBuffer + chunk);
    }

    // Tool use completed - use index to find the tool since content_block_stop doesn't have content_block
    if (type === "content_block_stop" && index !== undefined) {
      const toolCallId = this.indexToToolId.get(index);
      if (!toolCallId) return; // Not a tool_use block (could be text or thinking)

      const pending = this.pendingToolCalls.get(toolCallId);
      if (!pending) return;

      // Parse complete input
      const inputBuffer = this.toolInputBuffers.get(toolCallId) ?? "{}";
      let parsedInput: unknown;
      try {
        parsedInput = JSON.parse(inputBuffer);
      } catch {
        parsedInput = { raw: inputBuffer };
      }

      // Update pending tool call with parsed input
      pending.arguments = parsedInput;

      // Emit ATIF step with tool_calls
      this.emitAgentStepWithToolCalls([pending]);

      // Clean up
      this.toolInputBuffers.delete(toolCallId);
      this.indexToToolId.delete(index);
    }
  }

  /**
   * Handle SDK tool result messages
   */
  private handleToolResults(message: SDKToolResultMessage): void {
    if (!message.tool_results) return;

    const results = message.tool_results.map((result) => ({
      source_call_id: result.tool_use_id,
      content: result.content,
    }));

    // Emit ATIF step with observation
    this.emitObservationStep(results);

    // Clean up pending tool calls
    for (const result of message.tool_results) {
      this.pendingToolCalls.delete(result.tool_use_id);
    }
  }

  /**
   * Emit ATIF agent step with tool_calls
   */
  private emitAgentStepWithToolCalls(toolCalls: PendingToolCall[]): void {
    const step: Step = {
      step_id: this.stepCounter++,
      timestamp: new Date().toISOString(),
      source: "agent",
      message: `Calling ${toolCalls.length} tool(s)`,
      tool_calls: toolCalls.map((tc) => ({
        tool_call_id: tc.tool_call_id,
        function_name: tc.function_name,
        arguments: tc.arguments,
      })) as ToolCall[], // Cast to satisfy readonly -> mutable
    };

    emitATIFStep(this.runId, this.sessionId, step);
  }

  /**
   * Emit ATIF observation step
   */
  private emitObservationStep(results: Array<{ source_call_id: string; content: unknown }>): void {
    const step: Step = {
      step_id: this.stepCounter++,
      timestamp: new Date().toISOString(),
      source: "system",
      message: `Tool results (${results.length})`,
      observation: {
        results: results.map((r) => ({
          source_call_id: r.source_call_id,
          content: r.content,
        })),
      },
    };

    emitATIFStep(this.runId, this.sessionId, step);
  }

  /**
   * Type guard for SDK stream event
   */
  private isSDKStreamEvent(message: unknown): message is SDKStreamEvent {
    return (
      typeof message === "object" &&
      message !== null &&
      (message as any).type === "stream_event" &&
      typeof (message as any).event === "object"
    );
  }

  /**
   * Type guard for SDK tool result message
   */
  private isSDKToolResultMessage(message: unknown): message is SDKToolResultMessage {
    return (
      typeof message === "object" &&
      message !== null &&
      Array.isArray((message as any).tool_results)
    );
  }
}

/**
 * Factory function to create SDK → ATIF adapter
 */
export const createSDKToATIFAdapter = (runId: string, sessionId: string): SDKToATIFAdapter => {
  return new SDKToATIFAdapter(runId, sessionId);
};
