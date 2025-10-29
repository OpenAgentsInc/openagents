# Test Coverage Audit — oa-bridge and translator (2025-10-29 10:07)

This audit summarizes what our current Rust tests verify across the bridge and translator crates, and why those behaviors matter. The suite is a mix of focused unit tests in `crates/oa-bridge` and `crates/acp-event-translator`, plus integration tests that boot the bridge, connect over WebSocket, and drive a simulated Codex CLI to exercise the full streaming path. The high‑level goal of these tests is to prove that incoming Codex JSONL events are parsed, normalized, authorized, and mirrored into our downstream surfaces (Convex and ACP emissions) in a way that matches the runtime behaviors the mobile app depends on.

In the translator crate (`crates/acp-event-translator/src/lib.rs`), tests ensure we correctly map Codex event shapes into Agent Client Protocol updates. The coverage here is deliberate: we verify both streaming deltas and final completions for assistant text and reasoning so that token‑by‑token UI updates and final messages are represented as `AgentMessageChunk` and `AgentThoughtChunk` respectively. We also cover tool‑shaped items: command executions become `ToolCall` events with `Execute` kind and status transitions, file changes map to `Edit` tool calls with file locations extracted so UIs can jump to paths, MCP tool calls are treated as `Fetch` tool calls using a readable title, and web searches are represented as `Search` tool calls. The translator also turns a `todo_list` into an ACP `Plan` with entries and completion status, establishing parity with the app’s plan renderer. Finally, a test asserts we ignore unrelated Codex lifecycle events so we only emit ACP updates for content that should stream into a session — this guards against spurious updates when Codex emits bookkeeping events.

On the WebSocket control surface (`crates/oa-bridge/src/controls.rs`), the parser is exercised with both happy‑path and defensive cases. We parse simple verbs like `interrupt`, `projects`, `skills`, `bridge.status`, and `convex.status` to prove the tolerant JSON parser returns the correct enum variants that downstream handlers switch on. We cover the shape of `project.save` by deserializing frontmatter into a typed project struct and the `project.delete` by extracting the id. The `run.submit` command is validated for required and optional fields — thread id, text payload, optional project id, and resume token — and malformed variants are rejected. There are specific tests to ensure a non‑JSON payload, multi‑line inputs, and missing `control` fields safely return `None`. These input‑validation checks are important since control messages arrive directly from clients over WS and must fail closed on ambiguity or shape mismatches.

For the WebSocket server itself (`crates/oa-bridge/src/ws.rs`), we test authentication and small parsing helpers that influence how the bridge spawns the CLI. The WS upgrade path requires a token; we verify rejection when the token is missing, rejection when it is wrong, acceptance via a query parameter, acceptance via an `Authorization: Bearer` header, and a defensive case where the server state carries no token configured and still rejects even if a client supplies one. This codifies the policy that our bridge never runs in open mode. We also unit test the helpers that extract an optional `cd` working directory and `resume` token from the first JSON line of a multi‑line payload, and the logic that normalizes resume arguments (including the default of “last” when none is provided). The mapping from item `type` strings to our internal message kinds used for Convex rows is covered, ensuring only known tool items are persisted as `cmd`, `file`, `search`, `mcp`, or `todo`. These cases matter because they shape how the bridge decides which Convex mutations to send and how it orders CLI flags for `exec resume`.

The Convex write helper module (`crates/oa-bridge/src/convex_write.rs`) is partially unit‑tested where a deterministic result exists and partially validated end‑to‑end in integration. We test the summarization logic that compacts very large streaming delta lines for logs so that debug output remains readable when the CLI emits big chunks. We also test the conversion of Convex `FunctionResult` values into JSON for inspection, which is used by the debug channel and keeps our logging consistent. The bulk of the function that mirrors ACP updates to Convex is exercised indirectly by the integration tests: it maps text chunks to streamed message rows, tool calls to structured `acp_tool_calls`, and plans to `acp_plan`. These behaviors are validated in the e2e scenarios where the bridge is configured in “noop” mode for Convex (to assert emission intent) or against a real local backend when available.

The file‑backed history utilities (`crates/oa-bridge/src/history.rs`) are covered from two angles: scanning and parsing. The scanner walks a sessions tree and returns the most recent JSONL threads with a cap, and tests prove that we ignore “old‑format” JSONL that lacks a top‑level `type` field, sort by mtime in descending order, and enforce the caller’s limit. The parser reads a single JSONL file and normalizes it into message, reasoning, and command items. We test both newer `response_item` shapes with content arrays and older `item.completed` variants to ensure we capture text regardless of schema evolution. The parser also derives a human‑readable title from the first assistant text (or other fallbacks) and picks up a `resume_id` from `thread.started`. A small cache wrapper is tested to confirm TTL behavior and filtering with `since_mtime` so that history listings can render fast in the UI and still fetch deltas. Together these tests guarantee that the bridge’s “history backfill” and tailers create a coherent, recent view of sessions for clients.

