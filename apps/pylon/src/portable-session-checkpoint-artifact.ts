import { Runtime } from "@openagentsinc/runtime-platform"
import { createHash, randomUUID } from "node:crypto"
import { chmod, lstat, mkdir, readFile, readlink, rename, rm, writeFile } from "node:fs/promises"
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

export class PylonPortableCheckpointArtifactError extends Error {
  readonly _tag = "PylonPortableCheckpointArtifactError"
  override readonly name = "PylonPortableCheckpointArtifactError"

  constructor(
    readonly code: "invalid_binding" | "private_material" | "repository_mismatch" | "unavailable",
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

export class PylonPortableCheckpointArtifactStore implements PortableCheckpointArtifactResolver {
  private readonly sources = new Map<string, Source>()
  private readonly artifacts = new Map<string, Readonly<{
    bundle: PylonPortableCheckpointBundle
    artifactRef: string
    digest: `sha256:${string}`
    bytes: Uint8Array
  }>>()

  constructor(private readonly custodyDirectory?: string) {
    if (custodyDirectory !== undefined && !isAbsolute(custodyDirectory)) {
      throw new PylonPortableCheckpointArtifactError("invalid_binding", "checkpoint artifact custody must be absolute")
    }
  }

  private artifactPaths(checkpointRef: string): Readonly<{ bytes: string; metadata: string }> | undefined {
    if (this.custodyDirectory === undefined) return undefined
    const name = createHash("sha256").update(checkpointRef).digest("hex")
    return {
      bytes: join(this.custodyDirectory, `${name}.tar.zst`),
      metadata: join(this.custodyDirectory, `${name}.json`),
    }
  }

  private async loadArtifact(checkpointRef: string): Promise<Readonly<{
    bundle: PylonPortableCheckpointBundle
    artifactRef: string
    digest: `sha256:${string}`
    bytes: Uint8Array
  }> | undefined> {
    const cached = this.artifacts.get(checkpointRef)
    if (cached !== undefined) return cached
    const paths = this.artifactPaths(checkpointRef)
    if (paths === undefined) return undefined
    try {
      const [metadataBytes, bytes] = await Promise.all([readFile(paths.metadata), readFile(paths.bytes)])
      try {
        const metadata = JSON.parse(new TextDecoder().decode(metadataBytes)) as {
          schema?: unknown
          checkpointRef?: unknown
          artifactRef?: unknown
          digest?: unknown
          bundle?: unknown
        }
        if (metadata.schema !== "openagents.portable_checkpoint_artifact_custody.v1" ||
            metadata.checkpointRef !== checkpointRef ||
            typeof metadata.artifactRef !== "string" || !SAFE_REF.test(metadata.artifactRef) ||
            typeof metadata.digest !== "string" || !/^sha256:[a-f0-9]{64}$/u.test(metadata.digest) ||
            sha256(bytes) !== metadata.digest || typeof metadata.bundle !== "object" || metadata.bundle === null) {
          throw new PylonPortableCheckpointArtifactError("invalid_binding", "persisted managed checkpoint artifact is invalid")
        }
        const retained = {
          bundle: metadata.bundle as PylonPortableCheckpointBundle,
          artifactRef: metadata.artifactRef,
          digest: metadata.digest as `sha256:${string}`,
          bytes: new Uint8Array(bytes),
        }
        if (retained.bundle.checkpoint.checkpointRef !== checkpointRef) {
          retained.bytes.fill(0)
          throw new PylonPortableCheckpointArtifactError("invalid_binding", "persisted checkpoint binding is invalid")
        }
        this.artifacts.set(checkpointRef, retained)
        return retained
      } finally {
        metadataBytes.fill(0)
        bytes.fill(0)
      }
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
    if (paths !== undefined) {
      const suffix = randomUUID()
      const bytesTemporary = `${paths.bytes}.${suffix}.tmp`
      const metadataTemporary = `${paths.metadata}.${suffix}.tmp`
      const metadataBytes = new TextEncoder().encode(canonicalJson({
        schema: "openagents.portable_checkpoint_artifact_custody.v1",
        checkpointRef,
        artifactRef: retained.artifactRef,
        digest: retained.digest,
        bundle: retained.bundle,
      }))
      try {
        await mkdir(this.custodyDirectory!, { recursive: true, mode: 0o700 })
        await chmod(this.custodyDirectory!, 0o700)
        await writeFile(bytesTemporary, owned, { mode: 0o600 })
        await writeFile(metadataTemporary, metadataBytes, { mode: 0o600 })
        await rename(bytesTemporary, paths.bytes)
        await rename(metadataTemporary, paths.metadata)
        await Promise.all([chmod(paths.bytes, 0o600), chmod(paths.metadata, 0o600)])
      } finally {
        metadataBytes.fill(0)
        await Promise.all([
          rm(bytesTemporary, { force: true }),
          rm(metadataTemporary, { force: true }),
        ])
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
