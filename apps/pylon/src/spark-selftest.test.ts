import { describe, expect, it } from "bun:test"
import { sparkModuleSelftest } from "./spark-backup-helper"

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
