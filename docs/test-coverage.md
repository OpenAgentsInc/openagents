# Test Coverage Audit (Rust + Tauri)

Scope: Rust crates under `crates/` and Tauri backend under `tauri/src-tauri`. Expo app intentionally excluded per request.

Referenced refactor context: Issue #1320 “Real-time Codex chat streaming in Desktop + Mobile (Convex-backed) + folder refactor” — split work has begun and partial streaming is live in both Tauri and the bridge.

## Current Test Inventory

- crates/codex-bridge
  - projects.rs
    - Round‑trip test for projects directory using a temp `OPENAGENTS_HOME`: creates, lists, and deletes a project; validates schema gate on save via frontmatter extraction. Covers legacy `.project.md` resolution as well as folder layout via `PROJECT.md`.
  - history.rs
    - scan_history_limits_and_order: creates multiple new‑format JSONL files and an old‑format file, verifies only new‑format are returned, limited and sorted by mtime desc.
    - parse_thread_extracts_items: parses JSONL with command started/completed and message items; asserts title inference and presence of cmd + assistant items.
    - history_cache_serves_from_cache_and_since_mtime: exercises `HistoryCache::get` stability with `since_mtime` returning empty deltas at max.
    - parse_thread_maps_response_item_message_and_reasoning: verifies mapping of `response_item` with `message` and `reasoning.summary` variants.
    - parse_thread_maps_item_completed_variants: verifies mapping of `item.completed` variants for `agent_message` and `reasoning`.
  - skills.rs
    - New: unit tests for valid SKILL.md parsing (license, allowed-tools, metadata, source tagging) and invalid schema rejection.
  - events.rs
    - New: serde round‑trip tests for key variants (thread.started, turn.completed with usage, error) and ThreadItem variants (command_execution, file_change). Confirms tag strings and snake_case enums/fields.
  - controls.rs
    - New: parser tests for `run.submit`, project save/delete, status verbs, and rejection of multi‑line/non‑JSON payloads.
  - ws.rs
    - New: payload parsing tests for `cd` directory extraction and `resume` token.
    - New: tool-row mapping test for `command_execution`/`file_change`/`web_search`/`mcp_tool_call`/`todo_list`.
  - util.rs
    - New: unit tests for `expand_home` and `detect_repo_root` heuristics.
  - watchers.rs
    - New: unit tests for `sessions_base_dir` env override and HOME fallback.
  - Notes: `main.rs` has been slimmed; most logic now lives in modules. Some legacy stubs remain (dead_code warnings) — slated for removal per #1320.

- crates/oa-validate
  - CLI tool only. No tests present. Includes logic for compiling JSON Schemas, scanning directories, and validating YAML frontmatter for projects/skills.

- crates/copy-sources
  - CLI utility for filtered repo export/clipboard copy. No tests present. Includes filesystem walking, binary/file skipping, and content encoding.

- tauri/src-tauri
  - convex.rs: contains pure mapping helpers with unit tests (e.g., `select_thread_key`, `map_thread_item`, `should_hide_message`). This module is wired and tests pass (3 tests).
  - lib.rs: imports convex module and focuses on wiring; mapping/filter rules have been extracted to convex.rs and are tested there.

## What’s Exercised vs. Gaps

- Stronger coverage
  - JSONL parsing/mapping for History: Multiple paths verified (response_item, item.completed, command lifecycle). Title/snippet inference and new/old format detection also covered.
  - Project metadata round‑trip: Ensures the new folder convention and legacy single‑file convention both work, including schema enforcement on save and ID derivation.

- Missing or weak coverage
  - codex-bridge
    - skills.rs: expand tests for optional fields, env override, and explicit user-over-registry precedence.
    - events.rs: add remaining variants if desired (e.g., TodoList, McpToolCall, WebSearch), though core coverage is in place.
    - main.rs: spawning logic, WS routing, Convex write paths, file watchers (sessions/projects/skills) are untested. These are the hardest to test pre‑refactor.
  - oa-validate: no tests for schema compilation, directory scans (special names vs. legacy suffixes), or helpful error aggregation.
  - copy-sources: no tests for skip rules (binary ext, Cargo.lock), include rules (docs, crates, specific expo subtrees), or platform path normalization.
  - tauri: convex helpers now tested; subscription loops and command wrappers themselves are not yet unit tested.

## Recommendations (Short‑Term)

- Tauri convex helpers
  - Already wired and tested. Consider adding tests for `map_message_row` with edge JSONs and small fake subscription adapter tests (see refactor plan below).

- Expand unit tests for `crates/codex-bridge/skills.rs`
  - Additional cases: empty/missing optional fields tolerated; `registry_skills_dirs` env override; user vs. registry precedence (dedup by id, prefer user) — may expose current preference behavior.

- Add serde contract tests for `crates/codex-bridge/events.rs`
  - Round‑trip JSON for each enum variant to ensure tags/rename rules stay stable. This guards the app’s generated TS bindings and JSONL renderers.

- Add focused tests to `crates/oa-validate`
  - `detect_file_kind` (implicit in code paths): project vs. skill inference from frontmatter.
  - `collect_markdown_files` and `collect_special_files`: directory shapes, legacy suffixes, special names.
  - Schema compile once; validate success and readable error aggregation on failure.

- Add focused tests to `crates/copy-sources`
  - `is_binary_ext` matrix; `should_skip_file` correctness.
  - `walk_filtered` excludes directories (node_modules, target, etc.) and includes expected config files. Use temp dirs with small fixtures.

