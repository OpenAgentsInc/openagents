/**
 * Task Integration for Researcher
 *
 * Creates and closes tasks in .openagents/tasks.jsonl for paper research work.
 */
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { createTask, closeTask as closeTaskService } from "../tasks/service.js";
import type { PaperRecord } from "./registry.js";

// ============================================================================
// Task Creation
// ============================================================================

/**
 * Create a research task for a paper.
 *
 * @returns The task ID if created, undefined if failed
 */
export const createResearchTask = async (
  paper: PaperRecord,
  tasksPath: string = ".openagents/tasks.jsonl"
): Promise<string | undefined> => {
  const program = Effect.gen(function* () {
    const task = yield* createTask({
      tasksPath,
      task: {
        title: `Research: ${paper.title}`,
        description: buildTaskDescription(paper),
        type: "task",
        priority: paper.priority === "HIGH" ? 1 : paper.priority === "MEDIUM" ? 2 : 3,
        status: "open",
        labels: ["research", "paper"],
        deps: [],
        comments: [],
      },
    });
    return task.id;
  });

  try {
    return await Effect.runPromise(
      program.pipe(Effect.provide(BunContext.layer))
    );
  } catch (error) {
    console.error("Failed to create research task:", error);
    return undefined;
  }
};

/**
 * Build the task description for a paper.
 */
const buildTaskDescription = (paper: PaperRecord): string => {
  const parts: string[] = [
    `## Research Paper: ${paper.title}`,
    "",
    "Find and summarize this academic paper for the OpenAgents research collection.",
    "",
    "### Details",
  ];

  if (paper.authors) {
    parts.push(`- **Authors:** ${paper.authors}`);
  }
  if (paper.year) {
    parts.push(`- **Year:** ${paper.year}`);
  }
  if (paper.urls.length > 0) {
    parts.push(`- **URLs:**`);
    for (const url of paper.urls) {
      parts.push(`  - ${url}`);
    }
  }
  parts.push(`- **Priority:** ${paper.priority}`);
  parts.push(`- **Source:** ${paper.sourceDoc}`);

  parts.push("");
  parts.push("### Output");
  parts.push("Create a summary at `docs/research/paper-summaries/<slug>-summary.md` following the standard format.");
  parts.push("");
  parts.push("### Acceptance Criteria");
  parts.push("- [ ] Paper content retrieved (abstract and/or full text)");
  parts.push("- [ ] Summary follows voyager-summary.md template");
  parts.push("- [ ] Includes 'Relevance to MechaCoder' section");
  parts.push("- [ ] Citation in BibTeX format");

  return parts.join("\n");
};

// ============================================================================
// Task Closing
// ============================================================================

/**
 * Close a research task when the summary is created.
 *
 * @returns true if closed successfully, false otherwise
 */
export const closeResearchTask = async (
  taskId: string,
  summaryPath: string,
  tasksPath: string = ".openagents/tasks.jsonl"
): Promise<boolean> => {
  const program = Effect.gen(function* () {
    yield* closeTaskService({
      tasksPath,
      id: taskId,
      reason: `Summary created at ${summaryPath}`,
    });
    return true;
  });

  try {
    return await Effect.runPromise(
      program.pipe(Effect.provide(BunContext.layer))
    );
  } catch (error) {
    console.error("Failed to close research task:", error);
    return false;
  }
};

// ============================================================================
// Task Lookup
// ============================================================================

/**
 * Find a research task by paper title.
 */
export const findResearchTask = async (
  paperTitle: string,
  tasksPath: string = ".openagents/tasks.jsonl"
): Promise<string | undefined> => {
  // Read tasks file and search for matching title
  try {
    const file = Bun.file(tasksPath);
    if (!(await file.exists())) {
      return undefined;
    }

    const content = await file.text();
    const lines = content.split("\n").filter((l) => l.trim());

    for (const line of lines) {
      const task = JSON.parse(line);
      if (
        task.title?.includes(`Research: ${paperTitle}`) ||
        task.title?.includes(paperTitle)
      ) {
        return task.id;
      }
    }

    return undefined;
  } catch {
    return undefined;
  }
};
