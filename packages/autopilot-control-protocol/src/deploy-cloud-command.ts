import { validateDeployRequest } from "./deploy-request-validate.js"

export type DeployCloudCommandInput = {
  target: "cloudrun" | "workers"
  ref: string
  env: "production" | "preview"
}

export type DeployCloudCommandResult = {
  ok: boolean
  command: string
  args: string[]
  reason: string
}

export function buildDeployCommand(input: DeployCloudCommandInput): DeployCloudCommandResult {
  const validation = validateDeployRequest(input)

  if (!validation.ok || validation.target === null) {
    return {
      ok: false,
      command: "",
      args: [],
      reason: validation.errors.join("; "),
    }
  }

  if (validation.target === "cloudrun") {
    return {
      ok: true,
      command: "deploy-cloudrun.sh",
      args: [validation.ref, "--env", validation.env],
      reason: "deploy command ready",
    }
  }

  return {
    ok: true,
    command: "wrangler",
    args: ["deploy", "--env", validation.env],
    reason: "deploy command ready",
  }
}
