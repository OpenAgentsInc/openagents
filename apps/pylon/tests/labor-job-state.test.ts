import { describe, expect, test } from "bun:test"

import { advanceLaborJob, type LaborJobState } from "../src/coordinator/labor-job-state"

describe("labor job state transitions", () => {
  test("accepts the live negotiated job happy path", () => {
    let state: LaborJobState = "quoted"

    let result = advanceLaborJob(state, "accept")
    expect(result).toEqual({
      next: "accepted",
      accepted: true,
      reason: "advanced quoted to accepted",
    })
    state = result.next

    result = advanceLaborJob(state, "start")
    expect(result).toEqual({
      next: "in_progress",
      accepted: true,
      reason: "advanced accepted to in_progress",
    })
    state = result.next

    result = advanceLaborJob(state, "deliver")
    expect(result).toEqual({
      next: "delivered",
      accepted: true,
      reason: "advanced in_progress to delivered",
    })
    state = result.next

    result = advanceLaborJob(state, "settle")
    expect(result).toEqual({
      next: "settled",
      accepted: true,
      reason: "advanced delivered to settled",
    })
  })

  test("cancels quoted jobs", () => {
    expect(advanceLaborJob("quoted", "cancel")).toEqual({
      next: "cancelled",
      accepted: true,
      reason: "cancelled quoted labor job",
    })
  })

  test("cancels accepted jobs", () => {
    expect(advanceLaborJob("accepted", "cancel")).toEqual({
      next: "cancelled",
      accepted: true,
      reason: "cancelled accepted labor job",
    })
  })

  test("cancels in-progress jobs", () => {
    expect(advanceLaborJob("in_progress", "cancel")).toEqual({
      next: "cancelled",
      accepted: true,
      reason: "cancelled in_progress labor job",
    })
  })

  test("cancels delivered jobs before settlement", () => {
    expect(advanceLaborJob("delivered", "cancel")).toEqual({
      next: "cancelled",
      accepted: true,
      reason: "cancelled delivered labor job",
    })
  })

  test("rejects settlement before delivery", () => {
    expect(advanceLaborJob("in_progress", "settle")).toEqual({
      next: "in_progress",
      accepted: false,
      reason: "illegal settle from in_progress",
    })
  })

  test("rejects duplicate acceptance", () => {
    expect(advanceLaborJob("accepted", "accept")).toEqual({
      next: "accepted",
      accepted: false,
      reason: "illegal accept from accepted",
    })
  })

  test("rejects cancellation after settlement", () => {
    expect(advanceLaborJob("settled", "cancel")).toEqual({
      next: "settled",
      accepted: false,
      reason: "cannot cancel settled labor job",
    })
  })

  test("rejects transitions from cancelled jobs", () => {
    expect(advanceLaborJob("cancelled", "start")).toEqual({
      next: "cancelled",
      accepted: false,
      reason: "illegal start from cancelled",
    })
  })
})
