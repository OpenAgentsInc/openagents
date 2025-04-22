import { Effect, Context, Data, Layer } from "effect";
import { solverTools } from "./tools";
import { solverContext } from "./tools";
import type { SolverState } from "./types";

// --- Tool-specific Error Types ---
export class GetIssueDetailsError extends Data.TaggedError("GetIssueDetailsError")<{ message: string; status?: number }> {}
export class UpdateIssueStatusError extends Data.TaggedError("UpdateIssueStatusError")<{ message: string; status?: number }> {}
export class CreatePlanError extends Data.TaggedError("CreatePlanError")<{ message: string }> {}
export class ToolContextError extends Data.TaggedError("ToolContextError")<{ message: string }> {}
export class ToolNotFoundError extends Data.TaggedError("ToolNotFoundError")<{ name: string }> {}
export class ToolExecutionError extends Data.TaggedError("ToolExecutionError")<{ name: string; cause: unknown }> {}

// --- Service Tag for Agent Context ---
export interface AgentContextService {
  readonly getSolverState: Effect.Effect<SolverState, never, never>;
  readonly updateSolverState: (update: Partial<SolverState>) => Effect.Effect<SolverState, never, never>;
}

// --- Format Tools for Anthropic API ---
export function formatToolsForAnthropic() {
  const formattedTools = [];
  
  // Format GetIssueDetails
  formattedTools.push({
    name: "GetIssueDetails",
    description: "Fetches comprehensive issue information from GitHub.",
    input_schema: {
      type: "object",
      properties: {
        owner: { type: "string", description: "Repository owner" },
        repo: { type: "string", description: "Repository name" },
        issueNumber: { type: "number", description: "Issue number" }
      },
      required: ["owner", "repo", "issueNumber"]
    }
  });
  
  // Format UpdateIssueStatus
  formattedTools.push({
    name: "UpdateIssueStatus",
    description: "Updates the status of a GitHub issue.",
    input_schema: {
      type: "object",
      properties: {
        owner: { type: "string", description: "Repository owner" },
        repo: { type: "string", description: "Repository name" },
        issueNumber: { type: "number", description: "Issue number" },
        status: { type: "string", description: "New status (e.g., 'open', 'closed')" },
        comment: { type: "string", description: "Optional comment to add" }
      },
      required: ["owner", "repo", "issueNumber", "status"]
    }
  });
  
  // Format CreateImplementationPlan
  formattedTools.push({
    name: "CreateImplementationPlan",
    description: "Creates a step-by-step implementation plan for the current issue.",
    input_schema: {
      type: "object",
      properties: {
        steps: { 
          type: "array", 
          description: "Optional custom steps for the plan",
          items: { type: "string" } 
        }
      },
      required: []
    }
  });
  
  return formattedTools;
}

// Execute a tool by name with params - this uses the underlying Vercel AI SDK tools
export async function executeToolByName(
  toolName: string, 
  params: Record<string, any>
): Promise<any> {
  try {
    // Get the tool from our solverTools with proper type guard
    if (!(toolName in solverTools)) {
      return { error: `Tool '${toolName}' not found` };
    }
    
    const tool = solverTools[toolName as keyof typeof solverTools];
    
    // Execute the tool with type assertion to avoid TypeScript errors
    const options = {}; // Empty options object required by Vercel AI SDK
    const result = await (tool.execute as any)(params, options);
    return result;
  } catch (error) {
    console.error(`Failed to execute tool ${toolName}:`, error);
    return { error: String(error) };
  }
}

// --- Toolkit Implementation Layer ---
export const SolverToolsImplementationLayer = Layer.empty;