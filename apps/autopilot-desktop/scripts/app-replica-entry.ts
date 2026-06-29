// Headless app-replica ENTRY (browser bundle).
//
// This boots the REAL desktop renderer — the same `Model` / `view` / `update` /
// `subscriptions` + the real Foldkit `Runtime.run` mount the live app uses — in
// headless Chromium, WITHOUT the Electrobun native shell. Two things that
// historically made this impossible are solved here:
//
//   1. Component styles. The old compile-plugin style path was removed; the
//      replica serves the same generated stylesheet as the packaged app,
//      including the central `--oa-*` token block and shared component classes.
//      No runtime shim, no compile-time style plugin, no throw — the real styled
//      view mounts.
//
//   2. The Electrobun bridge. The live app talks to the Bun main process over the
//      typed RPC (`window.bun` / `getRequest()` / `rpc`). In a plain browser that
//      surface is absent, so `khalaTurn`, `shellTurn`, token resolution, etc.
//      would reject. We install a TEST-CONTROLLED stub via the SAME `setRequest`
//      seam the live `main.ts` uses (bridge.ts), so the real Commands reach a
//      scripted fake instead of the network. The stub also drives the inbound
//      `khalaToken` push (the Bun→webview live-token channel) through the SAME
//      `pushInbound` the live `main.ts` handler uses, so streaming is faithful.
//
// The driver (CDP) dispatches REAL DOM keydown / mouse events at the page, so a
// keypress goes through the real keyboard subscription → forward gate →
// interpretKey → reducer → re-render, exactly like the live app. Nothing here
// shortcuts the input path.

import { Runtime } from "foldkit"
import { html } from "foldkit/html"
import type { Document } from "foldkit/html"

import { initialRuntimeState } from "../src/ui/initial-state.js"
import { Model, type Model as ModelType } from "../src/ui/model.js"
import { subscriptions } from "../src/ui/subscriptions.js"
import { update } from "../src/ui/update.js"
import { view } from "../src/ui/view.js"
import {
  pushInbound,
  setRequest,
  type DesktopRequests,
} from "../src/ui/bridge.js"
import { GotVerseKhalaToken } from "../src/ui/message.js"
import { latestVerseLocalPose } from "../src/ui/verse-local-pose.js"
import type { KhalaTurnResponse } from "../src/shared/rpc.js"

// ── Test-controlled bridge state ──────────────────────────────────────────────
//
// The harness installs a canned khala-turn script (deltas + terminal text +
// receipt) before driving the input. The stub replays it: it streams each delta
// through the inbound `khalaToken` push (correlated by turnId), then resolves the
// RPC with the terminal answer. Default behaviour with no script is an honest
// empty answer (so a turn never hangs).

type KhalaScript = Readonly<{
  // The streamed token deltas pushed via `GotVerseKhalaToken` (the live channel).
  deltas: ReadonlyArray<string>
  // The terminal RPC answer text (what `khalaTurn` resolves with).
  text: string
  ok?: boolean
  live?: boolean
  receipt?: unknown
  // When true, the RPC promise resolves BEFORE the streamed deltas are pushed —
  // reproducing the live race where the terminal answer lands first and late
  // deltas then append on top of it (the double-render bug).
  resolveBeforeStream?: boolean
}>

const replicaState: {
  khala: KhalaScript | null
  // Every method the stub bridge saw, for assertions / debugging.
  calls: Array<{ method: string; payload: unknown }>
} = { khala: null, calls: [] }

const record = (method: string, payload: unknown): void => {
  replicaState.calls.push({ method, payload })
}

// Push the scripted streamed deltas through the SAME inbound channel the live
// `main.ts` khalaToken handler uses. Returns a promise that resolves once all
// deltas have been pushed (each on its own microtask so the runtime can render
// between them, like real streaming frames).
const streamKhalaDeltas = async (
  turnId: string,
  deltas: ReadonlyArray<string>,
): Promise<void> => {
  for (const delta of deltas) {
    pushInbound(GotVerseKhalaToken({ turnId, delta }))
    // Yield so the Foldkit runtime processes + renders this delta before the next.
    await new Promise((resolve) => setTimeout(resolve, 0))
  }
}

const stubKhalaTurn = async (p: {
  prompt: string
  model?: string
  turnId?: string
}): Promise<KhalaTurnResponse> => {
  record("khalaTurn", p)
  const script = replicaState.khala
  const turnId = p.turnId ?? "verse.khala.replica"
  if (script === null) {
    return { ok: true, text: "", receipt: null, live: false }
  }
  const terminal: KhalaTurnResponse = {
    ok: script.ok ?? true,
    text: script.text,
    receipt: (script.receipt ?? null) as KhalaTurnResponse["receipt"],
    live: script.live ?? false,
  }
  if (script.resolveBeforeStream === true) {
    // RACE REPRODUCTION: terminal answer lands first, then late deltas stream.
    // We schedule the deltas AFTER this promise resolves (next macrotask) so the
    // reducer sees RespondedVerseKhala before the trailing GotVerseKhalaToken
    // messages — exactly the ordering that doubles the answer when the delta
    // handler keeps appending after the turn settled.
    setTimeout(() => {
      void streamKhalaDeltas(turnId, script.deltas)
    }, 0)
    return terminal
  }
  // Normal streaming-first ordering: stream all deltas, THEN resolve.
  await streamKhalaDeltas(turnId, script.deltas)
  return terminal
}