## Status + Next (from #1320)

Done (this branch):
- Tauri split: `convex.rs`, `subscriptions.rs`, `commands.rs`, `bridge.rs` with unit tests for mapping/hide rules.
- Bridge split (phase 1): `bootstrap.rs`, `codex_runner.rs`, `state.rs`, `controls.rs` created; main wired to use them.
- Streaming: bridge writes `messages:upsertStreamed`/`finalizeStreamed`; sessions tailer follows `~/.codex/sessions` and mirrors to Convex.
- Docs: desktop build guide, coverage plan, and version bump steps updated; desktop version bumped to 0.2.0.

Planned (next commits):
- Bridge split (phase 2): `ws.rs` (handlers), `convex_write.rs` (JSONL→Convex mappers), `watchers.rs` (projects/skills/sessions), and removal of legacy duplicates in main.
- Tests: unit tests for control parsing, JSONL mapping branches (assistant, reason, tool rows), sessions tailer integration (feature-gated where needed).

## Recommendations (Refactor‑Aided)

Refactor pieces to unlock proper unit tests and narrow integration tests:

- codex-bridge module split
  - `ws.rs`: WS route wiring with pure message framing helpers. Test message framing and control routing without sockets by feeding json payloads through handlers.
  - `codex_runner.rs`: command builder that injects flags (`--dangerously-bypass-approvals-and-sandbox`, sandbox config, model overrides). Unit test builder yields expected argv/env given input options.
  - `convex_write.rs`: pure mappers for JSONL item snapshots → Convex upsert/finalize args. Unit test with golden JSONL snippets for assistant/reason, command, file changes.
  - `fs_watch.rs`: pure debounce/edge rules for watchers with a trait‑based clock/scheduler; unit test timing/coalescing without touching the filesystem.
  - `sessions_watch.rs`: tailer state machine (offset tracking, last itemId), fed by an in‑memory line source; unit test incremental upsert/finalize behavior. (implemented: initial tailer; tests pending)
  - `history_scan.rs`: existing `history.rs` parsing utilities can move here; maintain and extend current tests.

- tauri split
  - `convex.rs`: keep mapping/filter helpers and command wrappers thin; unit test helpers directly (counts, docId preference, filtering rules).
  - `subscriptions.rs`: lift subscription loops and event emission; add a small adapter boundary so the core loop is testable with a fake stream of Convex `Value::Array` items.
  - `commands.rs`: keep parameter plumbing minimal; logic lives in helpers.

## Suggested New Test Cases (Examples)

- codex-bridge/history + convex_write
  - Old vs. new JSONL formats: ensure `file_is_new_format` rejects early for old sessions with legacy shapes or non‑JSON lines. (unit)
  - Long outputs: summarize/compact large `exec_command_output_delta` for logs; no panic on >24KB bodies. (unit)
  - Title inference: bold‑delimited title vs. fallback to first 6 words. (unit)
  - Streaming: `upsertStreamed`/`finalizeStreamed` argument building for assistant vs. reason (with itemId, partial, seq). (unit)

- codex-bridge/skills (added)
  - Allowed tools normalization: handle non‑string entries ignored; empty list vs. missing; preserves order.
  - Metadata pass‑through: nested structures retained via serde conversion.
  - Registry + user merge: two skills with same folder name id, user wins; stable sort by lowercase name.

- oa-validate
  - End‑to‑end run with temp tree: mix of `PROJECT.md`, `SKILL.md`, and legacy `*.project.md`/`*.skill.md` files; assert OK/FAIL counts and error messages include instance paths.

- copy-sources
  - Cross‑platform path normalization to forward slashes; ensure `Cargo.lock` is excluded; ensure top‑level `AGENTS.md`/`README.md` are included.

- tauri
  - `select_thread_key`: docId overrides threadId; None returns threadId.
  - `map_thread_item`: tolerates int/float counts and missing optional fields.
  - Message hide predicate: hides preface/system meta and environment blocks; does not hide normal assistant/user messages.

## How to Run

- Workspace crates: `cargo test --workspace`
- Tauri backend: `cd tauri/src-tauri && cargo test`

CI Note: Once tests are in place for bridge modules, enable coverage in CI via `cargo tarpaulin` and fail PRs under a minimum threshold for changed files.

## Coverage Measurement

- Add Rust coverage via `cargo tarpaulin` in CI for `crates/*` and `tauri/src-tauri`.
- Start with line coverage for parsing and helper modules. Exclude binaries/clipboard/network code paths or gate behind feature flags.

## Risk Areas Not Yet Tested

- WS server lifecycle and multi‑client broadcasting in codex-bridge.
- Child process spawn/respawn semantics for Codex runs and stdin EOF handling.
- Sessions tailer debouncing/backpressure and Convex function timeouts.
- Tauri bootstrap flows (auto‑deploy Convex, ensure bridge running) and port detection.

## Proposed Sequence (Pragmatic)

1) Wire Tauri convex tests and add hide‑predicate helper + tests.
2) Add `skills.rs` unit tests; add `events.rs` serde round‑trips.
3) Add tests to `oa-validate` and `copy-sources` for core helpers.
4) Begin codex-bridge refactor toward `convex_write.rs` and test mapping from JSONL → Convex args.
5) Incrementally extract watchers/tailer with isolated state machines and add tests.

This plan aligns with #1320’s refactor track and quickly raises confidence in core parsing/mapping while deferring heavier integration (WS/child processes) until seams exist.
