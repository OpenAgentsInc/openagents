/**
 * Update Progress With Guidance Spell
 *
 * Appends a "Healer Summary" section to progress.md with:
 * - What failed and why
 * - What Healer tried
 * - Recommended next steps for the next session
 *
 * This ensures context is preserved across sessions.
 */
import { Effect } from "effect";
import type {
  HealerSpell,
  HealerSpellResult,
  HealerContext,
  HealerSpellId,
} from "../types.js";

/**
 * Generate the Healer Summary markdown section.
 */
export const generateHealerSummary = (
  ctx: HealerContext,
  spellsTried: HealerSpellId[],
  spellsSucceeded: HealerSpellId[]
): string => {
  const lines: string[] = [];
  const timestamp = new Date().toISOString();

  lines.push("");
  lines.push("---");
  lines.push("");
  lines.push("## Healer Summary");
  lines.push("");
  lines.push(`**Invoked at:** ${timestamp}`);
  lines.push(`**Scenario:** ${ctx.heuristics.scenario}`);
  lines.push("");

  // Failure summary
  lines.push("### What Failed");
  lines.push("");

  if (ctx.subtask) {
    lines.push(`- **Subtask:** ${ctx.subtask.id}`);
    lines.push(`- **Description:** ${ctx.subtask.description.slice(0, 100)}...`);
  }

  if (ctx.heuristics.failureCount > 0) {
    lines.push(`- **Failure count:** ${ctx.heuristics.failureCount}`);
  }

  if (ctx.heuristics.errorPatterns.length > 0) {
    lines.push("- **Error patterns:**");
    for (const pattern of ctx.heuristics.errorPatterns.slice(0, 3)) {
      lines.push(`  - ${pattern}`);
    }
  }

  if (ctx.errorOutput) {
    lines.push("");
    lines.push("**Error excerpt:**");
    lines.push("```");
    // Show first 500 chars of error
    const excerpt = ctx.errorOutput.slice(0, 500);
    lines.push(excerpt);
    if (ctx.errorOutput.length > 500) {
      lines.push("... (truncated)");
    }
    lines.push("```");
  }

  // What Healer tried
  lines.push("");
  lines.push("### Healer Actions");
  lines.push("");

  if (spellsTried.length === 0) {
    lines.push("- No spells were executed");
  } else {
    for (const spell of spellsTried) {
      const succeeded = spellsSucceeded.includes(spell);
      const icon = succeeded ? "✓" : "✗";
      lines.push(`- ${icon} \`${spell}\``);
    }
  }

  // Recommended next steps
  lines.push("");
  lines.push("### Recommended Next Steps");
  lines.push("");

  const recommendations = generateRecommendations(ctx);
  for (const rec of recommendations) {
    lines.push(`- ${rec}`);
  }

  // Git status
  if (ctx.gitStatus.isDirty) {
    lines.push("");
    lines.push("### Git Status");
    lines.push("");
    lines.push(`- **Modified files:** ${ctx.gitStatus.modifiedFiles.length}`);
    lines.push(`- **Untracked files:** ${ctx.gitStatus.untrackedFiles.length}`);
    lines.push(`- **Last commit:** ${ctx.gitStatus.lastCommitSha.slice(0, 7)} - ${ctx.gitStatus.lastCommitMessage}`);
  }

  lines.push("");

  return lines.join("\n");
};

/**
 * Generate recommended next steps based on context.
 */
const generateRecommendations = (ctx: HealerContext): string[] => {
  const recs: string[] = [];

  switch (ctx.heuristics.scenario) {
    case "InitScriptTypecheckFailure":
      recs.push("Run `bun tsc --noEmit` to see all type errors");
      recs.push("Check recent changes for type mismatches");
      if (ctx.heuristics.hasMissingImports) {
        recs.push("Verify all imports are correctly specified");
      }
      break;

    case "InitScriptTestFailure":
      recs.push("Run `bun test` to see failing tests");
      recs.push("Check if tests need updating after recent changes");
      break;

    case "InitScriptEnvironmentFailure":
      recs.push("Check network connectivity");
      recs.push("Verify environment variables are set");
      recs.push("Ensure disk space is available");
      break;

    case "VerificationFailed":
      recs.push("Review the changes made in this session");
      recs.push("Consider reverting and trying a different approach");
      if (ctx.heuristics.isFlaky) {
        recs.push("This may be a flaky test - try running again");
      }
      break;

    case "SubtaskFailed":
      if (ctx.heuristics.failureCount >= 3) {
        recs.push("Task has failed multiple times - consider breaking it down");
        recs.push("Review if the subtask description is achievable");
      } else {
        recs.push("Check the error output for specific issues");
        recs.push("Verify the task requirements are clear");
      }
      break;

    case "RuntimeError":
      recs.push("Check the orchestrator logs for more details");
      recs.push("Verify the agent configuration is correct");
      break;

    default:
      recs.push("Review the error output above");
      recs.push("Check the agent logs for more context");
  }

  return recs;
};

/**
 * Update Progress With Guidance spell.
 *
 * Appends Healer summary to progress.md so the next session has context.
 */
export const updateProgressWithGuidance: HealerSpell = {
  id: "update_progress_with_guidance",
  description: "Leave crisp explanation + next steps in progress.md",
  requiresLLM: false,

  apply: (ctx: HealerContext): Effect.Effect<HealerSpellResult, Error, never> =>
    Effect.gen(function* () {
      // Generate the summary section
      // Note: spellsTried/spellsSucceeded will be empty here since this spell
      // runs as part of a sequence - the actual values come from HealerService
      const summary = generateHealerSummary(ctx, [], []);

      // The actual file write will be done by HealerService
      // which has the full context of which spells were tried
      // For now, we just prepare the content

      // Get the progress file path
      const progressPath = `${ctx.projectRoot}/.openagents/progress.md`;

      // Try to append to progress.md
      try {
        const existingContent = ctx.progressMd ?? "";
        const newContent = existingContent + summary;

        yield* Effect.tryPromise({
          try: async () => {
            await Bun.write(progressPath, newContent);
          },
          catch: (error) => new Error(`Failed to write progress.md: ${error}`),
        });

        return {
          success: true,
          changesApplied: true,
          summary: "Updated progress.md with Healer summary",
          filesModified: [progressPath],
        };
      } catch (error) {
        return {
          success: false,
          changesApplied: false,
          summary: `Failed to update progress.md: ${error}`,
          error: String(error),
        };
      }
    }),
};

// Export helper for use by HealerService
export { generateRecommendations };
