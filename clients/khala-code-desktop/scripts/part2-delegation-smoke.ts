#!/usr/bin/env bun
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  spawnCodexInstances,
  type KhalaCodexFleetCommandInput,
  type KhalaCodexFleetCommandResult,
} from "../src/bun/khala-codex-fleet-tools"

const MATRIX_ACCOUNT_KEY = "4db4cc18ebc55f39fb4da894"
const MATRIX_ACCOUNT_REF_HASH = `account.pylon.codex.${MATRIX_ACCOUNT_KEY}`
const LEGACY_DEAD_END = "codex_spawn_failed: No Pylon Codex assignment capacity is available right now"

const root = await mkdtemp(join(tmpdir(), "khala-code-part2-delegation-"))

try {
  const appPath = join(root, "apps", "pylon")
  const home = join(root, "pylon-home")
  await mkdir(appPath, { recursive: true })
  await mkdir(home, { recursive: true })
  await writeFile(join(appPath, "package.json"), JSON.stringify({ name: "@openagentsinc/pylon" }))

  let advertised = false
  const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
    const args = pylonArgs(input)
    const joined = args.join(" ")
    if (joined === "provider go-online --json") {
      return ok(advertised
        ? {
            ok: true,
            ownCapacityDispatch: {
              availableCodexAssignments: 4,
              codexAccounts: [{
                accountKey: MATRIX_ACCOUNT_KEY,
                available: 4,
                busy: 1,
                queued: 0,
                ready: 5,
              }],
              maxCodexAssignments: 5,
            },
            pylonRef: "pylon.local.part2",
          }
        : {
            ok: true,
            ownCapacityDispatch: {
              availableCodexAssignments: 0,
              maxCodexAssignments: 1,
            },
            pylonRef: "pylon.local.part2",
          })
    }
    if (joined === "codex accounts list --json") {
      return ok({
        accounts: [{
          accountRef: "codex-2",
          accountRefHash: MATRIX_ACCOUNT_REF_HASH,
          homeState: "present",
          provider: "codex",
          readiness: { state: "ready" },
        }],
        schema: "openagents.pylon.accounts_list.v0.3",
      })
    }
    if (joined === "accounts status --provider codex --json") {
      return ok({ accounts: [], schema: "openagents.pylon.accounts_status.v0.1" })
    }
    if (joined === "presence heartbeat --base-url https://openagents.com --json") {
      if (input.env?.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY !== "5") {
        throw new Error("part2 smoke expected desktop to advertise five Codex slots before heartbeat")
      }
      advertised = true
      return ok({
        heartbeatRef: "heartbeat.pylon.local.part2.capacity_advertised",
        pylonRef: "pylon.local.part2",
      })
    }
    if (args[0] === "khala" && args[1] === "spawn") {
      return ok({
        aggregate: {
          acceptedCount: 1,
          assignmentRefs: ["assignment.public.codex_agent_task.part2_demo"],
          durableRequestIds: ["durable.public.part2_demo"],
          ownerOnlyRawEventCount: 1,
          ownerOnlyTraceCount: 1,
          totalTokenRows: 1,
          totalVerifiedTokens: 100,
        },
        counter: { expectedMinimumDelta: 0, state: "not_checked" },
        ok: true,
        plan: {
          requestedCount: 1,
          slots: [{ account: { accountRef: "codex-2" }, slotIndex: 0 }],
          targetPylonRef: "pylon.local.part2",
        },
        results: [{
          assignmentRef: "assignment.public.codex_agent_task.part2_demo",
          blockerRefs: [],
          closeoutStatus: "accepted",
          durableRequestId: "durable.public.part2_demo",
          lifecycleEvents: [{
            assignmentEvent: "assignment_run.completed",
            observedAt: "2026-06-30T18:30:00.000Z",
            state: "completed",
            status: "accepted",
          }],
          ok: true,
          proof: { rawEventCount: 1, tokenRows: 1, totalTokens: 100, traceCount: 1 },
          runAccepted: true,
          slotIndex: 0,
          state: "completed",
        }],
        schema: "openagents.pylon.khala_spawn_run.v0.1",
      })
    }
    return failed(`unexpected command: ${joined}`)
  }

  const result = await spawnCodexInstances({
    count: 1,
    prompt: "Test delegating a piece of work to one Codex worker, targeting an open issue, and only do analysis. Do not change code.",
  }, {
    env: {
      OPENAGENTS_BUN_PATH: process.execPath,
      OPENAGENTS_PYLON_APP_PATH: appPath,
      PYLON_HOME: home,
    },
    runner,
  })

  const serialized = JSON.stringify(result)
  const modules = result.delegateTrace?.map(step => step.module) ?? []
  const requiredModules = [
    "ensure_pylon",
    "advertise_capacity",
    "select_account",
    "prepare_work",
    "dispatch",
    "verify_closeout",
  ]
  const missing = requiredModules.filter(module => !modules.includes(module))
  if (missing.length > 0) {
    throw new Error(`part2 smoke missing delegate module(s): ${missing.join(", ")}`)
  }
  if (serialized.includes(LEGACY_DEAD_END)) {
    throw new Error("part2 smoke regressed to the legacy opaque 0/1 capacity dead-end")
  }
  if (result.acceptedCount !== 1 || result.results[0]?.assignmentRef === null) {
    throw new Error("part2 smoke did not accept the demo assignment")
  }

  console.log("Part 2 delegation smoke: PASS")
  console.log(`assignmentRef=${result.results[0]?.assignmentRef}`)
  console.log(`pylonRef=${result.pylonRef}`)
  console.log(`delegate=${result.delegateSignature} status=${result.delegateStatus}`)
  for (const step of result.delegateTrace ?? []) {
    console.log(`- ${step.module}: ${step.status}${step.fallbackModule === undefined ? "" : ` -> ${step.fallbackModule}`}`)
  }
} finally {
  await rm(root, { force: true, recursive: true })
}

function pylonArgs(input: KhalaCodexFleetCommandInput): readonly string[] {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
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
