/**
 * MechaCoder MCP Bridge
 *
 * Provides MCP tools for Claude Code to coordinate with the MechaCoder orchestrator.
 * These tools enable Claude Code sessions to signal completion, request help, and read progress.
 */

import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import { readProgress } from "../agent/orchestrator/progress.js";

export interface McpBridgeOptions {
  openagentsDir: string;
  onSubtaskComplete?: (summary: string, filesModified: string[]) => Promise<void>;
  onHelpRequested?: (issue: string, suggestion?: string) => Promise<void>;
}

/**
 * Create an MCP server with MechaCoder-specific tools for Claude Code.
 *
 * Tools provided:
 * - subtask_complete: Signal that the current subtask is complete
 * - request_help: Request orchestrator intervention when stuck
 * - read_progress: Read the current session progress file
 */
export const createMechaCoderMcpServer = (options: McpBridgeOptions) => {
  return createSdkMcpServer({
    name: "mechacoder",
    version: "1.0.0",
    tools: [
      tool(
        "subtask_complete",
        "Signal that the current subtask is complete",
        {
          summary: z.string().describe("Brief summary of what was done"),
          filesModified: z.array(z.string()).describe("List of modified files"),
        },
        async (args) => {
          // Notify orchestrator if handler provided
          if (options.onSubtaskComplete) {
            await options.onSubtaskComplete(args.summary, args.filesModified);
          }

          return {
            content: [
              {
                type: "text" as const,
                text: `Subtask complete: ${args.summary}\nFiles modified: ${args.filesModified.join(", ")}`,
              },
            ],
          };
        }
      ),

      tool(
        "request_help",
        "Request orchestrator intervention when stuck",
        {
          issue: z.string().describe("What problem you're facing"),
          suggestion: z.string().optional().describe("Suggested resolution"),
        },
        async (args) => {
          // Notify orchestrator if handler provided
          if (options.onHelpRequested) {
            await options.onHelpRequested(args.issue, args.suggestion);
          }

          const message = args.suggestion
            ? `Help requested: ${args.issue}\nSuggested resolution: ${args.suggestion}`
            : `Help requested: ${args.issue}`;

          return {
            content: [
              {
                type: "text" as const,
                text: message,
              },
            ],
          };
        }
      ),

      tool(
        "read_progress",
        "Read the current session progress file",
        {},
        async () => {
          const progress = readProgress(options.openagentsDir);

          if (!progress) {
            return {
              content: [
                {
                  type: "text" as const,
                  text: "No progress file found. This is the first session or progress has not been written yet.",
                },
              ],
            };
          }

          return {
            content: [
              {
                type: "text" as const,
                text: progress,
              },
            ],
          };
        }
      ),
    ],
  });
};
