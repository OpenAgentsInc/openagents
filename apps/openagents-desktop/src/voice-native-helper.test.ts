import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { chmodSync, mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"
import { resolveVoiceHelperPath, verifyVoiceHelper } from "./voice-native-helper.ts"

describe("packaged voice helper oracle", () => {
  test("requires fixed architecture path, executable mode, digest, version and signature", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "oa-voice-helper-")); const helper = resolveVoiceHelperPath(root)
    mkdirSync(path.dirname(helper), { recursive: true }); writeFileSync(helper, "signed-helper"); chmodSync(helper, 0o755)
    const manifest = { protocolVersion: 1 as const, helperVersion: "0.1.0", architecture: process.arch, sha256: createHash("sha256").update("signed-helper").digest("hex") }
    expect(verifyVoiceHelper({ resourcesPath: root, manifest, verifySignature: candidate => candidate === helper })).toBe(helper)
    expect(() => verifyVoiceHelper({ resourcesPath: root, manifest: { ...manifest, sha256: "0".repeat(64) }, verifySignature: () => true })).toThrow("voice_helper_digest_mismatch")
    expect(() => verifyVoiceHelper({ resourcesPath: root, manifest, verifySignature: () => false })).toThrow("voice_helper_signature_invalid")
  })
  test("source launches absolute helper without shell or ambient PATH", async () => {
    const source = await readFile(new URL("./voice-native-helper.ts", import.meta.url), "utf8")
    expect(source).toContain("spawn(absolutePath")
    expect(source).toContain('PATH: ""')
    expect(source).toContain('HOME: "/var/empty"')
    expect(source).toContain("detached: false")
    expect(source).not.toContain("shell: true")
    expect(source).not.toContain("spawnSync")
  })
  test("Electron main obtains one bounded grant through the authenticated host-only route", async () => {
    const source = await readFile(new URL("./main.ts", import.meta.url), "utf8")
    expect(source).toContain("/api/desktop/audio/grant")
    expect(source).toContain('"x-openagents-desktop-device-ref": identity.deviceRef')
    expect(source).toContain("authorization: `Bearer ${credential.accessToken}`")
    expect(source).toContain('value.schema !== "openagents.audio.grant.v1"')
    expect(source).toContain("value.disclosureRef !== disclosureRef")
    expect(source).not.toContain("OPENAGENTS_AUDIO_TOKEN_SECRET")
  })
})
