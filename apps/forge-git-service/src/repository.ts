import { createHash, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import {
  chmod,
  lstat,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

import { BlobStore } from "@openagentsinc/oa-infra/blob-store";
import type { BlobStoreShape } from "@openagentsinc/oa-infra/blob-store";
import { Context, Effect, Layer, Option, Redacted } from "effect";

import { ForgeGitConfiguration, type ForgeGitConfigurationShape } from "./config.js";
import {
  ForgeGitBackupReceipt,
  ForgeGitMirrorReceipt,
  ForgeGitMirrorObservation,
  ForgeGitPackEvidence,
  ForgeGitRef,
  ForgeGitRepositoryError,
  type ForgeGitSession,
  type ForgeGitSignedRefPolicy,
} from "./model.js";

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();
const maxGitDiagnosticBytes = 16 * 1024;
const maxGitResultBytes = 16 * 1024 * 1024;

type GitCommandResult = Readonly<{
  stderr: string;
  stdout: Uint8Array;
}>;

export type ForgeGitReceiveResult = Readonly<{
  body: Uint8Array;
  changed: boolean;
  contentType: string;
  mirrorReceipt: Option.Option<ForgeGitMirrorReceipt>;
  refsAfter: ReadonlyArray<ForgeGitRef>;
  refsBefore: ReadonlyArray<ForgeGitRef>;
}>;

export interface ForgeGitRepositoryShape {
  /** Only the admission projector calls this after persisting a 30617 fact. */
  readonly provision: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitRepositoryError>;
  readonly advertise: (input: {
    readonly gitProtocol?: string;
    readonly operation: "git-upload-pack" | "git-receive-pack";
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<Uint8Array, ForgeGitRepositoryError>;
  readonly backup: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ForgeGitBackupReceipt, ForgeGitRepositoryError>;
  readonly listRefs: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadonlyArray<ForgeGitRef>, ForgeGitRepositoryError>;
  readonly observeMirror: (
    input: ForgeGitMirrorInput,
  ) => Effect.Effect<ForgeGitMirrorObservation, ForgeGitRepositoryError>;
  readonly projectMirror: (
    input: ForgeGitMirrorInput,
  ) => Effect.Effect<ForgeGitMirrorObservation, ForgeGitRepositoryError>;
  readonly receivePack: (input: {
    readonly body: ReadableStream<Uint8Array> | null;
    readonly gitProtocol?: string;
    readonly repositoryRef: string;
    readonly session: ForgeGitSession;
    /** Exact, verified 30618 triples consumed by the pre-receive hook. */
    readonly signedRefPolicies: ReadonlyArray<ForgeGitSignedRefPolicy>;
    readonly tenantRef: string;
  }) => Effect.Effect<ForgeGitReceiveResult, ForgeGitRepositoryError>;
  readonly restore: (input: {
    readonly receipt: ForgeGitBackupReceipt;
  }) => Effect.Effect<void, ForgeGitRepositoryError>;
  readonly uploadPack: (input: {
    readonly body: ReadableStream<Uint8Array> | null;
    readonly gitProtocol?: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadableStream<Uint8Array>, ForgeGitRepositoryError>;
  readonly verify: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitRepositoryError>;
}

export type ForgeGitMirrorInput = Readonly<{
  authorizationHeader?: Redacted.Redacted<string> | undefined;
  destinationRef: string;
  destinationUrl: string;
  expectedSourceObjectId: string;
  repositoryRef: string;
  sourceRef: string;
  tenantRef: string;
}>;

export class ForgeGitRepository extends Context.Service<
  ForgeGitRepository,
  ForgeGitRepositoryShape
>()("@openagentsinc/forge-git-service/Repository") {}

const repositoryError = (operation: string, code: string, status: number, cause?: unknown) =>
  new ForgeGitRepositoryError({
    ...(cause === undefined ? {} : { cause }),
    code,
    operation,
    status,
  });

const gitEnvironment = (
  gitProtocol: string | undefined,
  refRestrictions: ReadonlyArray<string> = [],
  signedRefPolicies: ReadonlyArray<ForgeGitSignedRefPolicy> = [],
): NodeJS.ProcessEnv => ({
  PATH: process.env["PATH"],
  HOME: process.env["HOME"],
  GIT_CONFIG_NOSYSTEM: "1",
  ...(gitProtocol === undefined ? {} : { GIT_PROTOCOL: gitProtocol }),
  OPENAGENTS_FORGE_ALLOWED_REFS_JSON: JSON.stringify(refRestrictions),
  OPENAGENTS_FORGE_SIGNED_REF_POLICIES_JSON: JSON.stringify(signedRefPolicies),
});

const mirrorEnvironment = (
  authorizationHeader: Redacted.Redacted<string> | undefined,
): NodeJS.ProcessEnv => {
  const environment = gitEnvironment(undefined);
  if (authorizationHeader === undefined) return environment;
  return {
    ...environment,
    GIT_CONFIG_COUNT: "1",
    GIT_CONFIG_KEY_0: "http.extraHeader",
    GIT_CONFIG_VALUE_0: Redacted.value(authorizationHeader),
  };
};

const assertMirrorRef = (ref: string, operation: string): void => {
  if (!/^refs\/(?:heads|tags)\/[^\s~^:?*\\[]+$/u.test(ref) || ref.includes("..")) {
    throw repositoryError(operation, "forge_git_mirror_ref_invalid", 400);
  }
};

const assertMirrorDestination = (destinationUrl: string): void => {
  if (destinationUrl.includes("\n") || destinationUrl.includes("\r")) {
    throw repositoryError(
      "ForgeGitRepository.assertMirrorDestination",
      "forge_git_mirror_destination_invalid",
      400,
    );
  }
  if (/^https?:\/\//iu.test(destinationUrl)) {
    const parsed = new URL(destinationUrl);
    if (parsed.username !== "" || parsed.password !== "") {
      throw repositoryError(
        "ForgeGitRepository.assertMirrorDestination",
        "forge_git_mirror_destination_contains_credentials",
        400,
      );
    }
  }
};

const collect = async (
  stream: NodeJS.ReadableStream,
  maximumBytes: number,
): Promise<Uint8Array> => {
  const chunks: Array<Uint8Array> = [];
  let total = 0;
  for await (const chunk of stream) {
    const bytes =
      typeof chunk === "string" ? textEncoder.encode(chunk) : new Uint8Array(chunk as Buffer);
    total += bytes.byteLength;
    if (total > maximumBytes) {
      throw new Error("git subprocess output exceeded its bound");
    }
    chunks.push(bytes);
  }
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
};

const pump = async (
  body: ReadableStream<Uint8Array> | null,
  stdin: NodeJS.WritableStream,
  maximumBytes: number,
): Promise<void> => {
  if (body === null) {
    stdin.end();
    return;
  }
  const reader = body.getReader();
  let total = 0;
  try {
    while (true) {
      const item = await reader.read();
      if (item.done) break;
      total += item.value.byteLength;
      if (total > maximumBytes) {
        throw repositoryError("ForgeGitRepository.pump", "forge_git_receive_pack_too_large", 413);
      }
      if (!stdin.write(item.value)) {
        await new Promise<void>((resolve, reject) => {
          stdin.once("drain", resolve);
          stdin.once("error", reject);
        });
      }
    }
    stdin.end();
  } finally {
    reader.releaseLock();
  }
};

const runGitBuffered = async (input: {
  readonly args: ReadonlyArray<string>;
  readonly body?: ReadableStream<Uint8Array> | null;
  readonly env?: NodeJS.ProcessEnv;
  readonly gitBinary: string;
  readonly maxInputBytes?: number;
  readonly maxOutputBytes?: number;
}): Promise<GitCommandResult> => {
  const child = spawn(input.gitBinary, input.args, {
    env: input.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const stdout = collect(child.stdout, input.maxOutputBytes ?? maxGitResultBytes);
  const stderr = collect(child.stderr, maxGitDiagnosticBytes);
  const exit = new Promise<number | null>((resolve, reject) => {
    child.once("error", reject);
    child.once("close", resolve);
  });
  try {
    await pump(input.body ?? null, child.stdin, input.maxInputBytes ?? maxGitResultBytes);
  } catch (error) {
    child.kill("SIGKILL");
    throw error;
  }
  const [exitCode, stdoutBytes, stderrBytes] = await Promise.all([exit, stdout, stderr]);
  const diagnostic = textDecoder.decode(stderrBytes);
  if (exitCode !== 0) {
    throw repositoryError(
      "ForgeGitRepository.runGitBuffered",
      "forge_git_process_failed",
      400,
      diagnostic,
    );
  }
  return { stderr: diagnostic, stdout: stdoutBytes };
};

const runGitStreaming = (input: {
  readonly args: ReadonlyArray<string>;
  readonly body: ReadableStream<Uint8Array> | null;
  readonly env: NodeJS.ProcessEnv;
  readonly gitBinary: string;
  readonly maxInputBytes: number;
}): ReadableStream<Uint8Array> => {
  const child = spawn(input.gitBinary, input.args, {
    env: input.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const diagnostics: Array<Uint8Array> = [];
  let diagnosticBytes = 0;
  child.stderr.on("data", (chunk: Buffer) => {
    if (diagnosticBytes >= maxGitDiagnosticBytes) return;
    const bytes = new Uint8Array(chunk);
    diagnostics.push(bytes);
    diagnosticBytes += bytes.byteLength;
  });

  return new ReadableStream<Uint8Array>({
    start(controller) {
      child.stdout.on("data", (chunk: Buffer) => {
        controller.enqueue(new Uint8Array(chunk));
        if (controller.desiredSize !== null && controller.desiredSize <= 0) {
          child.stdout.pause();
        }
      });
      child.stdout.on("end", () => controller.close());
      child.stdout.on("error", (error) => controller.error(error));
      child.once("error", (error) => controller.error(error));
      child.once("close", (exitCode) => {
        if (exitCode === 0) return;
        const diagnostic = textDecoder.decode(
          diagnostics.length === 0
            ? new Uint8Array()
            : Buffer.concat(diagnostics.map((bytes) => Buffer.from(bytes))),
        );
        controller.error(
          repositoryError(
            "ForgeGitRepository.uploadPack",
            "forge_git_process_failed",
            400,
            diagnostic,
          ),
        );
      });
      void pump(input.body, child.stdin, input.maxInputBytes).catch((error) => {
        child.kill("SIGKILL");
        controller.error(error);
      });
    },
    pull() {
      child.stdout.resume();
    },
    cancel() {
      child.kill("SIGKILL");
    },
  });
};

const serviceAdvertisement = (
  operation: "git-upload-pack" | "git-receive-pack",
  advertisement: Uint8Array,
): Uint8Array => {
  const payload = textEncoder.encode(`# service=${operation}\n`);
  const header = textEncoder.encode((payload.byteLength + 4).toString(16).padStart(4, "0"));
  const result = new Uint8Array(
    header.byteLength + payload.byteLength + 4 + advertisement.byteLength,
  );
  result.set(header, 0);
  result.set(payload, header.byteLength);
  result.set(textEncoder.encode("0000"), header.byteLength + payload.byteLength);
  result.set(advertisement, header.byteLength + payload.byteLength + 4);
  return result;
};

const hookSource = `#!/usr/bin/env node
const restrictions = JSON.parse(process.env.OPENAGENTS_FORGE_ALLOWED_REFS_JSON ?? "[]")
const allowed = new Set(restrictions)
const policies = JSON.parse(process.env.OPENAGENTS_FORGE_SIGNED_REF_POLICIES_JSON ?? "[]")
const policyByRef = new Map(policies.map(policy => [policy.refName, policy]))
const zero = /^[0]+$/
let input = ""
process.stdin.setEncoding("utf8")
process.stdin.on("data", chunk => { input += chunk })
process.stdin.on("end", () => {
  for (const line of input.trim().split("\\n")) {
    if (line === "") continue
    const fields = line.trim().split(/\\s+/)
    const [oldObjectId, newObjectId, refName] = fields
    if (oldObjectId === undefined || newObjectId === undefined || refName === undefined) {
      process.stderr.write("forge_git_receive_update_malformed\\n")
      process.exit(1)
    }
    if (allowed.size > 0 && !allowed.has(refName)) {
      process.stderr.write("Forge token does not permit this ref.\\n")
      process.exit(1)
    }
    if (refName.startsWith("refs/heads/")) {
      const policy = policyByRef.get(refName)
      if (policy === undefined || policy.oldObjectId !== oldObjectId || policy.newObjectId !== newObjectId) {
        process.stderr.write("forge_git_signed_state_required\\n")
        process.exit(1)
      }
      continue
    }
    // NIP-34 pointer refs are deliberately object-staging-only. They are
    // never projected until the matching event resolves out of purgatory.
    if (/^refs\\/nostr\\/[0-9a-f]{64}$/u.test(refName) && !zero.test(newObjectId)) continue
    process.stderr.write("forge_git_ref_not_admitted\\n")
    process.exit(1)
  }
})
`;

const pathExists = async (path: string): Promise<boolean> => {
  try {
    await stat(path);
    return true;
  } catch (error) {
    return (
      typeof error === "object" && error !== null && "code" in error && error.code !== "ENOENT"
    );
  }
};

const assertNotSymlink = async (path: string): Promise<void> => {
  try {
    const details = await lstat(path);
    if (details.isSymbolicLink()) {
      throw repositoryError(
        "ForgeGitRepository.assertNotSymlink",
        "forge_git_repository_path_unsafe",
        500,
      );
    }
  } catch (error) {
    if (typeof error === "object" && error !== null && "code" in error && error.code === "ENOENT") {
      return;
    }
    throw error;
  }
};

const hashBytes = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const makeRepositoryService = (
  configuration: ForgeGitConfigurationShape,
  blobStore: BlobStoreShape,
): ForgeGitRepositoryShape => {
  const repositoryPath = (tenantRef: string, repositoryRef: string): string =>
    join(configuration.repositoryRoot, tenantRef, `${repositoryRef}.git`);

  const git = (
    args: ReadonlyArray<string>,
    options: Omit<Parameters<typeof runGitBuffered>[0], "args" | "gitBinary"> = {},
  ) =>
    runGitBuffered({
      ...options,
      args,
      gitBinary: configuration.gitBinary,
    });

  const configureRepository = async (path: string): Promise<void> => {
    await git(["--git-dir", path, "config", "uploadpack.allowFilter", "true"]);
    await git(["--git-dir", path, "config", "uploadpack.allowReachableSHA1InWant", "true"]);
    await git(["--git-dir", path, "config", "uploadpack.allowTipSHA1InWant", "true"]);
    await git(["--git-dir", path, "config", "receive.advertisePushOptions", "true"]);
    const hookPath = join(path, "hooks", "pre-receive");
    await writeFile(hookPath, hookSource, { encoding: "utf8", mode: 0o755 });
    await chmod(hookPath, 0o755);
  };

  const ensureRepository = async (tenantRef: string, repositoryRef: string): Promise<string> => {
    const tenantPath = join(configuration.repositoryRoot, tenantRef);
    const path = repositoryPath(tenantRef, repositoryRef);
    await mkdir(configuration.repositoryRoot, { recursive: true });
    await assertNotSymlink(configuration.repositoryRoot);
    await mkdir(tenantPath, { recursive: true });
    await assertNotSymlink(tenantPath);
    await assertNotSymlink(path);
    if (!(await pathExists(join(path, "HEAD")))) {
      await git(["init", "--bare", "--initial-branch=main", path]);
    }
    await configureRepository(path);
    return path;
  };

  const requireRepository = async (tenantRef: string, repositoryRef: string): Promise<string> => {
    const path = repositoryPath(tenantRef, repositoryRef);
    await assertNotSymlink(join(configuration.repositoryRoot, tenantRef));
    await assertNotSymlink(path);
    if (!(await pathExists(join(path, "HEAD")))) {
      throw repositoryError(
        "ForgeGitRepository.requireRepository",
        "forge_git_repository_not_found",
        404,
      );
    }
    return path;
  };

  const listRefsAtPath = async (path: string): Promise<ReadonlyArray<ForgeGitRef>> => {
    const result = await git([
      "--git-dir",
      path,
      "for-each-ref",
      "--format=%(objectname)%00%(refname)",
    ]);
    const output = textDecoder.decode(result.stdout).trim();
    if (output === "") return [];
    return output.split("\n").map((line) => {
      const [objectId, refName] = line.split("\0", 2);
      if (objectId === undefined || refName === undefined) {
        throw repositoryError(
          "ForgeGitRepository.listRefs",
          "forge_git_ref_projection_failed",
          500,
        );
      }
      return ForgeGitRef.make({ objectId, refName });
    });
  };

  const mirrorPacks = Effect.fn("ForgeGitRepository.mirrorPacks")(function* (
    tenantRef: string,
    repositoryRef: string,
    path: string,
  ) {
    yield* Effect.tryPromise({
      try: () => git(["--git-dir", path, "repack", "-Ad"]),
      catch: (cause) =>
        repositoryError(
          "ForgeGitRepository.mirrorPacks.repack",
          "forge_git_mirror_failed",
          500,
          cause,
        ),
    });
    const packDirectory = join(path, "objects", "pack");
    const names = (yield* Effect.tryPromise({
      try: () => readdir(packDirectory),
      catch: (cause) =>
        repositoryError(
          "ForgeGitRepository.mirrorPacks.list",
          "forge_git_mirror_failed",
          500,
          cause,
        ),
    }))
      .filter((name) => name.endsWith(".pack") || name.endsWith(".idx"))
      .sort();
    const evidence: Array<ForgeGitPackEvidence> = [];
    for (const name of names) {
      const bytes = new Uint8Array(
        yield* Effect.tryPromise({
          try: () => readFile(join(packDirectory, name)),
          catch: (cause) =>
            repositoryError(
              "ForgeGitRepository.mirrorPacks.read",
              "forge_git_mirror_failed",
              500,
              cause,
            ),
        }),
      );
      const sha256 = hashBytes(bytes);
      const extension = name.endsWith(".pack") ? "pack" : "idx";
      const objectKey = `private/forge/git-mirror/${tenantRef}/${repositoryRef}/packs/${sha256}.${extension}`;
      yield* blobStore.put(objectKey, bytes, {
        contentType: "application/octet-stream",
      });
      evidence.push(
        ForgeGitPackEvidence.make({
          bytes: bytes.byteLength,
          objectKey,
          sha256,
        }),
      );
    }
    const createdAt = new Date().toISOString();
    const manifestKey = `private/forge/git-mirror/${tenantRef}/${repositoryRef}/manifests/${createdAt.replaceAll(":", "")}-${randomUUID()}.json`;
    const receipt = ForgeGitMirrorReceipt.make({
      createdAt,
      evidence,
      manifestKey,
      repositoryRef,
      tenantRef,
    });
    yield* blobStore.put(manifestKey, textEncoder.encode(JSON.stringify(receipt, null, 2)), {
      contentType: "application/json",
    });
    return receipt;
  });

  const advertise = Effect.fn("ForgeGitRepository.advertise")(function* (
    input: Parameters<ForgeGitRepositoryShape["advertise"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.advertise.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    const result = yield* Effect.tryPromise({
      try: () =>
        git(
          [
            input.operation === "git-upload-pack" ? "upload-pack" : "receive-pack",
            "--stateless-rpc",
            "--advertise-refs",
            path,
          ],
          { env: gitEnvironment(input.gitProtocol) },
        ),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.advertise",
              "forge_git_advertisement_failed",
              500,
              cause,
            ),
    });
    return serviceAdvertisement(input.operation, result.stdout);
  });

  const provision = Effect.fn("ForgeGitRepository.provision")(function* (
    input: Parameters<ForgeGitRepositoryShape["provision"]>[0],
  ) {
    yield* Effect.tryPromise({
      try: () => ensureRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.provision",
              "forge_git_repository_provision_failed",
              500,
              cause,
            ),
    });
  });

  const listRefs = Effect.fn("ForgeGitRepository.listRefs")(function* (
    input: Parameters<ForgeGitRepositoryShape["listRefs"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.listRefs.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    return yield* Effect.tryPromise({
      try: () => listRefsAtPath(path),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.listRefs",
              "forge_git_ref_projection_failed",
              500,
              cause,
            ),
    });
  });

  const observeMirror = Effect.fn("ForgeGitRepository.observeMirror")(function* (
    input: Parameters<ForgeGitRepositoryShape["observeMirror"]>[0],
  ) {
    const observation = yield* Effect.tryPromise({
      try: async () => {
        assertMirrorRef(input.sourceRef, "ForgeGitRepository.observeMirror.sourceRef");
        assertMirrorRef(input.destinationRef, "ForgeGitRepository.observeMirror.destinationRef");
        assertMirrorDestination(input.destinationUrl);
        const path = await requireRepository(input.tenantRef, input.repositoryRef);
        const source = await git(["--git-dir", path, "rev-parse", "--verify", input.sourceRef]);
        const sourceObjectId = textDecoder.decode(source.stdout).trim().toLowerCase();
        if (sourceObjectId !== input.expectedSourceObjectId.toLowerCase()) {
          throw repositoryError(
            "ForgeGitRepository.observeMirror",
            "forge_git_mirror_source_object_mismatch",
            409,
          );
        }

        const destination = await git(
          ["ls-remote", "--refs", input.destinationUrl, input.destinationRef],
          { env: mirrorEnvironment(input.authorizationHeader) },
        );
        const destinationLine = textDecoder.decode(destination.stdout).trim();
        const destinationObjectId =
          destinationLine === "" ? null : (destinationLine.split(/\s+/u)[0]?.toLowerCase() ?? null);
        const observedAt = new Date().toISOString();
        if (destinationObjectId === null) {
          return ForgeGitMirrorObservation.make({
            destinationObjectId: null,
            divergence: "destination_missing",
            observedAt,
            sourceObjectId,
          });
        }
        if (destinationObjectId === sourceObjectId) {
          return ForgeGitMirrorObservation.make({
            destinationObjectId,
            divergence: "in_sync",
            observedAt,
            sourceObjectId,
          });
        }

        const comparison = await mkdtemp(join(tmpdir(), "oa-forge-mirror-observation-"));
        try {
          await git(["init", "--bare", comparison]);
          await git([
            "--git-dir",
            comparison,
            "fetch",
            "--no-tags",
            path,
            `${input.sourceRef}:refs/openagents/source`,
          ]);
          await git(
            [
              "--git-dir",
              comparison,
              "fetch",
              "--no-tags",
              input.destinationUrl,
              `${input.destinationRef}:refs/openagents/destination`,
            ],
            { env: mirrorEnvironment(input.authorizationHeader) },
          );
          const isAncestor = async (ancestor: string, descendant: string): Promise<boolean> => {
            try {
              await git([
                "--git-dir",
                comparison,
                "merge-base",
                "--is-ancestor",
                ancestor,
                descendant,
              ]);
              return true;
            } catch {
              return false;
            }
          };
          const destinationIsAncestor = await isAncestor(
            "refs/openagents/destination",
            "refs/openagents/source",
          );
          const sourceIsAncestor = await isAncestor(
            "refs/openagents/source",
            "refs/openagents/destination",
          );
          return ForgeGitMirrorObservation.make({
            destinationObjectId,
            divergence: destinationIsAncestor
              ? "source_ahead"
              : sourceIsAncestor
                ? "destination_ahead"
                : "diverged",
            observedAt,
            sourceObjectId,
          });
        } finally {
          await rm(comparison, { force: true, recursive: true });
        }
      },
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.observeMirror",
              "forge_git_mirror_observation_failed",
              502,
              cause,
            ),
    });
    return observation;
  });

  const projectMirror = Effect.fn("ForgeGitRepository.projectMirror")(function* (
    input: Parameters<ForgeGitRepositoryShape["projectMirror"]>[0],
  ) {
    const before = yield* observeMirror(input);
    if (before.divergence === "in_sync") return before;
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.projectMirror.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    yield* Effect.tryPromise({
      try: () =>
        git(
          [
            "--git-dir",
            path,
            "push",
            "--porcelain",
            input.destinationUrl,
            `${input.sourceRef}:${input.destinationRef}`,
          ],
          { env: mirrorEnvironment(input.authorizationHeader) },
        ),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? repositoryError(
              "ForgeGitRepository.projectMirror",
              "forge_git_mirror_projection_rejected",
              409,
              cause,
            )
          : repositoryError(
              "ForgeGitRepository.projectMirror",
              "forge_git_mirror_projection_failed",
              502,
              cause,
            ),
    });
    const after = yield* observeMirror(input);
    if (
      after.divergence !== "in_sync" ||
      after.destinationObjectId?.toLowerCase() !== input.expectedSourceObjectId.toLowerCase()
    ) {
      return yield* repositoryError(
        "ForgeGitRepository.projectMirror",
        "forge_git_mirror_post_projection_mismatch",
        502,
      );
    }
    return after;
  });

  const receivePack = Effect.fn("ForgeGitRepository.receivePack")(function* (
    input: Parameters<ForgeGitRepositoryShape["receivePack"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.receivePack.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    const refsBefore = yield* Effect.tryPromise({
      try: () => listRefsAtPath(path),
      catch: (cause) =>
        repositoryError(
          "ForgeGitRepository.receivePack.refsBefore",
          "forge_git_ref_projection_failed",
          500,
          cause,
        ),
    });
    const result = yield* Effect.tryPromise({
      try: () =>
        git(["receive-pack", "--stateless-rpc", path], {
          body: input.body,
          env: gitEnvironment(
            input.gitProtocol,
            input.session.refRestrictions,
            input.signedRefPolicies,
          ),
          maxInputBytes: configuration.maxReceivePackBytes,
        }),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.receivePack",
              "forge_git_receive_pack_failed",
              400,
              cause,
            ),
    });
    const refsAfter = yield* Effect.tryPromise({
      try: () => listRefsAtPath(path),
      catch: (cause) =>
        repositoryError(
          "ForgeGitRepository.receivePack.refsAfter",
          "forge_git_ref_projection_failed",
          500,
          cause,
        ),
    });
    const changed = JSON.stringify(refsBefore) !== JSON.stringify(refsAfter);
    const mirrorReceipt =
      configuration.mirrorEnabled && changed
        ? yield* Effect.gen(function* () {
            return Option.some(yield* mirrorPacks(input.tenantRef, input.repositoryRef, path));
          }).pipe(
            Effect.catch((error) =>
              Effect.logWarning("Forge Git evidence mirror failed.", {
                code: error instanceof ForgeGitRepositoryError ? error.code : error._tag,
                repositoryRef: input.repositoryRef,
                tenantRef: input.tenantRef,
              }).pipe(Effect.as(Option.none<ForgeGitMirrorReceipt>())),
            ),
          )
        : Option.none<ForgeGitMirrorReceipt>();

    return {
      body: result.stdout,
      changed,
      contentType: "application/x-git-receive-pack-result",
      mirrorReceipt,
      refsAfter,
      refsBefore,
    };
  });

  const uploadPack = Effect.fn("ForgeGitRepository.uploadPack")(function* (
    input: Parameters<ForgeGitRepositoryShape["uploadPack"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.uploadPack.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    return runGitStreaming({
      args: ["upload-pack", "--stateless-rpc", path],
      body: input.body,
      env: gitEnvironment(input.gitProtocol),
      gitBinary: configuration.gitBinary,
      maxInputBytes: 8 * 1024 * 1024,
    });
  });

  const verify = Effect.fn("ForgeGitRepository.verify")(function* (
    input: Parameters<ForgeGitRepositoryShape["verify"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.verify.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    yield* Effect.tryPromise({
      try: () => git(["--git-dir", path, "fsck", "--full", "--strict"]),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.verify",
              "forge_git_repository_verification_failed",
              500,
              cause,
            ),
    });
  });

  const backup = Effect.fn("ForgeGitRepository.backup")(function* (
    input: Parameters<ForgeGitRepositoryShape["backup"]>[0],
  ) {
    const path = yield* Effect.tryPromise({
      try: () => requireRepository(input.tenantRef, input.repositoryRef),
      catch: (cause) =>
        cause instanceof ForgeGitRepositoryError
          ? cause
          : repositoryError(
              "ForgeGitRepository.backup.repository",
              "forge_git_repository_unavailable",
              500,
              cause,
            ),
    });
    const refs = yield* Effect.tryPromise({
      try: () => listRefsAtPath(path),
      catch: (cause) =>
        repositoryError("ForgeGitRepository.backup.refs", "forge_git_backup_failed", 500, cause),
    });
    const temporaryBundle = join(dirname(path), `.${input.repositoryRef}.${randomUUID()}.bundle`);
    yield* Effect.tryPromise({
      try: () => git(["--git-dir", path, "bundle", "create", temporaryBundle, "--all"]),
      catch: (cause) =>
        repositoryError("ForgeGitRepository.backup.bundle", "forge_git_backup_failed", 500, cause),
    });
    const bundle = yield* Effect.acquireUseRelease(
      Effect.tryPromise({
        try: async () => new Uint8Array(await readFile(temporaryBundle)),
        catch: (cause) =>
          repositoryError("ForgeGitRepository.backup.read", "forge_git_backup_failed", 500, cause),
      }),
      (bytes) => Effect.succeed(bytes),
      () => Effect.promise(() => rm(temporaryBundle, { force: true })),
    );
    const createdAt = new Date().toISOString();
    const bundleSha256 = hashBytes(bundle);
    const bundleKey = `private/forge/git-backups/${input.tenantRef}/${input.repositoryRef}/${createdAt.replaceAll(":", "")}-${bundleSha256}.bundle`;
    const receiptKey = `${bundleKey}.receipt.json`;
    const head = yield* Effect.tryPromise({
      try: () =>
        git(["--git-dir", path, "rev-parse", "--verify", "HEAD"]).catch(() => ({
          stderr: "",
          stdout: new Uint8Array(),
        })),
      catch: (cause) =>
        repositoryError("ForgeGitRepository.backup.head", "forge_git_backup_failed", 500, cause),
    });
    const headText = textDecoder.decode(head.stdout).trim();
    const receipt = ForgeGitBackupReceipt.make({
      bundleBytes: bundle.byteLength,
      bundleKey,
      bundleSha256,
      createdAt,
      headObjectId: headText === "" ? null : headText,
      receiptKey,
      refs,
      repositoryRef: input.repositoryRef,
      tenantRef: input.tenantRef,
    });
    yield* blobStore
      .put(bundleKey, bundle, {
        contentType: "application/x-git-bundle",
      })
      .pipe(
        Effect.mapError((cause) =>
          repositoryError(
            "ForgeGitRepository.backup.bundleStore",
            "forge_git_backup_failed",
            500,
            cause,
          ),
        ),
      );
    yield* blobStore
      .put(receiptKey, textEncoder.encode(JSON.stringify(receipt, null, 2)), {
        contentType: "application/json",
      })
      .pipe(
        Effect.mapError((cause) =>
          repositoryError(
            "ForgeGitRepository.backup.receiptStore",
            "forge_git_backup_failed",
            500,
            cause,
          ),
        ),
      );
    return receipt;
  });

  const restore = Effect.fn("ForgeGitRepository.restore")(function* (
    input: Parameters<ForgeGitRepositoryShape["restore"]>[0],
  ) {
    const receipt = input.receipt;
    const bundle = yield* blobStore
      .get(receipt.bundleKey)
      .pipe(
        Effect.mapError((cause) =>
          repositoryError(
            "ForgeGitRepository.restore.bundleRead",
            "forge_git_restore_failed",
            500,
            cause,
          ),
        ),
      );
    if (bundle === null || hashBytes(bundle) !== receipt.bundleSha256) {
      return yield* repositoryError(
        "ForgeGitRepository.restore.read",
        "forge_git_restore_bundle_invalid",
        409,
      );
    }
    const target = repositoryPath(receipt.tenantRef, receipt.repositoryRef);
    if (yield* Effect.promise(() => pathExists(join(target, "HEAD")))) {
      return yield* repositoryError(
        "ForgeGitRepository.restore.target",
        "forge_git_restore_target_exists",
        409,
      );
    }
    const staging = `${target}.restore-${randomUUID()}`;
    const temporaryBundle = `${staging}.bundle`;
    yield* Effect.tryPromise({
      try: async () => {
        await mkdir(dirname(target), { recursive: true });
        await writeFile(temporaryBundle, bundle);
        await git(["clone", "--bare", temporaryBundle, staging]);
        await configureRepository(staging);
        await git(["--git-dir", staging, "fsck", "--full", "--strict"]);
        const restoredRefs = await listRefsAtPath(staging);
        if (JSON.stringify(restoredRefs) !== JSON.stringify(receipt.refs)) {
          throw new Error("restored refs do not match the backup receipt");
        }
        await rename(staging, target);
        await rm(temporaryBundle, { force: true });
      },
      catch: (cause) =>
        repositoryError("ForgeGitRepository.restore", "forge_git_restore_failed", 500, cause),
    }).pipe(
      Effect.ensuring(
        Effect.promise(async () => {
          await rm(staging, { force: true, recursive: true });
          await rm(temporaryBundle, { force: true });
        }),
      ),
    );
  });

  return ForgeGitRepository.of({
    advertise,
    provision,
    backup,
    listRefs,
    observeMirror,
    projectMirror,
    receivePack,
    restore,
    uploadPack,
    verify,
  });
};

export const layerRepository = Layer.effect(
  ForgeGitRepository,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    const blobStore = yield* BlobStore;
    return makeRepositoryService(configuration, blobStore);
  }),
);

export const makeRepositoryLayer = (
  configuration: ForgeGitConfigurationShape,
  blobStore: BlobStoreShape,
): Layer.Layer<ForgeGitRepository> =>
  Layer.succeed(ForgeGitRepository, makeRepositoryService(configuration, blobStore));
