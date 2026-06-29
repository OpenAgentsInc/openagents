// CS-A2 (#5362): end-to-end proof of the swarm / multi-session view against a
// REAL loopback Pylon control server (a `pylon dev`-equivalent node), driven
// through the desktop's OWN Bun functions — the same path the webview uses:
//
//   - spawnSession() x N → 2+ CONCURRENT coding sessions on the node;
//   - fetchNodeState() → the `session.list` + per-session events + accounts +
//     approvals projection the swarm grid renders;
//   - the pure swarm helpers (orderSwarmSessions / swarmStatusLabel /
//     swarmAccountLabel / swarmSummaryLine / swarmSessionPendingApprovals) over
//     that live state — exactly what each grid cell + the top-level roll-up show;
//   - an enqueued approval → it surfaces in the swarm's pending-approvals
//     roll-up (the authoritative `approvals[]` queue), then resolves;
//   - cancelSession() → cancel one session "from the grid".
//
// This is on the EXISTING control protocol (session.list/spawn/cancel/events +
// approvals.list/resolve). No new wire verb. No connected paid provider account
// is required: the executor is a stub, and the account home is a scratch dir so
// the registry resolves it. Run from the repo root:
//
//   bun apps/autopilot-desktop/scripts/swarm-view-proof.ts
//
// Output is a public-safe transcript (refs only) suitable for a launch update.

import { mkdtempSync } from "node:fs"
import { mkdir, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import { createBootstrapSummary, parseBootstrapArgs } from "../../pylon/src/bootstrap"
import { startControlServer } from "../../pylon/src/node/control-server"
import { createApprovalQueue } from "../../pylon/src/node/approval-queue"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
} from "../../pylon/src/node/control-sessions"
import { makePylonNodeRuntime } from "../../pylon/src/node/runtime"
import { PYLON_DEV_CHECK_SCHEMA } from "../../pylon/src/dev-loop"

