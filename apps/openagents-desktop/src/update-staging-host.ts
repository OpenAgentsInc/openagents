import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  type UpdateChannel,
  type UpdateManifest,
  isMonotonicUpgrade,
  verifyArtifactDigest,
  verifySignedUpdateManifest,
} from "./update-contract.ts"
import { assertCredentialFreeHttpsUrl, decodeUpdateManifest } from "./release-publish.ts"

const MAX_MANIFEST_BYTES = 32 * 1024
const MAX_SIGNATURE_BYTES = 8 * 1024
const MAX_RELEASE_BYTES = 16 * 1024
const MAX_ARTIFACT_BYTES = 2 * 1024 * 1024 * 1024

export type DesktopUpdateProjection = Readonly<{
  phase: "current" | "checking" | "available" | "downloading" | "staged" | "rejected"
  channel: UpdateChannel
  installedVersion: string
  candidateVersion: string | null
  reason: string | null
}>

type UpdateDocument = Readonly<{
  version: 1
  channel: UpdateChannel
  installedVersion: string
  candidate: UpdateManifest | null
  artifactUrl: string | null
  stagedArtifactName: string | null
  reason: string | null
}>

type ReleasePointer = Readonly<{
  channel: UpdateChannel
  version: string
  artifactName: string
  artifactUrl: string
}>

export type DesktopUpdateStagingHost = Readonly<{
  snapshot: () => DesktopUpdateProjection
  check: () => Promise<DesktopUpdateProjection>
  download: () => Promise<DesktopUpdateProjection>
  openInstaller: () => Promise<DesktopUpdateProjection>
}>

const decodeReleasePointer = (value: unknown): ReleasePointer | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if ((row.channel !== "stable" && row.channel !== "rc") ||
    typeof row.version !== "string" || typeof row.artifactName !== "string" ||
    typeof row.artifactUrl !== "string") return null
  try {
    assertCredentialFreeHttpsUrl(row.artifactUrl)
  } catch {
    return null
  }
  return row as ReleasePointer
}

const readDocument = (
  file: string,
  installedVersion: string,
  channel: UpdateChannel,
): UpdateDocument => {
  try {
    const raw = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>
    const candidate = raw.candidate === null ? null : decodeUpdateManifest(raw.candidate)
    if (raw.version !== 1 || raw.channel !== channel || raw.installedVersion !== installedVersion ||
      (raw.candidate !== null && candidate === null) ||
      (raw.artifactUrl !== null && typeof raw.artifactUrl !== "string") ||
      (raw.stagedArtifactName !== null && typeof raw.stagedArtifactName !== "string") ||
      (raw.stagedArtifactName !== null && raw.stagedArtifactName !== candidate?.artifactName) ||
      (raw.reason !== null && typeof raw.reason !== "string")) throw new Error("invalid")
    if (typeof raw.artifactUrl === "string") assertCredentialFreeHttpsUrl(raw.artifactUrl)
    return {
      version: 1,
      channel,
      installedVersion,
      candidate,
      artifactUrl: raw.artifactUrl as string | null,
      stagedArtifactName: raw.stagedArtifactName as string | null,
      reason: raw.reason as string | null,
    }
  } catch {
    return { version: 1, channel, installedVersion, candidate: null, artifactUrl: null, stagedArtifactName: null, reason: null }
  }
}

const writeDocument = (file: string, value: UpdateDocument): void => {
  mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
  const temporary = `${file}.tmp`
  writeFileSync(temporary, JSON.stringify(value), { encoding: "utf8", mode: 0o600 })
  if (process.platform !== "win32") chmodSync(temporary, 0o600)
  renameSync(temporary, file)
}

const boundedBytes = async (response: Response, maximum: number): Promise<Uint8Array> => {
  if (!response.ok) throw new Error("feed_unavailable")
  const declared = Number(response.headers.get("content-length"))
  if (Number.isFinite(declared) && declared > maximum) throw new Error("response_too_large")
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (bytes.byteLength > maximum) throw new Error("response_too_large")
  return bytes
}

