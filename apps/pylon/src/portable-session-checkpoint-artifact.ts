import { Runtime } from "@openagentsinc/runtime-platform"
import { createCipheriv, createDecipheriv, createHash, randomBytes, randomUUID } from "node:crypto"
import { chmod, lstat, mkdir, open, readFile, readdir, readlink, rename, rm, stat } from "node:fs/promises"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"

import { canonicalJson } from "@openagentsinc/khala-sync"
import {
  PortableCheckpointCustodyObjectManifestSchema,
  PylonPortableCheckpointBundleSchema,
} from "@openagentsinc/portable-session-contract"
import { Schema } from "effect"
import type {
  PortableCheckpointCustodyObjectManifest,
  PortableCommandExecutionClaim,
  PylonPortableCheckpointBundle,
} from "@openagentsinc/portable-session-contract"

// The artifact resolver/store contract types live in the runtime-neutral
// contract package so non-Bun consumers (the Khala Sync server provisioner,
// the Worker typecheck graph) never import this Bun-typed module for its
// types. Re-exported here so existing Pylon-side importers keep their paths.
export type {
  PortableCheckpointArtifact,
  PortableCheckpointArtifactResolver,
  PortableCheckpointArtifactResolverInput,
  PortableCheckpointArtifactStore,
} from "@openagentsinc/portable-session-contract"
import type {
  PortableCheckpointArtifact,
  PortableCheckpointArtifactResolver,
  PortableCheckpointArtifactResolverInput,
} from "@openagentsinc/portable-session-contract"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_PATH = /(?:^|\/)(?:\.env(?:\.(?!example$)[^/]*)?|auth\.json|credentials(?:\.json)?|id_(?:rsa|dsa|ecdsa|ed25519)|\.npmrc|\.pypirc)$/iu
const FORBIDDEN_BYTES = /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|\bgh[pousr]_[A-Za-z0-9]{20,}\b|\bsk-[A-Za-z0-9_-]{20,}\b|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]{20,}=*/u
const DEFAULT_MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
const MAX_CUSTODY_METADATA_BYTES = 4 * 1024 * 1024
const AES_256_KEY_BYTES = 32
const AES_GCM_NONCE_BYTES = 12
const AES_GCM_TAG_BYTES = 16
const DEFAULT_RETENTION_SECONDS = 7 * 24 * 60 * 60
const DEFAULT_MAX_CUSTODY_FILES = 4_096
const DEFAULT_ORPHAN_TEMP_MAX_AGE_MS = 60 * 60 * 1_000

type CustodyLifecycleStep =
  | "delete_intent_durable"
  | "delete_object_removed"
  | "rewrap_ciphertext_durable"
  | "rewrap_replaced"

type CustodyLifecycleOptions = Readonly<{
  now?: () => Date
  retentionSeconds?: number
  maxCustodyFiles?: number
  orphanTempMaxAgeMs?: number
  faultInjector?: (step: CustodyLifecycleStep) => Promise<void> | void
}>

export type PylonPortableCheckpointCustodyKeyProvider = Readonly<{
  loadKey: (keyRef: string) => Promise<Uint8Array> | Uint8Array
}>

export type PylonPortableCheckpointCustodyConfig =
  | Readonly<{
    custodyDirectory: string
    policy: "owner_managed" | "openagents_managed"
    keyRef: string
    keyProvider: PylonPortableCheckpointCustodyKeyProvider
    maxArtifactBytes?: number
  }> & CustodyLifecycleOptions
  | Readonly<{
    custodyDirectory: string
    policy: "owner_device_not_required"
    maxArtifactBytes?: number
  }> & CustodyLifecycleOptions

export type PylonPortableCheckpointDeletionReceipt = Readonly<{
  schema: "openagents.portable_checkpoint_artifact_deletion_receipt.v1"
  receiptRef: string
  operationRef: string
  ownerRef: string
  sessionRef: string
  checkpointRef: string
  bundleDigest: `sha256:${string}`
  artifactDigest: `sha256:${string}`
  objectRef: string
  policy: PylonPortableCheckpointCustodyConfig["policy"]
  keyRef: string | null
  state: "deleted"
  verifiedAbsent: true
  occurredAt: string
  publicSafe: true
}>

export type PylonPortableCheckpointRewrapReceipt = Readonly<{
  schema: "openagents.portable_checkpoint_artifact_rewrap_receipt.v1"
  receiptRef: string
  operationRef: string
  checkpointRef: string
  objectRef: string
  policy: "owner_managed" | "openagents_managed"
  previousKeyRef: string
  keyRef: string
  digest: `sha256:${string}`
  state: "rewrapped"
  verified: true
  occurredAt: string
  publicSafe: true
}>

export class PylonPortableCheckpointArtifactError extends Error {
  readonly _tag = "PylonPortableCheckpointArtifactError"
  override readonly name = "PylonPortableCheckpointArtifactError"

  constructor(
    readonly code:
      | "artifact_too_large"
      | "custody_policy_mismatch"
      | "decrypt_failed"
      | "deletion_failed"
      | "invalid_binding"
      | "key_ref_mismatch"
      | "key_unavailable"
      | "plaintext_downgrade"
      | "private_material"
      | "repository_mismatch"
      | "replay_conflict"
      | "retention_expired"
      | "rewrap_failed"
      | "transport_invalid"
      | "unavailable",
    message: string,
  ) {
    super(message)
  }
}

type Source = Readonly<{
  bundle: PylonPortableCheckpointBundle
  workingDirectory: string
}>

