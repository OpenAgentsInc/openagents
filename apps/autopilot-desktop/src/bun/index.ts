import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { BrowserView, BrowserWindow } from "electrobun/bun"
import { discoverPylonHome } from "./node-home"
import {
  type NodeLaunchStatus,
  type SupervisedNode,
  superviseManagedNode,
} from "./node-launcher"
import { createNodeStatePoller } from "./node-state-poll"
import { createSessionNotifier } from "./notifier"
import { raiseOsNotification } from "./os-notification"
import {
  cancelSession,
  deployToCloud,
  fetchNodeState,
  readControlToken,
  resolveApproval,
  setCoordinatorPaused,
  spawnSession,
  submitIntent,
} from "./pylon-control"
import {
  activateTrainingWindow,
  admitTrainingRealGradientEvidence,
  buildTrainingEvidencePacket,
  claimTrainingWindowLease,
  fetchTrainingDashboard,
  fetchTrainingPromiseGates,
  fetchTrainingRuns,
  planTrainingRunWindow,
  readTrainingEvidencePacketSummary,
  reconcileTrainingWindow,
  requestTrainingBootstrapGrant,
} from "./training-runs"
import type {
  DesktopRPCSchema,
  TrainingOperatorReadinessPylonRefSource,
  TrainingOperatorReadinessResponse,
} from "../shared/rpc"

const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pollIntervalMs = Number(Bun.env.AUTOPILOT_DESKTOP_NODE_POLL_MS ?? "2000")
const trainingBaseUrl =
  Bun.env.OPENAGENTS_TRAINING_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  "https://openagents.com"
const trainingAdminToken =
  Bun.env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN ??
  Bun.env.OPENAGENTS_ADMIN_API_TOKEN ??
  null
const trainingAdminEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE === "1"
const trainingLeaseEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE === "1"
const trainingEvidenceEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE === "1"
const trainingEvidenceWriteEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE === "1"
const trainingEvidencePacketPath =
  Bun.env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH ?? null
const configuredTrainingWorkerReceiptsPath =
  Bun.env.OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH ?? null
const configuredTrainingPylonRef =
  Bun.env.OPENAGENTS_TRAINING_PYLON_REF ??
  Bun.env.PYLON_REF ??
  null
const trainingWorkerReceiptsFilename = "training-worker-receipts.json"

// CL-45: resolve the node home once per call so a node that starts after the app
// (or rotates its home) is picked up without a restart. Falls back to the env
// default. `readControlToken` returns null when the chosen home has no token,
// which the poll surfaces as an honest offline state.
function resolveHome(): string | null {
  return discoverPylonHome({ env: Bun.env.PYLON_HOME, cwd: process.cwd() })
}

function tokenForCommand(): string | null {
  const home = resolveHome()
  return home === null ? null : readControlToken(home)
}

function readIdentityPylonRef(home: string): string | null {
  const path = join(home, "identity.json")
  if (!existsSync(path)) return null
  try {
    const record = JSON.parse(readFileSync(path, "utf8")) as {
      pylonRef?: unknown
    }
    return typeof record.pylonRef === "string" && record.pylonRef.length > 0
      ? record.pylonRef
      : null
  } catch {
    return null
  }
}

function trainingPylonRefStatus(home = resolveHome()): {
  readonly pylonRef: string | null
  readonly source: TrainingOperatorReadinessPylonRefSource
} {
  const configured = configuredTrainingPylonRef?.trim() ?? ""
  if (configured.length > 0) {
    return { pylonRef: configured, source: "env" }
  }
  if (home === null) {
    return { pylonRef: null, source: "missing" }
  }
  const identityRef = readIdentityPylonRef(home)
  return identityRef === null
    ? { pylonRef: null, source: "missing" }
    : { pylonRef: identityRef, source: "identity" }
}

function trainingPylonRefForCommand(): string | null {
  return trainingPylonRefStatus().pylonRef
}

function trainingWorkerReceiptsPathForCommand(home = resolveHome()): string | null {
  const configured = configuredTrainingWorkerReceiptsPath?.trim() ?? ""
  if (configured.length > 0) return configured
  return home === null ? null : join(home, trainingWorkerReceiptsFilename)
}

