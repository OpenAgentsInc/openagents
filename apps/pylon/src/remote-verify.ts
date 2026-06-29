import { createHash } from "node:crypto"

import { describeExecutionProvider } from "./execution-provider.js"
import type { SshTarget } from "./ssh-target.js"

export type RemoteVerifyStep = {
  step: "materialize" | "sync" | "run" | "collect_artifacts" | "release"
  detailRef: string
}

export function planRemoteVerify(input: {
  target: SshTarget
  verify: string[]
  requiredArtifacts?: string[]
}): {
  providerKind: "static_ssh"
  steps: RemoteVerifyStep[]
  verifyRef: string
} {
  const provider = describeExecutionProvider("static_ssh")

  if (!provider.features.remoteRun) {
    throw new Error("Static SSH execution provider does not support remote runs")
  }

  const verifyRef = `verify.${createHash("sha256")
    .update(input.verify.join(" "))
    .digest("hex")
    .slice(0, 16)}`

  const stepRefs: RemoteVerifyStep["step"][] = ["materialize", "sync", "run"]

  if ((input.requiredArtifacts ?? []).length > 0) {
    stepRefs.push("collect_artifacts")
  }

  stepRefs.push("release")

  return {
    providerKind: "static_ssh",
    steps: stepRefs.map((step) => ({
      step,
      detailRef: `remote_verify.${step}`,
    })),
    verifyRef,
  }
}
