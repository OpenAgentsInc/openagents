import {
  CLAUDE_CODE_MCP_SERVER_NAME,
  createMechaCoderMcpServer,
  getAllowedClaudeCodeTools,
} from "./claude-code-mcp.js";
import { type SubagentResult, type Subtask } from "./types.js";

type QueryFn = (input: unknown) => AsyncIterable<any>;

export interface ClaudeCodeSubagentOptions {
  cwd: string;
  maxTurns?: number;
  systemPrompt?: string;
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
  let success = false;
  let error: string | undefined;
  let turns = 0;

  const mcpServers =
    options.mcpServers ??
    {
      [CLAUDE_CODE_MCP_SERVER_NAME]: createMechaCoderMcpServer({ openagentsDir: options.openagentsDir }),
    };

  const allowedTools = options.allowedTools ?? getAllowedClaudeCodeTools();

  try {
    for await (const message of query({
      prompt: defaultBuildPrompt(subtask),
      options: {
        cwd: options.cwd,
        ...(options.systemPrompt ? { systemPrompt: options.systemPrompt } : {}),
        maxTurns: options.maxTurns ?? 30,
        mcpServers,
        allowedTools,
      },
    })) {
      if (isToolCallMessage(message) && message.tool_calls) {
        for (const call of message.tool_calls) {
          const filePath = call?.input?.file_path || call?.input?.path;
          const name = call?.name;
          if (filePath && (name === "Edit" || name === "Write")) {
            filesModified.add(String(filePath));
          }
        }
      }

      if (isResultMessage(message) && message.type === "result") {
        turns = (message as any).turns ?? turns;
        if (message.subtype === "success") {
          success = true;
        } else {
          success = false;
          error = `Claude Code finished with: ${message.subtype ?? "unknown"}`;
        }
      }
    }
  } catch (e: any) {
    success = false;
    error = e?.message || String(e);
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

  return result;
};
