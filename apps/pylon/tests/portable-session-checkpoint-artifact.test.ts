import { Runtime } from "@openagentsinc/runtime-platform"
import { createHash } from "node:crypto"
import { lstat, mkdtemp, readFile, readdir, readlink, rm, stat, symlink, writeFile } from "node:fs/promises"
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

const custodyKey = Uint8Array.from({ length: 32 }, (_, index) => index + 1)
const custodyConfig = (directory: string, key = custodyKey, keyRef = "key.portable.checkpoint.2026-07") => ({
  custodyDirectory: directory,
  policy: "owner_managed" as const,
  keyRef,
  keyProvider: { loadKey: async () => Uint8Array.from(key) },
})

const onlyFile = async (directory: string): Promise<string> => {
  const files = await readdir(directory)
  expect(files).toHaveLength(1)
  const file = files.at(0)
  if (file === undefined) throw new Error("checkpoint custody file is absent")
  return join(directory, file)
}

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

const artifactFixture = async () => {
  const source = await fixture()
  const request = {
    ownerRef: source.bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: source.bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: source.bundle.checkpoint.checkpointRef,
    bundle: source.bundle,
  }
  const producer = new PylonPortableCheckpointArtifactStore()
  producer.register({ bundle: source.bundle, workingDirectory: source.root })
  return { ...source, request, artifact: await producer.resolve(request) }
}

const lifecycleBinding = (
  source: Awaited<ReturnType<typeof artifactFixture>>,
  operationRef: string,
) => ({
  operationRef,
  ownerRef: source.bundle.executionBinding.ownerRef,
  sessionRef: source.bundle.checkpoint.sessionRef,
  checkpointRef: source.bundle.checkpoint.checkpointRef,
  bundle: source.bundle,
})

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
  const first = new PylonPortableCheckpointArtifactStore(custodyConfig(custody))
  await first.registerArtifact({ bundle, artifact })
  artifact.bytes.fill(0)

  const restarted = new PylonPortableCheckpointArtifactStore(custodyConfig(custody))
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
  expect((await stat(await onlyFile(custody))).mode & 0o777).toBe(0o600)
})

test("encrypts checkpoint bytes and metadata with a fresh nonce", async () => {
  const { bundle, root } = await fixture()
  const producer = new PylonPortableCheckpointArtifactStore()
  producer.register({ bundle, workingDirectory: root })
  const input = {
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  }
  const artifact = await producer.resolve(input)
  const custody = join(root, ".encrypted-custody")
  await new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).registerArtifact({ bundle, artifact })
  const path = await onlyFile(custody)
  const firstText = await readFile(path, "utf8")
  const first = JSON.parse(firstText) as { nonceBase64: string }
  await new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).registerArtifact({ bundle, artifact })
  const secondText = await readFile(path, "utf8")
  const second = JSON.parse(secondText) as { nonceBase64: string }

  expect(second.nonceBase64).not.toBe(first.nonceBase64)
  expect(secondText).not.toContain("tracked.txt")
  expect(secondText).not.toContain("untracked\n")
  expect(secondText).not.toContain(bundle.checkpoint.checkpointRef)
  expect(secondText).not.toContain(Buffer.from(custodyKey).toString("base64"))
  expect(secondText).not.toContain(Buffer.from(custodyKey).toString("hex"))
})

test("fails closed for tamper, wrong key, and rotated key ref", async () => {
  const { bundle, root } = await fixture()
  const producer = new PylonPortableCheckpointArtifactStore()
  producer.register({ bundle, workingDirectory: root })
  const request = {
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.managed",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.managed.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  }
  const artifact = await producer.resolve(request)
  const custody = join(root, ".tamper-custody")
  await new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).registerArtifact({ bundle, artifact })
  const wrongKey = Uint8Array.from({ length: 32 }, (_, index) => 255 - index)
  await expect(new PylonPortableCheckpointArtifactStore(custodyConfig(custody, wrongKey)).resolve(request))
    .rejects.toMatchObject({ code: "decrypt_failed" })
  await expect(new PylonPortableCheckpointArtifactStore(
    custodyConfig(custody, custodyKey, "key.portable.checkpoint.2026-08"),
  ).resolve(request)).rejects.toMatchObject({ code: "key_ref_mismatch" })

  const path = await onlyFile(custody)
  const envelope = JSON.parse(await readFile(path, "utf8")) as { ciphertextBase64: string }
  const ciphertext = Buffer.from(envelope.ciphertextBase64, "base64")
  if (ciphertext.length === 0) throw new Error("checkpoint ciphertext is absent")
  ciphertext[0] ^= 1
  envelope.ciphertextBase64 = ciphertext.toString("base64")
  await writeFile(path, JSON.stringify(envelope), { mode: 0o600 })
  await expect(new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).resolve(request))
    .rejects.toMatchObject({ code: "decrypt_failed" })
})

