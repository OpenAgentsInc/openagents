## MechaCoder Next Tasks Plan (openagents repo)

You are MechaCoder, running inside the **openagents** repo.

Your job now is to:

1. Confirm the new `.openagents` task system is solid and close its epic.
2. Then continue the “Autonomous Coding Agent Infrastructure” epic (`openagents-42j`) in a deliberate order that strengthens the Golden Loop.

Do **not** ask for permission for any of the steps below. Do the work, keep tests green, and log your actions.

---

### 0. Always read specs first

At the start of a session:

1. Read:

   * `docs/mechacoder/spec.md`
   * `docs/mechacoder/GOLDEN-LOOP-v2.md`
   * `docs/mechacoder/MECHACODER-OPS.md`
2. Skim:

   * `src/tasks/*.ts` (schema, id, service, project, beads import)
   * `src/agent/*.ts` (especially `do-one-bead.ts`, `loop.ts`, `session.ts`)

Treat these as the **source of truth** for how the desktop Golden Loop should behave.

---

### 1. Finalize `.openagents` epic (openagents-5bb)

**Goal:** Confirm the `.openagents` task system is actually usable for a project and then close `openagents-5bb`.

1. Verify all child tasks are implemented and tested:

   * `openagents-5bb.1` – Task schema (Effect Schema for Task/Status/IssueType/Dependency)
   * `openagents-5bb.2` – TaskService (CRUD + ready selection)
   * `openagents-5bb.3` – ProjectService (project.json)
   * `openagents-5bb.4` – BeadsConverter (.beads → .openagents/tasks.jsonl)
   * `openagents-5bb.5` – ID generator
   * `openagents-5bb.6` – TaskPicker
   * `openagents-5bb.7` – Init CLI
   * `openagents-5bb.8` – Integration into `do-one-bead.ts`
   * `openagents-5bb.9` – Tests for TaskService/TaskSchema

   You can confirm by:

   * Inspecting the implementations in `src/tasks/*.ts`.
   * Running:

     ```bash
     bun test
     ```

     and verifying all `src/tasks/*.test.ts` suites pass (they currently do).

2. Smoke-test `.openagents` init end-to-end in **openagents**:

   * In a **throwaway directory** under `tmp/` or using a new project fixture:

     * Run the init CLI described by `openagents-5bb.7` (e.g. `bun run <init-command>`).
     * Confirm it creates `.openagents/project.json` and `.openagents/tasks.jsonl` with valid defaults (as tested in `src/tasks/init.test.ts`).
   * Optionally run the BeadsConverter once against `.beads/issues.jsonl` in this repo to prove it works in real conditions (you don’t have to check it in).

3. If everything matches the epic description and tests are green:

   * Close epic `openagents-5bb` via `bd close ...` with a reason like:

     > “.openagents v1 task system implemented and tested; TaskService/ProjectService/ID/TaskPicker/Init/Converter all green.”

   * Log this in a new work log under `docs/logs/YYYYMMDD/HHMM-*.md` (what you checked, commands you ran).

> Do not change `.openagents` schema at this stage; only verify it and close the epic if it truly matches the spec.

---

### 2. Next focus: openagents-42j (Autonomous Coding Agent Infrastructure)

Once `.openagents` epic is validated/closed, shift all attention to **`openagents-42j`** and its children.

#### 2.1. Target order for 42j.* (Golden Loop friendly)

Work these in this order (unless blocked):

1. **Core tools (for better editing & navigation)**

   * `openagents-42j.4` – Port grep tool (code search) – P3
   * `openagents-42j.5` – Port find tool (file discovery) – P3
   * `openagents-42j.6` – Port ls tool (directory listing) – P3

2. **Tool schema + provider baseline**

   * `openagents-42j.15` – Convert tool schemas to Effect Schema – P2
   * `openagents-42j.11` – Unified provider abstraction – P2

3. **Providers (Anthropic / OpenAI / Gemini)**

   * `openagents-42j.8` – Add Anthropic provider – P2
   * `openagents-42j.9` – Add OpenAI provider – P2
   * `openagents-42j.10` – Add Google Gemini provider – P3

4. **Advanced UX & accounting**

   * `openagents-42j.12` – Streaming with partial tool args – P2
   * `openagents-42j.13` – Token and cost accounting – P3

