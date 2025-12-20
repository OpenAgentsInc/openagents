# Autopilot Log Review

Date: 2025-12-19 23:15 CST
Author: Codex

## Scope
- docs/autopilot/README.md
- docs/logs/20251219/*.md (plans and analysis)
- docs/logs/20251219/*.rlog and *.json (spot-checked for errors, workflow, and recovery behavior)

## Key Findings
- Logging metadata reliability issues: multiple rlogs missing session id and token totals, and duplicate logs for the same run (docs/logs/20251219/2113-start-working.rlog vs docs/logs/20251219/2122-start-working.rlog; docs/logs/20251219/2125-start-working.rlog vs docs/logs/20251219/2138-start-working.rlog; docs/logs/20251219/2205-start-working.rlog vs docs/logs/20251219/2231-start-working.rlog).
- Tool usage errors are frequent: directory reads (EISDIR), edits without prior read, and missing file paths (docs/logs/20251219/2055-start-working.rlog, docs/logs/20251219/2144-start-working.rlog, docs/logs/20251219/2205-start-working.rlog).
- Issue tracker integrity problems: duplicate numbers, NULL ids, and claim failures that triggered manual sqlite3 edits (docs/logs/20251219/2113-start-working.rlog, docs/logs/20251219/2122-start-working.rlog).
- Workflow drift from docs: commits and pushes directly on main rather than using the branch and PR workflow (docs/logs/20251219/1916-use-issue-ready-to-get.rlog, docs/logs/20251219/1955-use-issue-ready-to-get.rlog, docs/logs/20251219/2113-start-working.rlog, docs/logs/20251219/2205-start-working.rlog).
- Failure handling gaps: runs stop after test failures or crashes without issue_block or resume linkage (docs/logs/20251219/1919-use-issue-ready-to-get.rlog, docs/logs/20251219/2144-start-working.rlog, docs/logs/20251219/2125-start-working.rlog).

## Recommendations
- Make log writing crash-safe and deterministic: always emit session id and token totals, write a final @end with reason, and avoid duplicate logs per session.
- Add hard guardrails for unsafe operations: block sqlite3 writes, require read-before-edit, and validate file paths before tool calls.
- Strengthen issue DB integrity and repair flow: enforce constraints, auto-resync issue_counter, and provide API-based cleanup for tests.
- Enforce the documented git workflow (branch per issue, no push to main by default).
- Upgrade failure behavior: on build or test failure or budget exhaustion, call issue_block with next steps and link the log path.

## Proposed Issues
| Title | Priority | Evidence / Notes |
| --- | --- | --- |
| Block sqlite3 writes from autopilot tool calls | high | Manual DELETE/UPDATE via sqlite3 appears in docs/logs/20251219/2113-start-working.rlog and docs/logs/20251219/2122-start-working.rlog, violating AGENTS.md. |
| Fix rlog header metadata and duplicate log generation | high | Blank id/tokens in docs/logs/20251219/2113-start-working.rlog, docs/logs/20251219/2125-start-working.rlog, docs/logs/20251219/2144-start-working.rlog, docs/logs/20251219/2205-start-working.rlog; duplicate runs in 2113/2122, 2125/2138, 2205/2231. |
| Crash-safe finalization and issue_block on failure | high | Runs end after failures or crashes without blocking (docs/logs/20251219/1919-use-issue-ready-to-get.rlog, docs/logs/20251219/2144-start-working.rlog, docs/logs/20251219/2125-start-working.rlog). |
| Issue DB integrity constraints + auto-resync issue_counter | high | Duplicate issue numbers and NULL ids prevent claims (docs/logs/20251219/2113-start-working.rlog, docs/logs/20251219/2122-start-working.rlog). |
| Safe issue cleanup API (issue_delete or test DB mode) | medium | Manual sqlite3 deletes used to clean test issues in docs/logs/20251219/2113-start-working.rlog. |
| Read/Edit guardrails and path validation | medium | EISDIR and "File has not been read yet" errors in docs/logs/20251219/2125-start-working.rlog, docs/logs/20251219/2138-start-working.rlog, docs/logs/20251219/2205-start-working.rlog. |
| Search hygiene: ignore target/ and avoid directory reads | medium | Reads into target/ and directory paths in docs/logs/20251219/2055-start-working.rlog and docs/logs/20251219/2205-start-working.rlog. |
| Enforce branch and PR workflow in autopilot | medium | Multiple commits pushed directly to main (docs/logs/20251219/1916-use-issue-ready-to-get.rlog, docs/logs/20251219/1955-use-issue-ready-to-get.rlog, docs/logs/20251219/2113-start-working.rlog). |
| Test failure policy and evidence capture | medium | Repeated "pre-existing" test failure claims without recorded output (docs/logs/20251219/1916-use-issue-ready-to-get.rlog, docs/logs/20251219/2205-start-working.rlog). |
| Resume linkage after crash or budget exhaustion | medium | Crash and budget cutoff without resume hints (docs/logs/20251219/2144-start-working.rlog, docs/logs/20251219/2125-start-working.rlog). |
