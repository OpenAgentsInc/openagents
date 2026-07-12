import { createHash } from "node:crypto"

export const OPENAGENTS_DESKTOP_UPDATE_SCHEMA =
  "openagents.desktop.update_manifest.v1" as const
export const OPENAGENTS_DESKTOP_PRODUCT = "openagents-desktop" as const
export const OPENAGENTS_DESKTOP_CHANNELS = ["stable", "rc"] as const

export type OpenAgentsDesktopChannel =
  (typeof OPENAGENTS_DESKTOP_CHANNELS)[number]

export type OpenAgentsDesktopUpdateManifest = Readonly<{
  schema: typeof OPENAGENTS_DESKTOP_UPDATE_SCHEMA
  app: typeof OPENAGENTS_DESKTOP_PRODUCT
  channel: OpenAgentsDesktopChannel
  version: string
  artifactName: string
  artifactSha256: string
  artifactByteLength: number
  releasedAt: string
  notesRef?: string
}>

export type OpenAgentsDesktopUpdateSignature = Readonly<{
  alg: "ed25519"
  kid: string
  sha256: string
  signature: string
}>

export type OpenAgentsDesktopRelease = Readonly<{
  channel: OpenAgentsDesktopChannel
  manifest: OpenAgentsDesktopUpdateManifest
  manifestBytes: Uint8Array
  signature: OpenAgentsDesktopUpdateSignature
  artifactUrl: string
}>

const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/
const SHA256 = /^[0-9a-f]{64}$/
const ARTIFACT = /^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$/
const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,119}$/
const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/

export const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex")

const record = (value: unknown): Record<string, unknown> | null =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown> : null

export const decodeOpenAgentsDesktopManifest = (
  value: unknown,
): OpenAgentsDesktopUpdateManifest | null => {
  const row = record(value)
  if (row === null || row.schema !== OPENAGENTS_DESKTOP_UPDATE_SCHEMA ||
    row.app !== OPENAGENTS_DESKTOP_PRODUCT ||
    (row.channel !== "stable" && row.channel !== "rc") ||
    typeof row.version !== "string" || !VERSION.test(row.version) ||
    (row.channel === "stable" && row.version.includes("-rc.")) ||
    typeof row.artifactName !== "string" || !ARTIFACT.test(row.artifactName) ||
    typeof row.artifactSha256 !== "string" || !SHA256.test(row.artifactSha256) ||
    !Number.isSafeInteger(row.artifactByteLength) || Number(row.artifactByteLength) <= 0 ||
    typeof row.releasedAt !== "string" || !ISO.test(row.releasedAt) ||
    (row.notesRef !== undefined &&
      (typeof row.notesRef !== "string" || !PUBLIC_REF.test(row.notesRef)))) return null
  return row as OpenAgentsDesktopUpdateManifest
}

export const decodeOpenAgentsDesktopSignature = (
  value: unknown,
): OpenAgentsDesktopUpdateSignature | null => {
  const row = record(value)
  if (row === null || row.alg !== "ed25519" ||
    typeof row.kid !== "string" || row.kid.length < 1 || row.kid.length > 64 ||
    typeof row.sha256 !== "string" || !SHA256.test(row.sha256) ||
    typeof row.signature !== "string" || row.signature.length < 1 ||
    row.signature.length > 512) return null
  return row as OpenAgentsDesktopUpdateSignature
}

export const admitOpenAgentsDesktopRelease = (input: Readonly<{
  manifestBytes: Uint8Array
  signature: unknown
  artifactUrl: string
}>): OpenAgentsDesktopRelease => {
  let raw: unknown
  try { raw = JSON.parse(new TextDecoder().decode(input.manifestBytes)) }
  catch { throw new Error("OpenAgents Desktop manifest JSON is invalid") }
  const manifest = decodeOpenAgentsDesktopManifest(raw)
  const signature = decodeOpenAgentsDesktopSignature(input.signature)
  if (manifest === null || signature === null) {
    throw new Error("OpenAgents Desktop release boundary rejected")
  }
  if (signature.sha256 !== sha256(input.manifestBytes)) {
    throw new Error("OpenAgents Desktop manifest digest mismatch")
  }
  const url = new URL(input.artifactUrl)
  if (url.protocol !== "https:" || url.username !== "" || url.password !== "") {
    throw new Error("OpenAgents Desktop artifact URL must be credential-free HTTPS")
  }
  return {
    channel: manifest.channel,
    manifest,
    manifestBytes: input.manifestBytes,
    signature,
    artifactUrl: url.toString(),
  }
}