test("rejects plaintext custody without an explicit device policy", async () => {
  const { bundle, root } = await fixture()
  expect(() => new PylonPortableCheckpointArtifactStore(root)).toThrowError(
    expect.objectContaining({ code: "plaintext_downgrade" }),
  )
  const downgradeCustody = await mkdtemp(join(tmpdir(), "oa-portable-plaintext-custody-"))
  roots.push(downgradeCustody)
  const name = createHash("sha256").update(bundle.checkpoint.checkpointRef).digest("hex")
  await writeFile(join(downgradeCustody, `${name}.json`), "{}")
  await expect(new PylonPortableCheckpointArtifactStore(custodyConfig(downgradeCustody)).resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.local",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.local.3",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  })).rejects.toMatchObject({ code: "plaintext_downgrade" })

  const producer = new PylonPortableCheckpointArtifactStore()
  producer.register({ bundle, workingDirectory: root })
  const request = {
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.portable.artifact.local",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.portable.artifact.local.3",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  }
  const artifact = await producer.resolve(request)
  const deviceCustody = join(root, ".device-custody")
  const policy = { custodyDirectory: deviceCustody, policy: "owner_device_not_required" as const }
  await new PylonPortableCheckpointArtifactStore(policy).registerArtifact({ bundle, artifact })
  const replay = await new PylonPortableCheckpointArtifactStore(policy).resolve(request)
  expect(replay.digest).toBe(artifact.digest)
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

test("enforces the persisted retention expiry with an injected clock", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".retention-custody")
  let now = new Date("2026-07-20T12:00:00.000Z")
  const config = {
    ...custodyConfig(custody),
    retentionSeconds: 10,
    now: () => now,
  }
  await new PylonPortableCheckpointArtifactStore(config).registerArtifact({
    bundle: source.bundle,
    artifact: source.artifact,
  })
  expect(
    (await new PylonPortableCheckpointArtifactStore(config).resolve(source.request)).digest,
  ).toBe(source.artifact.digest)
  now = new Date("2026-07-20T12:00:10.000Z")
  await expect(
    new PylonPortableCheckpointArtifactStore(config).resolve(source.request),
  ).rejects.toMatchObject({ code: "retention_expired" })
})

test("deletes checkpoint custody idempotently with a verified public-safe receipt", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".deletion-custody")
  const store = new PylonPortableCheckpointArtifactStore(custodyConfig(custody))
  await store.registerArtifact({ bundle: source.bundle, artifact: source.artifact })
  const input = lifecycleBinding(source, "operation.checkpoint.delete.1")
  const receipt = await store.deleteArtifact(input)
  const replay = await new PylonPortableCheckpointArtifactStore(
    custodyConfig(custody),
  ).deleteArtifact(input)

  expect(replay).toEqual(receipt)
  expect(receipt).toMatchObject({ state: "deleted", verifiedAbsent: true, publicSafe: true })
  expect(JSON.stringify(receipt)).not.toContain(Buffer.from(custodyKey).toString("base64"))
  expect(await readdir(custody)).toHaveLength(1)
  await expect(
    new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).resolve(source.request),
  ).rejects.toMatchObject({ code: "unavailable" })
  const objectName = createHash("sha256")
    .update(source.bundle.checkpoint.checkpointRef)
    .digest("hex")
  const resurrected = join(custody, `${objectName}.checkpoint.aesgcm`)
  await writeFile(resurrected, "forged object", { mode: 0o600 })
  await expect(
    new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).deleteArtifact(input),
  ).rejects.toMatchObject({ code: "deletion_failed" })
  await rm(resurrected)
  await expect(
    new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).deleteArtifact({
      ...input,
      ownerRef: "owner.portable.artifact.other",
    }),
  ).rejects.toMatchObject({ code: "invalid_binding" })
})

test("recovers deletion after a crash without retaining ciphertext or plaintext", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".deletion-crash-custody")
  let injected = false
  const crashing = new PylonPortableCheckpointArtifactStore({
    ...custodyConfig(custody),
    faultInjector: (step: string) => {
      if (step === "delete_object_removed" && !injected) {
        injected = true
        throw new Error("simulated process crash")
      }
    },
  })
  await crashing.registerArtifact({ bundle: source.bundle, artifact: source.artifact })
  const input = lifecycleBinding(source, "operation.checkpoint.delete.crash")
  await expect(crashing.deleteArtifact(input)).rejects.toThrow("simulated process crash")
  const receipt = await new PylonPortableCheckpointArtifactStore(
    custodyConfig(custody),
  ).deleteArtifact(input)
  expect(receipt.verifiedAbsent).toBe(true)
  const retainedText = (
    await Promise.all((await readdir(custody)).map((name) => readFile(join(custody, name), "utf8")))
  ).join("\n")
  expect(retainedText).not.toContain("tracked.txt")
  expect(retainedText).not.toContain("untracked\n")
  expect(retainedText).not.toContain(Buffer.from(custodyKey).toString("base64"))
})

