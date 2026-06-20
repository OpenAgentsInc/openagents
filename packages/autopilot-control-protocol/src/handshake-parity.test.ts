import { describe, expect, test } from "bun:test"

import { checkHandshakeParity } from "./handshake-parity.js"

describe("handshake parity", () => {
  test("accepts empty desktop and mobile session lists", () => {
    expect(checkHandshakeParity([], [])).toEqual({
      inSync: true,
      sharedRefs: [],
      desktopOnly: [],
      mobileOnly: [],
      stateMismatches: [],
    })
  })

  test("accepts matching refs and states", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "cloud.beta", state: "idle" },
    ], [
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "cloud.beta", state: "idle" },
    ])).toEqual({
      inSync: true,
      sharedRefs: ["local.alpha", "cloud.beta"],
      desktopOnly: [],
      mobileOnly: [],
      stateMismatches: [],
    })
  })

  test("reports desktop-only refs", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "desktop.only", state: "idle" },
    ], [
      { sessionRef: "local.alpha", state: "running" },
    ])).toEqual({
      inSync: false,
      sharedRefs: ["local.alpha"],
      desktopOnly: ["desktop.only"],
      mobileOnly: [],
      stateMismatches: [],
    })
  })

  test("reports mobile-only refs", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
    ], [
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "mobile.only", state: "idle" },
    ])).toEqual({
      inSync: false,
      sharedRefs: ["local.alpha"],
      desktopOnly: [],
      mobileOnly: ["mobile.only"],
      stateMismatches: [],
    })
  })

  test("reports shared refs with different states", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
    ], [
      { sessionRef: "local.alpha", state: "paused" },
    ])).toEqual({
      inSync: false,
      sharedRefs: ["local.alpha"],
      desktopOnly: [],
      mobileOnly: [],
      stateMismatches: ["local.alpha: desktop running, mobile paused"],
    })
  })

  test("reports ref and state differences together", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "desktop.only", state: "idle" },
    ], [
      { sessionRef: "local.alpha", state: "paused" },
      { sessionRef: "mobile.only", state: "running" },
    ])).toEqual({
      inSync: false,
      sharedRefs: ["local.alpha"],
      desktopOnly: ["desktop.only"],
      mobileOnly: ["mobile.only"],
      stateMismatches: ["local.alpha: desktop running, mobile paused"],
    })
  })

  test("does not require the same list order", () => {
    expect(checkHandshakeParity([
      { sessionRef: "local.alpha", state: "running" },
      { sessionRef: "cloud.beta", state: "idle" },
    ], [
      { sessionRef: "cloud.beta", state: "idle" },
      { sessionRef: "local.alpha", state: "running" },
    ])).toEqual({
      inSync: true,
      sharedRefs: ["local.alpha", "cloud.beta"],
      desktopOnly: [],
      mobileOnly: [],
      stateMismatches: [],
    })
  })
})
