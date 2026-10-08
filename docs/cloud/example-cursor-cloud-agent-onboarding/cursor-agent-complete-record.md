# OpenAgents Cloud Agent environment setup: complete record

**Date:** October 8, 2026 (America/Chicago)  
**Cursor agent:** `bc-83e18906-00e2-436c-978b-13a4932f58b0`  
**Run:** `run-7b90ddb0-9d8b-43cb-870a-22fdd0e4fd31`  
**Run result:** `FINISHED` at 10:38:19 AM CT (15:38:19.666 UTC), after 27 minutes 12.874 seconds.

## Executive summary

The agent prepared and tested a draft Cursor Cloud Agent environment for `OpenAgentsInc/openagents`. It installed the native build dependencies, selected the repository-pinned Rust 1.97.1 toolchain, fetched the root Cargo workspace, ran focused tests and formatting checks, created a snapshot, built it, and verified the build in a fresh Cloud Agent.

The Cursor HTML exports add UI-level details missing from the small v0 conversation response: the setup prompt, visible `AGENTS.md` diff, draft branch and pull request, and environment panel state. The 10:47 AM export says the environment is saved. A subsequent read-only `GET /v1/environments?limit=100` confirmed environment `7ce2f49e-c32a-11f1-bb68-864e54d14197` is listed as a saved personal environment, updated at 10:38:37 AM CT.

Two requested outcomes remain incomplete in the recorded run:

- **The environment was saved after the run.** The final report asked for Save; the 10:47 export says it was saved, and the current List Environments API confirms the saved environment exists. The build log still says warming was skipped because the build itself was a draft.
- **No application was started and no application-level end-to-end flow was demonstrated.** The fresh-agent check confirms there was no start script and that no server was started. The evidence proves package installation, Cargo fetches, focused tests, and build reproducibility, not a running product workflow.

The repository guidance was committed and pushed on a feature branch, and the HTML export shows draft PR #10979. The evidence does not show the PR merged.

## What the user asked Cursor to do

The HTML export records this prompt:

> `/create-environment /env-setup Set up a new development environment for this codebase. Follow the “Set up a new environment” workflow in the env-setup skill. Run the relevant applications and demonstrate that the environment works end to end.`

The target repository was `OpenAgentsInc/openagents`.

## Timeline

Times below are Central Daylight Time (UTC−05:00) unless marked UTC.

| Time | Event | Evidence |
| --- | --- | --- |
| 10:11:06 AM | The run started. Cursor reported no linked environment; egress was unrestricted and the repository was `github.com/OpenAgentsInc/openagents`. | Run metadata and `environment-info` tool result |
| 10:11–10:20 AM | The agent inspected `rust-toolchain.toml`, `Dockerfile`, `verification.md`, `README.md`, workspace manifests, setup scripts, and Rust verification guidance. It found Rust 1.97.1 active from the repository toolchain, while OpenSSL development metadata and some native tools were missing. | HTML transcript and run stream |
| 10:20–10:24 AM | The agent wrote an idempotent `cloud-agent-install.sh`. Its first version attempted to fetch both Cargo workspaces. Root fetch succeeded; the separate phone workspace fetch failed because `--locked` would require changing `crates/openagents-mobile/Cargo.lock`. The agent revised the script to fetch only the root workspace and recorded the phone-workspace limitation. | `edit_file` tool outputs, terminal output, final script |
| 10:24–10:29 AM | The agent ran the installer, checked tool versions and system capacity, and launched offline tests for `atif` and `coder-lease` plus a targeted Nostr test. The HTML export at 10:29 AM shows the setup UI, code changes, and a draft PR panel. | HTML export and run stream |
| 10:29–10:33 AM | The agent collected test results, ran `cargo fmt --check`, and recorded test logs as Cursor artifacts. It added Cloud Agent-specific instructions to `AGENTS.md`. | Tool outputs and downloaded artifacts |
| 10:33 AM | Commit `71c16fbba8` was created and pushed to `cursor/cloud-agent-env-notes-58b0`. The branch was used for draft PR #10979, “Describe the Cloud Agent toolchain.” | Git command output and HTML export |
| 10:31–10:35 AM | Cursor built draft environment `bld-20261008-26522d23-d3aa-427c-b6f2-3fc6ffbabdb8` under environment ID `7ce2f49e-c32a-11f1-bb68-864e54d14197`. Install exited 0, the snapshot became ready, and the build succeeded. Warming was skipped because the build was a draft. | Environment build logs and build status tool output |
| 10:36–10:37 AM | A fresh Cloud Agent verified the build: the pinned toolchain and native tools were present, root offline fetch exited 0, the corrupt lease-table test passed, `/tmp/cursor/start-user/` was absent, and no server was started. | Verification subagent transcript |
| 10:38:19 AM | The parent run finished and returned its final report. It said the environment was ready to save and asked the user to click **Save**. | Final `result` and run status |