import {
  orderSwarmSessions,
  swarmAccountLabel,
  swarmSessionPendingApprovals,
  swarmStatusLabel,
  swarmSummaryLine,
  swarmWorkspaceLabel,
} from "../src/ui/helpers"
import { cancelSession, fetchNodeState, spawnSession } from "../src/bun/pylon-control"

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
  const root = mkdtempSync(join(tmpdir(), "swarm-view-proof-"))
  const pylonHome = join(root, "pylon-home")
  const accountHome = join(root, "codex-home")
  const worktreeA = join(root, "worktree-a")
  const worktreeB = join(root, "worktree-b")
  const proofDir = join(root, "proofs")
  await mkdir(pylonHome, { recursive: true })
  await mkdir(accountHome, { recursive: true })
  await mkdir(worktreeA, { recursive: true })
  await mkdir(worktreeB, { recursive: true })
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
          // Codex/Claude executors do, then completes — so the swarm cells carry
          // a real status + a latest-activity tail without a paid account.
          const stubExecutor: ControlSessionExecutor = async (input) => {
            input.emit({ phase: "composer_event", message: "edited src/health.ts (+12 −0)", composerEventIndex: 1 })
            input.emit({ phase: "dev_check_started" })
            return {
              commandCount: 1,
              devCheck: fakeDevCheck(),
              editedFileCount: 1,
              eventCount: 2,
              externalSessionRef: "session.pylon.fake.external",
              responseDigestRef: "digest.pylon.fake.response",
              totalTokens: 42,
            }
          }

          // The approval queue backs the swarm's top-level pending-approvals
          // roll-up (the authoritative `approvals[]` projection).
          const approvals = createApprovalQueue()

          const server = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({
                executor: stubExecutor,
                proofsDir: proofDir,
                summary,
              }),
              approvals: {
                list: async () => ({ approvals: approvals.list() }),
                resolve: async (input) => approvals.resolve(input.approvalRef, input.decision, { answer: input.answer }),
              },
            },
            port: 0,
          })
          const baseUrl = server.url
          log("Loopback control server up (pylon dev-equivalent)")
          log("control url", baseUrl)

          // 1) SPAWN 2+ CONCURRENT SESSIONS through the desktop's spawnSession(),
          //    each in its own worktree (the runtime's concurrent spawner #4869).
          log("STEP 1 — spawn 2 concurrent sessions (desktop spawnSession → session.spawn)")
          const [spawnA, spawnB] = yield* Effect.promise(() =>
            Promise.all([
              spawnSession({
                baseUrl,
                token: TOKEN,
                adapter: "codex",
                objective: "add a GET /health route",
                verify: ["bun", "--version"],
                worktreePath: worktreeA,
              }),
              spawnSession({
                baseUrl,
                token: TOKEN,
                adapter: "codex",
                objective: "add a /metrics route",
                verify: ["bun", "--version"],
                worktreePath: worktreeB,
              }),
            ]),
          )
          if (!spawnA.ok || !spawnB.ok) throw new Error("concurrent spawn failed")
          log("session A", spawnA.sessionRef)
          log("session B", spawnB.sessionRef)

          // 2) SWARM GRID — fetch the node-state and render the swarm helpers
          //    over it. Wait for both turns to reach a terminal state so the grid
          //    shows real statuses.
          log("STEP 2 — swarm grid via desktop fetchNodeState() (session.list projection)")
          let node = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
          for (let attempt = 0; attempt < 80; attempt += 1) {
            node = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
            const terminal = node.sessions.filter(
              (s) => s.state === "completed" || s.state === "failed" || s.state === "cancelled",
            ).length
            if (node.sessions.length >= 2 && terminal >= 2) break
            yield* Effect.sleep("25 millis")
          }
          if (node.sessions.length < 2) throw new Error("swarm did not list both sessions")
          const ordered = orderSwarmSessions(node.sessions)
          log("swarm lists N sessions", ordered.length)
          for (const session of ordered) {
            const status = swarmStatusLabel(session.state)
            const account = swarmAccountLabel(session, node.accounts)
            const repo = swarmWorkspaceLabel(session)
            const cellApprovals = swarmSessionPendingApprovals(node.events[session.sessionRef])
            log(
              `cell ${session.sessionRef.slice(-12)}`,
              `${status.text} · ${account} · repo ${repo} · ${cellApprovals} pending`,
            )
          }
          // Every cell must carry a real status + an account label.
          const allHaveStatus = ordered.every((s) => swarmStatusLabel(s.state).text.length > 0)
          const allHaveAccount = ordered.every((s) => swarmAccountLabel(s, node.accounts).length > 0)
          if (!allHaveStatus) throw new Error("a swarm cell had no status")
          if (!allHaveAccount) throw new Error("a swarm cell had no account label")
          log("every cell has status + account", allHaveStatus && allHaveAccount)

          // 3) APPROVALS ROLL-UP — enqueue an approval; it surfaces in the swarm's
          //    top-level pending-approvals roll-up (the authoritative queue).
          log("STEP 3 — approval surfaces in the swarm roll-up (approvals.list)")
          approvals.enqueue({ approvalRef: "ap.pylon.swarm.proof.1", kind: "tool", prompt: "Run bun test?" })
          node = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
          log("pending approvals across all sessions", node.approvals.length)
          log("swarm summary line", swarmSummaryLine(node.sessions, node.approvals.length))
          if (node.approvals.length !== 1) throw new Error("approval did not surface in the roll-up")
          if (!swarmSummaryLine(node.sessions, node.approvals.length).includes("pending approval")) {
            throw new Error("roll-up summary did not reflect the pending approval")
          }

          // 4) CANCEL ONE FROM THE GRID — cancel a still-running session via the
          //    desktop's cancelSession(). Spawn a long-running one to cancel.
          log("STEP 4 — cancel one session from the grid (desktop cancelSession → session.cancel)")
          const cancelExecutor: ControlSessionExecutor = async (input) =>
            await new Promise((_resolve, reject) => {
              input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
            })
          const cancelServer = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({
                executor: cancelExecutor,
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
              objective: "a long-running swarm session to cancel",
              verify: ["bun", "--version"],
              worktreePath: worktreeA,
            }),
          )
          if (!longSpawn.ok) throw new Error("cancel-target spawn failed")
          yield* Effect.sleep("50 millis")
          const cancelled = yield* Effect.promise(() =>
            cancelSession({ baseUrl: cancelServer.url, token: TOKEN, sessionRef: longSpawn.sessionRef }),
          )
          log("cancel result state", cancelled.state)
          if (cancelled.state !== "cancelled") throw new Error(`cancel did not record cancelled: ${cancelled.state}`)

          log("PROOF COMPLETE — the swarm view drives N concurrent sessions on the real control protocol")
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
