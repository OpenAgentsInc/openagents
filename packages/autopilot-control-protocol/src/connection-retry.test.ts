import { describe, expect, test } from "bun:test"

import { nextRetry } from "./connection-retry.js"

describe("connection retry", () => {
  test("starts with a one second delay", () => {
    expect(nextRetry({ attempt: 0 })).toEqual({
      delayMs: 1_000,
      giveUp: false,
    })
  })

  test("doubles the delay for each attempt", () => {
    expect(nextRetry({ attempt: 1 })).toEqual({
      delayMs: 2_000,
      giveUp: false,
    })
    expect(nextRetry({ attempt: 2 })).toEqual({
      delayMs: 4_000,
      giveUp: false,
    })
  })

  test("uses the default thirty second cap", () => {
    expect(nextRetry({ attempt: 5 })).toEqual({
      delayMs: 30_000,
      giveUp: false,
    })
  })

  test("uses a custom cap", () => {
    expect(nextRetry({ attempt: 4, maxMs: 10_000 })).toEqual({
      delayMs: 10_000,
      giveUp: false,
    })
  })

  test("keeps uncapped delays below a custom cap", () => {
    expect(nextRetry({ attempt: 3, maxMs: 20_000 })).toEqual({
      delayMs: 8_000,
      giveUp: false,
    })
  })

  test("gives up starting at attempt eight", () => {
    expect(nextRetry({ attempt: 8 })).toEqual({
      delayMs: 30_000,
      giveUp: true,
    })
  })

  test("does not give up before attempt eight", () => {
    expect(nextRetry({ attempt: 7 })).toEqual({
      delayMs: 30_000,
      giveUp: false,
    })
  })
})
