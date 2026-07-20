import { Runtime } from "@openagentsinc/runtime-platform"
import { createCipheriv, createDecipheriv, createHash, randomBytes, randomUUID } from "node:crypto"
import { chmod, lstat, mkdir, open, readFile, readlink, rename, rm, stat } from "node:fs/promises"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"

import { canonicalJson } from "@openagentsinc/khala-sync"
import type { PylonPortableCheckpointBundle } from "@openagentsinc/portable-session-contract"

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
  }>
  | Readonly<{
    custodyDirectory: string
    policy: "owner_device_not_required"
    maxArtifactBytes?: number
  }>

export class PylonPortableCheckpointArtifactError extends Error {
  readonly _tag = "PylonPortableCheckpointArtifactError"
  override readonly name = "PylonPortableCheckpointArtifactError"

  constructor(
    readonly code:
      | "artifact_too_large"
      | "custody_policy_mismatch"
      | "decrypt_failed"
      | "invalid_binding"
      | "key_ref_mismatch"
      | "key_unavailable"
      | "plaintext_downgrade"
      | "private_material"
      | "repository_mismatch"
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

type RetainedArtifact = Readonly<{
  bundle: PylonPortableCheckpointBundle
  artifactRef: string
  digest: `sha256:${string}`
  bytes: Uint8Array
}>

type EncryptedCustodyConfig = Extract<
  PylonPortableCheckpointCustodyConfig,
  { policy: "owner_managed" | "openagents_managed" }
>

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
  } finally {
    await handle?.close().catch(() => undefined)
    await rm(temporary, { force: true })
  }
}