The Projects model (`crates/oa-bridge/src/projects.rs`) and Skills model (`crates/oa-bridge/src/skills.rs`) verify file‑system round‑trips and schema validation. For projects, we create a temporary OpenAgents home, save a project as `PROJECT.md` with validated YAML frontmatter, list it back, confirm the id and description fields are present, and then delete it cleanly. For skills, we write a valid Claude‑style `SKILL.md` frontmatter into a folder and confirm fields like name, description, license, allowed tools, and source tagging parse correctly. We also assert that invalid or schema‑non‑conforming skills are skipped, preventing malformed entries from leaking into clients. These tests defend our file→struct mapping and the JSON Schema enforcement that the bridge applies when listing and saving.

Small utility modules carry targeted tests because their correctness impacts multiple subsystems. In `crates/oa-bridge/src/util.rs` we expand `~/` prefixes against `HOME` so client‑supplied working directories resolve consistently, detect the repo root by finding a path that contains both `expo/` and `crates/`, respect `OPENAGENTS_HOME` over a default folder, read a persisted WebSocket token from `~/.openagents/bridge.json` when present, and generate a random 32‑byte hex token. In `crates/oa-bridge/src/watchers.rs` we validate the environment override for the sessions base directory and the fallback to `~/.codex/sessions` so the tailer watches the right place. These checks reduce configuration foot‑guns and make the bridge more predictable across different environments.

The integration tests (`crates/oa-bridge/tests/integration_full_flow.rs`) tie the pieces together and are the most representative of real operation. The first scenario spawns the bridge configured to emit ACP notifications without writing to a real Convex backend, connects a WebSocket client with the required token, sends a trivial `echo` followed by a `run.submit`, and ensures we observe three things in the broadcast: a “run submitted” debug record, an ACP agent message chunk, and an ACP agent thought chunk. The test also inspects the bridge’s stdout to confirm the code path that spawns the Codex process was hit. The second scenario submits two runs in sequence to the same process and asserts both submissions are reflected in the logs, exercising the resume logic and the “respawn per prompt” pathway. The last scenario, gated by environment variables, runs against a real locally managed Convex backend: it performs a `run.submit`, waits briefly, and then queries Convex to verify that either an ACP plan or one or more ACP tool calls exist for the thread. That end‑to‑end confirmation demonstrates that our JSONL→ACP→Convex mirroring is wired correctly when a backend is truly available.

What is not currently covered is also informative. We do not have unit tests around the Convex bootstrap and process‑lifecycle helpers in `bootstrap.rs` beyond exercising them implicitly during the “real Convex” e2e, and we do not explicitly unit‑test the child process signal handling and group termination in `ws.rs` and `codex_runner.rs`. The ACP mirror’s detailed field mapping is relied upon in integration rather than isolated unit tests, and the local file watchers for projects and skills are verified indirectly through their list/save helpers rather than through simulated filesystem events. Despite these omissions, the tests we do have map to the core user‑visible behaviors: authenticated WebSocket control, strict parsing and validation of commands, faithful translation of Codex events into ACP updates, correct categorization of tool rows, and a working path from Codex output to persisted, queryable data that the app consumes.

In short, the suite proves that the bridge is opinionated about what JSONL it accepts and how it maps that into a stable, typed surface for clients, while guarding the edges where malformed input or configuration could lead to undefined behavior. The translator enforces a contract between Codex and ACP, the server enforces authentication and shape, and the integration paths demonstrate that end‑to‑end, a user pressing “Run” results in coherent stream updates and persisted artifacts ready for display or history. This is why each of these tests exists: to ensure the bridge remains dependable as schemas evolve, CLIs change, and clients assume a consistent contract.


## Addendum: Recommended Tests (Gaps and Rationale)

After another pass through the codebase, below are high‑leverage tests to add. Each item states what to test and why it matters.

- Translator edge cases (acp‑event‑translator)
  - Command failure and in‑progress statuses: map `item.{started,completed}` with `status:"failed"|"in_progress"` for `command_execution` to `ToolCallStatus::{Failed,InProgress}`. Ensures UI status chips and retries render correctly.
  - Empty `aggregated_output`: when present but empty, do not create a text content chunk. Guards against blank tool output rows.
  - File change with multiple paths: include all locations in `ToolCall.locations`. Confirms jump‑to‑file affordances list all edits.
  - Web search without query: title fallback is `"Web search"`. Avoids odd “Web search: ” labels.
  - Todo list with missing/empty items: verify translator returns `None` (no Plan) or an empty plan consistently. Prevents empty plan panes.

