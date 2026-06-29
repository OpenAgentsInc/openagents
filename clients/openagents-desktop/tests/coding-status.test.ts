import { describe, expect, test } from "bun:test"

import {
  parseCodingProcesses,
  parseSupervisorLog,
  summarizeCodingProcesses,
} from "../src/shared/coding-status.js"

describe("openagents desktop coding status", () => {
  test("parses local coding agent processes", () => {
    const processes = parseCodingProcesses(`
  100     1  8.2      01:20 /Users/me/.codex/accounts/codex-4/codex/vendor/aarch64-apple-darwin/bin/codex exec --json
  101     1  0.0      02:00 bun apps/pylon/src/index.ts khala request --prompt Implement public issue #6932 --account-ref codex-6
  102     1  0.2      10:00 bash /Users/me/work/apps/pylon/scripts/codex-supervisor/codex-supervisor.sh
  103     1  5.0      00:30 bun scripts/khala-vertex-continual-learning-burn.mjs
`)

    expect(processes.map(process => process.kind)).toEqual([
      "codex_exec",
      "khala_request",
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
    expect(summarizeCodingProcesses(processes)).toMatchObject({
      burningCodexCount: 1,
      codexExecCount: 1,
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
})
