import { createHash } from "node:crypto"
import { chmod, lstat, mkdir, readFile, readlink, rename, rm, symlink, writeFile } from "node:fs/promises"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"

import { canonicalJson } from "@openagentsinc/khala-sync"

import type { PylonPortableControlSessionLifecycle } from "./node/control-sessions.js"
import type {
  PortableCheckpointArtifactResolver,
} from "./portable-session-checkpoint-artifact.js"
import type {
  PylonPortableLocalRehydrator,
  PylonPortableLocalStage,
} from "./portable-session-destination.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u

type ArtifactManifestFile = Readonly<{
  path: string
  mode: number
  sha256: `sha256:${string}`
  size: number
  linkTarget?: string
}>

type ArtifactManifest = Readonly<{
  schema: "openagents.portable_checkpoint_artifact.v1"
  artifactRef: string
  checkpointRef: string
  bundle: Parameters<PylonPortableLocalRehydrator["stage"]>[0]["bundle"]
  files: ReadonlyArray<ArtifactManifestFile>
}>

type TarEntry = Readonly<{
  bytes: Uint8Array
  linkTarget?: string
  mode: number
}>

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`

const safeRelativePath = (value: string): string => {
  if (value.length === 0 || value.startsWith("/") || value.includes("\0") || value.includes("\\") ||
      value.split("/").some(part => part === "" || part === "." || part === "..") ||
      value === ".git" || value.startsWith(".git/")) {
    throw new Error("checkpoint artifact contains an unsafe repository path")
  }
  return value
}

const tarText = (header: Uint8Array, offset: number, length: number): string =>
  new TextDecoder().decode(header.subarray(offset, offset + length)).replace(/\0.*$/u, "")

const parseTar = (bytes: Uint8Array): Map<string, TarEntry> => {
  const entries = new Map<string, TarEntry>()
  for (let offset = 0; offset + 512 <= bytes.byteLength;) {
    const header = bytes.subarray(offset, offset + 512)
    if (header.every(byte => byte === 0)) break
    const name = tarText(header, 0, 100)
    const prefix = tarText(header, 345, 155)
    const path = prefix.length === 0 ? name : `${prefix}/${name}`
    const size = Number.parseInt(tarText(header, 124, 12).trim() || "0", 8)
    const mode = Number.parseInt(tarText(header, 100, 8).trim() || "0", 8)
    const kind = header[156]
    if (!Number.isSafeInteger(size) || size < 0 || (kind !== 0x30 && kind !== 0x32) ||
        entries.has(path) || offset + 512 + size > bytes.byteLength) {
      throw new Error("checkpoint artifact tar is invalid")
    }
    const contentStart = offset + 512
    entries.set(path, {
      bytes: Uint8Array.from(bytes.subarray(contentStart, contentStart + size)),
      mode,
      ...(kind === 0x32 ? { linkTarget: tarText(header, 157, 100) } : {}),
    })
    offset = contentStart + Math.ceil(size / 512) * 512
  }
  return entries
}

const runGit = async (cwd: string, args: ReadonlyArray<string>): Promise<Uint8Array> => {
  const process = Bun.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  const [stdout, exitCode] = await Promise.all([new Response(process.stdout).bytes(), process.exited])
  if (exitCode !== 0) throw new Error("local checkpoint repository restore failed")
  return Uint8Array.from(stdout)
}

const repositoryEntryBytes = async (cwd: string, path: string): Promise<Uint8Array> => {
  const absolute = join(cwd, path)
  const info = await lstat(absolute)
  return info.isSymbolicLink()
    ? new TextEncoder().encode(await readlink(absolute))
    : Uint8Array.from(await readFile(absolute))
}

const repositorySnapshot = async (cwd: string) => {
  const revisionBytes = await runGit(cwd, ["rev-parse", "HEAD"])
  const repositoryRevisionRef = new TextDecoder().decode(revisionBytes).trim()
  revisionBytes.fill(0)
  const listedBytes = await runGit(cwd, ["ls-files", "-co", "--exclude-standard", "-z"])
  const deletedBytes = await runGit(cwd, ["ls-files", "--deleted", "-z"])
  const deleted = new Set(new TextDecoder().decode(deletedBytes).split("\0").filter(Boolean))
  const listed = new TextDecoder().decode(listedBytes).split("\0").filter(path => path.length > 0 && !deleted.has(path)).sort()
  listedBytes.fill(0)
  deletedBytes.fill(0)
  const postImage = createHash("sha256")
  for (const path of listed) {
    const bytes = await repositoryEntryBytes(cwd, path)
    postImage.update(path).update("\0").update(bytes).update("\0")
    bytes.fill(0)
  }
  const trackedDiff = await runGit(cwd, ["diff", "--binary", "HEAD", "--"])
  const untrackedBytes = await runGit(cwd, ["ls-files", "--others", "--exclude-standard", "-z"])
  const untracked = new TextDecoder().decode(untrackedBytes).split("\0").filter(Boolean).sort()
  untrackedBytes.fill(0)
  const diff = createHash("sha256").update(trackedDiff)
  trackedDiff.fill(0)
  for (const path of untracked) {
    const bytes = await repositoryEntryBytes(cwd, path)
    diff.update(path).update("\0").update(bytes).update("\0")
    bytes.fill(0)
  }
  return {
    repositoryRevisionRef,
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}`,
    diffDigest: `sha256:${diff.digest("hex")}`,
  }
}

