import { describe, expect, test } from "bun:test"

import {
  parseCodexSessionRollout,
  parseCodingProcesses,
  parseSupervisorLog,
  summarizeCodingProcesses,
} from "../src/shared/coding-status.js"

describe("openagents desktop coding status", () => {
  test("parses local coding agent processes", () => {
    const processes = parseCodingProcesses(`
  100     1  8.2      01:20 /Users/me/.codex/accounts/codex-4/codex/vendor/aarch64-apple-darwin/bin/codex exec --json
  101     1  0.0      02:00 bun apps/pylon/src/index.ts khala request --prompt Implement public issue #6932 --account-ref codex-6
  102   101  0.1      00:30 /Users/me/node_modules/@openai/codex/vendor/aarch64-apple-darwin/bin/codex exec --cd /tmp/workspace-one --assignment-ref assignment.public.khala_coding.test_6932
  103     1  0.2      10:00 bash /Users/me/work/apps/pylon/scripts/codex-supervisor/codex-supervisor.sh
  104     1  5.0      00:30 bun scripts/khala-vertex-continual-learning-burn.mjs
`)

    expect(processes.map(process => process.kind)).toEqual([
      "codex_exec",
      "khala_request",
      "codex_exec",
      "supervisor",
      "vertex_burn",
    ])
    expect(processes[0]).toMatchObject({
      accountRef: "codex-4",
      issueRef: null,
      pid: 100,
      status: "active",
    })
    expect(processes[1]).toMatchObject({
      accountRef: "codex-6",
      issueRef: "6932",
    })
    expect(processes[2]).toMatchObject({
      accountRef: "codex-6",
      assignmentRef: "assignment.public.khala_coding.test_6932",
      issueRef: "6932",
      parentPid: 101,
      workspacePath: "/tmp/workspace-one",
    })
    expect(summarizeCodingProcesses(processes)).toMatchObject({
      burningCodexCount: 1,
      codexExecCount: 2,
      khalaRequestCount: 1,
      supervisorCount: 1,
      vertexBurnCount: 1,
    })
  })

  test("parses supervisor heartbeat and dispatch events", () => {
    const summary = parseSupervisorLog(`
2026-06-29T02:27:07Z heartbeat ready_codex=5 desired_slots=12 tuned_account_slots=codex-3 codex-3 last_dispatch_time=1782700025925
2026-06-29T02:27:09Z slot=0 acc=codex-3 issue=#6973 NO-DISPATCH (refused rc=1 repeated=1); backoff 30s
2026-06-29T02:27:10Z slot=1 acc=codex-4 issue=#6932 OK (rc=0)
2026-06-29T02:27:11Z slot=2 acc=codex-5 issue=#6840 LOCKOUT (claimed)
Pylon presence failed: OpenAgents presence request failed (401): {"error":"unauthorized"}
`)

    expect(summary).toMatchObject({
      desiredSlots: 12,
      lockoutRecent: 1,
      noDispatchRecent: 1,
      okRecent: 1,
      readyCodex: 5,
    })
    expect(summary.lastDispatchAt).toBe("2026-06-29T02:27:05.925Z")
    expect(summary.events.map(event => event.status)).toEqual([
      "AUTH",
      "LOCKOUT",
      "OK",
      "NO-DISPATCH",
    ])
  })

  test("parses Codex rollout transcript messages", () => {
    const parsed = parseCodexSessionRollout(`
{"timestamp":"2026-06-29T02:44:00.928Z","type":"session_meta","payload":{"id":"019f1143-24ae-76b1-851b-494930817124","cwd":"/tmp/workspace-one","source":"exec"}}
{"timestamp":"2026-06-29T02:44:00.928Z","type":"event_msg","payload":{"type":"assignment","message":"assignment.public.khala_coding.test_6958 accepted"}}
{"timestamp":"2026-06-29T02:44:00.929Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Implement issue #6958"}]}}
{"timestamp":"2026-06-29T02:45:00.000Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\\"cmd\\":\\"git status\\"}","call_id":"call-1"}}
{"timestamp":"2026-06-29T02:45:01.000Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"clean"}}
{"timestamp":"2026-06-29T02:45:02.000Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"Check the working tree before editing."}]}}
{"timestamp":"2026-06-29T02:45:03.000Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-2","output":"permission denied","is_error":true}}
{"timestamp":"2026-06-29T02:46:00.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done."}]}}
`)

    expect(parsed).toMatchObject({
      assignmentRef: "assignment.public.khala_coding.test_6958",
      cwd: "/tmp/workspace-one",
      messageCount: 7,
      sessionId: "019f1143-24ae-76b1-851b-494930817124",
      source: "exec",
      title: "Implement issue #6958",
    })
    expect(parsed.messages.map(message => message.role)).toEqual([
      "event",
      "user",
      "tool",
      "tool",
      "reasoning",
      "tool",
      "assistant",
    ])
    expect(parsed.messages[2]).toMatchObject({
      detail: "call-1",
      kind: "tool call",
      status: "running",
      text: "{\"cmd\":\"git status\"}",
      title: "shell",
    })
    expect(parsed.messages[3]).toMatchObject({
      detail: "call-1",
      kind: "tool output",
      status: "ok",
      title: "tool result",
    })
    expect(parsed.messages[4]).toMatchObject({
      kind: "reasoning",
      status: "info",
      text: "Check the working tree before editing.",
      title: "reasoning",
    })
    expect(parsed.messages[5]).toMatchObject({
      detail: "call-2",
      kind: "tool output",
      status: "error",
      text: "permission denied",
      title: "tool error",
    })
    expect(parsed.messages.at(-1)?.text).toBe("Done.")
  })

  test("uses task-like rollout text instead of injected AGENTS context as title", () => {
    const parsed = parseCodexSessionRollout(`
{"timestamp":"2026-06-29T02:44:00.928Z","type":"session_meta","payload":{"id":"019f1143-24ae-76b1-851b-494930817124","cwd":"/tmp/workspace-one","source":"exec"}}
{"timestamp":"2026-06-29T02:44:00.929Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /tmp/workspace\\nDo work carefully."}]}}
{"timestamp":"2026-06-29T02:44:01.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"You are writing the title and body for a GitHub pull request that resolves a public issue."}]}}
{"timestamp":"2026-06-29T02:46:00.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"{\\"title\\":\\"fix(operator): accept account ref hash\\",\\"body\\":\\"Addresses #6637\\"}"}]}}
`)

    expect(parsed.title).toBe("fix(operator): accept account ref hash")
  })
})