// A minimal honest stub for every RPC verb the real Commands may reach. Verbs
// the replica scenarios exercise return scripted/canned values; the rest return
// honest "not configured / empty" shapes so nothing hangs.
const notConfigured = { ok: false } as const

// Most load/projection commands wrap their RPC in `Effect.catch` and synthesize a
// correctly-shaped DEGRADED projection on failure. Rather than hand-build every
// projection shape here (and risk a malformed `{}` crashing a reducer that reads
// nested fields), the stub REJECTS those verbs so each command falls back to its
// own well-typed degraded value — the same path a real offline node takes.
const degraded = (method: string): Promise<never> => {
  record(method, null)
  return Promise.reject(new Error(`replica-stub: ${method} unavailable`))
}

const stubRequest: DesktopRequests = {
  openExternal: async (p) => {
    record("openExternal", p)
    return { ok: true }
  },
  deployCloud: async (p) => {
    record("deployCloud", p)
    return { accepted: false, reason: "replica-stub", errors: [] }
  },
  submitIntent: async (p) => {
    record("submitIntent", p)
    return { ok: true, status: "queued" }
  },
  builtinAgentReadiness: () => degraded("builtinAgentReadiness"),
  startBuiltInAgent: async () => ({ ...notConfigured }) as never,
  appleFmReadiness: () => degraded("appleFmReadiness"),
  startAppleFmSession: async () => ({ ...notConfigured }) as never,
  inferenceGatewayReadiness: () => degraded("inferenceGatewayReadiness"),
  shellTurn: async (p) => {
    record("shellTurn", p)
    return { ok: true, text: "" } as never
  },
  verseTurn: async (p) => {
    record("verseTurn", p)
    return { ok: true, text: "" } as never
  },
  khalaTurn: stubKhalaTurn,
  installReadiness: () => degraded("installReadiness"),
  onboardingStatus: () => degraded("onboardingStatus"),
  identityChoiceState: () => degraded("identityChoiceState"),
  chooseIdentity: async (p) => {
    record("chooseIdentity", p)
    return degraded("chooseIdentity")
  },
  promiseSurfacingReadiness: () => degraded("promiseSurfacingReadiness"),
  surfacePromiseGap: async (p) => {
    record("surfacePromiseGap", p)
    return degraded("surfacePromiseGap")
  },
  listTrainingRuns: () => degraded("listTrainingRuns"),
  listTrainingDashboard: () => degraded("listTrainingDashboard"),
  listTrainingPromiseGates: () => degraded("listTrainingPromiseGates"),
  listTrainingOperatorReadiness: () => degraded("listTrainingOperatorReadiness"),
  listTrainingEvidencePacketSummary: () =>
    degraded("listTrainingEvidencePacketSummary"),
  listPublicActivityTimeline: () => degraded("listPublicActivityTimeline"),
  buildTrainingEvidencePacket: (p) => {
    record("buildTrainingEvidencePacket", p)
    return degraded("buildTrainingEvidencePacket")
  },
  planTrainingRunWindow: () => degraded("planTrainingRunWindow"),
  activateTrainingWindow: (p) => {
    record("activateTrainingWindow", p)
    return degraded("activateTrainingWindow")
  },
  reconcileTrainingWindow: (p) => {
    record("reconcileTrainingWindow", p)
    return degraded("reconcileTrainingWindow")
  },
  claimTrainingWindowLease: () => degraded("claimTrainingWindowLease"),
  requestTrainingBootstrapGrant: (p) => {
    record("requestTrainingBootstrapGrant", p)
    return degraded("requestTrainingBootstrapGrant")
  },
  admitTrainingRealGradientEvidence: (p) => {
    record("admitTrainingRealGradientEvidence", p)
    return degraded("admitTrainingRealGradientEvidence")
  },
  resolveApproval: async (p) => {
    record("resolveApproval", p)
    return { applied: true, duplicate: false, decision: p.decision }
  },
  setCoordinatorPaused: async (p) => {
    record("setCoordinatorPaused", p)
    return { paused: p.paused }
  },
  cancelSession: async (p) => {
    record("cancelSession", p)
    return { ok: true, state: "cancelled" }
  },
  spawnSession: async (p) => {
    record("spawnSession", p)
    return { ok: true, sessionRef: "replica-session" }
  },
  resolveManagedWorktree: async (p) => {
    record("resolveManagedWorktree", p)
    return { ok: false, error: "replica-stub" }
  },
  spawnAppleFmSession: async (p) => {
    record("spawnAppleFmSession", p)
    return ({ ...notConfigured }) as never
  },
  listManagedAccounts: () => degraded("listManagedAccounts"),
  addManagedAccount: (p) => {
    record("addManagedAccount", p)
    return degraded("addManagedAccount")
  },
  removeManagedAccount: (p) => {
    record("removeManagedAccount", p)
    return degraded("removeManagedAccount")
  },
  setManagedAccountPriority: (p) => {
    record("setManagedAccountPriority", p)
    return degraded("setManagedAccountPriority")
  },
}

