import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  runTwoCodexReadOnlySmoke,
  TWO_CODEX_READONLY_SMOKE_CLAIM_REF,
  TWO_CODEX_READONLY_SMOKE_COUNT,
  TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY,
} from "../src/bun/khala-codex-live-smoke"
import {
  type KhalaCodexFleetCommandInput,
  type KhalaCodexFleetCommandResult,
  type KhalaCodexFleetProgressPayload,
} from "../src/bun/khala-fleet-tools"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempPylonFixture(): Promise<{
  readonly env: Record<string, string>
}> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-live-smoke-"))
  tempDirs.push(root)
  const appPath = join(root, "apps", "pylon")
  const home = join(root, "pylon-home")
  await mkdir(appPath, { recursive: true })
  await mkdir(home, { recursive: true })
  await writeFile(join(appPath, "package.json"), JSON.stringify({ name: "@openagentsinc/pylon" }))
  return {
    env: {
      OPENAGENTS_BUN_PATH: process.execPath,
      OPENAGENTS_PYLON_APP_PATH: appPath,
      PYLON_HOME: home,
    },
  }
}

function ok(stdout: unknown): KhalaCodexFleetCommandResult {
  return {
    exitCode: 0,
    signal: null,
    stderr: "",
    stdout: typeof stdout === "string" ? stdout : JSON.stringify(stdout),
    timedOut: false,
  }
}

function failed(stderr: string): KhalaCodexFleetCommandResult {
  return {
    exitCode: 1,
    signal: null,
    stderr,
    stdout: "",
    timedOut: false,
  }
}

function pylonArgs(input: KhalaCodexFleetCommandInput): readonly string[] {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
}

const ACCOUNT_KEY = "aaaaaaaaaaaaaaaaaaaaaaaa"
const ACCOUNT_REF_HASH = `account.pylon.codex.${ACCOUNT_KEY}`
const ASSIGNMENT_ONE = "assignment.public.codex_agent_task.readonly_one"
const ASSIGNMENT_TWO = "assignment.public.codex_agent_task.readonly_two"

