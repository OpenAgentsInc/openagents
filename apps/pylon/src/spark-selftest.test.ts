import { describe, expect, it } from "bun:test"
import { sparkModuleSelftest } from "./spark-backup-helper.js"

describe("sparkModuleSelftest", () => {
  it("reports loaded when the module exposes defaultConfig + SdkBuilder.new", async () => {
    const r = await sparkModuleSelftest(async () => ({
      defaultConfig: () => ({}),
      SdkBuilder: { new: () => ({}) },
    }) as any)
    expect(r).toEqual({ moduleLoaded: true, reason: null })
  })
  it("reports loaded with the legacy connect() entrypoint", async () => {
    const r = await sparkModuleSelftest(async () => ({
      defaultConfig: () => ({}),
      connect: async () => ({}),
    }) as any)
    expect(r.moduleLoaded).toBe(true)
  })
  it("reports not-loaded with a redacted reason when the import throws", async () => {
    const r = await sparkModuleSelftest(async () => {
      throw new Error("Cannot find module '@breeztech/breez-sdk-spark'")
    })
    expect(r.moduleLoaded).toBe(false)
    expect(r.reason).toContain("Cannot find module")
  })
  it("reports not-loaded when the module is missing required exports", async () => {
    const r = await sparkModuleSelftest(async () => ({ defaultConfig: () => ({}) }) as any)
    expect(r.moduleLoaded).toBe(false)
  })
})

import { toSatNumber } from "./spark-backup-helper.js"

describe("toSatNumber (balance coercion #5166)", () => {
  it("accepts number, bigint, and decimal-string sat amounts", () => {
    expect(toSatNumber(50000)).toBe(50000)
    expect(toSatNumber(0)).toBe(0)
    expect(toSatNumber(50000n)).toBe(50000)
    expect(toSatNumber("50000")).toBe(50000)
  })
  it("rejects non-amounts as null", () => {
    expect(toSatNumber(undefined)).toBeNull()
    expect(toSatNumber(null)).toBeNull()
    expect(toSatNumber("abc")).toBeNull()
    expect(toSatNumber(NaN)).toBeNull()
  })
})
