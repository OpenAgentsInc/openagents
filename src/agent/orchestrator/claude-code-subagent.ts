import {
  CLAUDE_CODE_MCP_SERVER_NAME,
  createMechaCoderMcpServer,
  getAllowedClaudeCodeTools,
} from "./claude-code-mcp.js";
import {
  AbortError,
  type PermissionMode,
  type McpServerConfig,
  type SDKAssistantMessageError,
  type HookCallback,
  type HookCallbackMatcher,
} from "@anthropic-ai/claude-agent-sdk";
import {
  type SubagentResult,
  type Subtask,
  type ClaudeCodePermissionMode,
  type OrchestratorEvent,
} from "./types.js";

type QueryFn = (input: unknown) => AsyncIterable<any>;

interface RetryConfig {
  maxRetries: number;
  initialDelayMs: number;
  maxDelayMs: number;
  backoffMultiplier: number;
}

export interface ClaudeCodeSubagentOptions {
  cwd: string;
  maxTurns?: number;
  systemPrompt?: string;
  permissionMode?: ClaudeCodePermissionMode | PermissionMode;
  queryFn?: QueryFn;
  openagentsDir?: string;
  allowedTools?: string[];
  mcpServers?: Record<string, unknown>;
  signal?: AbortSignal;
  timeoutMs?: number;
  onEvent?: (event: OrchestratorEvent) => void;
  /** Resume a prior Claude Code session if available */
  resumeSessionId?: string;
  /** Fork the resumed session to a new branch */
  forkSession?: boolean;
  /** Callback for streaming text output */
  onOutput?: (text: string) => void;
}

const isToolCallMessage = (message: any): message is { tool_calls?: Array<{ name?: string; input?: any }> } =>
  typeof message === "object" && message !== null && Array.isArray((message as any).tool_calls);

const isResultMessage = (message: any): message is { type?: string; subtype?: string } =>
  typeof message === "object" && message !== null && "type" in message;

const defaultBuildPrompt = (subtask: Subtask): string =>
  `## Subtask: ${subtask.id}

${subtask.description}

Focus on minimal, correct changes.`;

const DEFAULT_RETRY_CONFIG: RetryConfig = {
  maxRetries: 3,
  initialDelayMs: 1000,
  maxDelayMs: 32000,
  backoffMultiplier: 2,
};

const DEFAULT_TIMEOUT_MS = 50 * 60 * 1000; // 50 minutes

const assistantErrorMessages: Record<SDKAssistantMessageError, { message: string; suggestion?: string }> = {
  authentication_failed: {
    message: "Claude Code authentication failed",
    suggestion: "Set a valid ANTHROPIC_API_KEY before retrying",
  },
  billing_error: {
    message: "Claude Code billing error",
    suggestion: "Check billing status for Anthropic API access",
  },
  rate_limit: {
    message: "Claude Code hit rate limits",
    suggestion: "Back off and retry later or fall back to the minimal subagent",
  },
  invalid_request: {
    message: "Claude Code rejected the request",
    suggestion: "Inspect Claude Code inputs and configuration for invalid parameters",
  },
  server_error: {
    message: "Claude Code server error",
    suggestion: "Retry after a short delay or fall back to the minimal subagent",
  },
  unknown: { message: "Claude Code returned an unknown error" },
};

const describeAssistantError = (errorType?: SDKAssistantMessageError): { message: string; suggestion?: string } => {
  if (!errorType) return { message: "Claude Code reported an error" };
  return assistantErrorMessages[errorType] ?? { message: `Claude Code reported ${errorType}` };
};

const createAbortController = (signal?: AbortSignal, timeoutMs?: number) => {
  const controller = new AbortController();
  let abortReason: string | undefined;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  const propagateAbort = () => {
    if (controller.signal.aborted) return;
    abortReason =
      typeof signal?.reason === "string"
        ? signal.reason
        : signal?.reason instanceof Error
          ? signal.reason.message
          : "Claude Code run aborted";
    controller.abort(new Error(abortReason));
  };

  if (signal?.aborted) {
    propagateAbort();
  } else if (signal) {
    signal.addEventListener("abort", propagateAbort);
  }

  const timeout = timeoutMs ?? DEFAULT_TIMEOUT_MS;
  if (timeout && timeout > 0) {
    timeoutId = setTimeout(() => {
      abortReason = `Claude Code timed out after ${timeout}ms`;
      controller.abort(new Error(abortReason));
    }, timeout);
  }

  const cleanup = () => {
    if (timeoutId) clearTimeout(timeoutId);
    if (signal) signal.removeEventListener("abort", propagateAbort);
  };

  return {
    controller,
    getAbortReason: () => abortReason,
    cleanup,
  };
};

