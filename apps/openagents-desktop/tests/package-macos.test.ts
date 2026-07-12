import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"

import config, { OPENAGENTS_DESKTOP_BUNDLE_ID, OPENAGENTS_DESKTOP_PROTOCOL } from "../forge.config.ts"

const root = path.resolve(import.meta.dir, "..")

describe("CUT-26 macOS artifact contract", () => {
  test("freezes independent identity, DMG+ZIP outputs, ASAR, and provider executable unpacking", () => {
    expect(OPENAGENTS_DESKTOP_BUNDLE_ID).toBe("com.openagents.desktop")
    expect(OPENAGENTS_DESKTOP_BUNDLE_ID).not.toContain("khala")
    expect(OPENAGENTS_DESKTOP_PROTOCOL).toBe("openagents")
    const asar = config.packagerConfig?.asar as { unpack?: string }
    expect(asar.unpack).toContain("claude-agent-sdk")
    expect(asar.unpack).toContain("@openai/codex")
    expect(config.makers).toHaveLength(2)
  })

  test("locks the hardened Electron fuse posture", () => {
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8")
    for (const expected of [
      "RunAsNode]: false",
      "EnableCookieEncryption]: true",
      "EnableNodeOptionsEnvironmentVariable]: false",
      "EnableNodeCliInspectArguments]: false",
      "EnableEmbeddedAsarIntegrityValidation]: true",
      "OnlyLoadAppFromAsar]: true",
      "strictlyRequireAllFuses: true",
    ]) expect(source).toContain(expected)
  })

  test("entitlements stay minimal and never disable library validation or permit debugging", () => {
    for (const name of ["entitlements.mac.plist", "entitlements.mac.inherit.plist"]) {
      const plist = readFileSync(path.join(root, "build", name), "utf8")
      expect(plist).toContain("allow-jit")
      expect(plist).toContain("allow-unsigned-executable-memory")
      expect(plist).not.toContain("disable-library-validation")
      expect(plist).not.toContain("get-task-allow")
      expect(plist).not.toContain("allow-dyld-environment-variables")
    }
  })

  test("notarization is API-key-only and never embeds credential material", () => {
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8")
    expect(source).toContain("ASC_API_PRIVATE_KEY_PATH")
    expect(source).toContain("ASC_API_KEY_ID")
    expect(source).toContain("ASC_API_ISSUER_ID")
    expect(source).not.toContain("appleIdPassword")
    expect(source).not.toContain("@openagents.com")
  })
})
