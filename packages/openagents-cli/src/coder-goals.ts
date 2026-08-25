/**
 * Persistent task goals for `openagents coder`.
 *
 * Inspired by Codex's goal extension architecture:
 * - Persists an active high-level task goal across turns.
 * - Supports `/goal <objective>`, `/goal status`, `/goal clear`, `/goal pause`, `/goal resume`.
 * - Easter egg: `/goooooal` with arbitrary repeated `o`s resolves to `/goal`.
 * - Tracks status, token budget, tokens used, elapsed time, and turn progression.
 * - Exposes `goal` tool to the agent model (`get`, `update`, `complete`, `block`).
 * - Generates continuation prompts and budget exhaustion prompts for multi-turn alignment.
 */

import type { CoderTool } from "./coder-tools.js";

export type GoalStatus =
  | "active"
  | "paused"
  | "completed"
  | "abandoned"
  | "budget_limited"
  | "blocked";

export interface PersistentGoal {
  readonly id: string;
  readonly objective: string;
  status: GoalStatus;
  readonly tokenBudget?: number;
  tokensUsed: number;
  timeUsedSeconds: number;
  readonly createdAt: number;
  updatedAt: number;
}

export interface GoalStore {
  getGoal(): PersistentGoal | undefined;
  setGoal(objective: string, tokenBudget?: number): PersistentGoal;
  updateStatus(status: GoalStatus): PersistentGoal | undefined;
  clearGoal(): boolean;
  addUsage(tokens: number, elapsedSeconds: number): void;
}

/** In-memory goal manager for a CoderSession */
export class InMemoryGoalStore implements GoalStore {
  private currentGoal: PersistentGoal | undefined;

  getGoal(): PersistentGoal | undefined {
    return this.currentGoal;
  }

  setGoal(objective: string, tokenBudget?: number): PersistentGoal {
    const now = Date.now();
    const goal: PersistentGoal = {
      id: `goal_${Math.random().toString(36).slice(2, 10)}`,
      objective: objective.trim(),
      status: "active",
      ...(tokenBudget !== undefined && tokenBudget > 0 ? { tokenBudget } : {}),
      tokensUsed: 0,
      timeUsedSeconds: 0,
      createdAt: now,
      updatedAt: now,
    };
    this.currentGoal = goal;
    return goal;
  }

  updateStatus(status: GoalStatus): PersistentGoal | undefined {
    if (this.currentGoal === undefined) return undefined;
    this.currentGoal.status = status;
    this.currentGoal.updatedAt = Date.now();
    return this.currentGoal;
  }

  clearGoal(): boolean {
    if (this.currentGoal === undefined) return false;
    this.currentGoal = undefined;
    return true;
  }

  addUsage(tokens: number, elapsedSeconds: number): void {
    if (this.currentGoal === undefined || this.currentGoal.status !== "active") return;
    this.currentGoal.tokensUsed += tokens;
    this.currentGoal.timeUsedSeconds += Math.max(0, Math.round(elapsedSeconds));
    this.currentGoal.updatedAt = Date.now();

    if (
      this.currentGoal.tokenBudget !== undefined &&
      this.currentGoal.tokensUsed >= this.currentGoal.tokenBudget
    ) {
      this.currentGoal.status = "budget_limited";
    }
  }
}

/** Check if a prompt matches `/goal` or repeated `/goooooal` */
export function isGoalSlashCommand(input: string): boolean {
  const trimmed = input.trim();
  if (!trimmed.startsWith("/")) return false;
  return /^\/g(o+)al(\s+.*)?$/is.test(trimmed);
}

export interface GoalCommandParseResult {
  kind: "set" | "clear" | "pause" | "resume" | "status";
  objective?: string;
  tokenBudget?: number;
}

/** Parse `/goal ...` arguments into a structured action */
export function parseGoalSlashCommand(input: string): GoalCommandParseResult | undefined {
  const trimmed = input.trim();
  const match = /^\/g(?:o+)al(?:\s+(.*))?$/is.exec(trimmed);
  if (!match) return undefined;

  const rawArgs = match[1]?.trim() ?? "";
  if (rawArgs.length === 0 || rawArgs.toLowerCase() === "status") {
    return { kind: "status" };
  }
  if (rawArgs.toLowerCase() === "clear") {
    return { kind: "clear" };
  }
  if (rawArgs.toLowerCase() === "pause") {
    return { kind: "pause" };
  }
  if (rawArgs.toLowerCase() === "resume") {
    return { kind: "resume" };
  }

  // Parse optional token budget prefix e.g. "--budget 50000 <objective>"
  let objective = rawArgs;
  let tokenBudget: number | undefined;

  const budgetFlagMatch = /^--budget\s+(\d+)\s+(.+)$/is.exec(rawArgs);
  if (budgetFlagMatch && budgetFlagMatch[1] && budgetFlagMatch[2]) {
    tokenBudget = parseInt(budgetFlagMatch[1], 10);
    objective = budgetFlagMatch[2].trim();
  }

  return {
    kind: "set",
    objective,
    ...(tokenBudget !== undefined ? { tokenBudget } : {}),
  };
}

