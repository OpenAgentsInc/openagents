// #5355: end-to-end proof of the coding composer loop against a REAL loopback
// Pylon control server (a `pylon dev`-equivalent node), driven through the
// desktop's OWN Bun control functions (spawnSession / fetchNodeState /
// cancelSession from src/bun/pylon-control.ts).
//
// This proves the composer's wiring path — spawn → live session-event tail →
// inline-approval projection → continuation (reply) spawn → cancel — on the
// EXISTING control protocol, WITHOUT needing connected paid provider accounts:
// the executor is a stub that emits a composer/tool/diff event the same way the
// real Codex/Claude executors do. Run from the repo root:
//
//   bun apps/autopilot-desktop/scripts/composer-loop-proof.ts
//
// Output is a public-safe transcript (refs only, no raw paths/prompts) suitable
// for pasting into a launch update.

import { mkdtempSync } from "node:fs"
import { mkdir, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import { createBootstrapSummary, parseBootstrapArgs } from "../../pylon/src/bootstrap"
import { startControlServer } from "../../pylon/src/node/control-server"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
} from "../../pylon/src/node/control-sessions"
import { makePylonNodeRuntime } from "../../pylon/src/node/runtime"
import { PYLON_DEV_CHECK_SCHEMA } from "../../pylon/src/dev-loop"

import {
  buildComposerContinuationObjective,
  composerCanReply,
} from "../src/ui/helpers"
import {
  cancelSession,
  fetchNodeState,
  spawnSession,
} from "../src/bun/pylon-control"

const TOKEN = "test-token-0123456789abcdef"

const fakeDevCheck = () => ({
  schema: PYLON_DEV_CHECK_SCHEMA,
  observedAt: "2026-06-18T00:00:00.000Z",
  action: "check" as const,
  state: "passed" as const,
  changeSummary: {
    repo: { state: "not_git" as const, rootRef: null, branch: null, commit: null },
    dirty: { state: "clean" as const, changedCount: 0, stagedCount: 0, unstagedCount: 0, untrackedCount: 0 },
    changedFileRefs: [],
    areaRefs: [],
    blockerRefs: [],
  },
  checkPlan: { state: "ready" as const, commandRefs: ["command.pylon.control_session.proof"], blockerRefs: [] },
  commandResults: [],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: [],
})

const log = (label: string, value?: unknown) =>
  value === undefined
    ? console.log(`\n=== ${label} ===`)
    : console.log(`  ${label}: ${typeof value === "string" ? value : JSON.stringify(value)}`)

