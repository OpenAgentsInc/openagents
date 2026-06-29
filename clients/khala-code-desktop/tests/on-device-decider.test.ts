import { describe, expect, test } from "bun:test"

import {
  backendSelectionOrder,
  createOnDeviceDecider,
  OnDeviceDeciderUnavailable,
  preferredOnDeviceBackend,
  type OnDeviceBackendKind,
  type OnDeviceBackendReadiness,
  type OnDeviceDeciderBackend,
  type OnDeviceDeciderResult,
} from "../src/shared/on-device-decider"

const fakeBackend = (
  kind: OnDeviceBackendKind,
  available: boolean,
  opts: { content?: string; throwOnProbe?: boolean } = {},
): OnDeviceDeciderBackend => ({
  kind,
  model: kind,
  async probe(): Promise<OnDeviceBackendReadiness> {
    if (opts.throwOnProbe === true) throw new Error("boom")
    return { backend: kind, available, model: kind, detail: available ? "ready" : "down" }
  },
  async complete(): Promise<OnDeviceDeciderResult> {
    return {
      backend: kind,
      model: kind,
      content: opts.content ?? `${kind}-said-yes`,
      usage: { promptTokens: 1, completionTokens: 1, totalTokens: 2, truth: "estimated" },
    }
  },
})

describe("preferredOnDeviceBackend (selection contract)", () => {
  test("Apple Silicon macOS prefers apple_fm", () => {
    expect(preferredOnDeviceBackend({ platform: "darwin", arch: "arm64" })).toBe("apple_fm")
  })
  test("iOS prefers apple_fm", () => {
    expect(preferredOnDeviceBackend({ platform: "ios", arch: "arm64" })).toBe("apple_fm")
  })
  test("Intel macOS prefers gpt_oss (no Apple FM)", () => {
    expect(preferredOnDeviceBackend({ platform: "darwin", arch: "x64" })).toBe("gpt_oss")
  })
  test("Linux and Windows prefer gpt_oss", () => {
    expect(preferredOnDeviceBackend({ platform: "linux", arch: "x64" })).toBe("gpt_oss")
    expect(preferredOnDeviceBackend({ platform: "win32", arch: "x64" })).toBe("gpt_oss")
  })
  test("selection order puts the preferred backend first", () => {
    expect(backendSelectionOrder("apple_fm")).toEqual(["apple_fm", "gpt_oss"])
    expect(backendSelectionOrder("gpt_oss")).toEqual(["gpt_oss", "apple_fm"])
  })
})

describe("createOnDeviceDecider", () => {
  const mac = { platform: "darwin", arch: "arm64" }
  const linux = { platform: "linux", arch: "x64" }

  test("selects the preferred backend when available", async () => {
    const decider = createOnDeviceDecider({
      platform: mac,
      backends: { apple_fm: fakeBackend("apple_fm", true), gpt_oss: fakeBackend("gpt_oss", true) },
    })
    const selection = await decider.select()
    expect(selection.preferred).toBe("apple_fm")
    expect(selection.selected).toBe("apple_fm")
    expect(selection.reason).toContain("apple_fm is available")
  })

  test("falls back to the other backend when the preferred is unavailable", async () => {
    const decider = createOnDeviceDecider({
      platform: mac,
      backends: { apple_fm: fakeBackend("apple_fm", false), gpt_oss: fakeBackend("gpt_oss", true) },
    })
    const selection = await decider.select()
    expect(selection.selected).toBe("gpt_oss")
    expect(selection.reason).toContain("fell back to gpt_oss")
  })

  test("non-Mac prefers and selects gpt_oss", async () => {
    const decider = createOnDeviceDecider({
      platform: linux,
      backends: { apple_fm: fakeBackend("apple_fm", true), gpt_oss: fakeBackend("gpt_oss", true) },
    })
    const selection = await decider.select()
    expect(selection.preferred).toBe("gpt_oss")
    expect(selection.selected).toBe("gpt_oss")
  })

  test("reports unavailable (and decide throws) when nothing is available", async () => {
    const decider = createOnDeviceDecider({
      platform: mac,
      backends: { apple_fm: fakeBackend("apple_fm", false), gpt_oss: fakeBackend("gpt_oss", false) },
    })
    const selection = await decider.select()
    expect(selection.selected).toBeNull()
    expect(selection.readiness).toHaveLength(2)
    await expect(decider.decide([{ role: "user", content: "hi" }])).rejects.toBeInstanceOf(
      OnDeviceDeciderUnavailable,
    )
  })

  test("a probe that throws is treated as unavailable, not fatal", async () => {
    const decider = createOnDeviceDecider({
      platform: mac,
      backends: {
        apple_fm: fakeBackend("apple_fm", true, { throwOnProbe: true }),
        gpt_oss: fakeBackend("gpt_oss", true),
      },
    })
    const selection = await decider.select()
    expect(selection.selected).toBe("gpt_oss")
    expect(selection.readiness[0]?.detail).toContain("probe failed")
  })

  test("decide routes to the selected backend and returns its result", async () => {
    const decider = createOnDeviceDecider({
      platform: mac,
      backends: { apple_fm: fakeBackend("apple_fm", true, { content: "YES" }) },
    })
    const result = await decider.decide([{ role: "user", content: "decide" }])
    expect(result.backend).toBe("apple_fm")
    expect(result.content).toBe("YES")
  })

  test("an unconfigured backend is unavailable, not a crash", async () => {
    const decider = createOnDeviceDecider({ platform: mac, backends: {} })
    const selection = await decider.select()
    expect(selection.selected).toBeNull()
    expect(selection.readiness.every((r) => !r.available)).toBe(true)
  })
})
