// CS-A3 (#5363): end-to-end proof of diff fidelity + transcript persistence
// against a REAL loopback Pylon control server (a `pylon dev`-equivalent node),
// driven through the desktop's OWN functions — the same path the webview uses:
//
//   - spawnSession() → a coding session that emits a tool/diff event;
//   - fetchNodeState() → the session.list + per-session event tail projection
//     the composer / session-detail pane render;
//   - persistAndMergeTranscripts() → the desktop's poll-time transcript store
//     (persist the polled event tail keyed by sessionRef under the node home,
//     merge the durable transcript back in);
//   - parseChangeSetFromEvents() → the structured ChangeSet the SHARED DiffReview
//     component renders (the UI port of apps/pylon/src/tas/diff-review.ts);
//   - a SIMULATED node restart (a fresh control server with no in-memory tail +
//     an empty session.list) → the transcript still reloads from the persisted
//     store, and the diff still renders from the reloaded transcript.
//
// This is on the EXISTING control protocol (session.spawn/list/events). No new
// wire verb. No connected paid provider account is required: the executor is a
// stub that emits a file-edit composer event the same way the real Codex/Claude
// executors do. Run from the repo root:
//
//   bun apps/autopilot-desktop/scripts/transcript-proof.ts
//
// Output is a public-safe transcript (refs only) suitable for a launch update.

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

import { parseChangeSetFromEvents } from "../src/ui/helpers"
import { fetchNodeState, spawnSession } from "../src/bun/pylon-control"
import { persistAndMergeTranscripts } from "../src/bun/transcript-store"

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
  const root = mkdtempSync(join(tmpdir(), "transcript-proof-"))
  const pylonHome = join(root, "pylon-home")
  const accountHome = join(root, "codex-home")
  const worktree = join(root, "worktree")
  const proofDir = join(root, "proofs")
  // The desktop transcript store keys persistence under the (discovered) node
  // home. Here we persist under a dedicated dir to model the desktop's node-home.
  const desktopNodeHome = join(root, "desktop-node-home")
  await mkdir(pylonHome, { recursive: true })
  await mkdir(accountHome, { recursive: true })
  await mkdir(worktree, { recursive: true })
  await mkdir(desktopNodeHome, { recursive: true })
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
          // Codex/Claude executors do — the content the diff viewer parses.
          const editExecutor: ControlSessionExecutor = async (input) => {
            input.emit({ phase: "composer_event", message: "edited src/health.ts (+12 −0)", composerEventIndex: 1 })
            input.emit({ phase: "composer_event", message: "completed: add tests/health.test.ts", composerEventIndex: 2 })
            input.emit({ phase: "dev_check_started" })
            return {
              commandCount: 1,
              devCheck: fakeDevCheck(),
              editedFileCount: 2,
              eventCount: 3,
              externalSessionRef: "session.pylon.fake.external",
              responseDigestRef: "digest.pylon.fake.response",
              totalTokens: 42,
            }
          }

          const server = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({ executor: editExecutor, proofsDir: proofDir, summary }),
            },
            port: 0,
          })
          const baseUrl = server.url
          log("Loopback control server up (pylon dev-equivalent)")
          log("control url", baseUrl)

          // 1) SPAWN — a coding turn that edits files.
          log("STEP 1 — spawn a coding turn that edits files (desktop spawnSession)")
          const spawn = yield* Effect.promise(() =>
            spawnSession({
              baseUrl,
              token: TOKEN,
              adapter: "codex",
              objective: "add a GET /health route and a test",
              verify: ["bun", "--version"],
              worktreePath: worktree,
            }),
          )
          if (!spawn.ok) throw new Error(`spawn failed: ${spawn.error}`)
          log("spawned sessionRef", spawn.sessionRef)

          // 2) POLL + PERSIST — fetch node-state, run it through the desktop's
          //    poll-time transcript merge (persist tail keyed by sessionRef).
          log("STEP 2 — poll + persist the event tail (persistAndMergeTranscripts)")
          let merged = null as Awaited<ReturnType<typeof fetchNodeState>> | null
          for (let attempt = 0; attempt < 60; attempt += 1) {
            const node = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
            merged = persistAndMergeTranscripts(desktopNodeHome, node)
            const session = merged.sessions.find((s) => s.sessionRef === spawn.sessionRef)
            if (session?.state === "completed" || session?.state === "failed") break
            yield* Effect.sleep("25 millis")
          }
          if (merged === null) throw new Error("no node state polled")
          const liveEvents = merged.events?.[spawn.sessionRef] ?? []
          log("persisted event count", liveEvents.length)

          // 3) STRUCTURED DIFF — render the ChangeSet the shared DiffReview shows.
          log("STEP 3 — structured diff from the event tail (parseChangeSetFromEvents)")
          const changeSet = parseChangeSetFromEvents(liveEvents)
          for (const file of changeSet.files) {
            log(`diff file`, `${file.status} ${file.path} (+${file.added} −${file.removed})`)
          }
          log("diff summary", changeSet.summary)
          if (changeSet.files.length < 2) throw new Error("diff did not render both edited files")
          const health = changeSet.files.find((f) => f.path === "src/health.ts")
          if (!health || health.added !== 12) throw new Error("structured +/- counts not parsed")

          // 4) SIMULATED RESTART — a fresh control server (empty in-memory tail)
          //    + an empty session.list. The persisted transcript must reload.
          log("STEP 4 — simulate a node restart (fresh server, empty session.list)")
          const freshServer = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({ executor: editExecutor, proofsDir: proofDir, summary }),
            },
            port: 0,
          })
          const afterRestart = yield* Effect.promise(() =>
            fetchNodeState({ baseUrl: freshServer.url, token: TOKEN }),
          )
          log("session.list after restart (in-memory tail gone)", afterRestart.sessions.length)
          const reloaded = persistAndMergeTranscripts(desktopNodeHome, afterRestart)
          const history = reloaded.sessions.find((s) => s.sessionRef === spawn.sessionRef)
          log("transcript re-surfaced as history row", history ? `${history.agentKind} · ${history.state}` : "(none)")
          const reloadedEvents = reloaded.events?.[spawn.sessionRef] ?? []
          log("reloaded transcript event count", reloadedEvents.length)
          if (!history) throw new Error("transcript did not reload after restart")
          if (reloadedEvents.length !== liveEvents.length) {
            throw new Error("reloaded transcript lost events across restart")
          }
          // The diff still renders from the reloaded transcript.
          const reloadedDiff = parseChangeSetFromEvents(reloadedEvents)
          if (reloadedDiff.files.length !== changeSet.files.length) {
            throw new Error("diff did not reload from the persisted transcript")
          }
          log("diff reloads from persisted transcript", reloadedDiff.summary)

          log("PROOF COMPLETE — diff renders + transcript survives a node restart on the real control protocol")
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