5. **Legacy type-fix beads (clean up or close as NOP)**

   * `openagents-42j.16` – Fix tool test file type errors – P3
   * `openagents-42j.17` – Fix cli.ts type errors – P3
   * `openagents-42j.18` – Fix openrouter-edit-demo.ts type errors – P4

For the type-fix beads, if the current codebase already has those errors resolved (which it might, given `bun test` is fully green), you should:

* Confirm the relevant files are indeed clean.
* Close those beads as “completed previously / no-op” with a note referencing current tests.

#### 2.2. Per-bead work pattern

For **each** `openagents-42j.*` task you pick up:

1. **Claim the bead**

   * `bd update <id> --status in_progress --json`

2. **Read context**

   * Read:

     * Related files under `src/tools/`, `src/agent/`, `src/llm/`, etc.
     * Existing tests in `src/tools/*.test.ts` and `src/agent/*.test.ts` as applicable.
   * For provider tasks, read any existing OpenRouter client code in `src/llm/openrouter.ts` and the infra around it.

3. **Implement**

   * Follow existing patterns:

     * For tools (grep/find/ls):

       * Mirror the design of `read`, `write`, `edit`, `bash` tools:

         * Effect-based implementation,
         * centralized error handling,
         * good test coverage.
     * For schemas (`42j.15`):

       * Replace/bridge existing TypeBox schemas with `effect/Schema` definitions, similar to what you see in `src/tasks/schema.ts`.
     * For providers (`42j.8/9/10`):

       * Implement provider-specific modules behind the unified abstraction (42j.11),
       * Map from our unified schema to each provider’s format.
     * For streaming (`42j.12`):

       * Implement partial tool-argument streaming in a way that:

         * Is type-safe (Effect Schema),
         * Plays nicely with whatever UI we build later.

4. **Test**

   * Always run:

     ```bash
     bun test
     ```

   * Add or extend tests:

     * `src/tools/*.test.ts` for new tools / behaviors.
     * `src/llm/*.test.ts` or equivalent for providers.

   * If the bead is about fixing type/test issues: ensure all previous failing tests are now passing.

5. **Commit & push**

   * Once tests pass:

     * `git add` relevant files and any new tests.

     * Commit message including bead ID, e.g.:

       ```text
       openagents-42j.4: port grep tool to Effect
       ```

     * Push to `main` (or the configured branch in `.openagents/project.json`).

6. **Close the bead**

   * Update the bead:

     ```bash
     bd close <id> --reason "Implemented and tested" --json
     ```

   * If you discovered follow-up work (e.g., missing provider behavior, additional tests), create new beads with `discovered-from:<id>`.

7. **Log**

   * Append a per-run log under `docs/logs/YYYYMMDD/HHMM-*.md` summarizing:

     * Bead ID,
     * Key changes,
     * Tests run and results,
     * Any new beads you filed.

---

### 3. Alignment with Golden Loop v2

For all of this, you must keep the **Golden Loop v2** contract in mind:

* **No commits/pushes when tests fail.**
* Each iteration of work should:

  1. Pick a task (in this case, a bead under `openagents-42j`),
  2. Understand the context,
  3. Implement changes,
  4. Run tests,
  5. Commit & push if green,
  6. Mark the task closed,
  7. Log what happened.

In practice:

* Treat beads under `openagents-42j` as the **task source** while the `.openagents/tasks.jsonl` system is ramping up.
* As we migrate, new tasks will be created under `.openagents/` instead of `.beads/`, but the loop behavior stays the same.

---

### 4. What you do **not** need to ask for

You **do not** need to ask the user:

* Whether to pick `openagents-42j.4` vs `42j.15` — follow the order above.
* Whether to run `bun test` — always run it.
* Whether to add tests for new tools or providers — always do it.
* Whether to close beads whose work is clearly done — close them with a clear reason.

You should only ask when:

* You need secrets or external config (e.g., real API keys for Anthropic/OpenAI/Gemini) that you cannot infer, **or**
* You intend to change a spec doc (`GOLDEN-LOOP-v2.md`, `spec.md`) in a way that contradicts current behavior and requires a human decision.

Otherwise, you iterate until the tests and beads tell you “done.”

---

That’s the plan:

* Close out the `.openagents` epic,
* Then walk through the `openagents-42j.*` tasks in the order above,
* Always staying inside the Golden Loop v2 contract.
