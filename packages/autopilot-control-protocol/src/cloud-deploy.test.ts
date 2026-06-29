import { describe, expect, test } from "bun:test"

import {
  buildDeployPlan,
  validateDeploy,
} from "./cloud-deploy.js"

describe("cloud deploy plan", () => {
  test("builds a byo-key deploy request and summary", () => {
    expect(buildDeployPlan({
      objective: "deploy the forum worker",
      selection: "byo_key",
      repoRef: "OpenAgentsInc/openagents#4932",
    })).toEqual({
      request: {
        type: "cloud.deploy",
        objective: "deploy the forum worker",
        selection: "byo_key",
      },
      summary: {
        selectionLabel: "Bring your own key",
        requiresCredits: false,
      },
    })
  })

  test("builds a credits deploy request and summary", () => {
    expect(buildDeployPlan({
      objective: "deploy the cloud core",
      selection: "credits",
    })).toEqual({
      request: {
        type: "cloud.deploy",
        objective: "deploy the cloud core",
        selection: "credits",
      },
      summary: {
        selectionLabel: "OpenAgents credits",
        requiresCredits: true,
      },
    })
  })
})

describe("cloud deploy validation", () => {
  test("rejects an empty objective", () => {
    expect(validateDeploy({
      objective: "   ",
      selection: "byo_key",
    })).toEqual({
      ok: false,
      errors: ["Objective is required"],
    })
  })

  test("rejects credits selection without a positive credit balance", () => {
    expect(validateDeploy({
      objective: "deploy with credits",
      selection: "credits",
      creditBalance: 0,
    })).toEqual({
      ok: false,
      errors: ["Credits selection requires a positive credit balance"],
    })
  })
})