/**
 * Check if an error is retryable (rate limit or server error)
 */
const isRetryableError = (errorType?: SDKAssistantMessageError | string): boolean => {
  return errorType === "rate_limit" || errorType === "server_error";
};

/**
 * Check if an error is a fatal auth error (should stop immediately)
 */
const isFatalAuthError = (errorType?: SDKAssistantMessageError | string): boolean => {
  return errorType === "authentication_failed" || errorType === "billing_error";
};

/**
 * Calculate delay for exponential backoff
 */
const calculateBackoffDelay = (
  attempt: number,
  config: RetryConfig = DEFAULT_RETRY_CONFIG
): number => {
  const delay = config.initialDelayMs * Math.pow(config.backoffMultiplier, attempt);
  return Math.min(delay, config.maxDelayMs);
};

/**
 * Sleep for a specified number of milliseconds
 */
const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Create SDK hooks for observability and event emission
 *
 * NOTE: Hooks are for side effects (logging, events) only.
 * Tool/file tracking is done via message parsing for consistency across real SDK and test mocks.
 */
const createClaudeCodeHooks = (
  onEvent?: (event: OrchestratorEvent) => void
): Partial<Record<string, HookCallbackMatcher[]>> => {
  const postToolUseHook: HookCallback = async (input) => {
    if (input.hook_event_name === "PostToolUse") {
      // Emit event if callback provided
      if (onEvent) {
        onEvent({
          type: "subtask_tool_use",
          toolName: input.tool_name,
          input: input.tool_input,
        } as any); // Cast to any since this event type doesn't exist yet
      }
    }
    return { continue: true };
  };

  const sessionEndHook: HookCallback = async (input) => {
    if (input.hook_event_name === "SessionEnd") {
      // Emit event if callback provided
      if (onEvent) {
        onEvent({
          type: "claude_code_session_end",
          reason: input.reason,
          sessionId: input.session_id,
        } as any); // Cast to any since this event type doesn't exist yet
      }
    }
    return { continue: true };
  };

  return {
    PostToolUse: [{ hooks: [postToolUseHook] }],
    SessionEnd: [{ hooks: [sessionEndHook] }],
  };
};

/**
 * Run a subtask using Claude Code Agent SDK's query() streaming API with retry logic.
 * Implements:
 * - Exponential backoff for rate limits
 * - Retry for server errors
 * - Immediate failure for auth errors
 * - Logs all errors to blockers for progress.md
 */