function trainingOperatorReadinessProjection(): TrainingOperatorReadinessResponse {
  const home = resolveHome()
  const controlToken = home === null ? null : readControlToken(home)
  const pylon = trainingPylonRefStatus(home)
  const adminTokenPresent = (trainingAdminToken?.trim() ?? "").length > 0
  const adminReady = trainingAdminEnabled && adminTokenPresent
  const leaseReady = trainingLeaseEnabled && pylon.pylonRef !== null
  const evidencePacketPathPresent =
    (trainingEvidencePacketPath?.trim() ?? "").length > 0
  const evidenceReady =
    trainingEvidenceEnabled && adminTokenPresent && evidencePacketPathPresent
  const localPylonReady = home !== null && controlToken !== null
  const blockerRefs: string[] = []

  if (!trainingAdminEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE")
  }
  if (!adminTokenPresent) {
    blockerRefs.push("env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN")
  }
  if (!trainingLeaseEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE")
  }
  if (pylon.pylonRef === null) {
    blockerRefs.push("pylon.identity.pylonRef")
  }
  if (home === null) {
    blockerRefs.push("pylon.home")
  }
  if (controlToken === null) {
    blockerRefs.push("pylon.control_token")
  }
  if (!trainingEvidenceEnabled) {
    blockerRefs.push("env.OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE")
  }
  if (!evidencePacketPathPresent) {
    blockerRefs.push("env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH")
  }

  return {
    ok: blockerRefs.length === 0,
    fetchedAt: new Date().toISOString(),
    sourceUrl: "desktop:training-operator-readiness",
    trainingBaseUrl,
    adminEnabled: trainingAdminEnabled,
    adminTokenPresent,
    adminReady,
    leaseEnabled: trainingLeaseEnabled,
    leaseReady,
    pylonRefPresent: pylon.pylonRef !== null,
    pylonRefSource: pylon.source,
    pylonRef: pylon.pylonRef,
    pylonHomePresent: home !== null,
    controlTokenPresent: controlToken !== null,
    localPylonReady,
    evidenceEnabled: trainingEvidenceEnabled,
    evidencePacketPathPresent,
    evidenceReady,
    blockerRefs,
  }
}

const rpc = BrowserView.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {
      // CL-26: the webview's "Deploy to Cloud" button routes here. We read the
      // node's control token and forward the gated deploy.cloud command — the
      // node enforces the OA_DEPLOY_ENABLE=1 fail-safe, so nothing deploys by
      // default.
      async deployCloud(params) {
        const token = tokenForCommand()
        if (token === null) {
          return { accepted: false, reason: "control token unavailable", errors: [] }
        }
        return deployToCloud({
          baseUrl: controlBaseUrl,
          token,
          target: params.target,
          ref: params.ref,
          ...(params.env ? { env: params.env } : {}),
        })
      },
      // CL-47: submit an "ask" (work intent) to the node.
      async submitIntent(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, status: "error", error: "control token unavailable" }
        return submitIntent({ baseUrl: controlBaseUrl, token, title: params.title, body: params.body })
      },
      async listTrainingRuns() {
        return fetchTrainingRuns({ baseUrl: trainingBaseUrl })
      },
      async listTrainingDashboard() {
        return fetchTrainingDashboard({ baseUrl: trainingBaseUrl })
      },
      async listTrainingPromiseGates() {
        return fetchTrainingPromiseGates({ baseUrl: trainingBaseUrl })
      },
      async listTrainingOperatorReadiness() {
        return trainingOperatorReadinessProjection()
      },
      async listTrainingEvidencePacketSummary() {
        return readTrainingEvidencePacketSummary({
          evidencePacketPath: trainingEvidencePacketPath,
        })
      },
      async buildTrainingEvidencePacket(params) {
        return buildTrainingEvidencePacket({
          enabled: trainingEvidenceWriteEnabled,
          evidencePacketPath: trainingEvidencePacketPath,
          trainingRunRef: params.trainingRunRef,
          workerReceiptsPath: trainingWorkerReceiptsPathForCommand(),
        })
      },
      async planTrainingRunWindow() {
        return planTrainingRunWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
        })
      },
      async activateTrainingWindow(params) {
        return activateTrainingWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
          windowRef: params.windowRef,
        })
      },
      async reconcileTrainingWindow(params) {
        return reconcileTrainingWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
          windowRef: params.windowRef,
        })
      },
      async claimTrainingWindowLease() {
        return claimTrainingWindowLease({
          baseUrl: trainingBaseUrl,
          enabled: trainingLeaseEnabled,
          pylonRef: trainingPylonRefForCommand(),
        })
      },
      async requestTrainingBootstrapGrant(params) {
        return requestTrainingBootstrapGrant({
          baseUrl: trainingBaseUrl,
          pylonRef: trainingPylonRefForCommand(),
          trainingRunRef: params.trainingRunRef,
        })
      },
      async admitTrainingRealGradientEvidence(params) {
        return admitTrainingRealGradientEvidence({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingEvidenceEnabled,
          evidencePacketPath: trainingEvidencePacketPath,
          trainingRunRef: params.trainingRunRef,
        })
      },
      // CL-48: resolve a pending approval (approve/deny).
      async resolveApproval(params) {
        const token = tokenForCommand()
        if (token === null) return { applied: false, duplicate: false, decision: params.decision }
        return resolveApproval({ baseUrl: controlBaseUrl, token, approvalRef: params.approvalRef, decision: params.decision })
      },
      // CL-51: pause/resume the node's autonomous coordinator loop.
      async setCoordinatorPaused(params) {
        const token = tokenForCommand()
        if (token === null) return { paused: params.paused }
        return setCoordinatorPaused({ baseUrl: controlBaseUrl, token, paused: params.paused })
      },
      // CL-52: cancel a running/queued session.
      async cancelSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, state: "error" }
        return cancelSession({ baseUrl: controlBaseUrl, token, sessionRef: params.sessionRef })
      },
      // CL-57: directly spawn a bounded session on the node.
      async spawnSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, sessionRef: "", error: "control token unavailable" }
        return spawnSession({
          baseUrl: controlBaseUrl,
          token,
          adapter: params.adapter,
          objective: params.objective,
          ...(params.verify ? { verify: params.verify } : {}),
          // #4998: thread the requested execution lane through to the node.
          ...(params.lane ? { lane: params.lane } : {}),
        })
      },
    },
    messages: {},
  },
})

