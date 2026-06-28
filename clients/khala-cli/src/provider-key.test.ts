import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"

import {
  clearProviderKey,
  describeStoredProviderKey,
  normalizeProviderName,
  providerKeyPath,
  readProviderKey,
  redactProviderKey,
  validateProviderKeyShape,
  writeProviderKey,
} from "./provider-key.js"

const makeEnv = (): Record<string, string | undefined> => {
  const dir = mkdtempSync(join(tmpdir(), "khala-byok-"))
  return { KHALA_TOKEN_PATH: join(dir, "agent-token") }
}

describe("normalizeProviderName", () => {
  test("accepts canonical providers and friendly aliases", () => {
    expect(normalizeProviderName("openrouter")).toBe("openrouter")
    expect(normalizeProviderName("OpenAI")).toBe("openai")
    expect(normalizeProviderName("claude")).toBe("anthropic")
    expect(normalizeProviderName("gemini")).toBe("google")
    expect(normalizeProviderName("grok")).toBe("xai")
  })

  test("rejects unknown providers", () => {
    expect(normalizeProviderName("nope")).toBeUndefined()
    expect(normalizeProviderName("")).toBeUndefined()
    expect(normalizeProviderName(undefined)).toBeUndefined()
  })
})

describe("validateProviderKeyShape", () => {
  test("accepts a plausible key", () => {
    expect(validateProviderKeyShape("sk-or-v1-abc12345")).toBe("sk-or-v1-abc12345")
  })

  test("rejects short keys and keys with whitespace", () => {
    expect(() => validateProviderKeyShape("short")).toThrow()
    expect(() => validateProviderKeyShape("has space inside")).toThrow()
    expect(() => validateProviderKeyShape(undefined)).toThrow()
  })
})

describe("redactProviderKey", () => {
  test("only ever exposes the last four characters", () => {
    expect(redactProviderKey("sk-or-v1-secret-tail")).toBe("...tail")
    expect(redactProviderKey("abcd")).toBe("****")
  })
})

describe("provider key store roundtrip", () => {
  const envs: Array<Record<string, string | undefined>> = []
  afterEach(async () => {
    for (const env of envs) await clearProviderKey(env)
    envs.length = 0
  })

  test("write, read, describe, then clear", async () => {
    const env = makeEnv()
    envs.push(env)

    expect(await readProviderKey(env)).toBeUndefined()

    const saved = await writeProviderKey(env, { provider: "openrouter", key: "sk-or-v1-abc12345" })
    expect(saved.provider).toBe("openrouter")

    const read = await readProviderKey(env)
    expect(read?.provider).toBe("openrouter")
    expect(read?.key).toBe("sk-or-v1-abc12345")
    expect(describeStoredProviderKey(read!)).toBe("openrouter · ...2345")

    // The raw key is persisted only in the 0600 store file, never elsewhere.
    const onDisk = readFileSync(providerKeyPath(env), "utf8")
    expect(onDisk).toContain("sk-or-v1-abc12345")

    expect(await clearProviderKey(env)).toBe(true)
    expect(await readProviderKey(env)).toBeUndefined()
    expect(await clearProviderKey(env)).toBe(false)
  })
})
