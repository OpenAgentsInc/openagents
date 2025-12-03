import {
  CLAUDE_CODE_MCP_SERVER_NAME,
  createMechaCoderMcpServer,
  getAllowedClaudeCodeTools,
} from "./claude-code-mcp.js";
import type { PermissionMode, McpServerConfig } from "@anthropic-ai/claude-agent-sdk";
import { type SubagentResult, type Subtask, type ClaudeCodePermissionMode } from "./types.js";

type QueryFn = (input: unknown) => AsyncIterable<any>;

export interface ClaudeCodeSubagentOptions {
  cwd: string;
  maxTurns?: number;
  systemPrompt?: string;
  permissionMode?: ClaudeCodePermissionMode | PermissionMode;
  queryFn?: QueryFn;
  openagentsDir?: string;
  allowedTools?: string[];
  mcpServers?: Record<string, unknown>;
}

const isToolCallMessage = (message: any): message is { tool_calls?: Array<{ name?: string; input?: any }> } =>
  typeof message === "object" && message !== null && Array.isArray((message as any).tool_calls);

const isResultMessage = (message: any): message is { type?: string; subtype?: string } =>
  typeof message === "object" && message !== null && "type" in message;

const defaultBuildPrompt = (subtask: Subtask): string =>
  `## Subtask: ${subtask.id}

${subtask.description}

Focus on minimal, correct changes.`;

/**
 * Run a subtask using Claude Code Agent SDK's query() streaming API.
 * Designed to be dependency-injected in tests via queryFn to avoid network calls.
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
  let success = false;
  let error: string | undefined;
  let turns = 0;

  const mcpServers: Record<string, McpServerConfig> =
    (options.mcpServers as Record<string, McpServerConfig> | undefined) ??
    {
      [CLAUDE_CODE_MCP_SERVER_NAME]: createMechaCoderMcpServer({
        openagentsDir: options.openagentsDir ?? "",
      }) as McpServerConfig,
    };

  const allowedTools = options.allowedTools ?? getAllowedClaudeCodeTools();

  try {
    for await (const message of query({
      prompt: defaultBuildPrompt(subtask),
      options: {
        cwd: options.cwd,
        ...(options.systemPrompt ? { systemPrompt: options.systemPrompt } : {}),
        maxTurns: options.maxTurns ?? 30,
        ...(options.permissionMode ? { permissionMode: options.permissionMode } : {}),
        mcpServers,
        allowedTools,
      },
    })) {
      // Track tool calls
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

      // Capture assistant messages for context
      if ((message as any).role === "assistant" && (message as any).content) {
        const content = String((message as any).content);
        if (content.length > 0 && content.length < 500) {
          assistantMessages.push(content);
        }
      }

      // Capture error messages as blockers
      if ((message as any).error || (message as any).isError) {
        const errorMsg = String((message as any).error || (message as any).message || "Unknown error");
        blockers.push(errorMsg);
      }

      if (isResultMessage(message) && message.type === "result") {
        turns = (message as any).turns ?? turns;
        if (message.subtype === "success") {
          success = true;
        } else {
          success = false;
          error = `Claude Code finished with: ${message.subtype ?? "unknown"}`;
          if (error) blockers.push(error);
        }
      }
    }
  } catch (e: any) {
    success = false;
    error = e?.message || String(e);
    blockers.push(error);
  }

  const result: SubagentResult = {
    success,
    subtaskId: subtask.id,
    filesModified: Array.from(filesModified),
    turns,
  };

  if (error) {
    result.error = error;
  }

  // Add session metadata for progress.md bridging
  if (toolsUsed.size > 0 || blockers.length > 0 || assistantMessages.length > 0) {
    result.sessionMetadata = {};

    if (toolsUsed.size > 0) {
      result.sessionMetadata.toolsUsed = Object.fromEntries(toolsUsed);
    }

    if (blockers.length > 0) {
      result.sessionMetadata.blockers = blockers;
    }

    // Use the last assistant message as summary
    if (assistantMessages.length > 0) {
      result.sessionMetadata.summary = assistantMessages[assistantMessages.length - 1];
    }
  }

  return result;
};
