import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"

import {
  lookupBackendProfile,
  resolveBackendProfile,
} from "../src/index.js"
import {
  PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY,
  PSIONIC_QWEN_LOCAL_PROFILE_ID,
} from "../src/backends/psionic-qwen/contract.js"

describe("archived Psionic Qwen backend profile", () => {
  test("is absent from the active registry and retains only archived identifiers", async () => {
    const defaultProfile = await Effect.runPromise(resolveBackendProfile())
    expect(defaultProfile.kind).toBe("apple_fm_bridge")

    const error = await captureError(lookupBackendProfile(PSIONIC_QWEN_LOCAL_PROFILE_ID))
    expect(error).toMatchObject({
      _tag: "ProbeBackendRegistryError",
      reason: `unknown backend profile: ${PSIONIC_QWEN_LOCAL_PROFILE_ID}`,
    })
    expect(PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY).toEndWith(".archived")
  })
})

async function captureError(effect: Effect.Effect<unknown, unknown>): Promise<unknown> {
  try {
    await Effect.runPromise(effect)
  } catch (error) {
    return error
  }

  throw new Error("expected effect to fail")
}
