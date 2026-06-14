import { describe, expect, test } from "bun:test"

import { evaluateDistributionReadiness } from "../src/coordinator/distribution-checklist"

describe("distribution checklist", () => {
  test("marks desktop ready when signed, notarized, artifact, BSDIFF, and feed are available", () => {
    expect(
      evaluateDistributionReadiness({
        target: "desktop",
        signed: true,
        notarized: true,
        artifactPublished: true,
        bsdiffAvailable: true,
        desktopFeedPublished: true,
      }),
    ).toEqual({
      ready: true,
      missing: [],
      steps: [
        { name: "signed", done: true },
        { name: "notarized", done: true },
        { name: "artifactPublished", done: true },
        { name: "bsdiffAvailable", done: true },
        { name: "desktopFeedPublished", done: true },
      ],
    })
  })

  test("reports all missing desktop gates when no booleans are supplied", () => {
    expect(evaluateDistributionReadiness({ target: "desktop" })).toEqual({
      ready: false,
      missing: [
        "signed",
        "notarized",
        "artifactPublished",
        "bsdiffAvailable",
        "desktopFeedPublished",
      ],
      steps: [
        { name: "signed", done: false },
        { name: "notarized", done: false },
        { name: "artifactPublished", done: false },
        { name: "bsdiffAvailable", done: false },
        { name: "desktopFeedPublished", done: false },
      ],
    })
  })

  test("reports only incomplete desktop gates", () => {
    expect(
      evaluateDistributionReadiness({
        target: "desktop",
        signed: true,
        notarized: false,
        artifactPublished: true,
        bsdiffAvailable: true,
        desktopFeedPublished: true,
      }),
    ).toEqual({
      ready: false,
      missing: ["notarized"],
      steps: [
        { name: "signed", done: true },
        { name: "notarized", done: false },
        { name: "artifactPublished", done: true },
        { name: "bsdiffAvailable", done: true },
        { name: "desktopFeedPublished", done: true },
      ],
    })
  })

  test("marks mobile ready after TestFlight upload and store submission", () => {
    expect(
      evaluateDistributionReadiness({
        target: "mobile",
        testflightUploaded: true,
        storeSubmitted: true,
      }),
    ).toEqual({
      ready: true,
      missing: [],
      steps: [
        { name: "testflightUploaded", done: true },
        { name: "storeSubmitted", done: true },
      ],
    })
  })

  test("reports mobile TestFlight upload and store submission as missing", () => {
    expect(evaluateDistributionReadiness({ target: "mobile" })).toEqual({
      ready: false,
      missing: ["testflightUploaded", "storeSubmitted"],
      steps: [
        { name: "testflightUploaded", done: false },
        { name: "storeSubmitted", done: false },
      ],
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