## Environment changes

### Install script

The final 51-line script is saved as [cloud-agent-install.sh](cloud-agent-install.sh). It:

- Installs build tools and native dependencies: `build-essential`, `pkg-config`, `clang`, `cmake`, `protobuf-compiler`, `libprotobuf-dev`, `libssl-dev`, `libsqlite3-dev`, `libclang-dev`, `ca-certificates`, `curl`, `git`, `ripgrep`, `jq`, `xz-utils`, `unzip`, `zstd`, `procps`, `bubblewrap`, and `python3`.
- Installs Rust through `rustup` only if `rustup` is missing, then installs Rust 1.97.1 with `rustfmt` and `clippy` and selects it as the default.
- Creates `~/work`.
- Runs `cargo fetch --locked --manifest-path Cargo.toml` for the root workspace.
- Does **not** fetch the separate phone workspace because its checked-in lock file does not resolve with `--locked` on this toolchain.

The installer ran successfully in the environment build. The final report says the install was also run twice to verify idempotence; the saved rerun evidence is [install-idempotent.txt](install-idempotent.txt).

### Repository guidance

The agent added 20 lines to `AGENTS.md`. The HTML export shows the new “Cursor Cloud specific instructions” section. It documents:

- Rust 1.97.1, `rustfmt`, `clippy`, `protoc`, OpenSSL, SQLite, libclang, and `bubblewrap` as available after setup.
- One Cargo output directory per checkout: `~/work/openagents-target-agentN`.
- `CARGO_BUILD_JOBS=2` when compiling `openagents` or `verse` on a 4-core, approximately 16 GB machine.
- Building `openagents` before using `openagents lease`.
- The day-to-day `cargo test -p` and `cargo fmt` checks.
- The unresolved phone workspace lock-file issue, PostgreSQL prerequisites for relay release checks, and the fact that this setup does not fetch Psionic.

The change was committed as `71c16fbba8` with subject “Describe the Cloud Agent toolchain and the workspaces it leaves alone.” The branch was pushed. The HTML export shows draft PR #10979; no merge is documented.

The exact section added to `AGENTS.md` was:

```markdown
## Cursor Cloud specific instructions

Cloud Agents already have the pinned toolchain (Rust 1.97.1, rustfmt, and
Clippy), `protoc`, OpenSSL, SQLite, libclang, and `bubblewrap`. The root
workspace crates are fetched. Put each checkout's Cargo output in
`~/work/openagents-target-agentN`, as the Velocity section describes. This
machine has 4 cores and about 16 GB of memory. Set `CARGO_BUILD_JOBS=2`
when you compile `openagents` or `verse`, so a cold build stays within
that memory.

Build `openagents` with `cargo build -p openagents-cli --bin openagents`
before you use `openagents lease`. The day-to-day check is still
`cargo test -p` for the crates you edit, plus `cargo fmt`.

`cargo fetch --locked --manifest-path crates/openagents-mobile/Cargo.toml`
stops because Cargo wants to update that lock file. Leave the phone
workspace until the lock file resolves. Relay release checks need
PostgreSQL (`initdb`, `pg_ctl`, `createdb`). Psionic stays its own
workspace, and this setup does not fetch it.
```

## Verification results

### Setup build

The saved [environment build log](environment-build-install-logs.txt) reports:

- Install exit code: `0`.
- Snapshot created and ready.
- Build `bld-20261008-26522d23-d3aa-427c-b6f2-3fc6ffbabdb8` status: `SUCCEEDED`.
- `userFacingSnapshotId` equals the build ID.
- Warming skipped because the build was a draft.

### Tool versions in the fresh agent