const validateManifest = (
  manifest: ArtifactManifest,
  input: Parameters<PylonPortableLocalRehydrator["stage"]>[0],
  artifactRef: string,
): void => {
  if (manifest.schema !== "openagents.portable_checkpoint_artifact.v1" ||
      manifest.artifactRef !== artifactRef || manifest.checkpointRef !== input.bundle.checkpoint.checkpointRef ||
      canonicalJson(manifest.bundle) !== canonicalJson(input.bundle) ||
      !Array.isArray(manifest.files) ||
      canonicalJson(manifest.files) !== canonicalJson([...manifest.files].sort((a, b) => a.path.localeCompare(b.path)))) {
    throw new Error("checkpoint artifact manifest binding is invalid")
  }
  const paths = new Set<string>()
  for (const file of manifest.files) {
    safeRelativePath(file.path)
    if (paths.has(file.path) || !Number.isSafeInteger(file.size) || file.size < 0 ||
        (file.mode !== 0o644 && file.mode !== 0o755 && file.mode !== 0o120000) ||
        !/^sha256:[a-f0-9]{64}$/u.test(file.sha256) ||
        (file.mode === 0o120000) !== (file.linkTarget !== undefined)) {
      throw new Error("checkpoint artifact manifest inventory is invalid")
    }
    paths.add(file.path)
  }
}

