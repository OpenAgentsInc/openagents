import { createHash } from "node:crypto"
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { chmodSync, readFileSync, statSync } from "node:fs"
import path from "node:path"
import { createInterface } from "node:readline"
import type { VoiceNativeMedia } from "./voice-host.ts"

export const VoiceHelperRelativePath = path.join("native", process.arch, "oa-desktop-audio")
export type VoiceHelperManifest = Readonly<{ protocolVersion: 1; helperVersion: string; architecture: string; sha256: string }>

export const resolveVoiceHelperPath = (resourcesPath: string): string => path.join(resourcesPath, VoiceHelperRelativePath)
export const verifyVoiceHelper = (input: Readonly<{
  resourcesPath: string
  manifest: VoiceHelperManifest
  verifySignature: (absolutePath: string) => boolean
}>): string => {
  const absolutePath = resolveVoiceHelperPath(input.resourcesPath)
  if (input.manifest.protocolVersion !== 1 || input.manifest.architecture !== process.arch) throw new Error("voice_helper_manifest_mismatch")
  const stats = statSync(absolutePath)
  if (!stats.isFile() || (stats.mode & 0o111) === 0) throw new Error("voice_helper_not_executable")
  const digest = createHash("sha256").update(readFileSync(absolutePath)).digest("hex")
  if (digest !== input.manifest.sha256) throw new Error("voice_helper_digest_mismatch")
  if (!input.verifySignature(absolutePath)) throw new Error("voice_helper_signature_invalid")
  return absolutePath
}

export const spawnVoiceHelper = (absolutePath: string): ChildProcessWithoutNullStreams => spawn(absolutePath, [], {
  cwd: path.dirname(absolutePath),
  env: { LANG: "C", LC_ALL: "C", HOME: "/var/empty", PATH: "" },
  stdio: ["pipe", "pipe", "pipe"],
  detached: false,
  windowsHide: true,
})

export const createPackagedVoiceNativeMedia = (input: Readonly<{
  resourcesPath: string
  verifySignature: (absolutePath: string) => boolean
}>): VoiceNativeMedia => ({
  open: request => {
    const manifest = JSON.parse(readFileSync(path.join(input.resourcesPath, "native", process.arch, "manifest.json"), "utf8")) as VoiceHelperManifest
    const child = spawnVoiceHelper(verifyVoiceHelper({ ...input, manifest }))
    let closed = false
    const lines = createInterface({ input: child.stdout })
    lines.on("line", line => {
      let state: unknown
      try { state = JSON.parse(line) } catch { request.onState("crashed"); return }
      if (typeof state !== "object" || state === null || !("state" in state)) { request.onState("crashed"); return }
      const tag = (state as { state: string }).state
      if (tag === "live") request.onState("live")
      else if (tag === "offline") request.onState("offline")
      else if (tag === "backpressured") request.onState("backpressured")
      else if (tag === "refused") request.onState("revoked")
    })
    child.once("exit", () => { if (!closed) request.onState("crashed") })
    child.stdin.write(JSON.stringify({ command: "start", protocol_version: 1, identity: request.identity, disclosure_ref: request.disclosureRef }) + "\n")
    return {
      setCaptureEnabled: enabled => { if (!closed) child.stdin.write(JSON.stringify({ command: "set_capture", enabled }) + "\n") },
      close: reason => {
        if (closed) return
        closed = true
        child.stdin.write(JSON.stringify({ command: "stop", reason }) + "\n")
        child.stdin.end(); lines.close(); child.kill("SIGTERM")
      },
    }
  },
})

// Test/release preparation only; production verification still refuses a
// non-executable helper even if a permissive unpacker stripped its mode.
export const ensureVoiceHelperExecutableForPackaging = (absolutePath: string): void => chmodSync(absolutePath, 0o755)
