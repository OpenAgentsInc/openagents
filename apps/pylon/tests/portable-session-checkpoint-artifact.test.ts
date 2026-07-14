import { Runtime } from "@openagentsinc/runtime-platform"
import { createHash } from "node:crypto"
import { lstat, mkdtemp, readFile, readlink, rm, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { afterEach, expect, test } from "vite-plus/test"
import { canonicalJson } from "@openagentsinc/khala-sync"

import {
  PylonPortableCheckpointArtifactError,
  PylonPortableCheckpointArtifactStore,
} from "../src/portable-session-checkpoint-artifact.js"
import type { PylonPortableCheckpointBundle } from "../src/portable-session-operation-ledger.js"

const roots: string[] = []
const sha = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const git = async (cwd: string, args: string[]): Promise<string> => {
  const proc = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  const [stdout, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited])
  if (code !== 0) throw new Error(`git ${args[0]} failed`)
  return stdout.trim()
}

const fixture = async (): Promise<Readonly<{
  root: string
  bundle: PylonPortableCheckpointBundle
}>> => {
  const root = await mkdtemp(join(tmpdir(), "oa-portable-artifact-"))
  roots.push(root)
  await git(root, ["init", "-q"])
  await git(root, ["config", "user.name", "Portable Test"])
  await git(root, ["config", "user.email", "portable@example.invalid"])
  await writeFile(join(root, "tracked.txt"), "tracked\n", { mode: 0o644 })
  await git(root, ["add", "tracked.txt"])
  await git(root, ["commit", "-qm", "fixture"])
  await writeFile(join(root, "scratch.txt"), "untracked\n", { mode: 0o644 })
  const revision = await git(root, ["rev-parse", "HEAD"])
  const postImage = createHash("sha256")
  postImage.update("scratch.txt").update("\0").update("untracked\n").update("\0")
  postImage.update("tracked.txt").update("\0").update("tracked\n").update("\0")
  const graph = {
    rootAgentRef: "agent.portable.artifact.root",
    nodes: [{
      agentRef: "agent.portable.artifact.root",
      threadRef: "thread.portable.artifact.root",
      transcriptRef: "transcript.portable.artifact.root",
      activityCursor: 7,
      lifecycle: "quiesced" as const,
      attachmentGeneration: 1,
    }],
  }
  const threadCursors = [{
    threadRef: "thread.portable.artifact.root",
    transcriptRef: "transcript.portable.artifact.root",
    activityCursor: 7,
    eventCursor: 11,
  }]
  const payload = {
    schema: "openagents.portable_checkpoint.v1" as const,
    checkpointRef: "checkpoint.portable.artifact.1",
    sessionRef: "session.portable.artifact",
    sourceAttachmentRef: "attachment.portable.artifact.local.1",
    sourceGeneration: 1,
    repositoryRef: "repository.OpenAgentsInc.openagents",
    repositoryRevisionRef: revision,
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}`,
    diffDigest: sha("fixture-diff"),
    eventLogCursor: 11,
    catalogGenerationRef: "catalog.portable.artifact.1",
    graphDigest: sha(canonicalJson(graph)),
    approvalRefs: [],
    artifactRefs: [],
    receiptRefs: ["receipt.portable.artifact.source"],
    secretMaterial: "excluded" as const,
    processState: "excluded" as const,
  }
  return {
    root,
    bundle: {
      checkpoint: { ...payload, digest: sha(canonicalJson(payload)) },
      executionBinding: {
        schema: "openagents.portable_session_execution_binding.v1",
        sessionRef: payload.sessionRef,
        ownerRef: "owner.portable.artifact",
        runRef: "run.portable.artifact",
        repositoryRef: payload.repositoryRef,
        pinnedBaseRef: revision,
      },
      graph,
      threadCursors,
    },
  }
}

const rebindRepository = async (
  root: string,
  bundle: PylonPortableCheckpointBundle,
): Promise<PylonPortableCheckpointBundle> => {
  const revision = await git(root, ["rev-parse", "HEAD"])
  const deleted = new Set((await git(root, ["ls-files", "--deleted"])).split("\n").filter(Boolean))
  const paths = (await git(root, ["ls-files", "-co", "--exclude-standard"])).split("\n")
    .filter(path => path.length > 0 && !deleted.has(path)).sort()
  const postImage = createHash("sha256")
  for (const path of paths) {
    const absolute = join(root, path)
    const info = await lstat(absolute)
    const bytes = info.isSymbolicLink()
      ? new TextEncoder().encode(await readlink(absolute))
      : await readFile(absolute)
    postImage.update(path).update("\0").update(bytes).update("\0")
  }
  const { digest: _digest, ...oldPayload } = bundle.checkpoint
  const payload = {
    ...oldPayload,
    repositoryRevisionRef: revision,
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}`,
  }
  return {
    ...bundle,
    checkpoint: { ...payload, digest: sha(canonicalJson(payload)) },
  }
}