export class PylonPortableCheckpointArtifactStore implements PortableCheckpointArtifactResolver {
  private readonly sources = new Map<string, Source>()
  private readonly artifacts = new Map<string, RetainedArtifact>()
  private readonly custody?: PylonPortableCheckpointCustodyConfig
  private readonly maxArtifactBytes: number

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
    if (!Number.isSafeInteger(maxArtifactBytes) || maxArtifactBytes <= 0 ||
        maxArtifactBytes > DEFAULT_MAX_ARTIFACT_BYTES ||
        (custody !== undefined && custody.policy !== "owner_device_not_required" &&
          (!SAFE_REF.test(custody.keyRef) || typeof custody.keyProvider.loadKey !== "function"))) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact custody configuration is invalid")
    }
    this.custody = custody
    this.maxArtifactBytes = maxArtifactBytes
  }

  private artifactPaths(checkpointRef: string): Readonly<{
    encrypted: string
    plaintext: string
    legacyBytes: string
    legacyMetadata: string
    objectRef: string
  }> | undefined {
    if (this.custody === undefined) return undefined
    const name = createHash("sha256").update(checkpointRef).digest("hex")
    return {
      encrypted: join(this.custody.custodyDirectory, `${name}.checkpoint.aesgcm`),
      plaintext: join(this.custody.custodyDirectory, `${name}.checkpoint.json`),
      legacyBytes: join(this.custody.custodyDirectory, `${name}.tar.zst`),
      legacyMetadata: join(this.custody.custodyDirectory, `${name}.json`),
      objectRef: `checkpoint-custody:${name}`,
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
    if (payload.schema !== "openagents.portable_checkpoint_artifact_custody_payload.v2" ||
        payload.checkpointRef !== checkpointRef || typeof payload.artifactRef !== "string" ||
        !SAFE_REF.test(payload.artifactRef) || typeof payload.digest !== "string" ||
        !/^sha256:[a-f0-9]{64}$/u.test(payload.digest) || typeof payload.bundle !== "object" ||
        payload.bundle === null || typeof payload.bytesBase64 !== "string" ||
        !/^[A-Za-z0-9+/]+={0,2}$/u.test(payload.bytesBase64)) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "persisted checkpoint custody binding is invalid")
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
      return { bundle, artifactRef: payload.artifactRef, digest: payload.digest as `sha256:${string}`, bytes }
    } catch (error) {
      bytes.fill(0)
      throw error
    }
  }

  private async loadEncrypted(checkpointRef: string, paths: NonNullable<ReturnType<typeof this.artifactPaths>>, config: EncryptedCustodyConfig): Promise<RetainedArtifact> {
    const maximum = Math.ceil(this.maxPayloadBytes() * 4 / 3) + 4096
    const envelopeBytes = await this.readBounded(paths.encrypted, maximum)
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

  private async loadArtifact(checkpointRef: string): Promise<RetainedArtifact | undefined> {
    const cached = this.artifacts.get(checkpointRef)
    if (cached !== undefined) return cached
    const paths = this.artifactPaths(checkpointRef)
    const custody = this.custody
    if (paths === undefined || custody === undefined) return undefined
    try {
      const encryptedExists = await this.exists(paths.encrypted)
      const plaintextExists = await this.exists(paths.plaintext)
      const legacyExists = await this.exists(paths.legacyMetadata) || await this.exists(paths.legacyBytes)
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
      if (retained !== undefined) this.artifacts.set(checkpointRef, retained)
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
    const retained = {
      bundle: input.bundle,
      artifactRef: input.artifact.artifactRef,
      digest: input.artifact.digest,
      bytes: owned,
    }
    const paths = this.artifactPaths(checkpointRef)
    const custody = this.custody
    if (paths !== undefined && custody !== undefined) {
      const payloadBytes = new TextEncoder().encode(canonicalJson({
        schema: "openagents.portable_checkpoint_artifact_custody_payload.v2",
        checkpointRef,
        artifactRef: retained.artifactRef,
        digest: retained.digest,
        bundle: retained.bundle,
        bytesBase64: Buffer.from(owned).toString("base64"),
      }))
      if (payloadBytes.byteLength > this.maxPayloadBytes()) {
        payloadBytes.fill(0)
        owned.fill(0)
        throw new PylonPortableCheckpointArtifactError("artifact_too_large", "checkpoint custody payload exceeds its size bound")
      }
      try {
        await mkdir(custody.custodyDirectory, { recursive: true, mode: 0o700 })
        await chmod(custody.custodyDirectory, 0o700)
        if (custody.policy === "owner_device_not_required") {
          await atomicPrivateWrite(paths.plaintext, payloadBytes)
        } else {
          const config = custody
          const key = await this.loadKey(config)
          const nonce = Uint8Array.from(randomBytes(AES_GCM_NONCE_BYTES))
          const aad = new TextEncoder().encode(canonicalJson(encryptedHeader(paths.objectRef, config)))
          let ciphertext: Buffer | undefined
          let authTag: Buffer | undefined
          let envelopeBytes: Uint8Array | undefined
          try {
            const cipher = createCipheriv("aes-256-gcm", key, nonce, { authTagLength: AES_GCM_TAG_BYTES })
            cipher.setAAD(aad)
            ciphertext = Buffer.concat([cipher.update(payloadBytes), cipher.final()])
            authTag = cipher.getAuthTag()
            envelopeBytes = new TextEncoder().encode(canonicalJson({
              ...encryptedHeader(paths.objectRef, config),
              nonceBase64: Buffer.from(nonce).toString("base64"),
              authTagBase64: authTag.toString("base64"),
              ciphertextBase64: ciphertext.toString("base64"),
            }))
            await atomicPrivateWrite(paths.encrypted, envelopeBytes)
          } finally {
            key.fill(0)
            nonce.fill(0)
            aad.fill(0)
            ciphertext?.fill(0)
            authTag?.fill(0)
            envelopeBytes?.fill(0)
          }
        }
      } finally {
        payloadBytes.fill(0)
      }
    }
    existing?.bytes.fill(0)
    this.artifacts.set(checkpointRef, retained)
  }

  async resolve(input: PortableCheckpointArtifactResolverInput): Promise<PortableCheckpointArtifact> {
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
