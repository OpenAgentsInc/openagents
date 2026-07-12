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
  connection: (identity: Parameters<VoiceNativeMedia["open"]>[0]["identity"], disclosureRef: string) => Promise<Readonly<{ gatewayUrl: string; grant: string }>>
}>): VoiceNativeMedia => ({
  open: async request => {
    const connection = await input.connection(request.identity, request.disclosureRef)
    const manifest = JSON.parse(readFileSync(path.join(input.resourcesPath, "native", process.arch, "manifest.json"), "utf8")) as VoiceHelperManifest
    const child = spawnVoiceHelper(verifyVoiceHelper({ ...input, manifest }))
    let closed = false
    const lines = createInterface({ input: child.stdout })
    lines.on("line", line => {
      let state: unknown
      try { state = JSON.parse(line) } catch { request.onState("crashed"); return }
      if (typeof state !== "object" || state === null || !("state" in state)) { request.onState("crashed"); return }
      const record = state as Record<string, unknown>
      const tag = record.state
      if (tag === "live") request.onState("live")
      else if (tag === "ack" && typeof record.sequence === "number" && typeof record.generation === "number") request.onAck(record.sequence, record.generation)
      else if (tag === "packet" && typeof record.sequence === "number" && typeof record.generation === "number" && typeof record.payloadLength === "number" && typeof record.sha256 === "string") request.onPacket({ sequence: record.sequence, generation: record.generation, payloadLength: record.payloadLength, sha256: record.sha256 })
      else if (tag === "offline") request.onState("offline")
      else if (tag === "backpressured") request.onState("backpressured")
      else if (tag === "device_changed") request.onState("device_changed")
      else if (tag === "playback" && typeof record.speechRef === "string") request.onControl({ kind: "playback", speechRef: record.speechRef.slice(0, 256), state: "speaking" })
      else if (tag === "playback_canceled" && typeof record.speechRef === "string" && typeof record.outcomeRef === "string") request.onControl({ kind: "playback", speechRef: record.speechRef.slice(0, 256), state: "canceled", outcomeRef: record.outcomeRef.slice(0, 256) })
      else if (tag === "transcript" && typeof record.utteranceRef === "string" && typeof record.text === "string" && typeof record.final === "boolean") request.onControl({ kind: "transcript", utteranceRef: record.utteranceRef.slice(0, 256), text: record.text.slice(0, 16_384), final: record.final })
      else if (tag === "activity" && ["speech_detected", "transcribing", "awaiting_confirmation", "executing", "speaking", "listening"].includes(String(record.activity))) request.onControl({ kind: "activity", activity: record.activity as "speech_detected" | "transcribing" | "awaiting_confirmation" | "executing" | "speaking" | "listening" })
      else if (tag === "command_proposal" && typeof record.proposalRef === "string" && typeof record.utteranceRef === "string" && typeof record.turnRef === "string" && typeof record.targetRef === "string" && ["chat.open", "workspace.files", "workspace.home", "workspace.review", "conversation.interrupt", "conversation.followup"].includes(String(record.commandId)) && typeof record.expiresAtMs === "number" && Number.isSafeInteger(record.expiresAtMs)) request.onControl({ kind: "proposal", proposalRef: record.proposalRef.slice(0, 256), utteranceRef: record.utteranceRef.slice(0, 256), turnRef: record.turnRef.slice(0, 256), targetRef: record.targetRef.slice(0, 256), commandId: String(record.commandId), expiresAtMs: record.expiresAtMs })
      else if (tag === "refused") request.onState("revoked")
    })
    child.once("exit", () => { if (!closed) request.onState("crashed") })
    child.stdin.write(JSON.stringify({ command: "start", protocol_version: 1, identity: request.identity, disclosure_ref: request.disclosureRef, gateway_url: connection.gatewayUrl, application_grant: connection.grant }) + "\n")
    return {
      setCaptureEnabled: enabled => { if (!closed) child.stdin.write(JSON.stringify({ command: "set_capture", enabled }) + "\n") },
      speak: async value => {
        if (closed) return false
        const url = connection.gatewayUrl.replace(/^wss:/u, "https:").replace(/\/v1\/stream$/u, "/v1/speak")
        if (!url.startsWith("https://") || url === connection.gatewayUrl) return false
        try {
          const response = await fetch(url, { method: "POST", headers: { "content-type": "application/json", "x-openagents-audio-grant": connection.grant }, body: JSON.stringify(value) })
          return response.ok
        } catch { return false }
      },
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