// ── Driver-facing control surface (read by the CDP driver) ────────────────────
declare global {
  interface Window {
    __OA_REPLICA__?: {
      // The replica has mounted the real Foldkit program.
      ready: boolean
      // Script the next khala turn (deltas + terminal text + ordering).
      scriptKhala: (script: KhalaScript) => void
      // Every RPC verb the stub bridge has seen (for assertions).
      calls: () => ReadonlyArray<{ method: string; payload: unknown }>
      // A read-only snapshot of dynamic Verse state the dynamic tests assert on:
      // how many isolated scenes are spawned, and the live avatar restore pose
      // (updated every frame by the host pose callback). Stashed by a thin update
      // wrapper so the tests never reach into Foldkit internals.
      verseState: () => {
        spawnedSceneCount: number
        spawnedSceneIds: ReadonlyArray<string>
        spawnedPortalCount: number
        gameScreenActive: boolean
        avatarPose: {
          x: number
          y: number
          z: number
          yaw: number
        } | null
      }
    }
    __OA_REPLICA_CRASH__?: string
  }
}

setRequest(stubRequest)

// Latest model, stashed by a thin update wrapper so the dynamic tests can read
// spawn state + the live avatar pose without reaching into Foldkit internals.
let latestModel: ModelType | null = null
const recordingUpdate: typeof update = (model, message) => {
  const result = update(model, message)
  latestModel = Array.isArray(result) ? result[0] : result
  return result
}

// NOT frozen — `start()` flips `ready` to true once the real program mounts so
// the CDP driver can wait deterministically.
window.__OA_REPLICA__ = {
  ready: false,
  scriptKhala: (script: KhalaScript) => {
    replicaState.khala = script
  },
  calls: () => [...replicaState.calls],
  verseState: () => {
    const m = latestModel
    const scenes =
      m === null
        ? []
        : (m.verseSpawnedScenes as ReadonlyArray<{
            sceneId: string
            showPortal: boolean
          }>)
    // The LIVE avatar pose (updated every frame by the host pose callback into
    // the pose cache) — what the dynamic movement test needs. Falls back to the
    // model's restore pose before the first frame pose lands.
    const pose = latestVerseLocalPose() ?? m?.verseSceneRestorePose ?? null
    return {
      spawnedSceneCount: scenes.length,
      spawnedSceneIds: scenes.map((s) => s.sceneId),
      spawnedPortalCount: scenes.filter((s) => s.showPortal).length,
      // M8: whether the in-world Khala crossy-road arcade screen is toggled on.
      gameScreenActive:
        m === null ? false : (m as { verseGameScreenActive?: boolean }).verseGameScreenActive === true,
      avatarPose:
        pose === null
          ? null
          : { x: pose.x, y: pose.y, z: pose.z, yaw: pose.yaw },
    }
  },
}

// Crash overlay: render a `data-replica-crash` <pre> with the error + stack so a
// view/update throw (e.g. an unsolved style/bridge issue) is VISIBLE to the
// driver instead of a silent blank screen. The harness fails hard if this exists.
const ch = html<never>()
const crashView = (error: Error): Document => ({
  title: "replica-crash",
  body: ch.pre(
    [ch.DataAttribute("replica-crash", "true"), ch.Id("replica-crash")],
    [`${error.message}\n${error.stack ?? ""}`],
  ),
})

// The real Foldkit program — the SAME Model / init / update / view /
// subscriptions the live `main.ts` runs, mounted into #root.
const start = (): void => {
  Runtime.run(
    Runtime.makeProgram<ModelType, import("../src/ui/message.js").Message>({
      Model,
      init: initialRuntimeState,
      update: recordingUpdate,
      view,
      subscriptions,
      container: document.getElementById("root"),
      crash: {
        view: ({ error }) => crashView(error),
        report: ({ error }) => {
          window.__OA_REPLICA_CRASH__ = `${error.message}\n${error.stack ?? ""}`
          console.error("[app-replica] crash:", error)
        },
      },
    }),
  )
  // Signal readiness so the driver can wait deterministically.
  if (window.__OA_REPLICA__ !== undefined) window.__OA_REPLICA__.ready = true
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start)
} else {
  start()
}