const window = new BrowserWindow({
  title: "Autopilot Desktop",
  url: "views://autopilot-desktop/index.html",
  // Open large by default (Electrobun defaults to 800×600, too small for the
  // sidebar + dashboard). A roomy 1400×900 window with a small offset.
  frame: { x: 80, y: 60, width: 1400, height: 900 },
  rpc,
})

// CL-30: each poll, fold the session list into the notifier so a session that
// newly enters a notify-worthy state (needs_decision / failed / completed)
// raises a native OS notification and updates the in-app notification center.
const notifier = createSessionNotifier({ raise: raiseOsNotification })

// #5011 (the §0 install seam): adopt an already-running local node, or launch
// the local Pylon runtime when none is discovered, so a fresh install reaches
// node-online without a separate setup step. The launched node lands in a
// `.pylon-local` home that `discoverPylonHome` already scans, so `resolveHome`
// and the poller below pick it up unchanged. The supervisor keeps a *launched*
// child alive (restart-on-crash with backoff), surfaces honest
// launching/online/failed status, and stops the child on app close. An adopted
// (already-running) node is never double-spawned and never killed by us; an
// unavailable result (packaged build, Phase 2) leaves the app honest-offline.
//
// NOTE (UI follow-up): the launch status is logged Bun-side here; surfacing the
// launching/online/failed badge in the webview needs a new Bun→webview message
// (e.g. `nodeLaunchStatus`) wired into the Foldkit node-status projection. That
// touches src/shared/rpc.ts + src/ui/* (owned by another worker right now), so
// it is intentionally deferred — the live node-state poll still drives the
// honest online/offline projection in the meantime.
let managedNode: SupervisedNode | null = null
managedNode = superviseManagedNode({
  cwd: process.cwd(),
  env: Bun.env,
  controlBaseUrl,
  onStatus(status: NodeLaunchStatus) {
    console.log(
      `[autopilot-desktop] local node status: ${status}` +
        (managedNode?.home() ? ` (home: ${managedNode.home()})` : ""),
    )
  },
})

const poller = createNodeStatePoller({
  intervalMs: Number.isFinite(pollIntervalMs) && pollIntervalMs > 0 ? pollIntervalMs : 2000,
  onState(message) {
    rpc.send.nodeState(message)
    rpc.send.notifications(notifier.ingest(message.sessions))
  },
  async fetchNodeState() {
    const token = tokenForCommand()
    if (token === null) throw new Error("Pylon control token is not available")
    return fetchNodeState({
      baseUrl: controlBaseUrl,
      token,
    })
  },
})

window.webview.on("dom-ready", () => {
  poller.start()
})

window.on("close", () => {
  poller.stop()
  // Stop supervising and kill only a node we launched ourselves; an adopted
  // node we did not start is left running, and a deliberate stop never triggers
  // a restart.
  managedNode?.stop()
})
