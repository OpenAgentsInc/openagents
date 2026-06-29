import { describe, expect, test } from "bun:test"

import { classifyShipMode, explain } from "../src/coordinator/ship-mode"

describe("ship mode classifier", () => {
  test("returns none when there is no mobile change", () => {
    const input = {
      deployedFingerprint: "deployed",
      currentFingerprint: "current",
      hasMobileChange: false,
    }

    expect(classifyShipMode(input)).toBe("none")
    expect(explain(input)).toContain("No mobile change")
  })

  test("returns ota when mobile change exists and fingerprints match", () => {
    const input = {
      deployedFingerprint: "fingerprint-1",
      currentFingerprint: "fingerprint-1",
      hasMobileChange: true,
    }

    expect(classifyShipMode(input)).toBe("ota")
    expect(explain(input)).toContain("ship via EAS Update OTA")
  })

  test("returns rebuild when mobile change exists and fingerprints differ", () => {
    const input = {
      deployedFingerprint: "fingerprint-1",
      currentFingerprint: "fingerprint-2",
      hasMobileChange: true,
    }

    expect(classifyShipMode(input)).toBe("rebuild")
    expect(explain(input)).toContain("runtime fingerprint changed")
  })

  test("returns rebuild when mobile change exists and no deployed fingerprint is available", () => {
    const input = {
      deployedFingerprint: null,
      currentFingerprint: "fingerprint-1",
      hasMobileChange: true,
    }

    expect(classifyShipMode(input)).toBe("rebuild")
    expect(explain(input)).toContain("no deployed runtime fingerprint")
  })
})
