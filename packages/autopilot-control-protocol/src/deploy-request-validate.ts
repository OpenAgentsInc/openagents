import type { DeployValidationResult } from "./cloud-deploy.js"

export type DeployRequestTarget = "cloudrun" | "workers"
export type DeployRequestEnv = "production" | "preview"

export type DeployRequestValidationResult = DeployValidationResult & {
  target: DeployRequestTarget | null
  ref: string
  env: DeployRequestEnv
}

const DEPLOY_REQUEST_TARGETS = new Set<DeployRequestTarget>(["cloudrun", "workers"])
const DEPLOY_REQUEST_ENVS = new Set<DeployRequestEnv>(["production", "preview"])

function parseTarget(target: unknown): DeployRequestTarget | null {
  if (typeof target !== "string") return null
  if (!DEPLOY_REQUEST_TARGETS.has(target as DeployRequestTarget)) return null
  return target as DeployRequestTarget
}

function parseEnv(env: unknown): DeployRequestEnv | null {
  if (env === undefined) return "preview"
  if (typeof env !== "string") return null
  if (!DEPLOY_REQUEST_ENVS.has(env as DeployRequestEnv)) return null
  return env as DeployRequestEnv
}

export function validateDeployRequest(input: {
  target: unknown
  ref: unknown
  env?: unknown
}): DeployRequestValidationResult {
  const errors: string[] = []
  const target = parseTarget(input.target)
  const ref = typeof input.ref === "string" ? input.ref.trim() : ""
  const parsedEnv = parseEnv(input.env)
  const env = parsedEnv ?? "preview"

  if (target === null) {
    errors.push("target must be cloudrun or workers")
  }

  if (ref.length === 0) {
    errors.push("ref is required")
  }

  if (parsedEnv === null) {
    errors.push("env must be production or preview")
  }

  return {
    ok: errors.length === 0,
    target,
    ref,
    env,
    errors,
  }
}
