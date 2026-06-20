import { describe, expect, test } from "bun:test"

import { describeOtaDirective } from "./ota-directive-view.js"

describe("OTA directive view", () => {
  test("describes noUpdateAvailable directives", () => {
    expect(describeOtaDirective({
      type: "noUpdateAvailable",
    })).toEqual({
      type: "noUpdateAvailable",
      headline: "No OTA update available",
    })
  })

  test("describes rollBackToEmbedded directives", () => {
    expect(describeOtaDirective({
      type: "rollBackToEmbedded",
      parameters: {
        commitTime: "2026-06-13T00:00:00.000Z",
      },
    })).toEqual({
      type: "rollBackToEmbedded",
      headline: "Roll back to embedded app bundle",
    })
  })

  test("treats unknown directive types as unknown", () => {
    expect(describeOtaDirective({
      type: "manifest",
    })).toEqual({
      type: "unknown",
      headline: "Unknown OTA directive",
    })
  })

  test("treats malformed directive types as unknown", () => {
    expect(describeOtaDirective({
      type: ["rollBackToEmbedded"],
    })).toEqual({
      type: "unknown",
      headline: "Unknown OTA directive",
    })
  })

  test("treats non-object payloads as unknown", () => {
    expect(describeOtaDirective(null)).toEqual({
      type: "unknown",
      headline: "Unknown OTA directive",
    })

    expect(describeOtaDirective("rollBackToEmbedded")).toEqual({
      type: "unknown",
      headline: "Unknown OTA directive",
    })
  })

  test("does not mutate the source directive", () => {
    const directive = Object.freeze({
      type: "noUpdateAvailable",
      extra: Object.freeze({
        untouched: true,
      }),
    })

    describeOtaDirective(directive)

    expect(directive).toEqual({
      type: "noUpdateAvailable",
      extra: {
        untouched: true,
      },
    })
  })
})
