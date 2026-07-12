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
    const asar = config.packagerConfig?.asar as { unpack?: string; unpackDir?: string }
    expect(asar.unpack).toContain("claude-agent-sdk")
    expect(asar.unpack).toContain("@openai/codex")
    expect(asar.unpackDir).toBe("dist/renderer")
    const ignore = config.packagerConfig?.ignore
    expect(typeof ignore).toBe("function")
    expect(typeof ignore === "function" && ignore("/node_modules/effect/index.js")).toBe(true)
    expect(config.makers).toHaveLength(2)
    const manifest = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8")) as { scripts: Record<string, string> }
    expect(manifest.scripts["make:mac"]).toContain("prepare-macos-maker.ts")
    const makerPreparation = readFileSync(path.join(root, "scripts", "prepare-macos-maker.ts"), "utf8")
    expect(makerPreparation).toContain('"macos-alias"')
    expect(makerPreparation).toContain('"fs-xattr"')
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
      "LoadBrowserProcessSpecificV8Snapshot]: false",
      "GrantFileProtocolExtraPrivileges]: false",
      "strictlyRequireAllFuses: true",
    ]) expect(source).toContain(expected)
    const main = readFileSync(path.join(root, "src", "main.ts"), "utf8")
    expect(main).toContain('path.join(process.resourcesPath, "app.asar.unpacked", "dist", "renderer", "index.html")')
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
    expect(source).toContain('"code-sign"')
    expect(source).toContain('"signing-identity"')
    expect(source).toContain('"/Electron Framework.framework/Versions/Current/"')
    expect(source).toContain('file.endsWith(".pak")')
    expect(source).not.toContain("appleIdPassword")
    expect(source).not.toContain("@openagents.com")
  })
})