export const createPylonPortableLocalRehydrator = (input: Readonly<{
  targetRef: string
  custodyRoot: string
  artifacts: PortableCheckpointArtifactResolver
  lifecycle: PylonPortableControlSessionLifecycle
}>): PylonPortableLocalRehydrator => {
  if (!SAFE_REF.test(input.targetRef) || !isAbsolute(input.custodyRoot)) {
    throw new Error("portable local rehydrator configuration is invalid")
  }
  const operationDirectory = (operationRef: string) => join(
    input.custodyRoot,
    createHash("sha256").update(operationRef).digest("hex"),
  )
  const stagePath = (operationRef: string) => `${operationDirectory(operationRef)}.json`
  const readPersistedStage = async (operationRef: string): Promise<PylonPortableLocalStage> => {
    try {
      const parsed = JSON.parse(await readFile(stagePath(operationRef), "utf8")) as {
        schema?: unknown
        stage?: unknown
      }
      if (parsed.schema !== "openagents.pylon.portable_local_stage.v1" || typeof parsed.stage !== "object" || parsed.stage === null) {
        throw new Error("invalid stage")
      }
      const stage = parsed.stage as PylonPortableLocalStage
      if (stage.operationRef !== operationRef) throw new Error("invalid stage")
      return stage
    } catch {
      throw new Error("portable local stage is unavailable")
    }
  }

  return {
    stage: async operation => {
      const existing = await readPersistedStage(operation.operationRef).catch(() => undefined)
      if (existing !== undefined) return existing
      const checkpoint = operation.bundle.checkpoint
      const artifact = await input.artifacts.resolve({
        ownerRef: operation.bundle.executionBinding.ownerRef,
        targetRef: input.targetRef,
        sessionRef: checkpoint.sessionRef,
        attachmentRef: operation.destinationAttachmentRef,
        generation: operation.destinationGeneration,
        checkpointRef: checkpoint.checkpointRef,
        bundle: operation.bundle,
      })
      const compressed = artifact.bytes
      let tarBytes: Uint8Array | undefined
      let entries: Map<string, TarEntry> | undefined
      let lifecycleStaged = false
      const directory = operationDirectory(operation.operationRef)
      try {
        if (sha256(compressed) !== artifact.digest) throw new Error("checkpoint artifact digest is invalid")
        tarBytes = Bun.zstdDecompressSync(compressed)
        entries = parseTar(tarBytes)
        const manifestEntry = entries.get("manifest.json")
        const bundleEntry = entries.get("repository.bundle")
        if (manifestEntry === undefined || bundleEntry === undefined || manifestEntry.linkTarget !== undefined ||
            bundleEntry.linkTarget !== undefined) throw new Error("checkpoint artifact is incomplete")
        const manifest = JSON.parse(new TextDecoder().decode(manifestEntry.bytes)) as ArtifactManifest
        validateManifest(manifest, operation, artifact.artifactRef)
        const expectedEntries = new Set(["manifest.json", "repository.bundle", ...manifest.files.map(file => `post-image/${file.path}`)])
        if (entries.size !== expectedEntries.size || [...entries.keys()].some(path => !expectedEntries.has(path))) {
          throw new Error("checkpoint artifact contains unmanifested entries")
        }
        await rm(directory, { recursive: true, force: true })
        await mkdir(input.custodyRoot, { recursive: true, mode: 0o700 })
        await chmod(input.custodyRoot, 0o700)
        const bundlePath = `${directory}.bundle`
        await writeFile(bundlePath, bundleEntry.bytes, { mode: 0o600 })
        try {
          await runGit(input.custodyRoot, ["clone", "--no-checkout", bundlePath, directory])
          await runGit(directory, ["checkout", "--detach", checkpoint.repositoryRevisionRef])
          await runGit(directory, ["remote", "remove", "origin"])
        } finally {
          await rm(bundlePath, { force: true })
        }
        const trackedBytes = await runGit(directory, ["ls-files", "-z"])
        const manifestPaths = new Set(manifest.files.map(file => file.path))
        for (const path of new TextDecoder().decode(trackedBytes).split("\0").filter(Boolean)) {
          if (!manifestPaths.has(path)) await rm(join(directory, path), { force: true })
        }
        trackedBytes.fill(0)
        for (const file of manifest.files) {
          const entry = entries.get(`post-image/${file.path}`)
          if (entry === undefined || entry.mode !== file.mode) throw new Error("checkpoint post-image mode is invalid")
          const absolute = join(directory, file.path)
          if (!relative(resolve(directory), resolve(absolute)).startsWith("..")) {
            await mkdir(dirname(absolute), { recursive: true })
          } else {
            throw new Error("checkpoint post-image escapes destination")
          }
          if (file.mode === 0o120000) {
            const target = entry.linkTarget
            if (target === undefined || target !== file.linkTarget || isAbsolute(target) || target.includes("\\") ||
                target.split("/").some(part => part === "..") ||
                relative(resolve(directory), resolve(dirname(absolute), target)).startsWith("..") ||
                sha256(target) !== file.sha256 || file.size !== new TextEncoder().encode(target).byteLength) {
              throw new Error("checkpoint symlink is invalid")
            }
            await rm(absolute, { force: true })
            await symlink(target, absolute)
          } else {
            if (entry.linkTarget !== undefined || entry.bytes.byteLength !== file.size || sha256(entry.bytes) !== file.sha256) {
              throw new Error("checkpoint post-image file is invalid")
            }
            await writeFile(absolute, entry.bytes, { mode: file.mode })
            await chmod(absolute, file.mode)
          }
        }
        const snapshot = await repositorySnapshot(directory)
        if (snapshot.repositoryRevisionRef !== checkpoint.repositoryRevisionRef ||
            snapshot.repositoryPostImageDigest !== checkpoint.repositoryPostImageDigest ||
            snapshot.diffDigest !== checkpoint.diffDigest) {
          throw new Error("restored repository does not match checkpoint digests")
        }
        const workspaceRef = stableRef(
          "workspace.pylon.portable.rehydrated",
          `${checkpoint.checkpointRef}:${checkpoint.repositoryPostImageDigest}`,
        )
        const staged = await input.lifecycle.stageDestination({
          sessionRef: checkpoint.sessionRef,
          sourceAttachmentRef: checkpoint.sourceAttachmentRef,
          sourceGeneration: checkpoint.sourceGeneration,
          destinationAttachmentRef: operation.destinationAttachmentRef,
          destinationGeneration: operation.destinationGeneration,
          checkpointRef: checkpoint.checkpointRef,
          agentRefs: operation.bundle.graph.nodes.map(node => node.agentRef),
          workingDirectory: directory,
          workspaceRef,
        })
        lifecycleStaged = true
        const stage: PylonPortableLocalStage = {
          operationRef: operation.operationRef,
          sessionRef: checkpoint.sessionRef,
          checkpointRef: checkpoint.checkpointRef,
          checkpointDigest: checkpoint.digest,
          sourceAttachmentRef: checkpoint.sourceAttachmentRef,
          sourceGeneration: checkpoint.sourceGeneration,
          destinationAttachmentRef: operation.destinationAttachmentRef,
          destinationGeneration: operation.destinationGeneration,
          repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
          diffDigest: checkpoint.diffDigest,
          graphDigest: checkpoint.graphDigest,
          stagedAgentRefs: operation.bundle.graph.nodes.map(node => node.agentRef),
          threadCursors: operation.bundle.threadCursors,
          capabilityLeaseRefs: [...operation.capabilityLeaseRefs],
          acceptingWork: false,
          evidenceRefs: [
            stableRef("receipt.pylon.portable.artifact_materialized", `${checkpoint.checkpointRef}:${artifact.digest}`),
            ...staged.evidenceRefs,
          ],
        }
        const temporary = `${stagePath(operation.operationRef)}.tmp`
        await writeFile(temporary, `${canonicalJson({ schema: "openagents.pylon.portable_local_stage.v1", stage })}\n`, { mode: 0o600 })
        await rename(temporary, stagePath(operation.operationRef))
        await chmod(stagePath(operation.operationRef), 0o600)
        return stage
      } catch (error) {
        if (lifecycleStaged) {
          await input.lifecycle.abortDestination({
            sessionRef: checkpoint.sessionRef,
            destinationAttachmentRef: operation.destinationAttachmentRef,
            destinationGeneration: operation.destinationGeneration,
            checkpointRef: checkpoint.checkpointRef,
            agentRefs: operation.bundle.graph.nodes.map(node => node.agentRef),
            workingDirectory: directory,
          }).catch(() => undefined)
        }
        await rm(directory, { recursive: true, force: true })
        throw error
      } finally {
        compressed.fill(0)
        tarBytes?.fill(0)
        for (const entry of entries?.values() ?? []) entry.bytes.fill(0)
      }
    },
    readStage: readPersistedStage,
    activate: async operation => {
      const directory = operationDirectory(operation.stage.operationRef)
      const workspaceRef = stableRef(
        "workspace.pylon.portable.rehydrated",
        `${operation.stage.checkpointRef}:${operation.stage.repositoryPostImageDigest}`,
      )
      const activation = await input.lifecycle.activateDestination({
        sessionRef: operation.stage.sessionRef,
        destinationAttachmentRef: operation.stage.destinationAttachmentRef,
        destinationGeneration: operation.stage.destinationGeneration,
        checkpointRef: operation.stage.checkpointRef,
        agentRefs: operation.stage.stagedAgentRefs,
        workingDirectory: directory,
        workspaceRef,
      })
      return {
        activatedAgentRefs: activation.activatedAgentRefs,
        acceptedWorkRefs: activation.acceptedWorkRefs,
        evidenceRefs: [operation.authorityEvidenceRef, ...activation.evidenceRefs],
      }
    },
    abort: async operation => {
      const directory = operationDirectory(operation.stage.operationRef)
      const aborted = await input.lifecycle.abortDestination({
        sessionRef: operation.stage.sessionRef,
        destinationAttachmentRef: operation.stage.destinationAttachmentRef,
        destinationGeneration: operation.stage.destinationGeneration,
        checkpointRef: operation.stage.checkpointRef,
        agentRefs: operation.stage.stagedAgentRefs,
        workingDirectory: directory,
      })
      await rm(stagePath(operation.stage.operationRef), { force: true })
      return {
        cleanedAgentRefs: operation.stage.stagedAgentRefs,
        releasedCapabilityLeaseRefs: operation.stage.capabilityLeaseRefs,
        processes: "released",
        scratch: "released",
        ports: "released",
        evidenceRefs: aborted.evidenceRefs,
      }
    },
  }
}
