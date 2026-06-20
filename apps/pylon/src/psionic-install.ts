import { createHash } from "node:crypto"
import { mkdir, chmod, writeFile } from "node:fs/promises"
import { arch, platform, totalmem } from "node:os"
import { dirname, join } from "node:path"
import type { BootstrapSummary } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

export type PsionicInstallKind = "binary" | "model"
export type PsionicInstallState = "installed" | "blocked" | "failed"

export type PsionicInstallProjection = {
  schema: "openagents.pylon.psionic_install.v0.3"
  kind: PsionicInstallKind
  state: PsionicInstallState
  channel?: string
  modelKey?: string
  platformRef: string
  backendRef: string | null
  artifactRef: string | null
  digestRef: string | null
  cacheRef: string | null
  blockerRefs: string[]
  contentRedacted: true
}

export type PsionicInstallOptions = {
  channel?: string
  modelKey?: string
  manifestUrl?: string
  consent?: boolean
  env?: Record<string, string | undefined>
  fetch?: typeof fetch
  platform?: NodeJS.Platform
  arch?: string
  totalMemoryBytes?: number
  availableDiskBytes?: number
}

type PsionicReleaseManifest = {
  schema: "openagents.psionic.release_manifest.v0.3"
  channel: string
  version: string
  platform: string
  binary: {
    url: string
    sha256: string
    artifactRef: string
    binaryRef: string
  }
}

type PsionicModelManifest = {
  schema: "openagents.psionic.model_artifact_manifest.v0.3"
  modelKey: string
  modelRef: string
  url: string
  sha256: string
  artifactRef: string
}

const SUPPORTED_PLATFORM_REFS = new Set(["darwin-arm64", "linux-x64", "linux-arm64"])
const DEFAULT_BINARY_MEMORY_BYTES = 4 * 1024 * 1024 * 1024
const DEFAULT_MODEL_MEMORY_BYTES = 6 * 1024 * 1024 * 1024
const DEFAULT_BINARY_DISK_BYTES = 256 * 1024 * 1024
const DEFAULT_MODEL_DISK_BYTES = 3 * 1024 * 1024 * 1024

export async function installPsionicBinary(
  summary: BootstrapSummary,
  options: PsionicInstallOptions = {},
): Promise<PsionicInstallProjection> {
  const machine = checkPsionicInstallMachine("binary", options)
  if (machine.blockerRefs.length > 0) return blockedProjection("binary", machine, options)

  const manifestUrl = options.manifestUrl ?? options.env?.PYLON_PSIONIC_RELEASE_MANIFEST_URL
  if (!manifestUrl) {
    return blockedProjection("binary", machine, options, ["blocker.psionic_installer.release_manifest_unconfigured"])
  }

  const manifest = await fetchJson<PsionicReleaseManifest>(manifestUrl, options.fetch)
  if (manifest.schema !== "openagents.psionic.release_manifest.v0.3" || manifest.platform !== machine.platformRef) {
    return blockedProjection("binary", machine, options, ["blocker.psionic_installer.release_manifest_unverified"])
  }
  if (!isPublicRef(manifest.binary.artifactRef) || !isPublicRef(manifest.binary.binaryRef) || !isSha256(manifest.binary.sha256)) {
    return blockedProjection("binary", machine, options, ["blocker.psionic_installer.release_manifest_unverified"])
  }

  const bytes = await fetchBytes(manifest.binary.url, options.fetch)
  const digest = sha256(bytes)
  if (digest !== manifest.binary.sha256.toLowerCase()) {
    return blockedProjection("binary", machine, options, ["blocker.psionic_installer.artifact_digest_mismatch"])
  }

  const filePath = psionicBinaryPath(summary, digest)
  await mkdir(dirname(filePath), { recursive: true })
  await writeFile(filePath, bytes)
  await chmod(filePath, 0o755)

  return safeProjection({
    schema: "openagents.pylon.psionic_install.v0.3",
    kind: "binary",
    state: "installed",
    channel: manifest.channel,
    platformRef: machine.platformRef,
    backendRef: manifest.binary.binaryRef,
    artifactRef: manifest.binary.artifactRef,
    digestRef: `artifact.digest.sha256.${digest}`,
    cacheRef: `cache.psionic.binary.sha256.${digest.slice(0, 16)}`,
    blockerRefs: [],
    contentRedacted: true,
  })
}