describe("Khala Code live two-Codex smoke harness", () => {
  test("streams two lifecycle refs and verifies per-slot token accounting", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    const progress: KhalaCodexFleetProgressPayload[] = []

    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      if (input.cmd[0] === "git" && input.cmd[1] === "ls-remote") {
        return ok("0123456789abcdef0123456789abcdef01234567\trefs/heads/main\n")
      }
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            codexAccounts: [{
              accountKey: ACCOUNT_KEY,
              available: 2,
              busy: 0,
              queued: 0,
              ready: 2,
            }],
            maxCodexAssignments: 2,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [{
            accountRef: "codex-live-smoke",
            accountRefHash: ACCOUNT_REF_HASH,
            homeState: "present",
            provider: "codex",
          }],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [{
            accountRef: "codex-live-smoke",
            accountRefHash: ACCOUNT_REF_HASH,
            provider: "codex",
            quota: { state: "available" },
            readiness: { state: "ready" },
          }],
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            codexAccounts: [{
              accountKey: ACCOUNT_KEY,
              available: 2,
              busy: 0,
              queued: 0,
              ready: 2,
            }],
            maxCodexAssignments: 2,
          },
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "khala apm --base-url https://openagents.com --json") {
        return ok({
          active: { serverAssignmentCount: 0, serverAssignments: [] },
          counted: { completedTokenRows: 0, completedTokensPerMinute: 0 },
          schema: "openagents.pylon.khala_apm.v0.1",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        expect(args).toContain("--count")
        expect(args).toContain(String(TWO_CODEX_READONLY_SMOKE_COUNT))
        expect(args).toContain("--max-parallel")
        expect(args).toContain(String(TWO_CODEX_READONLY_SMOKE_COUNT))
        expect(args).toContain("--execute")
        expect(args).toContain("--lifecycle-ndjson")
        expect(args).toContain("--json")
        expect(args).toContain("--repo")
        expect(args).toContain("OpenAgentsInc/openagents")
        expect(args).toContain("--branch")
        expect(args).toContain("main")
        expect(args).toContain("--commit")
        expect(args).toContain("0123456789abcdef0123456789abcdef01234567")
        expect(args).toContain("--verify")
        expect(args).toContain(TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY)
        expect(args).not.toContain("--fixture")
        expect(args).toContain("--objective")
        const objective = args[args.indexOf("--objective") + 1] ?? ""
        expect(objective).toContain("Do not edit")
        expect(objective).toContain(`Claim: ${TWO_CODEX_READONLY_SMOKE_CLAIM_REF}.`)
        expect(input.env?.OPENAGENTS_PYLON_DISABLE_ASSIGNMENT_PR).toBe("1")

        for (const [slotIndex, assignmentRef] of [ASSIGNMENT_ONE, ASSIGNMENT_TWO].entries()) {
          await input.onStderrLine?.(JSON.stringify({
            assignmentEvent: "assignment_run.runtime_started",
            assignmentRef,
            message: "assignment lifecycle event",
            observedAt: `2026-06-30T00:00:0${slotIndex}.000Z`,
            schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
            slotIndex,
            state: "running",
          }))
          await input.onStderrLine?.(JSON.stringify({
            assignmentEvent: "assignment_run.runtime_progress",
            assignmentRef,
            message: "assignment lifecycle event",
            observedAt: `2026-06-30T00:00:0${slotIndex}.500Z`,
            phase: "runtime_active",
            schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
            slotIndex,
            state: "running",
          }))
        }

        return ok({
          aggregate: {
            acceptedCount: 2,
            assignmentRefs: [ASSIGNMENT_ONE, ASSIGNMENT_TWO],
            durableRequestIds: ["durable.one", "durable.two"],
            ownerOnlyRawEventCount: 2,
            ownerOnlyTraceCount: 2,
            totalTokenRows: 2,
            totalVerifiedTokens: 42,
          },
          blockerRefs: [],
          counter: {
            delta: 42,
            expectedMinimumDelta: 42,
            state: "increment_observed",
          },
          ok: true,
          plan: {
            requestedCount: 2,
            slots: [
              { account: { accountRef: "codex-live-smoke" }, slotIndex: 0 },
              { account: { accountRef: "codex-live-smoke" }, slotIndex: 1 },
            ],
            targetPylonRef: "pylon.local.test",
          },
          results: [
            batchResult(0, ASSIGNMENT_ONE, "durable.one", 19),
            batchResult(1, ASSIGNMENT_TWO, "durable.two", 23),
          ],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const summary = await runTwoCodexReadOnlySmoke({
      env: fixture.env,
      fetch: async () => {
        throw new Error("fixture tests must not call the live public counter")
      },
      onProgress: payload => {
        progress.push(payload)
      },
      runner,
      work: {
        branch: "main",
        commit: "0123456789abcdef0123456789abcdef01234567",
        kind: "repository",
        repo: "OpenAgentsInc/openagents",
      },
    })

    expect(summary).toMatchObject({
      acceptedCount: 2,
      mode: "repository",
      ok: true,
      pylonRef: "pylon.local.test",
      readOnlyVerify: TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY,
      requestedCount: 2,
      tokensVerified: 42,
    })
    expect(summary.assignmentRefs).toEqual([ASSIGNMENT_ONE, ASSIGNMENT_TWO])
    expect(summary.streamedAssignmentRefs).toEqual([ASSIGNMENT_ONE, ASSIGNMENT_TWO])
    expect(summary.perSlotTokens).toEqual([19, 23])
    expect(summary.slotSummaries).toHaveLength(2)
    expect(summary.slotSummaries[0]).toContain("proof: 19 verified tokens")
    expect(summary.progressPayloadCount).toBe(4)
    expect(summary.progressEventCount).toBe(10)
    expect(summary.publicCounterReconciliation.state).toBe("skipped")
    expect(summary.blockerRefs).toEqual([])
    expect(progress.at(-1)?.events.some(event => event.assignmentRef === ASSIGNMENT_TWO)).toBe(true)
    expect(calls.some(call => pylonArgs(call)[0] === "khala" && pylonArgs(call)[1] === "spawn")).toBe(true)
  })
})

function batchResult(
  slotIndex: number,
  assignmentRef: string,
  durableRequestId: string,
  totalTokens: number,
): Record<string, unknown> {
  return {
    assignmentRef,
    blockerRefs: [],
    closeoutStatus: "accepted",
    durableRequestId,
    lifecycleEvents: [
      {
        assignmentEvent: "assignment_run.runtime_started",
        message: "assignment_run.runtime_started",
        observedAt: "2026-06-30T00:00:00.000Z",
        slotIndex,
        state: "running",
      },
      {
        assignmentEvent: "assignment_run.completed",
        message: "assignment_run.completed",
        observedAt: "2026-06-30T00:00:01.000Z",
        slotIndex,
        state: "accepted",
        status: "accepted",
      },
    ],
    ok: true,
    proof: {
      rawEventCount: 1,
      tokenRows: 1,
      totalTokens,
      traceCount: 1,
    },
    runAccepted: true,
    slotIndex,
    state: "accepted",
  }
}
