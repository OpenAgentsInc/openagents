
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { importBeadsIssues } from "./beads.js";
import { runWithTestContext } from "./test-helpers.js";
import { readTasks } from "./service.js";


describe("importBeadsIssues", () => {
  test("converts beads issues.jsonl into .openagents/tasks.jsonl", async () => {
    const result = await runWithTestContext(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "beads-import" });
        const beadsPath = path.join(dir, "issues.jsonl");
        const tasksPath = path.join(dir, ".openagents", "tasks.jsonl");

        const issues = [
          {
            id: "oa-1",
            title: "Root task",
            description: "Test import",
            status: "open",
            priority: 1,
            issue_type: "task",
            labels: ["cli"],
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T01:00:00Z",
            dependencies: [],
          },
          {
            id: "oa-1.1",
            title: "Child task",
            status: "in_progress",
            priority: 2,
            issue_type: "task",
            created_at: "2025-01-02T00:00:00Z",
            dependencies: [
              { issue_id: "oa-1.1", depends_on_id: "oa-1", type: "parent-child" },
              { issue_id: "oa-1.1", depends_on_id: "oa-1", type: "blocks" },
            ],
            close_reason: "done",
            closed_at: null,
            acceptance_criteria: "ac",
            notes: "note",
            estimated_minutes: 30,
          },
        ];

        yield* fs.writeFile(
          beadsPath,
          new TextEncoder().encode(issues.map((i) => JSON.stringify(i)).join("\n") + "\n"),
        );

        const importResult = yield* importBeadsIssues(beadsPath, tasksPath);
        const tasks = yield* readTasks(tasksPath);

        return { importResult, tasks };
      }),
    );

    expect(result.importResult.count).toBe(2);
    expect(result.tasks).toHaveLength(2);
    const child = result.tasks.find((t) => t.id === "oa-1.1");
    // Duplicate dependencies are deduplicated - we keep only the first one
    expect(child?.deps).toEqual([
      { id: "oa-1", type: "parent-child" },
    ]);
    expect(child?.acceptanceCriteria).toBe("ac");
    expect(child?.notes).toBe("note");
    expect(child?.estimatedMinutes).toBe(30);
    expect(child?.closeReason).toBe("done");
    // SQLite returns undefined for NULL values
    expect(child?.closedAt).toBeUndefined();
  });
});