test("rewraps authenticated ciphertext and preserves the old object until the new object is durable", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".rewrap-custody")
  const nextKey = Uint8Array.from({ length: 32 }, (_, index) => 200 - index)
  let injected = false
  const crashing = new PylonPortableCheckpointArtifactStore({
    ...custodyConfig(custody),
    faultInjector: (step: string) => {
      if (step === "rewrap_ciphertext_durable" && !injected) {
        injected = true
        throw new Error("simulated rewrap crash")
      }
    },
  })
  await crashing.registerArtifact({ bundle: source.bundle, artifact: source.artifact })
  const input = {
    ...lifecycleBinding(source, "operation.checkpoint.rewrap.1"),
    keyRef: "key.portable.checkpoint.2026-08",
    keyProvider: { loadKey: async () => Uint8Array.from(nextKey) },
  }
  await expect(crashing.rewrapArtifact(input)).rejects.toThrow("simulated rewrap crash")
  expect(
    (await new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).resolve(source.request))
      .digest,
  ).toBe(source.artifact.digest)

  const rewrapStore = new PylonPortableCheckpointArtifactStore(custodyConfig(custody))
  const receipt = await rewrapStore.rewrapArtifact(input)
  expect(receipt).toMatchObject({
    previousKeyRef: "key.portable.checkpoint.2026-07",
    keyRef: "key.portable.checkpoint.2026-08",
    state: "rewrapped",
    verified: true,
  })
  expect(
    await new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).rewrapArtifact(input),
  ).toEqual(receipt)
  expect((await rewrapStore.resolve(source.request)).digest).toBe(source.artifact.digest)
  const nextConfig = custodyConfig(custody, nextKey, "key.portable.checkpoint.2026-08")
  expect(
    (await new PylonPortableCheckpointArtifactStore(nextConfig).resolve(source.request)).digest,
  ).toBe(source.artifact.digest)
  await expect(
    new PylonPortableCheckpointArtifactStore(custodyConfig(custody)).resolve(source.request),
  ).rejects.toMatchObject({ code: "key_ref_mismatch" })
  const retainedText = (
    await Promise.all((await readdir(custody)).map((name) => readFile(join(custody, name), "utf8")))
  ).join("\n")
  expect(retainedText).not.toContain("tracked.txt")
  expect(retainedText).not.toContain("untracked\n")
  expect(retainedText).not.toContain(Buffer.from(custodyKey).toString("base64"))
  expect(retainedText).not.toContain(Buffer.from(nextKey).toString("base64"))
})

test("finishes rewrap after a crash that replaced the old ciphertext", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".rewrap-replaced-custody")
  const nextKey = Uint8Array.from({ length: 32 }, (_, index) => 150 - index)
  let injected = false
  const config = {
    ...custodyConfig(custody),
    faultInjector: (step: string) => {
      if (step === "rewrap_replaced" && !injected) {
        injected = true
        throw new Error("simulated post-replace crash")
      }
    },
  }
  const crashing = new PylonPortableCheckpointArtifactStore(config)
  await crashing.registerArtifact({ bundle: source.bundle, artifact: source.artifact })
  const input = {
    ...lifecycleBinding(source, "operation.checkpoint.rewrap.crash"),
    keyRef: "key.portable.checkpoint.replaced",
    keyProvider: { loadKey: async () => Uint8Array.from(nextKey) },
  }
  await expect(crashing.rewrapArtifact(input)).rejects.toThrow("simulated post-replace crash")
  const receipt = await new PylonPortableCheckpointArtifactStore(
    custodyConfig(custody),
  ).rewrapArtifact(input)
  expect(receipt.verified).toBe(true)
  expect(
    (
      await new PylonPortableCheckpointArtifactStore(
        custodyConfig(custody, nextKey, input.keyRef),
      ).resolve(source.request)
    ).digest,
  ).toBe(source.artifact.digest)
})

test("removes bounded orphan custody temp files before resolution", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".orphan-temp-custody")
  const config = { ...custodyConfig(custody), orphanTempMaxAgeMs: 0 }
  await new PylonPortableCheckpointArtifactStore(config).registerArtifact({
    bundle: source.bundle,
    artifact: source.artifact,
  })
  const name = createHash("sha256").update(source.bundle.checkpoint.checkpointRef).digest("hex")
  const orphan = join(custody, `${name}.checkpoint.aesgcm.orphan.tmp`)
  await writeFile(orphan, "not plaintext", { mode: 0o600 })
  await new PylonPortableCheckpointArtifactStore(config).resolve(source.request)
  await expect(stat(orphan)).rejects.toMatchObject({ code: "ENOENT" })
})

test("fails closed when checkpoint custody exceeds its file-count bound", async () => {
  const source = await artifactFixture()
  const custody = join(source.root, ".bounded-file-custody")
  const config = { ...custodyConfig(custody), maxCustodyFiles: 1 }
  await new PylonPortableCheckpointArtifactStore(config).registerArtifact({
    bundle: source.bundle,
    artifact: source.artifact,
  })
  await writeFile(join(custody, "unexpected.object"), "bounded", { mode: 0o600 })
  await expect(
    new PylonPortableCheckpointArtifactStore(config).resolve(source.request),
  ).rejects.toMatchObject({ code: "artifact_too_large" })
})