export const runClaudeCodeSubagent = async (
  subtask: Subtask,
  options: ClaudeCodeSubagentOptions
): Promise<SubagentResult> => {
  const query = options.queryFn ?? (await import("@anthropic-ai/claude-agent-sdk")).query;

  const filesModified: Set<string> = new Set();
  const toolsUsed: Map<string, number> = new Map();
  const blockers: string[] = [];
  const assistantMessages: string[] = [];
  const suggestedNextSteps: string[] = [];
  let success = false;
  let error: string | undefined;
  let turns = 0;
  let lastErrorType: SDKAssistantMessageError | string | undefined;
  let totalCostUsd: number | undefined;
  let usage: any | undefined;
  let sessionId: string | undefined = options.resumeSessionId;

  const mcpServers: Record<string, McpServerConfig> =
    (options.mcpServers as Record<string, McpServerConfig> | undefined) ??
    {
      [CLAUDE_CODE_MCP_SERVER_NAME]: createMechaCoderMcpServer({
        openagentsDir: options.openagentsDir ?? "",
      }) as McpServerConfig,
    };

  const allowedTools = options.allowedTools ?? getAllowedClaudeCodeTools();

  const retryConfig = DEFAULT_RETRY_CONFIG;
  let retryAttempt = 0;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  // Create hooks for observability and event emission
  const hooks = createClaudeCodeHooks(options.onEvent);

  // Retry loop for rate limits and server errors
  while (retryAttempt <= retryConfig.maxRetries) {
    const { controller, getAbortReason, cleanup } = createAbortController(options.signal, timeoutMs);

    try {
      for await (const message of query({
        prompt: defaultBuildPrompt(subtask),
        options: {
          cwd: options.cwd,
          // Use Claude Code's system prompt with CLAUDE.md context
          systemPrompt: options.systemPrompt ?? { type: "preset", preset: "claude_code" },
          maxTurns: options.maxTurns ?? 300,
          ...(options.permissionMode ? { permissionMode: options.permissionMode } : {}),
          ...(options.resumeSessionId ? { resume: options.resumeSessionId } : {}),
          ...(options.forkSession ? { forkSession: true } : {}),
          mcpServers,
          allowedTools,
          abortController: controller,
          hooks,
          settingSources: ["project"], // Load CLAUDE.md for project context
          includePartialMessages: !!options.onOutput, // Enable streaming if callback provided
        },
      })) {
        // Handle streaming partial messages
        if ((message as any).type === "stream_event" && options.onOutput) {
          const event = (message as any).event;
          // Extract text delta from content_block_delta events
          if (event?.type === "content_block_delta" && event?.delta?.type === "text_delta") {
            options.onOutput(event.delta.text);
          }
          // Add newline at end of text blocks
          if (event?.type === "content_block_stop") {
            options.onOutput("\n");
          }
        }

        // Log all non-stream messages as JSON for visibility
        if (options.onOutput && (message as any).type !== "stream_event") {
          const msgType = (message as any).type || (message as any).role || "unknown";
          // Log tool use from assistant messages
          if ((message as any).role === "assistant" && Array.isArray((message as any).content)) {
            for (const block of (message as any).content) {
              if (block.type === "tool_use") {
                const toolJson = JSON.stringify({ tool: block.name, id: block.id, input: block.input }, null, 0);
                options.onOutput(`\n[TOOL_USE] ${toolJson}\n`);
              }
            }
          }
          // Log tool results
          if ((message as any).role === "user" && Array.isArray((message as any).content)) {
            for (const block of (message as any).content) {
              if (block.type === "tool_result") {
                const truncatedContent = typeof block.content === "string" && block.content.length > 200
                  ? block.content.slice(0, 200) + "..."
                  : block.content;
                const resultJson = JSON.stringify({ tool_result: block.tool_use_id, content: truncatedContent }, null, 0);
                options.onOutput(`[TOOL_RESULT] ${resultJson}\n`);
              }
            }
          }
          // Log result/system messages
          if (msgType === "result" || msgType === "system") {
            const resultJson = JSON.stringify(message, null, 0);
            options.onOutput(`[${msgType.toUpperCase()}] ${resultJson}\n`);
          }
        }

        // Legacy: Log tool calls as JSON (for messages with tool_calls array)
        if (isToolCallMessage(message) && message.tool_calls && options.onOutput) {
          for (const call of message.tool_calls) {
            if (call?.name) {
              const toolJson = JSON.stringify({ tool: call.name, input: call.input });
              options.onOutput(`\n[TOOL] ${toolJson}\n`);
            }
          }
        }
        // Track tool calls from messages
        // NOTE: Hooks provide event emission, but message parsing tracks usage/files
        // for consistency across real SDK and test mocks
        if (isToolCallMessage(message) && message.tool_calls) {
          for (const call of message.tool_calls) {
            const name = call?.name;
            if (name) {
              toolsUsed.set(name, (toolsUsed.get(name) || 0) + 1);
            }

            const filePath = call?.input?.file_path || call?.input?.path;
            if (filePath && (name === "Edit" || name === "Write")) {
              filesModified.add(String(filePath));
            }
          }
        }

        // Track tool_use blocks from assistant content (SDK message format)
        if ((message as any).role === "assistant" && Array.isArray((message as any).content)) {
          for (const block of (message as any).content) {
            if (block.type === "tool_use" && block.name) {
              toolsUsed.set(block.name, (toolsUsed.get(block.name) || 0) + 1);
              const filePath = block.input?.file_path || block.input?.path;
              if (filePath && (block.name === "Edit" || block.name === "Write")) {
                filesModified.add(String(filePath));
              }
            }
          }
        }

        // Capture assistant messages for context
        if ((message as any).role === "assistant" && (message as any).content) {
          const content = String((message as any).content);
          if (content.length > 0 && content.length < 500) {
            assistantMessages.push(content);
          }
        }

        // Capture session IDs for resumption
        const messageSessionId = (message as any)?.session_id;
        if (typeof messageSessionId === "string") {
          sessionId = messageSessionId;
        }

        // Capture SDK error messages with error types
        if ((message as any).error) {
          lastErrorType = (message as any).error as SDKAssistantMessageError;
          const assistantError = describeAssistantError(lastErrorType as SDKAssistantMessageError);
          error = assistantError.message;
          blockers.push(assistantError.message);
          if (assistantError.suggestion) {
            suggestedNextSteps.push(assistantError.suggestion);
          }

          // Check for fatal auth errors - stop immediately
          if (isFatalAuthError(lastErrorType)) {
            success = false;
            const fatalMessage =
              assistantError.suggestion && !assistantError.message.includes(assistantError.suggestion)
                ? `${assistantError.message}. ${assistantError.suggestion}`
                : assistantError.message;
            error = fatalMessage;
            blockers.push(fatalMessage);
            controller.abort(new Error(fatalMessage));
            break; // Exit message loop
          }
        } else if ((message as any).isError) {
          const errorMsg = String((message as any).message || "Unknown error");
          blockers.push(errorMsg);
        }

        if (isResultMessage(message) && message.type === "result") {
          const permissionDenials = (message as any).permission_denials;
          if (Array.isArray(permissionDenials)) {
            for (const denial of permissionDenials) {
              const toolName = denial?.tool_name;
              const reason = toolName ? `Permission denied for ${toolName}` : "Permission denied for tool use";
              blockers.push(reason);
              suggestedNextSteps.push("Update Claude Code permissions or enable bypassPermissions for automation runs");
            }
          }

          turns = (message as any).turns ?? (message as any).num_turns ?? turns;

          // Capture usage and cost data
          if ((message as any).usage) {
            usage = (message as any).usage;
          }
          if (typeof (message as any).total_cost_usd === "number") {
            totalCostUsd = (message as any).total_cost_usd;
          }

          if (message.subtype === "success") {
            success = true;
          } else {
            success = false;
            error = `Claude Code finished with: ${message.subtype ?? "unknown"}`;
            if (error) blockers.push(error);
          }
          break;
        }
      }

      // If successful or fatal error, break retry loop
      if (controller.signal.aborted) {
        success = false;
        if (!error) {
          error = getAbortReason() ?? "Claude Code run aborted";
        }
        blockers.push(error);
        break;
      }

      if (success || isFatalAuthError(lastErrorType)) {
        break;
      }

      // If we have a retryable error and haven't exhausted retries, retry
      if (lastErrorType && isRetryableError(lastErrorType) && retryAttempt < retryConfig.maxRetries) {
        const delay = calculateBackoffDelay(retryAttempt, retryConfig);
        const retryMsg = `Retrying after ${lastErrorType} (attempt ${retryAttempt + 1}/${retryConfig.maxRetries}, delay ${delay}ms)`;
        blockers.push(retryMsg);
        await sleep(delay);
        retryAttempt++;
        // Reset error state for retry
        lastErrorType = undefined;
        continue;
      }

      // No retryable error or retries exhausted - exit loop
      break;
    } catch (e: any) {
      success = false;
      const abortReason = getAbortReason();
      if (e instanceof AbortError || e?.name === "AbortError") {
        error = abortReason ?? e?.message ?? "Claude Code run aborted";
      } else if (typeof e?.status === "number" && (e.status === 401 || e.status === 403)) {
        error = "Claude Code authentication failed (401/403)";
        suggestedNextSteps.push("Verify ANTHROPIC_API_KEY and SDK authentication");
      } else if (typeof e?.status === "number" && e.status === 429) {
        error = "Claude Code rate limited (429)";
        suggestedNextSteps.push("Back off and retry after rate limit resets");
        lastErrorType = "rate_limit";
      } else {
        error = e?.message || String(e);
      }
      const catchMsg = `Exception during query: ${error}`;
      blockers.push(catchMsg);

      if (abortReason) {
        break;
      }

      // Check if exception might be retryable (network errors, etc)
      const errorLower = (error ?? "").toLowerCase();
      const isNetworkError =
        errorLower.includes("network") ||
        errorLower.includes("timeout") ||
        errorLower.includes("econnrefused");

      if (isNetworkError && retryAttempt < retryConfig.maxRetries) {
        const delay = calculateBackoffDelay(retryAttempt, retryConfig);
        const retryMsg = `Retrying after network error (attempt ${retryAttempt + 1}/${retryConfig.maxRetries}, delay ${delay}ms)`;
        blockers.push(retryMsg);
        await sleep(delay);
        retryAttempt++;
        continue;
      }

      // Non-retryable exception or retries exhausted
      break;
    } finally {
      cleanup();
    }
  }

  if (success) {
    error = undefined;
  }

  if (!success && !error) {
    error = "Claude Code ended without a result";
    blockers.push(error);
  }

  // Add hint if retries were exhausted (signals orchestrator to fallback)
  if (!success && retryAttempt >= retryConfig.maxRetries && lastErrorType) {
    const exhaustedMsg = `Retries exhausted after ${retryAttempt} attempts. Consider fallback to minimal subagent.`;
    blockers.push(exhaustedMsg);
    if (!error) {
      error = exhaustedMsg;
    }
  }

  const result: SubagentResult = {
    success,
    subtaskId: subtask.id,
    filesModified: Array.from(filesModified),
    turns,
    agent: "claude-code",
  };

  if (sessionId) {
    result.claudeCodeSessionId = sessionId;
    result.sessionMetadata = {
      ...(result.sessionMetadata ?? {}),
      sessionId,
      ...(options.resumeSessionId && options.forkSession && sessionId !== options.resumeSessionId
        ? { forkedFromSessionId: options.resumeSessionId }
        : {}),
    };

    if (options.resumeSessionId && options.forkSession && sessionId !== options.resumeSessionId) {
      result.claudeCodeForkedFromSessionId = options.resumeSessionId;
    }

    // Log session for debugging
    console.log(`[Claude Code] Session: ${sessionId}${options.resumeSessionId ? ` (resumed from ${options.resumeSessionId})` : " (new)"}`);
  }

  if (error) {
    result.error = error;
  }

  // Add session metadata for progress.md bridging
  if (
    toolsUsed.size > 0 ||
    blockers.length > 0 ||
    assistantMessages.length > 0 ||
    suggestedNextSteps.length > 0 ||
    usage ||
    totalCostUsd !== undefined
  ) {
    const sessionMetadata = { ...(result.sessionMetadata ?? {}) };

    if (toolsUsed.size > 0) {
      sessionMetadata.toolsUsed = Object.fromEntries(toolsUsed);
    }

    if (blockers.length > 0) {
      sessionMetadata.blockers = blockers;
    }

    if (suggestedNextSteps.length > 0) {
      sessionMetadata.suggestedNextSteps = Array.from(new Set(suggestedNextSteps));
    }

    // Use the last assistant message as summary
    if (assistantMessages.length > 0) {
      sessionMetadata.summary = assistantMessages[assistantMessages.length - 1];
    }

    // Add token usage and cost data
    if (usage) {
      sessionMetadata.usage = {
        inputTokens: usage.input_tokens ?? usage.inputTokens,
        outputTokens: usage.output_tokens ?? usage.outputTokens,
        cacheReadInputTokens: usage.cache_read_input_tokens ?? usage.cacheReadInputTokens,
        cacheCreationInputTokens: usage.cache_creation_input_tokens ?? usage.cacheCreationInputTokens,
      };
    }

    if (totalCostUsd !== undefined) {
      sessionMetadata.totalCostUsd = totalCostUsd;
    }

    result.sessionMetadata = sessionMetadata;
  }

  return result;
};