const tarFiles = (bytes: Uint8Array): Map<string, Uint8Array> => {
  const files = new Map<string, Uint8Array>()
  for (let offset = 0; offset + 512 <= bytes.byteLength;) {
    const header = bytes.subarray(offset, offset + 512)
    if (header.every(byte => byte === 0)) break
    const text = (start: number, length: number) =>
      new TextDecoder().decode(header.subarray(start, start + length)).replace(/\0.*$/u, "")
    const name = text(0, 100)
    const prefix = text(345, 155)
    const path = prefix.length === 0 ? name : `${prefix}/${name}`
    const size = Number.parseInt(text(124, 12).trim() || "0", 8)
    const start = offset + 512
    files.set(path, bytes.slice(start, start + size))
    offset = start + Math.ceil(size / 512) * 512
  }
  return files
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

test("exports a canonical private tar.zst bound to the exact checkpoint", async () => {
  const { bundle, root } = await fixture()
  const store = new PylonPortableCheckpointArtifactStore()
  store.register({ bundle, workingDirectory: root })
  const artifact = await store.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  expect(sha(artifact.bytes)).toBe(artifact.digest)
  const files = tarFiles(Runtime.zstdDecompressSync(artifact.bytes))
  expect([...files.keys()]).toEqual([
    "manifest.json",
    "repository.bundle",
    "post-image/scratch.txt",
    "post-image/tracked.txt",
  ])
  const manifest = JSON.parse(new TextDecoder().decode(files.get("manifest.json")))
  expect(manifest).toEqual({
    schema: "openagents.portable_checkpoint_artifact.v1",
    artifactRef: artifact.artifactRef,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
    files: [
      { path: "scratch.txt", mode: 0o644, sha256: sha("untracked\n"), size: 10 },
      { path: "tracked.txt", mode: 0o644, sha256: sha("tracked\n"), size: 8 },
    ],
  })
  const reverse = new PylonPortableCheckpointArtifactStore()
  await reverse.registerArtifact({ bundle, artifact })
  artifact.bytes.fill(0)
  const replay = await reverse.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.local",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.local.3",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  expect(sha(replay.bytes)).toBe(replay.digest)
})

test("retains managed checkpoint custody across process restart", async () => {
  const { bundle, root } = await fixture()
  const producer = new PylonPortableCheckpointArtifactStore()
  producer.register({ bundle, workingDirectory: root })
  const artifact = await producer.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  const custody = join(root, ".private-checkpoint-custody")
  const first = new PylonPortableCheckpointArtifactStore(custody)
  await first.registerArtifact({ bundle, artifact })
  artifact.bytes.fill(0)

  const restarted = new PylonPortableCheckpointArtifactStore(custody)
  const replay = await restarted.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.local",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.local.3",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  expect(sha(replay.bytes)).toBe(replay.digest)
})

test("rejects credential-shaped post-images", async () => {
  const first = await fixture()
  await writeFile(join(first.root, ".env"), "TOKEN=not-exported\n")
  const store = new PylonPortableCheckpointArtifactStore()
  store.register({ bundle: first.bundle, workingDirectory: first.root })
  await expect(store.resolve({
    ownerRef: first.bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: first.bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: first.bundle.checkpoint.checkpointRef,
    bundle: first.bundle,
  })).rejects.toBeInstanceOf(PylonPortableCheckpointArtifactError)
})

test("preserves tracked deletions as absence from the post-image", async () => {
  const first = await fixture()
  await rm(join(first.root, "tracked.txt"))
  const bundle = await rebindRepository(first.root, first.bundle)
  const store = new PylonPortableCheckpointArtifactStore()
  store.register({ bundle, workingDirectory: first.root })
  const artifact = await store.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  const files = tarFiles(Runtime.zstdDecompressSync(artifact.bytes))
  const manifest = JSON.parse(new TextDecoder().decode(files.get("manifest.json")))
  expect(manifest.files.map((file: { path: string }) => file.path)).toEqual(["scratch.txt"])
  expect(files.has("post-image/tracked.txt")).toBe(false)
})

test("preserves bounded relative symlinks and rejects escaping links", async () => {
  const first = await fixture()
  await symlink("tracked.txt", join(first.root, "alias.txt"))
  await git(first.root, ["add", "alias.txt"])
  await git(first.root, ["commit", "--amend", "-qm", "fixture with bounded link"])
  const bundle = await rebindRepository(first.root, first.bundle)
  const store = new PylonPortableCheckpointArtifactStore()
  store.register({ bundle, workingDirectory: first.root })
  const artifact = await store.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })
  const files = tarFiles(Runtime.zstdDecompressSync(artifact.bytes))
  const manifest = JSON.parse(new TextDecoder().decode(files.get("manifest.json")))
  expect(manifest.files.find((file: { path: string }) => file.path === "alias.txt")).toEqual({
    path: "alias.txt",
    mode: 0o120000,
    sha256: sha("tracked.txt"),
    size: 11,
    linkTarget: "tracked.txt",
  })

  await rm(join(first.root, "alias.txt"))
  await symlink("../outside", join(first.root, "alias.txt"))
  const unsafeBundle = await rebindRepository(first.root, bundle)
  const unsafe = new PylonPortableCheckpointArtifactStore()
  unsafe.register({ bundle: unsafeBundle, workingDirectory: first.root })
  await expect(unsafe.resolve({
    ownerRef: unsafeBundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: unsafeBundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: unsafeBundle.checkpoint.checkpointRef,
    bundle: unsafeBundle,
  })).rejects.toBeInstanceOf(PylonPortableCheckpointArtifactError)
})