| Command | Result |
| --- | --- |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `cargo fmt --version` | `rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14)` |
| `cargo clippy --version` | `clippy 0.1.97 (8bab26f4f6 2026-07-14)` |
| `python3 --version` | `Python 3.12.3` |
| `protoc --version` | `libprotoc 3.21.12` |
| `pkg-config --modversion openssl` | `3.0.13` |
| `pkg-config --modversion sqlite3` | `3.45.1` |
| `bwrap --version` | `bubblewrap 0.9.0` |
| `git --version` | `git version 2.43.0` |

### Tests and formatting

The downloaded logs are the primary evidence: [ATIF and lease tests](cargo-atif-coder-lease.log), [Nostr signature test](cargo-nostr-signature.log), and the install version/idempotence check [install-idempotent.txt](install-idempotent.txt).

- `cargo test --offline -p atif -p coder-lease`: exit `0`.
  - `atif`: 56 unit tests, 3 integration tests, and 1 doctest passed.
  - `coder-lease`: 66 unit tests, 2 dead-holder integration tests, and 5 shim integration tests passed.
- `cargo test --offline -p nostr --lib free_agreement_requires_both_signatures_exact_terms_and_confirmation`: exit `0`; the selected test passed, with 449 filtered out.
- `cargo fmt -p atif -p coder-lease -p nostr -- --check`: clean (`FMT:0`).
- In the fresh agent, `cargo fetch --locked --offline` from `/workspace`: exit `0`.
- In the fresh agent, `cargo test --offline -p coder-lease --lib a_corrupt_table_is_an_error_not_a_reset`: exit `0`; 1 passed, 65 filtered out.

The HTML shows an intermediate “cargo test progress” tool action with exit code `1`. The final archived test log records the actual `atif` and `coder-lease` test command as `EXIT:0`, and the full suites pass. Treat the progress-check exit as an intermediate observation, not the final test result.

## Known limitations and pending actions

1. **Build warming was skipped.** The environment itself is saved (confirmed by the 10:47 export and List Environments API), but the build log records that warming was skipped because the build was draft at build time.
2. **Run an application-level end-to-end check.** The original prompt requested relevant applications and an end-to-end demonstration. The recorded verification did not launch an application or server; it says the start directory was absent and no server was started. The evidence supports environment and crate-level validation, not a running application workflow.
3. **Resolve the phone workspace lock file separately.** `cargo fetch --locked --manifest-path crates/openagents-mobile/Cargo.toml` exited `101` because Cargo wanted to update the lock file. The install script now leaves that workspace untouched.
4. **Provide PostgreSQL for relay release checks.** The new `AGENTS.md` note says those checks require `initdb`, `pg_ctl`, and `createdb`.
5. **Fetch Psionic separately if needed.** The setup does not fetch this separate workspace.
6. **Review and merge the documentation PR.** Commit `71c16fbba8` is on the pushed feature branch. The HTML export marks PR #10979 as draft; the run does not show it merged.

The fresh-agent verification also says it could not find the build ID in environment variables or under `/tmp`; Cursor environment metadata supplied the requested build identity. The environment metadata did not expose `environment.json` because the environment was personal/override and owner-restricted.

## What the HTML export adds

The 10:29 export at [Set up openagents environment ｜ Cursor (10_8_2026 10：29：09 AM).html](cursor-agent-html-export.html) adds these details to the earlier API message-only export:

- The exact `/create-environment /env-setup` request.
- The visible progress sequence, file/search summaries, and setup UI.
- The 20-line `AGENTS.md` addition and the draft branch/PR panel.
- The panel had a **Save** action. The later 10:47 export says it was saved; the saved-environment API confirms that state.