export const openDesktopUpdateStagingHost = (input: Readonly<{
  root: string
  installedVersion: string
  channel: UpdateChannel
  fetch?: typeof globalThis.fetch
  pin?: PinnedReleaseKey
  openPath: (artifactPath: string) => Promise<string>
  baseUrl?: string
}>): DesktopUpdateStagingHost => {
  const fetchImpl = input.fetch ?? globalThis.fetch
  const pin = input.pin ?? PRODUCTION_RELEASE_KEY_PIN
  const baseUrl = input.baseUrl ?? `https://updates.openagents.com/desktop/openagents/${input.channel}`
  const documentFile = path.join(input.root, "state.json")
  let document = readDocument(documentFile, input.installedVersion, input.channel)
  let transient: DesktopUpdateProjection["phase"] | null = null
  let busy = false

  const stagedPath = (): string | null => document.stagedArtifactName === null
    ? null
    : path.join(input.root, document.stagedArtifactName)
  const clearStaged = (): void => {
    const staged = stagedPath()
    if (staged !== null) rmSync(staged, { force: true })
  }
  const snapshot = (): DesktopUpdateProjection => ({
    phase: transient ?? (document.reason !== null
      ? "rejected"
      : document.stagedArtifactName !== null && stagedPath() !== null && existsSync(stagedPath()!)
        ? "staged"
        : document.candidate !== null ? "available" : "current"),
    channel: input.channel,
    installedVersion: input.installedVersion,
    candidateVersion: document.candidate?.version ?? null,
    reason: document.reason,
  })
  const reject = (reason: string): DesktopUpdateProjection => {
    clearStaged()
    document = { ...document, candidate: null, artifactUrl: null, stagedArtifactName: null, reason: reason.slice(0, 120) }
    writeDocument(documentFile, document)
    transient = null
    return snapshot()
  }

  const check = async (): Promise<DesktopUpdateProjection> => {
    if (busy) return snapshot()
    busy = true
    transient = "checking"
    try {
      const [manifestResponse, signatureResponse, releaseResponse] = await Promise.all([
        fetchImpl(`${baseUrl}/manifest.json`),
        fetchImpl(`${baseUrl}/manifest.sig.json`),
        fetchImpl(`${baseUrl}/release.json`),
      ])
      const manifestBytes = await boundedBytes(manifestResponse, MAX_MANIFEST_BYTES)
      const signatureBytes = await boundedBytes(signatureResponse, MAX_SIGNATURE_BYTES)
      const releaseBytes = await boundedBytes(releaseResponse, MAX_RELEASE_BYTES)
      let signature: unknown
      let release: unknown
      try {
        signature = JSON.parse(new TextDecoder().decode(signatureBytes))
        release = JSON.parse(new TextDecoder().decode(releaseBytes))
      } catch {
        return reject("feed_schema_invalid")
      }
      const verified = verifySignedUpdateManifest(manifestBytes, signature, pin, input.channel)
      if (!verified.ok) return reject(verified.reason)
      const pointer = decodeReleasePointer(release)
      if (pointer === null || pointer.channel !== verified.manifest.channel ||
        pointer.version !== verified.manifest.version || pointer.artifactName !== verified.manifest.artifactName) {
        return reject("release_pointer_mismatch")
      }
      if (!isMonotonicUpgrade(input.installedVersion, verified.manifest.version, input.channel).admissible) {
        clearStaged()
        document = { ...document, candidate: null, artifactUrl: null, stagedArtifactName: null, reason: null }
      } else {
        clearStaged()
        document = { ...document, candidate: verified.manifest, artifactUrl: pointer.artifactUrl, stagedArtifactName: null, reason: null }
      }
      writeDocument(documentFile, document)
      transient = null
      return snapshot()
    } catch (error) {
      return reject(error instanceof Error ? error.message : "update_check_failed")
    } finally {
      busy = false
    }
  }

  const download = async (): Promise<DesktopUpdateProjection> => {
    if (busy || document.candidate === null || document.artifactUrl === null) return snapshot()
    busy = true
    transient = "downloading"
    try {
      if (document.candidate.artifactByteLength > MAX_ARTIFACT_BYTES) return reject("artifact_too_large")
      const bytes = await boundedBytes(await fetchImpl(document.artifactUrl), MAX_ARTIFACT_BYTES)
      if (!verifyArtifactDigest(document.candidate, bytes)) return reject("artifact_rejected")
      mkdirSync(input.root, { recursive: true, mode: 0o700 })
      const name = document.candidate.artifactName
      const temporary = path.join(input.root, `${name}.tmp`)
      writeFileSync(temporary, bytes, { mode: 0o600 })
      const destination = path.join(input.root, name)
      renameSync(temporary, destination)
      document = { ...document, stagedArtifactName: name, reason: null }
      writeDocument(documentFile, document)
      transient = null
      return snapshot()
    } catch (error) {
      return reject(error instanceof Error ? error.message : "update_download_failed")
    } finally {
      busy = false
    }
  }

  const openInstaller = async (): Promise<DesktopUpdateProjection> => {
    const artifact = stagedPath()
    if (artifact === null || !existsSync(artifact)) return snapshot()
    const error = await input.openPath(artifact)
    if (error !== "") return reject("installer_open_failed")
    return snapshot()
  }

  return { snapshot, check, download, openInstaller }
}
