import { describe, expect, test } from "bun:test"

import { evaluateDistributionReadiness } from "../src/coordinator/distribution-checklist"

describe("distribution checklist", () => {
  test("marks desktop ready when signed, notarized, and BSDIFF are available", () => {
    expect(
      evaluateDistributionReadiness({
        target: "desktop",
        signed: true,
        notarized: true,
        bsdiffAvailable: true,
      }),
    ).toEqual({
      ready: true,
      missing: [],
      steps: [
        { name: "signed", done: true },
        { name: "notarized", done: true },
        { name: "bsdiffAvailable", done: true },
      ],
    })
  })

  test("reports all missing desktop gates when no booleans are supplied", () => {
    expect(evaluateDistributionReadiness({ target: "desktop" })).toEqual({
      ready: false,
      missing: ["signed", "notarized", "bsdiffAvailable"],
      steps: [
        { name: "signed", done: false },
        { name: "notarized", done: false },
        { name: "bsdiffAvailable", done: false },
      ],
    })
  })

  test("reports only incomplete desktop gates", () => {
    expect(
      evaluateDistributionReadiness({
        target: "desktop",
        signed: true,
        notarized: false,
        bsdiffAvailable: true,
      }),
    ).toEqual({
      ready: false,
      missing: ["notarized"],
      steps: [
        { name: "signed", done: true },
        { name: "notarized", done: false },
        { name: "bsdiffAvailable", done: true },
      ],
    })
  })

  test("marks mobile ready after store submission", () => {
    expect(
      evaluateDistributionReadiness({
        target: "mobile",
        storeSubmitted: true,
      }),
    ).toEqual({
      ready: true,
      missing: [],
      steps: [{ name: "storeSubmitted", done: true }],
    })
  })

  test("reports mobile store submission as missing", () => {
    expect(evaluateDistributionReadiness({ target: "mobile" })).toEqual({
      ready: false,
      missing: ["storeSubmitted"],
      steps: [{ name: "storeSubmitted", done: false }],
    })
  })

  test("marks OTA ready after publish", () => {
    expect(
      evaluateDistributionReadiness({
        target: "ota",
        otaPublished: true,
      }),
    ).toEqual({
      ready: true,
      missing: [],
      steps: [{ name: "otaPublished", done: true }],
    })
  })

  test("reports OTA publish as missing", () => {
    expect(evaluateDistributionReadiness({ target: "ota" })).toEqual({
      ready: false,
      missing: ["otaPublished"],
      steps: [{ name: "otaPublished", done: false }],
    })
  })
})