export async function installPsionicModelArtifact(
  summary: BootstrapSummary,
  options: PsionicInstallOptions,
): Promise<PsionicInstallProjection> {
  const machine = checkPsionicInstallMachine("model", options)
  if (machine.blockerRefs.length > 0) return blockedProjection("model", machine, options)

  const modelKey = options.modelKey
  if (!modelKey) return blockedProjection("model", machine, options, ["blocker.psionic_installer.model_key_missing"])
  const manifestUrl = options.manifestUrl ?? options.env?.[`PYLON_PSIONIC_MODEL_MANIFEST_${envKey(modelKey)}`]
  if (!manifestUrl) return blockedProjection("model", machine, options, ["blocker.psionic_installer.model_manifest_unconfigured"])

  const manifest = await fetchJson<PsionicModelManifest>(manifestUrl, options.fetch)
  if (
    manifest.schema !== "openagents.psionic.model_artifact_manifest.v0.3" ||
    manifest.modelKey !== modelKey ||
    !isPublicRef(manifest.modelRef) ||
    !isPublicRef(manifest.artifactRef) ||
    !isSha256(manifest.sha256)
  ) {
    return blockedProjection("model", machine, options, ["blocker.psionic_installer.model_manifest_unverified"])
  }

  const bytes = await fetchBytes(manifest.url, options.fetch)
  const digest = sha256(bytes)
  if (digest !== manifest.sha256.toLowerCase()) {
    return blockedProjection("model", machine, options, ["blocker.psionic_installer.artifact_digest_mismatch"])
  }

  const filePath = psionicModelPath(summary, modelKey, digest)
  await mkdir(dirname(filePath), { recursive: true })
  await writeFile(filePath, bytes)

  return safeProjection({
    schema: "openagents.pylon.psionic_install.v0.3",
    kind: "model",
    state: "installed",
    modelKey,
    platformRef: machine.platformRef,
    backendRef: manifest.modelRef,
    artifactRef: manifest.artifactRef,
    digestRef: `artifact.digest.sha256.${digest}`,
    cacheRef: `cache.psionic.model.${modelKey}.sha256.${digest.slice(0, 16)}`,
    blockerRefs: [],
    contentRedacted: true,
  })
}

export function checkPsionicInstallMachine(kind: PsionicInstallKind, options: PsionicInstallOptions = {}) {
  const currentPlatform = options.platform ?? platform()
  const currentArch = options.arch ?? arch()
  const platformRef = `${currentPlatform}-${currentArch}`
  const blockerRefs = new Set<string>()

  if (currentPlatform !== "darwin" && currentPlatform !== "linux") {
    blockerRefs.add("blocker.psionic_installer.unsupported_platform")
  }
  if (!SUPPORTED_PLATFORM_REFS.has(platformRef)) {
    blockerRefs.add("blocker.psionic_installer.unsupported_architecture")
  }
  if (options.consent !== true) {
    blockerRefs.add("blocker.psionic_installer.operator_consent_required")
  }
  if ((options.totalMemoryBytes ?? totalmem()) < (kind === "model" ? DEFAULT_MODEL_MEMORY_BYTES : DEFAULT_BINARY_MEMORY_BYTES)) {
    blockerRefs.add("blocker.psionic_installer.memory_budget_unmet")
  }
  if ((options.availableDiskBytes ?? Number.POSITIVE_INFINITY) < (kind === "model" ? DEFAULT_MODEL_DISK_BYTES : DEFAULT_BINARY_DISK_BYTES)) {
    blockerRefs.add("blocker.psionic_installer.disk_budget_unmet")
  }
  if (options.env?.PYLON_PSIONIC_MODEL_WORKLOAD_ACTIVE === "1") {
    blockerRefs.add("blocker.psionic_installer.competing_model_workload")
  }

  return {
    platformRef,
    backendRef: currentPlatform === "darwin" && currentArch === "arm64" ? "backend.psionic.metal" : "backend.psionic.cpu",
    blockerRefs: [...blockerRefs],
  }
}

export function psionicBinaryPath(summary: BootstrapSummary, digest: string) {
  return join(summary.paths.cache, "psionic", "binaries", "sha256", digest, "psionic-openai-server")
}

export function psionicModelPath(summary: BootstrapSummary, modelKey: string, digest: string) {
  return join(summary.paths.cache, "psionic", "models", "sha256", digest, `${modelKey}.gguf`)
}

function blockedProjection(
  kind: PsionicInstallKind,
  machine: ReturnType<typeof checkPsionicInstallMachine>,
  options: PsionicInstallOptions,
  extraBlockers: string[] = [],
): PsionicInstallProjection {
  return safeProjection({
    schema: "openagents.pylon.psionic_install.v0.3",
    kind,
    state: "blocked",
    channel: options.channel,
    modelKey: options.modelKey,
    platformRef: machine.platformRef,
    backendRef: machine.backendRef,
    artifactRef: null,
    digestRef: null,
    cacheRef: null,
    blockerRefs: [...new Set([...machine.blockerRefs, ...extraBlockers])],
    contentRedacted: true,
  })
}

function safeProjection(projection: PsionicInstallProjection): PsionicInstallProjection {
  assertPublicProjectionSafe(projection)
  return projection
}

async function fetchJson<T>(url: string, fetchImpl: typeof fetch = fetch): Promise<T> {
  const response = await fetchImpl(url)
  if (!response.ok) throw new Error(`manifest fetch failed: HTTP ${response.status}`)
  return await response.json() as T
}

async function fetchBytes(url: string, fetchImpl: typeof fetch = fetch): Promise<Uint8Array> {
  const response = await fetchImpl(url)
  if (!response.ok) throw new Error(`artifact fetch failed: HTTP ${response.status}`)
  return new Uint8Array(await response.arrayBuffer())
}

function sha256(bytes: Uint8Array) {
  return createHash("sha256").update(bytes).digest("hex")
}

function isSha256(value: string) {
  return /^[a-f0-9]{64}$/i.test(value)
}

function isPublicRef(value: string) {
  return /^[a-z][a-z0-9._-]+$/.test(value)
}

function envKey(value: string) {
  return value.toUpperCase().replace(/[^A-Z0-9]+/g, "_")
}