It does not include complete tool outputs. The v0 conversation endpoint returned only three parent messages. The first v1 SSE capture is an event record, but its provider-level completion entries alone omit or truncate some result payloads. Re-fetching the retained stream exposed lower-level interaction completion results; the SDK conversation recovered the remaining truncated edit and task results. One `pr_management` call remains without any response (audit below). Cursor documents the v1 route as `GET /v1/agents/{id}/runs/{runId}/stream` ([Cloud Agents API](https://cursor.com/docs/cloud-agent/api/endpoints)). Cursor staff have also said transcript exports omit tool outputs and suggested logging `postToolUse` results for future runs ([transcript discussion](https://forum.cursor.com/t/accessing-the-full-agent-transcript-in-cursor/157311/5)).

## Completeness audit: tool calls and responses

The recheck recovered the two outputs previously thought missing. The retained v1 stream replay includes an `interaction_update` completion payload for the truncated `grep_search`; the SDK conversation contains results for the truncated `edit_file` and delegated `task` records.

- The re-fetched stream has 121 unique provider call IDs: 120 reached `completed`, and one `pr_management` call remained `running`. The completed calls include `get_mcp_tools` and `await` events as well as user-facing tools.
- There are 117 completed provider event records with an inline `result`; three provider records were marked truncated (`grep_search`, `edit_file`, and `task`).
- The retained stream contains 99 lower-level `tool-call-completed` interaction updates with result payloads. This lower-level result stream restores the full `grep_search` response even though its corresponding provider record says `truncated`.
- The SDK conversation contains 82 parent and 7 nested child tool-call steps. It restores the structured results for the `edit_file` and delegated `task` calls.
- **One tool call has no response to recover:** `pr_management` (call ID begins `call-62505700-...-92`) appears only as `running` in the retained stream. It has no completion in the retained replay, no result in the SDK or legacy v0 conversation, and no matching artifact. A separate GitHub PR lookup confirms that draft PR #10979 exists, but it cannot reproduce the missing Cursor tool response.

This means all 120 provider calls marked `completed` have a recoverable result across the stream's interaction updates and the SDK transcript. The full chain still does **not** have a response for the one `pr_management` call that never completed. The recheck sources and counts are recorded in [the recovery audit](cursor-agent-recovery-audit.json); the newly extracted 99 interaction results are in [the interaction result JSONL](cursor-agent-interaction-tool-results.jsonl).

The captured parent run ended `FINISHED`, with a terminal result and `done` event. Run completion does not turn a still-running tool call into a successful call or a response.

## APIs used and repeatable collection procedure

All Cursor API requests used HTTP Basic authentication with the API key as username and an empty password (`curl -u "$CURSOR_API_KEY:"`). The key must stay in an environment variable and out of files and logs.

For a future run, collect these sources while the run is live, then once more after it finishes:

1. `GET https://api.cursor.com/v1/agents/{agentId}` to obtain durable metadata and `latestRunId`.
2. `GET https://api.cursor.com/v1/agents/{agentId}/runs?limit=100` and follow `nextCursor` until absent; save the run list.
3. `GET https://api.cursor.com/v1/agents/{agentId}/runs/{runId}` to save terminal status/result.
4. Immediately open `GET https://api.cursor.com/v1/agents/{agentId}/runs/{runId}/stream` with `Accept: text/event-stream`. Save every byte, event ID, and timestamp. On disconnect, reconnect with `Last-Event-ID` set to the last fully persisted SSE event ID. Continue through terminal result and `done`. Do not rely on a later reconnect as a durable full-history export: the endpoint has a retention window and can return `410 stream_expired`.
5. Use the official `@cursor/sdk` as a second structured source after completion:

   ```js
   import { Agent } from "@cursor/sdk";
   const run = await Agent.getRun(runId, {
     runtime: "cloud",
     agentId,
     apiKey: process.env.CURSOR_API_KEY,
   });
   const turns = await run.conversation();
   ```

   Save the complete returned JSON. Recursively walk nested `task` results for delegated agent conversation steps; do not assume child calls are separate top-level runs.
6. `GET /v1/agents/{agentId}/artifacts`, then `GET /v1/agents/{agentId}/artifacts/download?path=...` for each artifact; follow the returned temporary signed URL immediately and retain the downloaded files and manifest.
7. Optionally call legacy `GET /v0/agents/{agentId}/conversation` and save it as a UI/text supplement. It is not a source for full tool I/O.
8. Save a browser HTML export as a visual cross-check. It can show visible changes and final status but does not include all raw tool outputs.

For each source, normalize tool start/completion events by call ID and compare the sets. A completion status without a `result` is not a complete response. Keep separate counts for provider event IDs, actual tool-call IDs, and nested SDK tool calls because their schemas/counting differ.

The official docs describe v1 Basic/Bearer authentication, the run stream, its retention behavior, and environment visibility: [Cloud Agents API](https://cursor.com/docs/cloud-agent/api/endpoints). The environment endpoint excludes drafts; this run's environment was verified saved by listing it after the export.

## Evidence index

All files are in the same scratch folder as this record.

| Evidence | Contents |
| --- | --- |
| [Full run stream](cursor-agent-run-stream.sse) | Initially captured v1 SSE stream through `FINISHED`; provider-level entries mark some outputs truncated. |
| [Run stream API recheck](cursor-agent-run-stream-recheck.sse) | Later no-cursor retained replay from the API; confirms interaction-level completions and the still-running PR tool call. |
| [Interaction tool results](cursor-agent-interaction-tool-results.jsonl) | 99 `tool-call-completed` interaction updates with result payloads, including the full grep result. |
| [Recovery audit](cursor-agent-recovery-audit.json) | API recheck status, source coverage, counts, recovered truncations, and the one unresolved call. |
| [Completed tool outputs](cursor-agent-tool-outputs.jsonl) | 117 completed tool-call entries with result data. |
| [All tool-call events](cursor-agent-tool-calls.jsonl) | 330 normalized provider tool event rows, including partial and running updates. |
| [Cursor HTML export (10:29)](cursor-agent-html-export.html) | Original UI transcript and environment/PR snapshot from 10:29 AM CT. |
| [Cursor HTML export (10:47)](cursor-agent-html-export-1047.html) | Final UI export; says the environment is saved, though its panel still reads Draft. |
| [Extracted final HTML text](cursor-export-1047.txt) | Searchable text extraction of the 10:47 export. |
| [SDK conversation](cursor-agent-sdk-conversation.json) | Structured parent transcript, including nested verification subagent conversation/results. |
| [SDK tool-call rows](cursor-agent-sdk-tool-calls.jsonl) | Normalized 82 parent and 7 nested child tool calls from the SDK response. |
| [Saved environments API response](cursor-environments-current.json) | Read-only `GET /v1/environments?limit=100`; confirms the environment is saved. |
| [Run status recheck](cursor-agent-run-status-recheck.json) | Current read of the final run state and result. |
| [Run list recheck](cursor-agent-runs-recheck.json) | Confirms the target run and draft PR URL in `git.branches`. |
| [Artifact list recheck](cursor-agent-artifacts-recheck.json) | Confirms Cursor still exposes only three artifacts. |
| [SDK conversation recheck](cursor-agent-sdk-conversation-recheck.json) | Second completed-run transcript read; contains 89 parent and nested child tool calls, no `pr_management` result. |
| [Legacy conversation recheck](cursor-agent-conversation-recheck.json) | Confirms v0 still returns text messages without tool-call results. |
| [GitHub PR cross-check](cursor-github-pr-crosscheck.json) | Independent PR state lookup; not the original Cursor tool response. |
| [Artifact manifest](cursor-agent-artifacts.json) | The three files Cursor exposed through the agent artifact API. |
| [Fresh-agent metadata](cursor-agent-verification-metadata.json) | Metadata for the build-verification agent. |
| [Fresh-agent run listing](cursor-agent-verification-runs.json) | Confirms that verification task has no v1 API run entries. |
| [Parent agent metadata snapshot](cursor-agent-metadata.json) | Captured before the run finished; it still reports `ACTIVE`, so it is superseded by final run status. |
| [Extracted HTML text](cursor-export-1029.txt) | Searchable text extraction of the HTML export. |
| [Final install script](cloud-agent-install.sh) | Full final script after removing the failing phone-workspace fetch. |
| [Environment build logs](environment-build-install-logs.txt) | Cursor build, install, clone, snapshot, and build-status logs. |
| [ATIF and lease test log](cargo-atif-coder-lease.log) | Full test output for `atif` and `coder-lease`. |
| [Nostr test log](cargo-nostr-signature.log) | Full output for the targeted Nostr signature test. |
| [Install check](install-idempotent.txt) | Installer rerun exit and verified tool versions. |
| [Fresh-agent verification transcript](cursor-agent-verification-conversation.json) | The separate verification task's request and final report. |
| [Final run status](cursor-agent-run-status.json) | Authoritative final status and timestamps (`FINISHED`). |
| [Parent v0 message response](cursor-agent-conversation.json) | Three user/assistant text messages; this response omits tool outputs. |

The stream capture recorded one transient `stream_error` while resuming. The capture resumed from the last event ID, continued receiving events, and ended with the run's `FINISHED` result and `done` event. The earlier parent agent metadata file was fetched before completion and still says `ACTIVE`; use the final run status file as the authoritative status. The 10:47 HTML has an internal UI mismatch (final message says saved; side panel still says Draft), resolved for saved-state existence by the subsequent API list response. This does not change the tool-response completeness gaps above.