- WebSocket forwarder streaming logic (ws::start_stream_forwarders)
  - Assistant finalization without prior deltas: feed only a final `agent_message` and assert the snapshot path runs (debug `bridge.assistant_written`). Proves the fallback snapshot code path works.
  - Assistant finalize after deltas: feed `agent_message.delta` then final `agent_message` and assert we attempt to finalize the streamed item (debug `assistant.final`). Prevents duplicate snapshots.
  - Reasoning summary fallback: feed a `reasoning` item with empty `text` but a `summary` array; assert the combined summary text is used. Matches current JSONL emitter behavior.
  - Large delta summarization: inject a line >24KB and assert the console log uses `summarize_exec_delta_for_log` while still broadcasting the original line. Keeps logs readable without losing data.

- Convex writer behavior without backend (convex_write)
  - stream_upsert_or_append when `convex_ready=false`: assert a `bridge.convex_write` debug event with `ok:false` is broadcast. Documents the degraded‑mode semantics.
  - try_finalize_stream_kind with and without prior stream: verify it returns `false` when no entry exists, and `true` after an upsert (state‑only path, no backend). Ensures stable `itemId` handling (`stream:assistant|reason`).
  - finalize_streaming_for_thread drains all kinds for a given thread. Verifies multi‑kind cleanup.

- Bootstrap/health (bootstrap)
  - convex_health(true/false): spin a tiny HTTP server that 200s `/instance_version` and assert true; assert false for a closed port. Prevents regressions in the health probe.
  - create_threads_table and insert_demo_thread: exercise against a temp sqlite DB, then assert presence via `util::list_sqlite_tables`. Validates demo controls (`convex.create_threads`/`convex.create_demo_thread`).

- Projects model (projects)
  - Legacy single‑file support: write `{id}.project.md`, list, and confirm id derivation and fields. Confirms backward compatibility.
  - Schema reject path: `save_project` should error on invalid frontmatter (e.g., missing `name`/`workingDir`). Prevents invalid saves slipping through.
  - Repo field round‑trip: include provider/remote/url/branch, save+list, and assert all optional fields map correctly.

- Skills model (skills)
  - registry_skills_dirs precedence: env override vs repo `./skills` directory. Ensures deterministic source discovery.
  - Invalid YAML parsing: malformed YAML frontmatter is skipped without panics. Hardens file ingestion.

- History utilities (history)
  - derive_started_ts_from_path: test both filename pattern (`rollout-YYYY-MM-DDTHH-MM-SS…`) and directory fallback (`/.../YYYY/MM/DD/...`). Ensures stable timeline ordering.
  - item.started command normalization: ensure `item.started` for `command_execution` produces a `cmd` entry with `status:"in_progress"` and empty sample. Matches UI “running” indicator.
  - extract_title_and_snippet_ext preference: assert that assistant text beats reasoning and user text for title inference. Keeps titles consistent.

- WS controls parsing (controls)
  - Echo synonyms: `echo`, `debug.echo`, and `debug.ping` produce the same `Echo` variant. Avoids drift in client code using old names.
  - Strict `run.submit` types: verify non‑string `threadDocId`/`text` are rejected. Strengthens validation.

- WS token precedence (ws)
  - extract_token precedence: header should win over query when both are present. Locks the security model. (Place test in‑file to access the private helper.)

- Codex runner args and detection (codex_runner)
  - build_bin_and_args default injection: ensure defaults (`--dangerously-bypass-...`, sandbox, model, config) are added when missing but not duplicated when present. Prevents flag bloat/regressions.
  - Resume flag ordering with positional prompt: verify `spawn_codex_child_with_prompt` orders `exec --json [resume …] <prompt>` correctly and never appends `-`. Prevents ambiguous CLI parsing with older codex builds. (Assertion can be done by exposing an args‑builder behind `#[cfg(test)]` or by instrumenting a tiny fake binary that prints argv.)

- Backfill and demo controls (ws)
  - `convex.backfill`: with `CODEXD_HISTORY_DIR` pointed at a temp tree containing valid JSONL, assert we broadcast a `bridge.convex_backfill` status and attempt thread/message upserts (in NOOP mode). Confirms operator controls are tested.
  - `convex.create_demo`/`convex.create_threads`/`convex.create_demo_thread`: send controls and assert `bridge.convex_status` shows created tables. Validates admin flows without requiring a running backend.

- SQLite helpers (util)
  - list_sqlite_tables: create a temp DB and tables, then assert the ordered names are returned. Ensures diagnostics remain reliable across platforms.

These additions focus on edge‑case handling, degraded/no‑backend behavior, and admin/ops controls. Together they would round out confidence in the pieces that currently rely mostly on integration coverage or are only exercised implicitly during full‑flow tests.
