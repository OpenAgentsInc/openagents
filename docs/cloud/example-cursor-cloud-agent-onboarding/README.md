# Cursor Cloud Agent onboarding example

This directory preserves the evidence from the October 8, 2026 Cursor Cloud
Agent environment setup for `OpenAgentsInc/openagents`. Use it as a worked
example of environment setup, validation, and transcript collection.

## What happened

The agent installed native build dependencies, selected the repository-pinned
Rust 1.97.1 toolchain, fetched the root Cargo workspace, ran focused tests and
formatting checks, built an environment snapshot, and checked that build in a
fresh Cloud Agent. It also committed Cloud Agent guidance to a feature branch
and opened draft PR #10979. It did not start an application or demonstrate an
application-level end-to-end flow.

The environment was saved after the setup run. The final HTML export says it
was saved, and `cursor-environments-current.json` records the matching saved
personal environment. The build log says warming was skipped because the
environment build was a draft when it ran.

See [the complete analysis](cursor-agent-complete-record.md) for the timeline,
commands, results, limitations, and evidence index.

## Transcript completeness

The saved files provide a detailed record, but they are not a complete
byte-for-byte tool call and response transcript. The captured v1 SSE stream
ends with the run's `FINISHED` result and `done` event, but some tool responses
are truncated or absent. The SDK conversation recovers the `AGENTS.md` edit
result, the delegated verification result, and seven nested child tool calls.
Two responses remain unavailable:

1. A completed `grep_search` of the install log has a truncated result.
2. The `pr_management` call has a start event but no response event. The HTML
   shows draft PR #10979 but does not contain the exact API response.

Do not treat the final assistant summary or visible HTML as a substitute for
these missing tool outputs. The record distinguishes provider event counts,
actual tool-call records, and SDK conversation steps because those sources use
different schemas.

## Evidence files

- `cursor-agent-run-stream.sse`: captured v1 SSE stream, including progress,
  tool events, final run result, and terminal event.
- `cursor-agent-tool-calls.jsonl`: normalized provider tool-call event rows,
  including starts, partial updates, and completions.
- `cursor-agent-tool-outputs.jsonl`: completed tool event rows with result
  payloads when the stream retained them.
- `cursor-agent-sdk-conversation.json`: structured parent transcript,
  including the delegated verification conversation and nested results.
- `cursor-agent-sdk-tool-calls.jsonl`: normalized SDK tool calls, including
  the seven nested child calls.
- `cursor-agent-stream-segment.sse`: last segment fetched by the capture
  script; the full assembled stream is the primary stream artifact.
- `cursor-agent-html-export.html` and
  `cursor-agent-html-export-1047.html`: browser exports from 10:29 AM and
  10:47 AM Central time.
- `cursor-export-1029.txt` and `cursor-export-1047.txt`: searchable text
  extractions of those HTML exports.
- `cursor-agent-conversation.json`: legacy v0 text-only conversation result.
- `cursor-agent-run-status.json` and `cursor-agent-metadata.json`: run status
  and earlier agent metadata. The earlier metadata predates completion.
- `cursor-agent-verification-conversation.json`,
  `cursor-agent-verification-metadata.json`, and
  `cursor-agent-verification-runs.json`: delegated verification task records.
- `cursor-agent-artifacts.json`, `environment-build-install-logs.txt`,
  `cursor-environments-current.json`, `cargo-atif-coder-lease.log`,
  `cargo-nostr-signature.log`, and `install-idempotent.txt`: artifact
  manifest, build evidence, saved environment confirmation, tests, and
  installation checks.
- `cloud-agent-install.sh`: final environment install script.
- `capture-cursor-run.sh`: the original run-specific capture script. It
  contains that run's IDs and scratch path; adapt it before reuse.

These files include the agent's prompts, tool arguments, repository paths, and
tool outputs. Review them for sensitive information before copying this folder
outside the repository or publishing it elsewhere. The API key used for
collection is not included.

## APIs used

The collector used Cursor Cloud Agents v1 with HTTP Basic authentication. Set
`CURSOR_API_KEY` in the shell environment and use the key as the username with
an empty password, for example `curl -u "$CURSOR_API_KEY:"`. Do not put the
key in a source file, command transcript, or committed artifact.

The collection used these API routes:

| Method and route | Purpose |
| --- | --- |
| `GET /v1/agents/{agentId}` | Read agent metadata and `latestRunId`. |
| `GET /v1/agents/{agentId}/runs?limit=100` | List runs; follow `nextCursor` until it is absent. |
| `GET /v1/agents/{agentId}/runs/{runId}` | Read the run's status and final result. |
| `GET /v1/agents/{agentId}/runs/{runId}/stream` | Capture live SSE events. Resume with `Last-Event-ID` after persisting the last complete event. |
| `GET /v1/agents/{agentId}/artifacts` | List downloadable agent artifacts. |
| `GET /v1/agents/{agentId}/artifacts/download?path=...` | Obtain a temporary signed download URL for an artifact. |
| `GET /v0/agents/{agentId}/conversation` | Retrieve the legacy text-only conversation supplement. |
| `GET /v1/environments?limit=100` | Confirm that a saved environment is visible to the API key. |

The official [Cloud Agents API reference](https://cursor.com/docs/cloud-agent/api/endpoints)
documents authentication, routes, pagination, and stream retention. The
stream is run-scoped and expires after its retention window; collect it while
the run is active rather than relying on it as a permanent archive.

The collector also used the official `@cursor/sdk` to retrieve a structured
conversation after the run finished:

```js
import { Agent } from "@cursor/sdk";

const run = await Agent.getRun(runId, {
  runtime: "cloud",
  agentId,
  apiKey: process.env.CURSOR_API_KEY,
});

const conversation = await run.conversation();
```

Save the full JSON response. Recursively inspect task results for nested
delegated-agent conversation steps; child calls may be embedded in the parent
task result rather than listed as separate top-level runs. The SDK response
complements the SSE stream but does not guarantee recovery of every missing
provider response.

## Repeatable collection sequence

1. Set `CURSOR_API_KEY` without printing it. Record the agent ID and run ID.
2. Fetch the agent record and paginate the run list. Save the raw responses.
3. Start the SSE capture as soon as the run begins. Persist each event before
   advancing the saved `Last-Event-ID`.
4. If the connection drops, resume with the last persisted event ID. Append
   only new events, and deduplicate by SSE event ID when the server replays an
   event.
5. Poll the run endpoint until it reaches a terminal state. Keep capturing
   until the stream emits the final result and terminal event when available.
6. Retrieve the structured SDK conversation after completion. Walk nested
   task results and retain child tool results.
7. Download every artifact from the artifact manifest, following each
   temporary signed URL immediately.
8. Save the HTML export as a visual cross-check, then compare tool call IDs
   across the SSE and SDK records. Mark a response missing if its call has no
   result payload; do not infer a result from a later summary.
9. For environment work, use `GET /v1/environments` to distinguish a saved
   environment from a draft. The list omits drafts and deleted environments.

The raw evidence is retained here so future reviews can reproduce the audit
without depending on the Cursor UI export alone.