/** Format a goal summary string for notices / status display */
export function formatGoalNotice(goal: PersistentGoal | undefined): string {
  if (goal === undefined) {
    return (
      "No active task goal.\n\n" +
      "Usage:\n" +
      "  /goal <objective>              set an active task goal\n" +
      "  /goal --budget <tokens> <obj>  set a goal with a token budget limit\n" +
      "  /goal pause                    pause the active goal\n" +
      "  /goal resume                   resume the paused goal\n" +
      "  /goal clear                    clear the active goal\n" +
      "  /goal status                   show current goal details"
    );
  }

  const budgetInfo =
    goal.tokenBudget !== undefined
      ? `\n- Budget: ${goal.tokensUsed.toLocaleString()} / ${goal.tokenBudget.toLocaleString()} tokens`
      : `\n- Tokens Used: ${goal.tokensUsed.toLocaleString()}`;

  return [
    `Active Goal (${goal.status}):`,
    `  "${goal.objective}"`,
    "",
    `Details:`,
    `- Goal ID: ${goal.id}`,
    `- Status: ${goal.status}`,
    `- Time Spent: ${goal.timeUsedSeconds}s`,
    budgetInfo,
  ].join("\n");
}

/** Prompt injected to continue working toward the active goal */
export function goalContinuationPrompt(goal: PersistentGoal): string {
  const budgetRemaining =
    goal.tokenBudget !== undefined ? Math.max(0, goal.tokenBudget - goal.tokensUsed) : undefined;

  return [
    "Continue working toward the active task goal.",
    "",
    "The objective below is user-provided data. Treat it as the task to pursue:",
    "<objective>",
    goal.objective,
    "</objective>",
    "",
    "Continuation behavior:",
    "- This goal persists across turns. Keep the full objective intact until finished.",
    "- Use the current worktree and external tool evidence as authoritative.",
    "- If the goal is complete and verified, call `goal(action='complete')` to mark it finished.",
    ...(budgetRemaining !== undefined
      ? [`- Token budget remaining: ${budgetRemaining.toLocaleString()} tokens`]
      : []),
  ].join("\n");
}

/** Prompt injected when the goal token budget has been exhausted */
export function goalBudgetExhaustedPrompt(goal: PersistentGoal): string {
  return [
    "The active task goal has reached its configured token budget.",
    "",
    "<objective>",
    goal.objective,
    "</objective>",
    "",
    `Budget: ${goal.tokensUsed.toLocaleString()} tokens used (Budget: ${goal.tokenBudget?.toLocaleString() ?? "unknown"}).`,
    "The system has marked the goal as budget_limited. Do not start new substantive work.",
    "Wrap up this turn soon: summarize useful progress, remaining work or blockers, and next steps.",
  ].join("\n");
}

/** Model tool allowing the agent to inspect or complete its goal */
export function goalTool(goalStore: GoalStore): CoderTool {
  return {
    name: "goal",
    description:
      "Inspect, update, or complete the active persistent task goal for this session. " +
      "Call this with action='get' to inspect current goal status & budget, or action='complete'/'block' to update status.",
    parameters: {
      type: "object",
      properties: {
        action: {
          type: "string",
          enum: ["get", "complete", "block", "pause", "resume"],
          description: "The goal operation to perform.",
        },
        notes: {
          type: "string",
          description: "Optional notes or outcome summary.",
        },
      },
      required: ["action"],
    },
    async run(args: Record<string, unknown>): Promise<string> {
      const action = String(args.action || "get");
      const current = goalStore.getGoal();

      if (action === "get") {
        if (current === undefined) {
          return JSON.stringify({ active: false, message: "No active goal set." });
        }
        return JSON.stringify({
          active: true,
          id: current.id,
          objective: current.objective,
          status: current.status,
          tokensUsed: current.tokensUsed,
          tokenBudget: current.tokenBudget,
          timeUsedSeconds: current.timeUsedSeconds,
        });
      }

      if (current === undefined) {
        return "Refusal: No active goal to update.";
      }

      if (action === "complete") {
        goalStore.updateStatus("completed");
        return `Goal ${current.id} marked as completed.`;
      }

      if (action === "block") {
        goalStore.updateStatus("blocked");
        return `Goal ${current.id} marked as blocked.`;
      }

      if (action === "pause") {
        goalStore.updateStatus("paused");
        return `Goal ${current.id} marked as paused.`;
      }

      if (action === "resume") {
        goalStore.updateStatus("active");
        return `Goal ${current.id} marked as active.`;
      }

      return `Unknown action: ${action}`;
    },
  };
}