async function main() {
  const root = mkdtempSync(join(tmpdir(), "composer-loop-proof-"))
  const pylonHome = join(root, "pylon-home")
  const accountHome = join(root, "codex-home")
  const worktree = join(root, "worktree")
  const proofDir = join(root, "proofs")
  await mkdir(pylonHome, { recursive: true })
  await mkdir(accountHome, { recursive: true })
  await mkdir(worktree, { recursive: true })
  await writeFile(
    join(pylonHome, "config.json"),
    `${JSON.stringify({ dev: { accounts: [{ ref: "codex-a", provider: "codex", home: accountHome }] } })}\n`,
  )
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })

  let ok = true
  try {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime

          // A stub coding executor that emits a tool/diff event like the real
          // Codex/Claude executors do — the content the composer transcript tails.
          const firstTurnExecutor: ControlSessionExecutor = async (input) => {
            input.emit({ phase: "composer_event", message: "edited src/health.ts (+12 −0)", composerEventIndex: 1 })
            input.emit({ phase: "composer_event", message: "added GET /health route", composerEventIndex: 2 })
            input.emit({ phase: "dev_check_started" })
            return {
              commandCount: 1,
              devCheck: fakeDevCheck(),
              editedFileCount: 1,
              eventCount: 3,
              externalSessionRef: "session.pylon.fake.external",
              responseDigestRef: "digest.pylon.fake.response",
              totalTokens: 42,
            }
          }

          const server = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({
                executor: firstTurnExecutor,
                proofsDir: proofDir,
                summary,
              }),
            },
            port: 0,
          })
          const baseUrl = server.url
          log("Loopback control server up (pylon dev-equivalent)")
          log("control url", baseUrl)

          // 1) SPAWN — the composer's first coding turn, through the desktop's
          //    own spawnSession() Bun function (carrying the repo/worktree path).
          log("STEP 1 — composer spawn (desktop spawnSession → session.spawn)")
          const objective1 = "add a GET /health route and a test"
          const spawn = yield* Effect.promise(() =>
            spawnSession({
              baseUrl,
              token: TOKEN,
              adapter: "codex",
              objective: objective1,
              verify: ["bun", "--version"],
              worktreePath: worktree,
            }),
          )
          if (!spawn.ok) throw new Error(`spawn failed: ${spawn.error}`)
          log("spawned sessionRef", spawn.sessionRef)

          // 2) LIVE TRANSCRIPT — poll the desktop's fetchNodeState() (the same
          //    projection the webview renders), waiting for the turn to finish
          //    and the session-event tail (incl. the tool/diff lines) to land.
          log("STEP 2 — live transcript via desktop fetchNodeState() (session-event tail)")
          let state = ""
          let events: Array<{ phase: string; detail: string }> = []
          for (let attempt = 0; attempt < 60; attempt += 1) {
            const node = yield* Effect.promise(() =>
              fetchNodeState({ baseUrl, token: TOKEN }),
            )
            const session = node.sessions.find((s) => s.sessionRef === spawn.sessionRef)
            state = session?.state ?? ""
            events = (node.events[spawn.sessionRef] ?? []).map((e) => ({ phase: e.phase, detail: e.detail }))
            if (state === "completed" || state === "failed") break
            yield* Effect.sleep("25 millis")
          }
          log("turn state", state)
          log("transcript event phases", events.map((e) => e.phase).join(" → "))
          const diffLine = events.find((e) => e.detail.includes("edited"))
          log("tool/diff line surfaced", diffLine ? diffLine.detail : "(none)")
          if (state !== "completed") throw new Error("first turn did not complete")
          if (!diffLine) throw new Error("no tool/diff event surfaced in the transcript")

          // 3) APPROVALS — confirm the node-state projection carries the pending
          //    approval queue the composer renders inline (empty here: the stub
          //    executor requests none; the desktop projection path is exercised).
          log("STEP 3 — inline approvals projection (desktop reads approvals[])")
          const nodeForApprovals = yield* Effect.promise(() =>
            fetchNodeState({ baseUrl, token: TOKEN }),
          )
          log("pending approvals (projection wired)", nodeForApprovals.approvals.length)

          // 4) REPLY / CONTINUE — once the turn is terminal, the composer unlocks
          //    a follow-up. The continuation objective carries the prior turn and
          //    spawns a NEW bounded session in the same worktree (no new verb).
          log("STEP 4 — reply/continue (continuation spawn carrying prior turn)")
          log("composerCanReply(completed)", composerCanReply(state))
          const followUp = "now add an edge-case test for /health"
          const objective2 = buildComposerContinuationObjective([objective1], followUp)
          log("continuation objective carries prior turn", objective2.includes(objective1))
          const replySpawn = yield* Effect.promise(() =>
            spawnSession({
              baseUrl,
              token: TOKEN,
              adapter: "codex",
              objective: objective2,
              verify: ["bun", "--version"],
              worktreePath: worktree,
            }),
          )
          if (!replySpawn.ok) throw new Error(`reply spawn failed: ${replySpawn.error}`)
          log("continuation sessionRef", replySpawn.sessionRef)

          // 5) CANCEL — cancel a running turn through the desktop's cancelSession().
          //    Spawn a long-running turn, then cancel it.
          log("STEP 5 — cancel (desktop cancelSession → session.cancel)")
          const cancelServerExecutor: ControlSessionExecutor = async (input) =>
            await new Promise((_resolve, reject) => {
              input.abortSignal.addEventListener(
                "abort",
                () => reject(new Error("cancelled")),
                { once: true },
              )
            })
          const cancelServer = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({
                executor: cancelServerExecutor,
                proofsDir: proofDir,
                summary,
              }),
            },
            port: 0,
          })
          const longSpawn = yield* Effect.promise(() =>
            spawnSession({
              baseUrl: cancelServer.url,
              token: TOKEN,
              adapter: "codex",
              objective: "a long-running turn to cancel",
              verify: ["bun", "--version"],
              worktreePath: worktree,
            }),
          )
          if (!longSpawn.ok) throw new Error("cancel-target spawn failed")
          // Give it a moment to reach running.
          yield* Effect.sleep("50 millis")
          const cancelled = yield* Effect.promise(() =>
            cancelSession({ baseUrl: cancelServer.url, token: TOKEN, sessionRef: longSpawn.sessionRef }),
          )
          log("cancel result state", cancelled.state)
          if (cancelled.state !== "cancelled") throw new Error(`cancel did not record cancelled: ${cancelled.state}`)

          log("PROOF COMPLETE — composer loop drives the real control protocol end-to-end")
        }),
      ),
    )
  } catch (error) {
    ok = false
    console.error("\nPROOF FAILED:", error instanceof Error ? error.message : error)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
  process.exit(ok ? 0 : 1)
}

void main()
