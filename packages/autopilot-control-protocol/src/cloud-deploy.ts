import {
  buildDeployCloudRequest,
  type CloudSelection,
  type DeployCloudRequest,
} from "./cloud-client.js"

export type DeploySelection = "byo_key" | "credits"

export type DeployPlanInput = {
  objective: string
  selection: DeploySelection
  repoRef?: string
}

export type DeployPlan = {
  request: DeployCloudRequest
  summary: {
    selectionLabel: string
    requiresCredits: boolean
  }
}

export type DeployValidationInput = DeployPlanInput & {
  creditBalance?: number
}

export type DeployValidationResult = {
  ok: boolean
  errors: string[]
}

const SELECTION_LABELS: Record<DeploySelection, string> = {
  byo_key: "Bring your own key",
  credits: "OpenAgents credits",
}

export function buildDeployPlan(input: DeployPlanInput): DeployPlan {
  return {
    request: buildDeployCloudRequest({
      objective: input.objective,
      selection: input.selection satisfies CloudSelection,
    }),
    summary: {
      selectionLabel: SELECTION_LABELS[input.selection],
      requiresCredits: input.selection === "credits",
    },
  }
}

export function validateDeploy(input: DeployValidationInput): DeployValidationResult {
  const errors: string[] = []

  if (input.objective.trim().length === 0) {
    errors.push("Objective is required")
  }

  if (input.selection === "credits" && !(typeof input.creditBalance === "number" && input.creditBalance > 0)) {
    errors.push("Credits selection requires a positive credit balance")
  }

  return {
    ok: errors.length === 0,
    errors,
  }
}
