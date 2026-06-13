import { createHash } from "node:crypto"

import { describeExecutionProvider } from "./execution-provider"

export type CloudSessionRequest = {
  providerKind: "openagents_cloud"
  objectiveRef: string
  verify: string[]
  workspaceRef: string
  timeoutSeconds: number
}

export type CloudSessionLease = {
  leaseRef: string
  state: "requested" | "ready" | "released"
}

export function buildCloudSessionRequest(input: {
  objective: string
  verify: string[]
  workspaceRef: string
  timeoutSeconds?: number
}): CloudSessionRequest {
  const provider = describeExecutionProvider("openagents_cloud")

  if (!provider.features.remoteRun) {
    throw new Error("OpenAgents Cloud execution provider cannot run remotely")
  }

  const objectiveHash = createHash("sha256")
    .update(input.objective)
    .digest("hex")
    .slice(0, 16)

  return {
    providerKind: "openagents_cloud",
    objectiveRef: `objective.${objectiveHash}`,
    verify: input.verify,
    workspaceRef: input.workspaceRef,
    timeoutSeconds: Math.min(
      1200,
      Math.max(1, input.timeoutSeconds ?? 600),
    ),
  }
}
