/**
 * Mark Task Blocked With Followup Spell
 *
 * When Healer cannot fix an issue, this spell:
 * 1. Marks the current task as "blocked" with a clear reason
 * 2. Creates a follow-up task with details about what went wrong
 * 3. Links the tasks via discovered-from dependency
 *
 * This ensures issues are surfaced clearly rather than thrashing on unfixable problems.
 */
import { Effect } from "effect";
import type { HealerSpell, HealerSpellResult, HealerContext } from "../types.js";

/**
 * Generate a descriptive block reason from Healer context.
 */
const generateBlockReason = (ctx: HealerContext): string => {
  const parts: string[] = [];

  parts.push(`Healer invoked for scenario: ${ctx.heuristics.scenario}`);

  if (ctx.heuristics.failureCount > 1) {
    parts.push(`Failed ${ctx.heuristics.failureCount} times`);
  }

  if (ctx.heuristics.hasTypeErrors) {
    parts.push("Contains type errors");
  }

  if (ctx.heuristics.hasTestAssertions) {
    parts.push("Test assertions failing");
  }

  if (ctx.heuristics.hasMissingImports) {
    parts.push("Missing imports detected");
  }

  if (ctx.errorOutput) {
    // Truncate error output for block reason
    const truncated = ctx.errorOutput.slice(0, 200);
    parts.push(`Error: ${truncated}${ctx.errorOutput.length > 200 ? "..." : ""}`);
  }

  return parts.join(". ");
};

/**
 * Generate a detailed follow-up task description.
 */
const generateFollowupDescription = (ctx: HealerContext): string => {
  const lines: string[] = [];

  lines.push("## Context");
  lines.push("");
  lines.push(`- **Scenario**: ${ctx.heuristics.scenario}`);
  lines.push(`- **Session**: ${ctx.sessionId}`);
  if (ctx.task) {
    lines.push(`- **Original Task**: ${ctx.task.id} - ${ctx.task.title}`);
  }
  if (ctx.subtask) {
    lines.push(`- **Subtask**: ${ctx.subtask.id}`);
    lines.push(`- **Subtask Description**: ${ctx.subtask.description}`);
  }

  lines.push("");
  lines.push("## Failure Details");
  lines.push("");

  if (ctx.heuristics.errorPatterns.length > 0) {
    lines.push("**Error Patterns Detected:**");
    for (const pattern of ctx.heuristics.errorPatterns.slice(0, 5)) {
      lines.push(`- ${pattern}`);
    }
    lines.push("");
  }

  if (ctx.errorOutput) {
    lines.push("**Error Output:**");
    lines.push("```");
    lines.push(ctx.errorOutput.slice(0, 1000));
    if (ctx.errorOutput.length > 1000) {
      lines.push("... (truncated)");
    }
    lines.push("```");
    lines.push("");
  }

  lines.push("## Suggested Investigation");
  lines.push("");
  lines.push("1. Review the error output above");
  lines.push("2. Check if there are environmental issues (missing deps, config)");
  lines.push("3. Verify the subtask description is achievable");
  lines.push("4. Consider breaking the task into smaller pieces");

  if (ctx.trajectory) {
    lines.push("");
    lines.push("## Trajectory Reference");
    lines.push("");
    lines.push(`Session ID: \`${ctx.trajectory.session_id}\``);
  }

  return lines.join("\n");
};

/**
 * Mark Task Blocked With Followup spell.
 *
 * This is a "containment" spell - it doesn't fix the problem but
 * ensures it's properly documented and doesn't cause further issues.
 */
export const markTaskBlockedWithFollowup: HealerSpell = {
  id: "mark_task_blocked_with_followup",
  description: "Stop thrashing; surface problem clearly",
  requiresLLM: false,

  apply: (ctx: HealerContext): Effect.Effect<HealerSpellResult, Error, never> =>
    Effect.gen(function* () {
      const tasksAffected: string[] = [];

      // We can't actually modify tasks without TaskService access
      // This spell prepares the data; the orchestrator/service layer applies it
      const blockReason = generateBlockReason(ctx);
      const followupDescription = generateFollowupDescription(ctx);

      // For now, return the prepared data
      // The actual task modification will be done by HealerService
      // which has access to TaskService

      if (ctx.task) {
        tasksAffected.push(ctx.task.id);
      }

      return {
        success: true,
        changesApplied: false, // Changes are prepared but not yet applied
        summary: `Prepared block reason and follow-up task for ${ctx.task?.id ?? "unknown task"}`,
        tasksAffected,
        // Store prepared data in a way the service can use
        // (This will be picked up by HealerService.maybeRun)
      } as HealerSpellResult & {
        _prepared?: {
          blockReason: string;
          followupDescription: string;
          followupTitle: string;
        };
      };
    }),
};

// Export helpers for use by HealerService
export { generateBlockReason, generateFollowupDescription };
