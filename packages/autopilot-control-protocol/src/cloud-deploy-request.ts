import type { DeployPlan } from "./cloud-deploy.js"

type CloudDeployVerb = DeployPlan["request"]["type"]

export type CloudDeployRequestInput = {
  sessionRef: string
  artifactRef?: string
  repoRef?: string
  clientRequestId: string
}

export type CloudDeployRequest = {
  verb: CloudDeployVerb
  sessionRef: string
  artifactRef: string | null
  repoRef: string | null
  clientRequestId: string
  idempotencyKey: string
}

export type BuildCloudDeployRequestResult =
  | {
      ok: true
      request: CloudDeployRequest
    }
  | {
      ok: false
      error: string
    }

export function buildCloudDeployRequest(input: CloudDeployRequestInput): BuildCloudDeployRequestResult {
  const missingSessionRef = input.sessionRef.trim().length === 0
  const missingClientRequestId = input.clientRequestId.trim().length === 0

  if (missingSessionRef && missingClientRequestId) {
    return { ok: false, error: "sessionRef and clientRequestId are required" }
  }

  if (missingSessionRef) {
    return { ok: false, error: "sessionRef is required" }
  }

  if (missingClientRequestId) {
    return { ok: false, error: "clientRequestId is required" }
  }

  return {
    ok: true,
    request: {
      verb: "cloud.deploy",
      sessionRef: input.sessionRef,
      artifactRef: input.artifactRef ?? null,
      repoRef: input.repoRef ?? null,
      clientRequestId: input.clientRequestId,
      idempotencyKey: input.clientRequestId,
    },
  }
}