type PostImageEntry = Readonly<{
  path: string
  mode: number
  kind: "file" | "symlink"
  linkTarget?: string
  bytes: Uint8Array
  digest: `sha256:${string}`
}>

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const isSha256 = (value: unknown): value is `sha256:${string}` =>
  typeof value === "string" && /^sha256:[a-f0-9]{64}$/u.test(value)

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`

const runGit = async (cwd: string, args: ReadonlyArray<string>): Promise<Uint8Array> => {
  const proc = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  const [stdout, exitCode] = await Promise.all([
    new Response(proc.stdout).bytes(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new PylonPortableCheckpointArtifactError("unavailable", "checkpoint repository export failed")
  }
  return new Uint8Array(stdout)
}

const safeRelativePath = (value: string): string => {
  if (value.length === 0 || value.startsWith("/") || value.includes("\0") ||
      value.split("/").some(part => part === "" || part === "." || part === "..") ||
      value === ".git" || value.startsWith(".git/") || FORBIDDEN_PATH.test(value)) {
    throw new PylonPortableCheckpointArtifactError("private_material", "checkpoint contains an unsafe repository path")
  }
  return value
}

const assertNoPrivateBytes = (bytes: Uint8Array): void => {
  if (FORBIDDEN_BYTES.test(new TextDecoder().decode(bytes))) {
    throw new PylonPortableCheckpointArtifactError("private_material", "checkpoint contains credential-shaped material")
  }
}

const postImageEntries = async (cwd: string): Promise<ReadonlyArray<PostImageEntry>> => {
  const listedBytes = await runGit(cwd, ["ls-files", "-co", "--exclude-standard", "-z"])
  const deletedBytes = await runGit(cwd, ["ls-files", "--deleted", "-z"])
  const deleted = new Set(new TextDecoder().decode(deletedBytes).split("\0").filter(Boolean))
  const listed = new TextDecoder().decode(listedBytes).split("\0")
    .filter(path => path.length > 0 && !deleted.has(path)).sort()
  listedBytes.fill(0)
  deletedBytes.fill(0)
  const root = resolve(cwd)
  const entries: PostImageEntry[] = []
  try {
    for (const rawPath of listed) {
      const path = safeRelativePath(rawPath)
      const absolute = join(root, path)
      const info = await lstat(absolute)
      if (!info.isFile() && !info.isSymbolicLink()) {
        throw new PylonPortableCheckpointArtifactError("private_material", "checkpoint contains an unsupported repository entry")
      }
      let bytes: Uint8Array
      let kind: PostImageEntry["kind"]
      let linkTarget: string | undefined
      if (info.isSymbolicLink()) {
        kind = "symlink"
        linkTarget = await readlink(absolute)
        if (isAbsolute(linkTarget) || linkTarget.includes("\\") ||
            linkTarget.split("/").some(part => part === "..") ||
            relative(root, resolve(dirname(absolute), linkTarget)).startsWith("..")) {
          throw new PylonPortableCheckpointArtifactError("private_material", "checkpoint symlink escapes its repository")
        }
        bytes = new TextEncoder().encode(linkTarget)
      } else {
        kind = "file"
        bytes = await readFile(absolute)
      }
      assertNoPrivateBytes(bytes)
      entries.push({
        path,
        mode: kind === "symlink" ? 0o120000 : ((info.mode & 0o111) === 0 ? 0o644 : 0o755),
        kind,
        ...(linkTarget === undefined ? {} : { linkTarget }),
        bytes,
        digest: sha256(bytes),
      })
    }
    return entries
  } catch (error) {
    for (const entry of entries) entry.bytes.fill(0)
    throw error
  }
}

const writeString = (header: Uint8Array, offset: number, length: number, value: string): void => {
  const bytes = new TextEncoder().encode(value)
  if (bytes.byteLength > length) {
    throw new PylonPortableCheckpointArtifactError("unavailable", "checkpoint archive path exceeds ustar limits")
  }
  header.set(bytes, offset)
}

const writeOctal = (header: Uint8Array, offset: number, length: number, value: number): void => {
  writeString(header, offset, length, value.toString(8).padStart(length - 1, "0"))
}

const tarPath = (path: string): Readonly<{ name: string; prefix: string }> => {
  if (new TextEncoder().encode(path).byteLength <= 100) return { name: path, prefix: "" }
  for (let index = path.lastIndexOf("/"); index > 0; index = path.lastIndexOf("/", index - 1)) {
    const prefix = path.slice(0, index)
    const name = path.slice(index + 1)
    if (new TextEncoder().encode(prefix).byteLength <= 155 && new TextEncoder().encode(name).byteLength <= 100) {
      return { name, prefix }
    }
  }
  throw new PylonPortableCheckpointArtifactError("unavailable", "checkpoint archive path exceeds ustar limits")
}

const tarEntry = (
  path: string,
  bytes: Uint8Array,
  mode: number,
  kind: "file" | "symlink" = "file",
  linkTarget = "",
): Uint8Array => {
  const header = new Uint8Array(512)
  const split = tarPath(path)
  writeString(header, 0, 100, split.name)
  writeOctal(header, 100, 8, mode)
  writeOctal(header, 108, 8, 0)
  writeOctal(header, 116, 8, 0)
  writeOctal(header, 124, 12, kind === "file" ? bytes.byteLength : 0)
  writeOctal(header, 136, 12, 0)
  header.fill(0x20, 148, 156)
  header[156] = kind === "file" ? 0x30 : 0x32
  if (kind === "symlink") writeString(header, 157, 100, linkTarget)
  writeString(header, 257, 6, "ustar")
  writeString(header, 263, 2, "00")
  writeString(header, 345, 155, split.prefix)
  const checksum = header.reduce((sum, byte) => sum + byte, 0)
  writeString(header, 148, 8, `${checksum.toString(8).padStart(6, "0")}\0 `)
  const contentLength = kind === "file" ? Math.ceil(bytes.byteLength / 512) * 512 : 0
  const output = new Uint8Array(512 + contentLength)
  output.set(header)
  if (kind === "file") output.set(bytes, 512)
  return output
}

const archive = (entries: ReadonlyArray<Readonly<{
  path: string
  bytes: Uint8Array
  mode: number
  kind?: "file" | "symlink"
  linkTarget?: string
}>>): Uint8Array => {
  const encoded = entries.map(entry => tarEntry(
    entry.path,
    entry.bytes,
    entry.mode,
    entry.kind,
    entry.linkTarget,
  ))
  const length = encoded.reduce((sum, entry) => sum + entry.byteLength, 1024)
  const output = new Uint8Array(length)
  let offset = 0
  for (const entry of encoded) {
    output.set(entry, offset)
    offset += entry.byteLength
  }
  return output
}

const sameBundle = (left: PylonPortableCheckpointBundle, right: PylonPortableCheckpointBundle): boolean =>
  canonicalJson(left) === canonicalJson(right)

const validateBundle = (value: unknown): PylonPortableCheckpointBundle => {
  let bundle: PylonPortableCheckpointBundle
  try {
    bundle = Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema)(value)
  } catch {
    throw new PylonPortableCheckpointArtifactError(
      "transport_invalid",
      "checkpoint custody transport bundle is invalid",
    )
  }
  const { digest, ...checkpointPayload } = bundle.checkpoint
  if (sha256(canonicalJson(checkpointPayload)) !== digest) {
    throw new PylonPortableCheckpointArtifactError(
      "transport_invalid",
      "checkpoint custody transport checkpoint digest is invalid",
    )
  }
  return bundle
}

type ArtifactTarEntry = Readonly<{
  bytes: Uint8Array
  mode: number
  linkTarget?: string
}>

const tarHeaderText = (header: Uint8Array, offset: number, length: number): string =>
  new TextDecoder().decode(header.subarray(offset, offset + length)).replace(/\0.*$/u, "")

const parseArtifactTar = (bytes: Uint8Array): Map<string, ArtifactTarEntry> => {
  const entries = new Map<string, ArtifactTarEntry>()
  try {
    let offset = 0
    let terminated = false
    while (offset + 512 <= bytes.byteLength) {
      const header = bytes.subarray(offset, offset + 512)
      if (header.every((byte) => byte === 0)) {
        if (
          offset + 1_024 > bytes.byteLength ||
          !bytes.subarray(offset, offset + 1_024).every((byte) => byte === 0) ||
          !bytes.subarray(offset + 1_024).every((byte) => byte === 0)
        ) {
          throw new PylonPortableCheckpointArtifactError(
            "transport_invalid",
            "checkpoint custody transport artifact termination is invalid",
          )
        }
        terminated = true
        break
      }
      const name = tarHeaderText(header, 0, 100)
      const prefix = tarHeaderText(header, 345, 155)
      const path = prefix.length === 0 ? name : `${prefix}/${name}`
      const size = Number.parseInt(tarHeaderText(header, 124, 12).trim() || "0", 8)
      const mode = Number.parseInt(tarHeaderText(header, 100, 8).trim() || "0", 8)
      const kind = header[156]
      const contentStart = offset + 512
      const nextOffset = contentStart + Math.ceil(size / 512) * 512
      if (
        path.length === 0 ||
        entries.has(path) ||
        !Number.isSafeInteger(size) ||
        size < 0 ||
        !Number.isSafeInteger(mode) ||
        (kind !== 0x30 && kind !== 0x32) ||
        nextOffset > bytes.byteLength
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "transport_invalid",
          "checkpoint custody transport artifact archive is invalid",
        )
      }
      entries.set(path, {
        bytes: Uint8Array.from(bytes.subarray(contentStart, contentStart + size)),
        mode,
        ...(kind === 0x32 ? { linkTarget: tarHeaderText(header, 157, 100) } : {}),
      })
      offset = nextOffset
    }
    if (!terminated) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact is incomplete",
      )
    }
    return entries
  } catch (error) {
    for (const entry of entries.values()) entry.bytes.fill(0)
    throw error
  }
}

const validateArtifact = (retained: RetainedArtifact): void => {
  let tarBytes: Uint8Array | undefined
  let entries: Map<string, ArtifactTarEntry> | undefined
  try {
    if (sha256(retained.bytes) !== retained.digest) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact digest is invalid",
      )
    }
    try {
      tarBytes = Runtime.zstdDecompressSync(retained.bytes)
    } catch {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact compression is invalid",
      )
    }
    entries = parseArtifactTar(tarBytes)
    const manifestEntry = entries.get("manifest.json")
    const repositoryBundle = entries.get("repository.bundle")
    if (
      manifestEntry === undefined ||
      repositoryBundle === undefined ||
      manifestEntry.linkTarget !== undefined ||
      repositoryBundle.linkTarget !== undefined ||
      repositoryBundle.bytes.byteLength === 0
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact is incomplete",
      )
    }
    let manifest: Record<string, unknown>
    try {
      const decoded = JSON.parse(new TextDecoder().decode(manifestEntry.bytes)) as unknown
      if (typeof decoded !== "object" || decoded === null || Array.isArray(decoded))
        throw new Error()
      manifest = decoded as Record<string, unknown>
    } catch {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact manifest is invalid",
      )
    }
    if (
      manifest.schema !== "openagents.portable_checkpoint_artifact.v1" ||
      manifest.artifactRef !== retained.artifactRef ||
      manifest.checkpointRef !== retained.bundle.checkpoint.checkpointRef ||
      canonicalJson(manifest.bundle) !== canonicalJson(retained.bundle) ||
      !Array.isArray(manifest.files)
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact binding is invalid",
      )
    }
    const expected = new Set(["manifest.json", "repository.bundle"])
    for (const rawFile of manifest.files) {
      if (typeof rawFile !== "object" || rawFile === null || Array.isArray(rawFile)) {
        throw new PylonPortableCheckpointArtifactError(
          "transport_invalid",
          "checkpoint custody transport artifact inventory is invalid",
        )
      }
      const file = rawFile as Record<string, unknown>
      if (
        typeof file.path !== "string" ||
        typeof file.size !== "number" ||
        !Number.isSafeInteger(file.size) ||
        file.size < 0 ||
        typeof file.sha256 !== "string" ||
        !isSha256(file.sha256) ||
        typeof file.mode !== "number" ||
        ![0o644, 0o755, 0o120000].includes(file.mode)
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "transport_invalid",
          "checkpoint custody transport artifact inventory is invalid",
        )
      }
      safeRelativePath(file.path)
      const entryPath = `post-image/${file.path}`
      const entry = entries.get(entryPath)
      if (
        expected.has(entryPath) ||
        entry === undefined ||
        entry.bytes.byteLength !== file.size ||
        entry.mode !== file.mode ||
        sha256(entry.bytes) !== file.sha256 ||
        (file.mode === 0o120000) !== (typeof file.linkTarget === "string") ||
        (entry.linkTarget ?? undefined) !== (file.linkTarget ?? undefined)
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "transport_invalid",
          "checkpoint custody transport artifact inventory does not match",
        )
      }
      expected.add(entryPath)
    }
    if (entries.size !== expected.size || [...entries.keys()].some((path) => !expected.has(path))) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport artifact contains an unmanifested entry",
      )
    }
  } finally {
    tarBytes?.fill(0)
    if (entries !== undefined) {
      for (const entry of entries.values()) entry.bytes.fill(0)
    }
  }
}

type RetainedArtifact = Readonly<{
  bundle: PylonPortableCheckpointBundle
  artifactRef: string
  digest: `sha256:${string}`
  bytes: Uint8Array
  createdAt: string
  expiresAt: string
}>

type EncryptedCustodyConfig = Extract<
  PylonPortableCheckpointCustodyConfig,
  { policy: "owner_managed" | "openagents_managed" }
>

export type PylonPortableCheckpointLifecycleBinding = Readonly<{
  operationRef: string
  ownerRef: string
  sessionRef: string
  checkpointRef: string
  bundle: PylonPortableCheckpointBundle
}>

export type PylonPortableCheckpointCustodyObject = Readonly<{
  manifest: PortableCheckpointCustodyObjectManifest
  /** Private opaque bytes. Do not log, project, or add these bytes to evidence. */
  bytes: Uint8Array
}>

export type PylonPortableCheckpointCustodyExportInput = Readonly<{
  checkpointRef: string
  sourcePylonRef: string
  commandClaim: PortableCommandExecutionClaim
  byteLimit: number
}>

export type PylonPortableCheckpointCustodyImportInput = Readonly<{
  manifest: unknown
  /** Private opaque bytes. Do not log, project, or add these bytes to evidence. */
  bytes: Uint8Array
}>

export type PylonPortableCheckpointRewrapInput = PylonPortableCheckpointLifecycleBinding &
  Readonly<{
    keyRef: string
    keyProvider: PylonPortableCheckpointCustodyKeyProvider
  }>

const encryptedHeader = (objectRef: string, config: EncryptedCustodyConfig) => ({
  schema: "openagents.portable_checkpoint_artifact_custody_encrypted.v2" as const,
  algorithm: "aes-256-gcm" as const,
  objectRef,
  policy: config.policy,
  keyRef: config.keyRef,
})

const atomicPrivateWrite = async (path: string, bytes: Uint8Array): Promise<void> => {
  const temporary = `${path}.${randomUUID()}.tmp`
  let handle: Awaited<ReturnType<typeof open>> | undefined
  try {
    handle = await open(temporary, "wx", 0o600)
    await handle.writeFile(bytes)
    await handle.sync()
    await handle.close()
    handle = undefined
    await rename(temporary, path)
    await chmod(path, 0o600)
    const directory = await open(dirname(path), "r")
    try {
      await directory.sync()
    } finally {
      await directory.close()
    }
  } finally {
    await handle?.close().catch(() => undefined)
    await rm(temporary, { force: true })
  }
}

export class PylonPortableCheckpointArtifactStore implements PortableCheckpointArtifactResolver {
  private readonly sources = new Map<string, Source>()
  private readonly artifacts = new Map<string, RetainedArtifact>()
  private custody?: PylonPortableCheckpointCustodyConfig
  private readonly maxArtifactBytes: number
  private readonly retentionSeconds: number
  private readonly maxCustodyFiles: number
  private readonly orphanTempMaxAgeMs: number
  private readonly now: () => Date

  constructor(custody?: PylonPortableCheckpointCustodyConfig | string) {
    if (typeof custody === "string") {
      throw new PylonPortableCheckpointArtifactError(
        "plaintext_downgrade",
        "checkpoint artifact custody requires an explicit policy and key binding",
      )
    }
    if (custody !== undefined && !isAbsolute(custody.custodyDirectory)) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact custody must be absolute")
    }
    const maxArtifactBytes = custody?.maxArtifactBytes ?? DEFAULT_MAX_ARTIFACT_BYTES
    const retentionSeconds = custody?.retentionSeconds ?? DEFAULT_RETENTION_SECONDS
    const maxCustodyFiles = custody?.maxCustodyFiles ?? DEFAULT_MAX_CUSTODY_FILES
    const orphanTempMaxAgeMs = custody?.orphanTempMaxAgeMs ?? DEFAULT_ORPHAN_TEMP_MAX_AGE_MS
    if (!Number.isSafeInteger(maxArtifactBytes) || maxArtifactBytes <= 0 ||
        maxArtifactBytes > DEFAULT_MAX_ARTIFACT_BYTES ||
        !Number.isSafeInteger(retentionSeconds) || retentionSeconds <= 0 || retentionSeconds > 31_536_000 ||
        !Number.isSafeInteger(maxCustodyFiles) || maxCustodyFiles <= 0 || maxCustodyFiles > DEFAULT_MAX_CUSTODY_FILES ||
        !Number.isSafeInteger(orphanTempMaxAgeMs) || orphanTempMaxAgeMs < 0 ||
        (custody !== undefined && custody.policy !== "owner_device_not_required" &&
          (!SAFE_REF.test(custody.keyRef) || typeof custody.keyProvider.loadKey !== "function"))) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact custody configuration is invalid")
    }
    this.custody = custody
    this.maxArtifactBytes = maxArtifactBytes
    this.retentionSeconds = retentionSeconds
    this.maxCustodyFiles = maxCustodyFiles
    this.orphanTempMaxAgeMs = orphanTempMaxAgeMs
    this.now = custody?.now ?? (() => new Date())
  }

  private artifactPaths(checkpointRef: string): Readonly<{
    encrypted: string
    plaintext: string
    legacyBytes: string
    legacyMetadata: string
    deletion: string
    objectRef: string
  }> | undefined {
    if (this.custody === undefined) return undefined
    const name = createHash("sha256").update(checkpointRef).digest("hex")
    return {
      encrypted: join(this.custody.custodyDirectory, `${name}.checkpoint.aesgcm`),
      plaintext: join(this.custody.custodyDirectory, `${name}.checkpoint.json`),
      legacyBytes: join(this.custody.custodyDirectory, `${name}.tar.zst`),
      legacyMetadata: join(this.custody.custodyDirectory, `${name}.json`),
      deletion: join(this.custody.custodyDirectory, `${name}.checkpoint.deleted.json`),
      objectRef: `checkpoint-custody:${name}`,
    }
  }

  private nowIso(): string {
    const now = this.now()
    if (!(now instanceof Date) || !Number.isFinite(now.getTime())) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody clock is invalid",
      )
    }
    return now.toISOString()
  }

  private async prepareCustodyDirectory(): Promise<void> {
    const custody = this.custody
    if (custody === undefined) return
    await mkdir(custody.custodyDirectory, { recursive: true, mode: 0o700 })
    await chmod(custody.custodyDirectory, 0o700)
    const names = await readdir(custody.custodyDirectory)
    const nowMs = this.now().getTime()
    await Promise.all(
      names
        .filter((name) =>
          /^[a-f0-9]{64}\.checkpoint\.[A-Za-z0-9.]+\.[A-Za-z0-9-]+\.tmp$/u.test(name),
        )
        .map(async (name) => {
          const path = join(custody.custodyDirectory, name)
          const info = await stat(path)
          if (this.orphanTempMaxAgeMs === 0 || nowMs - info.mtimeMs >= this.orphanTempMaxAgeMs) {
            await rm(path, { force: true })
          }
        }),
    )
    if ((await readdir(custody.custodyDirectory)).length > this.maxCustodyFiles) {
      throw new PylonPortableCheckpointArtifactError(
        "artifact_too_large",
        "checkpoint custody exceeds its file-count bound",
      )
    }
  }

  private async ensureCustodyCapacity(paths: ReadonlyArray<string>): Promise<void> {
    const custody = this.custody
    if (custody === undefined) return
    const names = await readdir(custody.custodyDirectory)
    const additions = (
      await Promise.all(
        [...new Set(paths)].map(
          async (path): Promise<number> => ((await this.exists(path)) ? 0 : 1),
        ),
      )
    ).reduce((sum, value) => sum + value, 0)
    if (names.length + additions > this.maxCustodyFiles) {
      throw new PylonPortableCheckpointArtifactError(
        "artifact_too_large",
        "checkpoint custody exceeds its file-count bound",
      )
    }
  }

  private lifecycleBinding(input: PylonPortableCheckpointLifecycleBinding): Readonly<{
    operationRef: string
    ownerRef: string
    sessionRef: string
    checkpointRef: string
    bundleDigest: `sha256:${string}`
  }> {
    if (
      ![input.operationRef, input.ownerRef, input.sessionRef, input.checkpointRef].every((value) =>
        SAFE_REF.test(value),
      ) ||
      input.ownerRef !== input.bundle.executionBinding.ownerRef ||
      input.sessionRef !== input.bundle.checkpoint.sessionRef ||
      input.checkpointRef !== input.bundle.checkpoint.checkpointRef
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody lifecycle binding is invalid",
      )
    }
    return {
      operationRef: input.operationRef,
      ownerRef: input.ownerRef,
      sessionRef: input.sessionRef,
      checkpointRef: input.checkpointRef,
      bundleDigest: sha256(canonicalJson(input.bundle)),
    }
  }

  private async syncCustodyDirectory(): Promise<void> {
    const custody = this.custody
    if (custody === undefined) return
    const directory = await open(custody.custodyDirectory, "r")
    try {
      await directory.sync()
    } finally {
      await directory.close()
    }
  }

  private async readLifecycleRecord(path: string): Promise<Record<string, unknown> | undefined> {
    if (!(await this.exists(path))) return undefined
    const bytes = await this.readBounded(path, MAX_CUSTODY_METADATA_BYTES)
    try {
      const decoded = JSON.parse(new TextDecoder().decode(bytes)) as unknown
      if (typeof decoded !== "object" || decoded === null || Array.isArray(decoded)) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody lifecycle record is invalid",
        )
      }
      return decoded as Record<string, unknown>
    } catch (error) {
      if (error instanceof PylonPortableCheckpointArtifactError) throw error
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody lifecycle record is invalid",
      )
    } finally {
      bytes.fill(0)
    }
  }

  private assertLifecycleRecordBinding(
    record: Record<string, unknown>,
    expected: Readonly<Record<string, unknown>>,
  ): void {
    for (const [key, value] of Object.entries(expected)) {
      if (record[key] !== value) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody lifecycle record binding does not match",
        )
      }
    }
  }

  private assertArtifactSize(size: number): void {
    if (!Number.isSafeInteger(size) || size <= 0 || size > this.maxArtifactBytes) {
      throw new PylonPortableCheckpointArtifactError("artifact_too_large", "checkpoint artifact exceeds its custody size bound")
    }
  }

  private maxPayloadBytes(): number {
    return Math.ceil(this.maxArtifactBytes * 4 / 3) + MAX_CUSTODY_METADATA_BYTES
  }

  private async loadKey(config: EncryptedCustodyConfig): Promise<Uint8Array> {
    let provided: Uint8Array
    try {
      provided = await config.keyProvider.loadKey(config.keyRef)
    } catch {
      throw new PylonPortableCheckpointArtifactError("key_unavailable", "checkpoint artifact custody key is unavailable")
    }
    if (!(provided instanceof Uint8Array) || provided.byteLength !== AES_256_KEY_BYTES) {
      throw new PylonPortableCheckpointArtifactError("key_unavailable", "checkpoint artifact custody key is invalid")
    }
    return Uint8Array.from(provided)
  }

  private async exists(path: string): Promise<boolean> {
    try {
      return (await stat(path)).isFile()
    } catch (error) {
      if (error instanceof Error && "code" in error && error.code === "ENOENT") return false
      throw error
    }
  }

  private async readBounded(path: string, maximum: number): Promise<Uint8Array> {
    const info = await stat(path)
    if (!info.isFile() || info.size <= 0 || info.size > maximum) {
      throw new PylonPortableCheckpointArtifactError("artifact_too_large", "checkpoint custody object exceeds its size bound")
    }
    return new Uint8Array(await readFile(path))
  }

  private decodePayload(checkpointRef: string, payloadBytes: Uint8Array): RetainedArtifact {
    if (payloadBytes.byteLength > this.maxPayloadBytes()) {
      throw new PylonPortableCheckpointArtifactError("artifact_too_large", "checkpoint custody payload exceeds its size bound")
    }
    let payload: Record<string, unknown>
    try {
      payload = JSON.parse(new TextDecoder().decode(payloadBytes)) as Record<string, unknown>
    } catch {
      throw new PylonPortableCheckpointArtifactError("decrypt_failed", "checkpoint custody payload is invalid")
    }
    if (
      payload.schema !== "openagents.portable_checkpoint_artifact_custody_payload.v3" ||
      payload.checkpointRef !== checkpointRef ||
      typeof payload.artifactRef !== "string" ||
      !SAFE_REF.test(payload.artifactRef) ||
      typeof payload.digest !== "string" ||
      !/^sha256:[a-f0-9]{64}$/u.test(payload.digest) ||
      typeof payload.bundle !== "object" ||
      payload.bundle === null ||
      typeof payload.createdAt !== "string" ||
      typeof payload.expiresAt !== "string" ||
      !Number.isFinite(Date.parse(payload.createdAt)) ||
      !Number.isFinite(Date.parse(payload.expiresAt)) ||
      Date.parse(payload.expiresAt) <= Date.parse(payload.createdAt) ||
      typeof payload.bytesBase64 !== "string" ||
      !/^[A-Za-z0-9+/]+={0,2}$/u.test(payload.bytesBase64)
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "persisted checkpoint custody binding is invalid",
      )
    }
    const decoded = Buffer.from(payload.bytesBase64, "base64")
    const bytes = Uint8Array.from(decoded)
    decoded.fill(0)
    try {
      this.assertArtifactSize(bytes.byteLength)
      const bundle = payload.bundle as PylonPortableCheckpointBundle
      if (bundle.checkpoint?.checkpointRef !== checkpointRef || sha256(bytes) !== payload.digest) {
        throw new PylonPortableCheckpointArtifactError("invalid_binding", "persisted checkpoint artifact digest is invalid")
      }
      return {
        bundle,
        artifactRef: payload.artifactRef,
        digest: payload.digest as `sha256:${string}`,
        bytes,
        createdAt: payload.createdAt,
        expiresAt: payload.expiresAt,
      }
    } catch (error) {
      bytes.fill(0)
      throw error
    }
  }

  private async decodeEncryptedObject(
    checkpointRef: string,
    paths: NonNullable<ReturnType<typeof this.artifactPaths>>,
    config: EncryptedCustodyConfig,
    envelopeBytes: Uint8Array,
  ): Promise<RetainedArtifact> {
    let key: Uint8Array | undefined
    let plaintext: Uint8Array | undefined
    try {
      const envelope = JSON.parse(new TextDecoder().decode(envelopeBytes)) as Record<string, unknown>
      if (envelope.schema !== "openagents.portable_checkpoint_artifact_custody_encrypted.v2" ||
          envelope.algorithm !== "aes-256-gcm" || envelope.objectRef !== paths.objectRef) {
        throw new PylonPortableCheckpointArtifactError("invalid_binding", "encrypted checkpoint custody envelope binding is invalid")
      }
      if (envelope.policy !== config.policy) {
        throw new PylonPortableCheckpointArtifactError("custody_policy_mismatch", "checkpoint custody policy does not match the encrypted object")
      }
      if (envelope.keyRef !== config.keyRef) {
        throw new PylonPortableCheckpointArtifactError("key_ref_mismatch", "checkpoint custody key reference does not match the encrypted object")
      }
      if (typeof envelope.nonceBase64 !== "string" || typeof envelope.authTagBase64 !== "string" ||
          typeof envelope.ciphertextBase64 !== "string") {
        throw new PylonPortableCheckpointArtifactError("decrypt_failed", "encrypted checkpoint custody fields are invalid")
      }
      const nonce = Uint8Array.from(Buffer.from(envelope.nonceBase64, "base64"))
      const authTag = Uint8Array.from(Buffer.from(envelope.authTagBase64, "base64"))
      const ciphertext = Uint8Array.from(Buffer.from(envelope.ciphertextBase64, "base64"))
      try {
        if (nonce.byteLength !== AES_GCM_NONCE_BYTES || authTag.byteLength !== AES_GCM_TAG_BYTES ||
            ciphertext.byteLength === 0 || ciphertext.byteLength > this.maxPayloadBytes()) {
          throw new PylonPortableCheckpointArtifactError("decrypt_failed", "encrypted checkpoint custody fields are invalid")
        }
        key = await this.loadKey(config)
        const aad = new TextEncoder().encode(canonicalJson(encryptedHeader(paths.objectRef, config)))
        try {
          const decipher = createDecipheriv("aes-256-gcm", key, nonce, { authTagLength: AES_GCM_TAG_BYTES })
          decipher.setAAD(aad)
          decipher.setAuthTag(authTag)
          const decrypted = Buffer.concat([decipher.update(ciphertext), decipher.final()])
          plaintext = Uint8Array.from(decrypted)
          decrypted.fill(0)
        } catch {
          throw new PylonPortableCheckpointArtifactError("decrypt_failed", "encrypted checkpoint custody authentication failed")
        } finally {
          aad.fill(0)
        }
      } finally {
        nonce.fill(0)
        authTag.fill(0)
        ciphertext.fill(0)
      }
      return this.decodePayload(checkpointRef, plaintext)
    } catch (error) {
      if (error instanceof PylonPortableCheckpointArtifactError) throw error
      throw new PylonPortableCheckpointArtifactError("decrypt_failed", "encrypted checkpoint custody envelope is invalid")
    } finally {
      envelopeBytes.fill(0)
      key?.fill(0)
      plaintext?.fill(0)
    }
  }

  private async loadEncrypted(
    checkpointRef: string,
    paths: NonNullable<ReturnType<typeof this.artifactPaths>>,
    config: EncryptedCustodyConfig,
    objectPath = paths.encrypted,
  ): Promise<RetainedArtifact> {
    const envelopeBytes = await this.readBounded(objectPath, this.maximumEncryptedObjectBytes())
    return this.decodeEncryptedObject(checkpointRef, paths, config, envelopeBytes)
  }

  private maximumEncryptedObjectBytes(): number {
    return Math.ceil((this.maxPayloadBytes() * 4) / 3) + 4_096
  }

  private encodePayload(checkpointRef: string, retained: RetainedArtifact): Uint8Array {
    const payloadBytes = new TextEncoder().encode(
      canonicalJson({
        schema: "openagents.portable_checkpoint_artifact_custody_payload.v3",
        checkpointRef,
        artifactRef: retained.artifactRef,
        digest: retained.digest,
        bundle: retained.bundle,
        createdAt: retained.createdAt,
        expiresAt: retained.expiresAt,
        bytesBase64: Buffer.from(retained.bytes).toString("base64"),
      }),
    )
    if (payloadBytes.byteLength > this.maxPayloadBytes()) {
      payloadBytes.fill(0)
      throw new PylonPortableCheckpointArtifactError(
        "artifact_too_large",
        "checkpoint custody payload exceeds its size bound",
      )
    }
    return payloadBytes
  }

  private async writeEncrypted(
    path: string,
    paths: NonNullable<ReturnType<typeof this.artifactPaths>>,
    config: EncryptedCustodyConfig,
    payloadBytes: Uint8Array,
  ): Promise<void> {
    const key = await this.loadKey(config)
    const nonce = Uint8Array.from(randomBytes(AES_GCM_NONCE_BYTES))
    const aad = new TextEncoder().encode(canonicalJson(encryptedHeader(paths.objectRef, config)))
    let ciphertext: Buffer | undefined
    let authTag: Buffer | undefined
    let envelopeBytes: Uint8Array | undefined
    try {
      const cipher = createCipheriv("aes-256-gcm", key, nonce, {
        authTagLength: AES_GCM_TAG_BYTES,
      })
      cipher.setAAD(aad)
      ciphertext = Buffer.concat([cipher.update(payloadBytes), cipher.final()])
      authTag = cipher.getAuthTag()
      envelopeBytes = new TextEncoder().encode(
        canonicalJson({
          ...encryptedHeader(paths.objectRef, config),
          nonceBase64: Buffer.from(nonce).toString("base64"),
          authTagBase64: authTag.toString("base64"),
          ciphertextBase64: ciphertext.toString("base64"),
        }),
      )
      await atomicPrivateWrite(path, envelopeBytes)
    } finally {
      key.fill(0)
      nonce.fill(0)
      aad.fill(0)
      ciphertext?.fill(0)
      authTag?.fill(0)
      envelopeBytes?.fill(0)
    }
  }

  private assertNotExpired(retained: RetainedArtifact): void {
    if (Date.parse(retained.expiresAt) <= this.now().getTime()) {
      retained.bytes.fill(0)
      throw new PylonPortableCheckpointArtifactError(
        "retention_expired",
        "checkpoint artifact retention expired",
      )
    }
  }

  private async loadArtifact(
    checkpointRef: string,
    allowExpired = false,
  ): Promise<RetainedArtifact | undefined> {
    const cached = this.artifacts.get(checkpointRef)
    if (cached !== undefined) {
      if (!allowExpired) this.assertNotExpired(cached)
      return cached
    }
    const paths = this.artifactPaths(checkpointRef)
    const custody = this.custody
    if (paths === undefined || custody === undefined) return undefined
    try {
      if (await this.exists(paths.deletion)) {
        throw new PylonPortableCheckpointArtifactError(
          "unavailable",
          "checkpoint artifact was deleted",
        )
      }
      const encryptedExists = await this.exists(paths.encrypted)
      const plaintextExists = await this.exists(paths.plaintext)
      const legacyExists =
        (await this.exists(paths.legacyMetadata)) || (await this.exists(paths.legacyBytes))
      let retained: RetainedArtifact | undefined
      if (custody.policy === "owner_device_not_required") {
        if (encryptedExists) {
          throw new PylonPortableCheckpointArtifactError("custody_policy_mismatch", "encrypted checkpoint custody requires its configured key policy")
        }
        if (legacyExists) {
          throw new PylonPortableCheckpointArtifactError("plaintext_downgrade", "legacy split-file checkpoint custody is rejected")
        }
        if (plaintextExists) {
          const payload = await this.readBounded(paths.plaintext, this.maxPayloadBytes())
          try {
            retained = this.decodePayload(checkpointRef, payload)
          } finally {
            payload.fill(0)
          }
        }
      } else {
        if (plaintextExists || legacyExists) {
          throw new PylonPortableCheckpointArtifactError("plaintext_downgrade", "plaintext checkpoint custody is rejected by the configured policy")
        }
        if (encryptedExists) retained = await this.loadEncrypted(checkpointRef, paths, custody)
      }
      if (retained !== undefined) {
        if (!allowExpired) this.assertNotExpired(retained)
        this.artifacts.set(checkpointRef, retained)
      }
      return retained
    } catch (error) {
      if (error instanceof PylonPortableCheckpointArtifactError) throw error
      if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined
      throw new PylonPortableCheckpointArtifactError("unavailable", "persisted managed checkpoint artifact is unavailable")
    }
  }

  register(input: Readonly<{
    bundle: PylonPortableCheckpointBundle
    workingDirectory: string
  }>): void {
    const checkpoint = input.bundle.checkpoint
    if (!SAFE_REF.test(checkpoint.checkpointRef) || !resolve(input.workingDirectory).startsWith("/")) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact source is invalid")
    }
    const existing = this.sources.get(checkpoint.checkpointRef)
    if (existing !== undefined &&
        (existing.workingDirectory !== resolve(input.workingDirectory) || !sameBundle(existing.bundle, input.bundle))) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact source conflicts with an existing binding")
    }
    this.sources.set(checkpoint.checkpointRef, {
      bundle: input.bundle,
      workingDirectory: resolve(input.workingDirectory),
    })
  }

  async registerArtifact(input: Readonly<{
    bundle: PylonPortableCheckpointBundle
    artifact: PortableCheckpointArtifact
  }>): Promise<void> {
    const checkpointRef = input.bundle.checkpoint.checkpointRef
    if (!SAFE_REF.test(checkpointRef) || !SAFE_REF.test(input.artifact.artifactRef) ||
        !/^sha256:[a-f0-9]{64}$/u.test(input.artifact.digest) ||
        input.artifact.bytes.byteLength === 0 || sha256(input.artifact.bytes) !== input.artifact.digest) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "managed checkpoint artifact is invalid")
    }
    this.assertArtifactSize(input.artifact.bytes.byteLength)
    const existing = await this.loadArtifact(checkpointRef)
    if (existing !== undefined &&
        (!sameBundle(existing.bundle, input.bundle) ||
         existing.artifactRef !== input.artifact.artifactRef ||
         existing.digest !== input.artifact.digest)) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "managed checkpoint artifact conflicts with its binding")
    }
    // Bun may surface Buffer-compatible Uint8Arrays whose `.slice()` aliases
    // the caller's backing store. Custody always owns a real byte-for-byte copy
    // so the caller can zero its transport buffer immediately.
    const owned = Uint8Array.from(input.artifact.bytes)
    const createdAt = existing?.createdAt ?? this.nowIso()
    const expiresAt =
      existing?.expiresAt ??
      new Date(Date.parse(createdAt) + this.retentionSeconds * 1_000).toISOString()
    const retained = {
      bundle: input.bundle,
      artifactRef: input.artifact.artifactRef,
      digest: input.artifact.digest,
      bytes: owned,
      createdAt,
      expiresAt,
    }
    const paths = this.artifactPaths(checkpointRef)
    const custody = this.custody
    if (paths !== undefined && custody !== undefined) {
      const payloadBytes = this.encodePayload(checkpointRef, retained)
      try {
        await this.prepareCustodyDirectory()
        await this.ensureCustodyCapacity([
          custody.policy === "owner_device_not_required" ? paths.plaintext : paths.encrypted,
        ])
        if (custody.policy === "owner_device_not_required") {
          await atomicPrivateWrite(paths.plaintext, payloadBytes)
        } else {
          await this.writeEncrypted(paths.encrypted, paths, custody, payloadBytes)
        }
      } finally {
        payloadBytes.fill(0)
      }
    }
    existing?.bytes.fill(0)
    this.artifacts.set(checkpointRef, retained)
  }

  private ciphertextDigest(envelopeBytes: Uint8Array): `sha256:${string}` {
    try {
      const envelope = JSON.parse(new TextDecoder().decode(envelopeBytes)) as Record<
        string,
        unknown
      >
      if (
        typeof envelope.ciphertextBase64 !== "string" ||
        !/^[A-Za-z0-9+/]+={0,2}$/u.test(envelope.ciphertextBase64)
      ) {
        throw new Error()
      }
      const decoded = Buffer.from(envelope.ciphertextBase64, "base64")
      try {
        if (decoded.byteLength === 0 || decoded.toString("base64") !== envelope.ciphertextBase64) {
          throw new Error()
        }
        return sha256(decoded)
      } finally {
        decoded.fill(0)
      }
    } catch {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport ciphertext is invalid",
      )
    }
  }

  private assertTransportClaim(
    claim: PortableCommandExecutionClaim,
    bundle: PylonPortableCheckpointBundle,
    at: number,
  ): void {
    if (
      claim.ownerRef !== bundle.executionBinding.ownerRef ||
      claim.sessionRef !== bundle.checkpoint.sessionRef ||
      claim.sourceAttachmentRef !== bundle.checkpoint.sourceAttachmentRef ||
      claim.sourceGeneration !== bundle.checkpoint.sourceGeneration ||
      claim.executorEnvironmentRef.length === 0 ||
      claim.destinationTargetRef.length === 0 ||
      claim.state !== "claimed" ||
      claim.terminalStatus !== null ||
      claim.outcomeRef !== null ||
      Date.parse(claim.claimedAt) > at ||
      Date.parse(claim.leaseExpiresAt) <= at
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody transport command claim does not match the checkpoint",
      )
    }
  }

  async exportCustodyObject(
    input: PylonPortableCheckpointCustodyExportInput,
  ): Promise<PylonPortableCheckpointCustodyObject> {
    const custody = this.custody
    const paths = this.artifactPaths(input.checkpointRef)
    if (
      custody === undefined ||
      paths === undefined ||
      custody.policy === "owner_device_not_required"
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "plaintext_downgrade",
        "checkpoint custody export requires encrypted custody",
      )
    }
    if (
      !Number.isSafeInteger(input.byteLimit) ||
      input.byteLimit <= 0 ||
      input.byteLimit > this.maximumEncryptedObjectBytes()
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "artifact_too_large",
        "checkpoint custody transport byte limit is invalid",
      )
    }
    await this.prepareCustodyDirectory()
    if (await this.exists(paths.deletion)) {
      throw new PylonPortableCheckpointArtifactError(
        "unavailable",
        "checkpoint custody transport cannot export a deletion tombstone",
      )
    }
    const retained = await this.loadArtifact(input.checkpointRef)
    if (retained === undefined) {
      throw new PylonPortableCheckpointArtifactError(
        "unavailable",
        "checkpoint custody object is unavailable",
      )
    }
    const bundle = validateBundle(retained.bundle)
    validateArtifact(retained)
    const now = this.now().getTime()
    this.assertTransportClaim(input.commandClaim, bundle, now)
    if (!SAFE_REF.test(input.sourcePylonRef)) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody transport source Pylon is invalid",
      )
    }
    const bytes = await this.readBounded(paths.encrypted, this.maximumEncryptedObjectBytes())
    try {
      if (bytes.byteLength > input.byteLimit) {
        throw new PylonPortableCheckpointArtifactError(
          "artifact_too_large",
          "checkpoint custody object exceeds its transport byte limit",
        )
      }
      const createdAtMs = Math.max(
        Date.parse(retained.createdAt),
        Date.parse(input.commandClaim.claimedAt),
      )
      const expiresAtMs = Math.min(
        Date.parse(retained.expiresAt),
        Date.parse(input.commandClaim.leaseExpiresAt),
      )
      const retentionSeconds = Math.floor((expiresAtMs - createdAtMs) / 1_000)
      if (retentionSeconds <= 0) {
        throw new PylonPortableCheckpointArtifactError(
          "retention_expired",
          "checkpoint custody transport retention has expired",
        )
      }
      const manifest: PortableCheckpointCustodyObjectManifest = {
        schema: "openagents.portable_checkpoint_custody_object_manifest.v1",
        objectRef: paths.objectRef,
        objectDigest: sha256(bytes),
        artifactRef: retained.artifactRef,
        artifactDigest: retained.digest,
        checkpointRef: bundle.checkpoint.checkpointRef,
        checkpointDigest: bundle.checkpoint.digest,
        bundleDigest: sha256(canonicalJson(bundle)),
        ciphertextDigest: this.ciphertextDigest(bytes),
        commandClaim: input.commandClaim,
        ownerRef: bundle.executionBinding.ownerRef,
        sourcePylonRef: input.sourcePylonRef,
        targetRef: input.commandClaim.destinationTargetRef,
        sessionRef: bundle.checkpoint.sessionRef,
        sourceAttachmentRef: bundle.checkpoint.sourceAttachmentRef,
        sourceGeneration: bundle.checkpoint.sourceGeneration,
        custodyPolicy: custody.policy,
        keyRef: custody.keyRef,
        byteLimit: input.byteLimit,
        createdAt: new Date(createdAtMs).toISOString(),
        expiresAt: new Date(expiresAtMs).toISOString(),
        retentionSeconds,
        secretMaterial: "excluded",
      }
      Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(manifest)
      return { manifest, bytes }
    } catch (error) {
      bytes.fill(0)
      throw error
    }
  }

  async importCustodyObject(
    input: PylonPortableCheckpointCustodyImportInput,
  ): Promise<PortableCheckpointCustodyObjectManifest> {
    const custody = this.custody
    if (custody === undefined || custody.policy === "owner_device_not_required") {
      throw new PylonPortableCheckpointArtifactError(
        "plaintext_downgrade",
        "checkpoint custody import requires encrypted custody",
      )
    }
    let manifest: PortableCheckpointCustodyObjectManifest
    try {
      manifest = Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema)(
        input.manifest,
      )
    } catch {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport manifest is invalid",
      )
    }
    const paths = this.artifactPaths(manifest.checkpointRef)
    if (paths === undefined) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody transport has no configured object path",
      )
    }
    const now = this.now().getTime()
    const createdAt = Date.parse(manifest.createdAt)
    const expiresAt = Date.parse(manifest.expiresAt)
    if (
      manifest.objectRef !== paths.objectRef ||
      manifest.custodyPolicy !== custody.policy ||
      manifest.keyRef !== custody.keyRef ||
      manifest.byteLimit > this.maximumEncryptedObjectBytes() ||
      input.bytes.byteLength === 0 ||
      input.bytes.byteLength > manifest.byteLimit ||
      manifest.objectDigest !== sha256(input.bytes) ||
      manifest.ciphertextDigest !== this.ciphertextDigest(input.bytes) ||
      expiresAt <= createdAt ||
      manifest.retentionSeconds !== Math.floor((expiresAt - createdAt) / 1_000) ||
      expiresAt <= now
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "transport_invalid",
        "checkpoint custody transport binding is invalid or expired",
      )
    }
    await this.prepareCustodyDirectory()
    if (await this.exists(paths.deletion)) {
      throw new PylonPortableCheckpointArtifactError(
        "unavailable",
        "checkpoint custody transport cannot replace a deletion tombstone",
      )
    }
    if (
      (await this.exists(paths.plaintext)) ||
      (await this.exists(paths.legacyBytes)) ||
      (await this.exists(paths.legacyMetadata))
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "plaintext_downgrade",
        "checkpoint custody transport found a plaintext object representation",
      )
    }
    const transportBytes = Uint8Array.from(input.bytes)
    let retained: RetainedArtifact | undefined
    try {
      retained = await this.decodeEncryptedObject(
        manifest.checkpointRef,
        paths,
        custody,
        Uint8Array.from(transportBytes),
      )
      const bundle = validateBundle(retained.bundle)
      validateArtifact(retained)
      this.assertTransportClaim(manifest.commandClaim, bundle, createdAt)
      if (
        manifest.artifactRef !== retained.artifactRef ||
        manifest.artifactDigest !== retained.digest ||
        manifest.checkpointRef !== bundle.checkpoint.checkpointRef ||
        manifest.checkpointDigest !== bundle.checkpoint.digest ||
        manifest.bundleDigest !== sha256(canonicalJson(bundle)) ||
        manifest.ownerRef !== bundle.executionBinding.ownerRef ||
        !SAFE_REF.test(manifest.sourcePylonRef) ||
        manifest.targetRef !== manifest.commandClaim.destinationTargetRef ||
        manifest.sessionRef !== bundle.checkpoint.sessionRef ||
        manifest.sourceAttachmentRef !== bundle.checkpoint.sourceAttachmentRef ||
        manifest.sourceGeneration !== bundle.checkpoint.sourceGeneration ||
        manifest.createdAt !==
          new Date(
            Math.max(Date.parse(retained.createdAt), Date.parse(manifest.commandClaim.claimedAt)),
          ).toISOString() ||
        manifest.expiresAt !==
          new Date(
            Math.min(
              Date.parse(retained.expiresAt),
              Date.parse(manifest.commandClaim.leaseExpiresAt),
            ),
          ).toISOString()
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody transport does not match its decrypted artifact",
        )
      }

      if (await this.exists(paths.encrypted)) {
        const existing = await this.readBounded(
          paths.encrypted,
          this.maximumEncryptedObjectBytes(),
        )
        try {
          if (
            existing.byteLength !== transportBytes.byteLength ||
            !existing.every((byte, index) => byte === transportBytes[index])
          ) {
            throw new PylonPortableCheckpointArtifactError(
              "replay_conflict",
              "checkpoint custody transport conflicts with an existing object",
            )
          }
        } finally {
          existing.fill(0)
        }
        const cached = this.artifacts.get(manifest.checkpointRef)
        cached?.bytes.fill(0)
        this.artifacts.set(manifest.checkpointRef, retained)
        retained = undefined
        return manifest
      }

      await this.ensureCustodyCapacity([paths.encrypted])
      await atomicPrivateWrite(paths.encrypted, transportBytes)
      let verified: RetainedArtifact | undefined
      try {
        verified = await this.loadEncrypted(manifest.checkpointRef, paths, custody)
        validateArtifact(verified)
        if (
          verified.digest !== retained.digest ||
          verified.artifactRef !== retained.artifactRef ||
          !sameBundle(verified.bundle, retained.bundle)
        ) {
          throw new PylonPortableCheckpointArtifactError(
            "transport_invalid",
            "checkpoint custody transport atomic write verification failed",
          )
        }
        const cached = this.artifacts.get(manifest.checkpointRef)
        cached?.bytes.fill(0)
        this.artifacts.set(manifest.checkpointRef, verified)
        verified = undefined
      } catch (error) {
        await rm(paths.encrypted, { force: true })
        await this.syncCustodyDirectory()
        throw error
      } finally {
        verified?.bytes.fill(0)
      }
      return manifest
    } finally {
      retained?.bytes.fill(0)
      transportBytes.fill(0)
    }
  }

  async deleteArtifact(
    input: PylonPortableCheckpointLifecycleBinding,
  ): Promise<PylonPortableCheckpointDeletionReceipt> {
    const custody = this.custody
    const paths = this.artifactPaths(input.checkpointRef)
    if (custody === undefined || paths === undefined) {
      throw new PylonPortableCheckpointArtifactError(
        "deletion_failed",
        "checkpoint artifact custody is not configured",
      )
    }
    const binding = this.lifecycleBinding(input)
    const keyRef = custody.policy === "owner_device_not_required" ? null : custody.keyRef
    const common = {
      operationRef: binding.operationRef,
      ownerRef: binding.ownerRef,
      sessionRef: binding.sessionRef,
      checkpointRef: binding.checkpointRef,
      bundleDigest: binding.bundleDigest,
      objectRef: paths.objectRef,
      policy: custody.policy,
      keyRef,
    }
    await this.prepareCustodyDirectory()
    await this.ensureCustodyCapacity([paths.deletion])
    let record = await this.readLifecycleRecord(paths.deletion)
    let artifactDigest: `sha256:${string}`
    if (record !== undefined) {
      this.assertLifecycleRecordBinding(record, common)
      if (!isSha256(record.artifactDigest)) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody deletion digest is invalid",
        )
      }
      artifactDigest = record.artifactDigest
      if (
        record.schema === "openagents.portable_checkpoint_artifact_deletion_receipt.v1" &&
        record.state === "deleted" &&
        record.verifiedAbsent === true &&
        record.publicSafe === true &&
        typeof record.receiptRef === "string" &&
        SAFE_REF.test(record.receiptRef) &&
        typeof record.occurredAt === "string" &&
        Number.isFinite(Date.parse(record.occurredAt))
      ) {
        const objectPath =
          custody.policy === "owner_device_not_required" ? paths.plaintext : paths.encrypted
        if (
          (await this.exists(objectPath)) ||
          (await this.exists(paths.plaintext)) ||
          (await this.exists(paths.encrypted)) ||
          (await this.exists(paths.legacyBytes)) ||
          (await this.exists(paths.legacyMetadata))
        ) {
          throw new PylonPortableCheckpointArtifactError(
            "deletion_failed",
            "checkpoint custody deletion receipt no longer verifies object absence",
          )
        }
        return {
          schema: "openagents.portable_checkpoint_artifact_deletion_receipt.v1",
          receiptRef: record.receiptRef,
          ...common,
          artifactDigest,
          state: "deleted",
          verifiedAbsent: true,
          occurredAt: record.occurredAt,
          publicSafe: true,
        }
      }
      if (
        record.schema !== "openagents.portable_checkpoint_artifact_deletion_intent.v1" ||
        record.state !== "deleting"
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody deletion record is invalid",
        )
      }
    } else {
      const retained = await this.loadArtifact(input.checkpointRef, true)
      if (retained === undefined || !sameBundle(retained.bundle, input.bundle)) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody deletion does not match its artifact",
        )
      }
      artifactDigest = retained.digest
      record = {
        schema: "openagents.portable_checkpoint_artifact_deletion_intent.v1",
        ...common,
        artifactDigest,
        state: "deleting",
        publicSafe: true,
      }
      await atomicPrivateWrite(paths.deletion, new TextEncoder().encode(canonicalJson(record)))
      await custody.faultInjector?.("delete_intent_durable")
    }

    const objectPath =
      custody.policy === "owner_device_not_required" ? paths.plaintext : paths.encrypted
    const markerPath = `${objectPath}.deleting.${createHash("sha256").update(input.operationRef).digest("hex").slice(0, 24)}.tmp`
    if (await this.exists(objectPath)) {
      await rename(objectPath, markerPath)
      await this.syncCustodyDirectory()
    }
    await rm(markerPath, { force: true })
    await this.syncCustodyDirectory()
    if ((await this.exists(objectPath)) || (await this.exists(markerPath))) {
      throw new PylonPortableCheckpointArtifactError(
        "deletion_failed",
        "checkpoint custody deletion could not verify object absence",
      )
    }
    if (
      (await this.exists(paths.plaintext)) ||
      (await this.exists(paths.encrypted)) ||
      (await this.exists(paths.legacyBytes)) ||
      (await this.exists(paths.legacyMetadata))
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "deletion_failed",
        "checkpoint custody deletion found a conflicting object representation",
      )
    }
    await custody.faultInjector?.("delete_object_removed")
    const occurredAt = this.nowIso()
    const receipt: PylonPortableCheckpointDeletionReceipt = {
      schema: "openagents.portable_checkpoint_artifact_deletion_receipt.v1",
      receiptRef: stableRef(
        "receipt.portable-checkpoint-deletion",
        `${input.operationRef}:${paths.objectRef}`,
      ),
      ...common,
      artifactDigest,
      state: "deleted",
      verifiedAbsent: true,
      occurredAt,
      publicSafe: true,
    }
    await atomicPrivateWrite(paths.deletion, new TextEncoder().encode(canonicalJson(receipt)))
    const cached = this.artifacts.get(input.checkpointRef)
    cached?.bytes.fill(0)
    this.artifacts.delete(input.checkpointRef)
    return receipt
  }

  async rewrapArtifact(
    input: PylonPortableCheckpointRewrapInput,
  ): Promise<PylonPortableCheckpointRewrapReceipt> {
    const custody = this.custody
    const paths = this.artifactPaths(input.checkpointRef)
    if (
      custody === undefined ||
      paths === undefined ||
      custody.policy === "owner_device_not_required"
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "rewrap_failed",
        "encrypted checkpoint artifact custody is not configured",
      )
    }
    const binding = this.lifecycleBinding(input)
    if (
      !SAFE_REF.test(input.keyRef) ||
      input.keyRef === custody.keyRef ||
      typeof input.keyProvider.loadKey !== "function"
    ) {
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody rewrap key binding is invalid",
      )
    }
    const nextConfig: EncryptedCustodyConfig = {
      ...custody,
      keyRef: input.keyRef,
      keyProvider: input.keyProvider,
    }
    const operationHash = createHash("sha256")
      .update(input.operationRef)
      .digest("hex")
      .slice(0, 24)
    const stagedPath = `${paths.encrypted}.rewrap.${operationHash}.tmp`
    const journalPath = `${paths.encrypted}.rewrap.${operationHash}.json`
    const common = {
      operationRef: binding.operationRef,
      ownerRef: binding.ownerRef,
      sessionRef: binding.sessionRef,
      checkpointRef: binding.checkpointRef,
      bundleDigest: binding.bundleDigest,
      objectRef: paths.objectRef,
      policy: custody.policy,
      previousKeyRef: custody.keyRef,
      keyRef: input.keyRef,
    }
    await this.prepareCustodyDirectory()
    await this.ensureCustodyCapacity([stagedPath, journalPath])
    const journal = await this.readLifecycleRecord(journalPath)
    let completedReceipt: PylonPortableCheckpointRewrapReceipt | undefined
    if (journal !== undefined) {
      this.assertLifecycleRecordBinding(journal, common)
      if (
        journal.schema === "openagents.portable_checkpoint_artifact_rewrap_receipt.v1" &&
        journal.state === "rewrapped" &&
        journal.verified === true &&
        journal.publicSafe === true &&
        typeof journal.receiptRef === "string" &&
        typeof journal.digest === "string" &&
        isSha256(journal.digest) &&
        typeof journal.occurredAt === "string" &&
        Number.isFinite(Date.parse(journal.occurredAt))
      ) {
        completedReceipt = {
          schema: "openagents.portable_checkpoint_artifact_rewrap_receipt.v1",
          receiptRef: journal.receiptRef,
          operationRef: input.operationRef,
          checkpointRef: input.checkpointRef,
          objectRef: paths.objectRef,
          policy: custody.policy,
          previousKeyRef: custody.keyRef,
          keyRef: input.keyRef,
          digest: journal.digest,
          state: "rewrapped",
          verified: true,
          occurredAt: journal.occurredAt,
          publicSafe: true,
        }
      } else if (
        journal.schema !== "openagents.portable_checkpoint_artifact_rewrap_intent.v1" ||
        journal.state !== "staged"
      ) {
        throw new PylonPortableCheckpointArtifactError(
          "invalid_binding",
          "checkpoint custody rewrap record is invalid",
        )
      }
    }

    let retained: RetainedArtifact | undefined
    let finalUsesNextKey = false
    try {
      retained = await this.loadEncrypted(input.checkpointRef, paths, nextConfig)
      finalUsesNextKey = true
    } catch (error) {
      if (
        !(error instanceof PylonPortableCheckpointArtifactError) ||
        error.code !== "key_ref_mismatch"
      )
        throw error
    }
    if (retained === undefined && (await this.exists(stagedPath))) {
      retained = await this.loadEncrypted(input.checkpointRef, paths, nextConfig, stagedPath)
    }
    if (retained === undefined)
      retained = await this.loadEncrypted(input.checkpointRef, paths, custody)
    if (!sameBundle(retained.bundle, input.bundle)) {
      retained.bytes.fill(0)
      throw new PylonPortableCheckpointArtifactError(
        "invalid_binding",
        "checkpoint custody rewrap does not match its artifact",
      )
    }
    if (completedReceipt !== undefined) {
      if (!finalUsesNextKey || retained.digest !== completedReceipt.digest) {
        retained.bytes.fill(0)
        throw new PylonPortableCheckpointArtifactError(
          "rewrap_failed",
          "checkpoint custody rewrap receipt no longer verifies ciphertext",
        )
      }
      retained.bytes.fill(0)
      this.custody = nextConfig
      return completedReceipt
    }

    if (!finalUsesNextKey && !(await this.exists(stagedPath))) {
      const payload = this.encodePayload(input.checkpointRef, retained)
      try {
        await this.writeEncrypted(stagedPath, paths, nextConfig, payload)
      } finally {
        payload.fill(0)
      }
      const verified = await this.loadEncrypted(input.checkpointRef, paths, nextConfig, stagedPath)
      try {
        if (
          verified.digest !== retained.digest ||
          verified.artifactRef !== retained.artifactRef ||
          !sameBundle(verified.bundle, retained.bundle)
        ) {
          throw new PylonPortableCheckpointArtifactError(
            "rewrap_failed",
            "checkpoint custody rewrap verification failed",
          )
        }
      } finally {
        verified.bytes.fill(0)
      }
      await atomicPrivateWrite(
        journalPath,
        new TextEncoder().encode(
          canonicalJson({
            schema: "openagents.portable_checkpoint_artifact_rewrap_intent.v1",
            ...common,
            digest: retained.digest,
            state: "staged",
            publicSafe: true,
          }),
        ),
      )
      await custody.faultInjector?.("rewrap_ciphertext_durable")
    }
    if (!finalUsesNextKey) {
      await rename(stagedPath, paths.encrypted)
      await chmod(paths.encrypted, 0o600)
      await this.syncCustodyDirectory()
      finalUsesNextKey = true
    }
    const final = await this.loadEncrypted(input.checkpointRef, paths, nextConfig)
    try {
      if (final.digest !== retained.digest || !sameBundle(final.bundle, retained.bundle)) {
        throw new PylonPortableCheckpointArtifactError(
          "rewrap_failed",
          "checkpoint custody rewrap final verification failed",
        )
      }
    } finally {
      final.bytes.fill(0)
    }
    await custody.faultInjector?.("rewrap_replaced")
    const receipt: PylonPortableCheckpointRewrapReceipt = {
      schema: "openagents.portable_checkpoint_artifact_rewrap_receipt.v1",
      receiptRef: stableRef(
        "receipt.portable-checkpoint-rewrap",
        `${input.operationRef}:${paths.objectRef}:${input.keyRef}`,
      ),
      operationRef: input.operationRef,
      checkpointRef: input.checkpointRef,
      objectRef: paths.objectRef,
      policy: custody.policy,
      previousKeyRef: custody.keyRef,
      keyRef: input.keyRef,
      digest: retained.digest,
      state: "rewrapped",
      verified: true,
      occurredAt: this.nowIso(),
      publicSafe: true,
    }
    await atomicPrivateWrite(
      journalPath,
      new TextEncoder().encode(canonicalJson({ ...common, ...receipt })),
    )
    const cached = this.artifacts.get(input.checkpointRef)
    cached?.bytes.fill(0)
    retained.bytes.fill(0)
    this.artifacts.delete(input.checkpointRef)
    this.custody = nextConfig
    return receipt
  }

  async resolve(input: PortableCheckpointArtifactResolverInput): Promise<PortableCheckpointArtifact> {
    await this.prepareCustodyDirectory()
    const checkpoint = input.bundle.checkpoint
    const retained = await this.loadArtifact(input.checkpointRef)
    if (retained !== undefined) {
      if (input.ownerRef !== input.bundle.executionBinding.ownerRef ||
          input.sessionRef !== checkpoint.sessionRef ||
          input.checkpointRef !== checkpoint.checkpointRef ||
          input.generation !== checkpoint.sourceGeneration + 1 ||
          !sameBundle(retained.bundle, input.bundle)) {
        throw new PylonPortableCheckpointArtifactError("invalid_binding", "managed checkpoint artifact request does not match")
      }
      return {
        artifactRef: retained.artifactRef,
        digest: retained.digest,
        bytes: Uint8Array.from(retained.bytes),
      }
    }
    const source = this.sources.get(input.checkpointRef)
    if (source === undefined) {
      throw new PylonPortableCheckpointArtifactError("unavailable", "checkpoint artifact source is unavailable")
    }
    if (![input.ownerRef, input.targetRef, input.sessionRef, input.attachmentRef, input.checkpointRef]
      .every(value => SAFE_REF.test(value)) ||
      input.ownerRef !== input.bundle.executionBinding.ownerRef ||
      input.sessionRef !== checkpoint.sessionRef ||
      input.checkpointRef !== checkpoint.checkpointRef ||
      input.generation !== checkpoint.sourceGeneration + 1 ||
      !sameBundle(source.bundle, input.bundle)) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact request does not match its source")
    }

    const headBytes = await runGit(source.workingDirectory, ["rev-parse", "HEAD"])
    const head = new TextDecoder().decode(headBytes).trim()
    headBytes.fill(0)
    if (head !== checkpoint.repositoryRevisionRef) {
      throw new PylonPortableCheckpointArtifactError("repository_mismatch", "checkpoint repository revision changed before export")
    }
    const postImage = await postImageEntries(source.workingDirectory)
    let bundleBytes: Uint8Array | undefined
    let manifestBytes: Uint8Array | undefined
    let tarBytes: Uint8Array | undefined
    try {
      if (postImage.reduce((total, entry) => total + entry.bytes.byteLength, 0) > this.maxArtifactBytes) {
        throw new PylonPortableCheckpointArtifactError("artifact_too_large", "checkpoint repository post-image exceeds its size bound")
      }
      const postImageDigest = createHash("sha256")
      for (const entry of postImage) {
        postImageDigest.update(entry.path).update("\0").update(entry.bytes).update("\0")
      }
      if (`sha256:${postImageDigest.digest("hex")}` !== checkpoint.repositoryPostImageDigest) {
        throw new PylonPortableCheckpointArtifactError("repository_mismatch", "checkpoint post-image changed before export")
      }
      bundleBytes = await runGit(source.workingDirectory, ["bundle", "create", "-", "HEAD"])
      const artifactRef = stableRef("artifact.portable-checkpoint", `${checkpoint.checkpointRef}:${checkpoint.digest}`)
      const manifest = {
        schema: "openagents.portable_checkpoint_artifact.v1",
        artifactRef,
        checkpointRef: checkpoint.checkpointRef,
        bundle: input.bundle,
        files: postImage.map(entry => ({
          path: entry.path,
          mode: entry.mode,
          sha256: entry.digest,
          size: entry.bytes.byteLength,
          ...(entry.linkTarget === undefined ? {} : { linkTarget: entry.linkTarget }),
        })),
      }
      manifestBytes = new TextEncoder().encode(canonicalJson(manifest))
      tarBytes = archive([
        { path: "manifest.json", bytes: manifestBytes, mode: 0o600 },
        { path: "repository.bundle", bytes: bundleBytes, mode: 0o600 },
        ...postImage.map(entry => ({
          path: `post-image/${entry.path}`,
          bytes: entry.bytes,
          mode: entry.mode,
          kind: entry.kind,
          ...(entry.linkTarget === undefined ? {} : { linkTarget: entry.linkTarget }),
        })),
      ])
      const compressed = Runtime.zstdCompressSync(tarBytes)
      this.assertArtifactSize(compressed.byteLength)
      return {
        artifactRef,
        digest: sha256(compressed),
        bytes: compressed,
      }
    } finally {
      bundleBytes?.fill(0)
      manifestBytes?.fill(0)
      tarBytes?.fill(0)
      for (const entry of postImage) entry.bytes.fill(0)
    }
  }
}
