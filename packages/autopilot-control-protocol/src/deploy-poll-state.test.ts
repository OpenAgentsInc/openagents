import { describe, expect, test } from "bun:test"
import { nextDeployPoll } from "./deploy-poll-state.js"

describe("deploy poll state", () => {
  test("marks deployed as terminal without polling", () => {
    expect(nextDeployPoll({ state: "deployed", attempts: 1 })).toEqual({
      shouldPoll: false,
      delayMs: 0,
      done: true,
    })
  })

  test("marks failed as terminal without polling", () => {
    expect(nextDeployPoll({ state: "failed", attempts: 3 })).toEqual({
      shouldPoll: false,
      delayMs: 0,
      done: true,
    })
  })

  test("polls queued deploys with linear backoff", () => {
    expect(nextDeployPoll({ state: "queued", attempts: 2 })).toEqual({
      shouldPoll: true,
      delayMs: 4000,
      done: false,
    })
  })

  test("polls building deploys with capped delay", () => {
    expect(nextDeployPoll({ state: "building", attempts: 10 })).toEqual({
      shouldPoll: true,
      delayMs: 15000,
      done: false,
    })
  })

  test("polls unknown states and normalizes terminal state casing", () => {
    expect(nextDeployPoll({ state: "unknown", attempts: 1 })).toEqual({
      shouldPoll: true,
      delayMs: 2000,
      done: false,
    })
    expect(nextDeployPoll({ state: " Deployed ", attempts: 10 })).toEqual({
      shouldPoll: false,
      delayMs: 0,
      done: true,
    })
  })
})
