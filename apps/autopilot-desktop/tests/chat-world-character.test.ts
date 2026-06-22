// #verse/mmo-characters-per-account: resolving the per-instance Verse character.
//
// The bug: OA_CHARACTER is set on the Bun launcher process at runtime but is NOT
// VITE_-prefixed, so it never reaches the webview/renderer (absent from the
// build-time `import.meta.env` define, and there is no `process.env` in the
// renderer). Two Autopilot windows therefore both fell back to "main" and shared
// one avatar. The fix injects the resolved value into the webview as
// `globalThis.__OA_CHARACTER`, which chatWorldCharacterId() now reads FIRST.
//
// These tests pin that precedence and the "main" default, without a live node.

import { afterEach, describe, expect, test } from "bun:test"

import { chatWorldCharacterId } from "../src/shared/chat-world-flags"

const CHAR_ENV_KEYS = ["OA_CHARACTER", "VITE_OA_CHARACTER"] as const

const savedEnv = new Map<string, string | undefined>(
  CHAR_ENV_KEYS.map((key) => [key, process.env[key]]),
)

const injected = (): { __OA_CHARACTER?: unknown } =>
  globalThis as { __OA_CHARACTER?: unknown }

afterEach(() => {
  for (const key of CHAR_ENV_KEYS) {
    const value = savedEnv.get(key)
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
  delete injected().__OA_CHARACTER
})

const clearCharEnv = (): void => {
  for (const key of CHAR_ENV_KEYS) delete process.env[key]
  delete injected().__OA_CHARACTER
}

describe("chatWorldCharacterId resolution", () => {
  test("defaults to 'main' when nothing is set (single-instance behavior unchanged)", () => {
    clearCharEnv()
    expect(chatWorldCharacterId()).toBe("main")
  })

  test("reads the injected globalThis.__OA_CHARACTER (the webview-plumbing fix)", () => {
    clearCharEnv()
    injected().__OA_CHARACTER = "alt"
    expect(chatWorldCharacterId()).toBe("alt")
  })

  test("injected value takes precedence over the env fallbacks", () => {
    clearCharEnv()
    process.env.OA_CHARACTER = "from-env"
    process.env.VITE_OA_CHARACTER = "from-vite"
    injected().__OA_CHARACTER = "from-inject"
    expect(chatWorldCharacterId()).toBe("from-inject")
  })

  test("falls back to OA_CHARACTER, then VITE_OA_CHARACTER, when not injected", () => {
    clearCharEnv()
    process.env.VITE_OA_CHARACTER = "from-vite"
    expect(chatWorldCharacterId()).toBe("from-vite")
    process.env.OA_CHARACTER = "from-env"
    expect(chatWorldCharacterId()).toBe("from-env")
  })

  test("trims and ignores a blank/whitespace injected value (falls through to default)", () => {
    clearCharEnv()
    injected().__OA_CHARACTER = "   "
    expect(chatWorldCharacterId()).toBe("main")
    injected().__OA_CHARACTER = "  alt  "
    expect(chatWorldCharacterId()).toBe("alt")
  })

  test("ignores a non-string injected value", () => {
    clearCharEnv()
    ;(injected() as { __OA_CHARACTER?: unknown }).__OA_CHARACTER = 42
    expect(chatWorldCharacterId()).toBe("main")
  })
})
