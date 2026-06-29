// CL-26 "Deploy to Cloud": a node-triggered deploy through OUR cloud pipeline
// (Cloud Run / Workers), surfaced as a button + status on the desktop and
// mobile clients. The deployable target is one of the node's OWN cloud services
// via the existing deploy scripts (e.g. apps/oa-updates/scripts/deploy-cloudrun.sh).
//
// This is NOT a "deploy the user's project" flow. It is a fail-safe operator
// action on the node's own services:
//   - validation/command-building reuse the shared cores from
//     @openagentsinc/autopilot-control-protocol (validateDeployRequest,
//     buildDeployCommand, projectDeployStatus) — never reimplemented here.
//   - execution is gated behind OA_DEPLOY_ENABLE=1. When unset, the command is
//     accepted=false / reason=deploy_disabled and NOTHING runs. Read-only-safe.

import {
  buildDeployCommand,
  projectDeployStatus,
  validateDeployRequest,
  type DeployRequestEnv,
  type DeployRequestTarget,
  type DeployStatusView,
} from "@openagentsinc/autopilot-control-protocol"

export type DeployCloudInput = {
  target: unknown
  ref: unknown
  env?: unknown
}

export type DeployCloudResult = {
  accepted: boolean
  reason: string
  errors?: string[]
  target?: DeployRequestTarget
  ref?: string
  env?: DeployRequestEnv
  startedAt?: string
}

// In-memory "last deploy" record — a single most-recent attempt per node, used
// by deploy.status to project a DeployStatusView. There is no spend authority
// and no persistence: a node restart clears it.
export type LastDeploy = {
  target: DeployRequestTarget
  ref: string
  env: DeployRequestEnv
  state: "queued"
  startedAt: string
}

// Minimal shape of a fire-and-forget spawn. We never read stdout/stderr; the
// command is launched and we move on. Bun.spawn satisfies this; tests inject a
// recording stub so they never actually deploy.
export type DeploySpawn = (command: string, args: string[]) => void

export interface DeployCloudActions {
  deployCloud: (input: DeployCloudInput) => Promise<DeployCloudResult>
  deployStatus: () => Promise<DeployStatusView>
}

export interface DeployCloudActionsOptions {
  // Returns whether execution is enabled. Defaults to reading OA_DEPLOY_ENABLE.
  // Injectable so tests can flip the gate without touching the real env.
  isEnabled?: () => boolean
  // Fire-and-forget spawn. Defaults to Bun.spawn with stdout/stderr ignored.
  // Injectable so tests assert it is/ isn't called.
  spawn?: DeploySpawn
  now?: () => Date
}

function defaultIsEnabled(): boolean {
  return Bun.env.OA_DEPLOY_ENABLE === "1"
}

function defaultSpawn(command: string, args: string[]): void {
  // Fire-and-forget: stdout/stderr ignored. We do NOT await the child; the
  // node records "queued" and surfaces it via deploy.status.
  Bun.spawn([command, ...args], { stdout: "ignore", stderr: "ignore", stdin: "ignore" })
}

export function createDeployCloudActions(options: DeployCloudActionsOptions = {}): DeployCloudActions {
  const isEnabled = options.isEnabled ?? defaultIsEnabled
  const spawn = options.spawn ?? defaultSpawn
  const now = options.now ?? (() => new Date())

  let lastDeploy: LastDeploy | null = null

  return {
    deployCloud: async (input: DeployCloudInput): Promise<DeployCloudResult> => {
      const validation = validateDeployRequest({ target: input.target, ref: input.ref, env: input.env })
      if (!validation.ok || validation.target === null) {
        return { accepted: false, reason: "invalid_request", errors: validation.errors }
      }

      // Fail-safe gate: when OA_DEPLOY_ENABLE is not "1", we never run anything.
      if (!isEnabled()) {
        return { accepted: false, reason: "deploy_disabled" }
      }

      const built = buildDeployCommand({
        target: validation.target,
        ref: validation.ref,
        env: validation.env,
      })
      if (!built.ok) {
        return { accepted: false, reason: "invalid_request", errors: [built.reason] }
      }

      const startedAt = now().toISOString()
      // Fire-and-forget: we ignore the child entirely (stdout/stderr ignored).
      spawn(built.command, built.args)

      lastDeploy = {
        target: validation.target,
        ref: validation.ref,
        env: validation.env,
        state: "queued",
        startedAt,
      }

      return {
        accepted: true,
        reason: "deploy command ready",
        target: validation.target,
        ref: validation.ref,
        env: validation.env,
        startedAt,
      }
    },

    deployStatus: async (): Promise<DeployStatusView> => {
      // projectDeployStatus is tolerant of an empty/unknown payload — a fresh
      // node with no deploy reports state "unknown" / status unavailable.
      return projectDeployStatus(lastDeploy)
    },
  }
}
