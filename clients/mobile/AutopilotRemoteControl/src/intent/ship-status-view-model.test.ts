import { describe, expect, test } from "bun:test"

import { shipStatusView, type IntentStatus } from "./ship-status-view-model"

const pipelineStatuses = ["received", "planning", "fanning_out", "shipping", "shipped"] as const satisfies readonly IntentStatus[]

describe("ship status view model", () => {
  test("maps each intent status to display tone and step", () => {
    expect(shipStatusView("received")).toEqual({
      label: "Received",
      tone: "info",
      stepIndex: 1,
      totalSteps: 5,
    })
    expect(shipStatusView("planning")).toEqual({
      label: "Planning",
      tone: "info",
      stepIndex: 2,
      totalSteps: 5,
    })
    expect(shipStatusView("fanning_out")).toEqual({
      label: "Fanning out",
      tone: "warning",
      stepIndex: 3,
      totalSteps: 5,
    })
    expect(shipStatusView("shipping")).toEqual({
      label: "Shipping",
      tone: "warning",
      stepIndex: 4,
      totalSteps: 5,
    })
    expect(shipStatusView("shipped")).toEqual({
      label: "Shipped",
      tone: "success",
      stepIndex: 5,
      totalSteps: 5,
    })
    expect(shipStatusView("failed")).toEqual({
      label: "Failed",
      tone: "danger",
      stepIndex: 5,
      totalSteps: 5,
    })
  })

  test("keeps pipeline step indexes monotonic", () => {
    const stepIndexes = pipelineStatuses.map((status) => shipStatusView(status).stepIndex)

    expect(stepIndexes).toEqual([1, 2, 3, 4, 5])
    for (let index = 1; index < stepIndexes.length; index += 1) {
      expect(stepIndexes[index]!).toBeGreaterThan(stepIndexes[index - 1]!)
    }
  })

  test("marks shipped as the successful terminal state", () => {
    const shipped = shipStatusView("shipped")

    expect(shipped.tone).toBe("success")
    expect(shipped.stepIndex).toBe(shipped.totalSteps)
  })

  test("marks failed as danger", () => {
    expect(shipStatusView("failed").tone).toBe("danger")
  })
})
