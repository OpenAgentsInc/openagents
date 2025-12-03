import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import { readProgress } from "./progress.js";
import { getProgressPath } from "./types.js";

export const CLAUDE_CODE_MCP_SERVER_NAME = "mechacoder";
export const CLAUDE_CODE_MCP_VERSION = "1.0.0";

export interface MechaCoderMcpOptions {
  openagentsDir?: string;
}

const formatProgressPayload = (openagentsDir?: string): string => {
  if (!openagentsDir) {
    return "Progress unavailable: openagentsDir not provided.";
  }

  const progress = readProgress(openagentsDir);
  if (!progress) {
    return `No progress file found at ${getProgressPath(openagentsDir)}`;
  }

  return JSON.stringify(progress, null, 2);
};

export const buildMechaCoderMcpTools = (options: MechaCoderMcpOptions = {}) => {
  return [
    tool(
      "subtask_complete",
      "Signal that the current subtask is complete",
      {
        summary: z.string().describe("Brief summary of what was done"),
        filesModified: z.array(z.string()).optional().default([]).describe("List of modified files"),
      },
      async (args) => ({
        content: [
          {
            type: "text",
            text:
              args.filesModified && args.filesModified.length > 0
                ? `Subtask complete: ${args.summary} (files: ${args.filesModified.join(", ")})`
                : `Subtask complete: ${args.summary}`,
          },
        ],
      })
    ),
    tool(
      "request_help",
      "Request orchestrator intervention when stuck",
      {
        issue: z.string().describe("Issue encountered"),
        suggestion: z.string().optional().describe("Suggested resolution"),
      },
      async (args) => ({
        content: [
          {
            type: "text",
            text: args.suggestion
              ? `Help requested: ${args.issue}\nSuggested: ${args.suggestion}`
              : `Help requested: ${args.issue}`,
          },
        ],
      })
    ),
    tool(
      "read_progress",
      "Read the current session progress file",
      {},
      async () => ({
        content: [
          {
            type: "text",
            text: formatProgressPayload(options.openagentsDir),
          },
        ],
      })
    ),
  ];
};

export const createMechaCoderMcpServer = (options: MechaCoderMcpOptions = {}) =>
  createSdkMcpServer({
    name: CLAUDE_CODE_MCP_SERVER_NAME,
    version: CLAUDE_CODE_MCP_VERSION,
    tools: buildMechaCoderMcpTools(options),
  });

export const getAllowedClaudeCodeTools = (serverName = CLAUDE_CODE_MCP_SERVER_NAME): string[] => [
  "Read",
  "Write",
  "Edit",
  "Bash",
  "Glob",
  "Grep",
  `mcp__${serverName}__subtask_complete`,
  `mcp__${serverName}__request_help`,
  `mcp__${serverName}__read_progress`,
];
