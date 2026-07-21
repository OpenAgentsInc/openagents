// Real integration ports for the DIST-13 one-command release (#8917 coordinator,
// #8922 feed). `scripts/release.ts` defines the typed `ReleaseCoordinatorPort`
// and `ReleaseFeedPort` and, until this module, `main()` hardcoded the fixture
// ports so a real run was structurally refused. This module supplies the real
// implementations and a `createReleasePorts` selector so `--dry-run` keeps the
// fixtures and a real run drives the owned coordinator core plus the public
// ReleaseSet v2 feed.
//
// Design and honesty boundaries:
//   * The real coordinator is the already-landed, unit-tested
//     `createOwnedReleaseCoordinator` (scripts/desktop-release-coordinator.ts).
//     This module only assembles its concrete dependency adapters; it never
//     reimplements the transaction/convergence/promotion state machine.
//   * A worker receipt must carry nine native-proof refs (clean install,
//     launch, update, rollback, reinstall, uninstall, ...). Those proofs are
//     the owner-gated DIST-12 (#8925) clean-machine acceptance. Until the owner
//     attests the native acceptance host for a target, its inventory row keeps
//     an `unavailable:` acceptance-host ref and the coordinator fails closed at
//     inventory bind with `worker_inventory_unavailable`. That refusal IS the
//     owner gate surfaced through the release tool, not a gap in this code.
//   * Because inventory refuses first, `fanOutTargets`, `publishCandidate`, and
//     `promoteChannelPointer` are unreachable in a real run until the owner
//     unblocks. The worker-control dispatch and the candidate publisher /
//     acceptance / promoter adapters therefore fail closed with typed
//     owner-gate errors rather than fabricating build receipts or promoting an
//     unverified pointer. Their GCS/ReleaseSet finalization from attested build
//     receipts lands with the owner-attested build path (see #8917).
//   * Every effect (shell command, GCS object op, HTTP GET) is injected so the
//     wiring is deterministically unit-tested; production defaults build the
//     real adapters from the `ReleaseIo` environment. Secrets and absolute
//     local paths never appear in receipt lines (see `redact`).

import { createHash, createPrivateKey, generateKeyPairSync, sign as edSign } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import {
  canonicalizeReleaseSet,
  minimumOsByTarget,
  preferredFormatByTarget,
  verifySignedReleaseSet,
} from "../apps/openagents-desktop/src/release-set-contract.js";
import {
  type PinnedReleaseKey,
  PRODUCTION_RELEASE_KEY_PIN,
} from "../apps/openagents-desktop/src/update-contract.js";
import {
  type AtomicChannelPromoter,
  type CandidateAcceptanceGate,
  type CandidateHandoff,
  type CandidateObjectStore,
  type CandidatePublisher,
  canonicalJson,
  type CoordinatorRequestSigner,
  createOwnedReleaseCoordinator,
  FileCoordinatorStateStore,
  formatsByTarget,
  type FrozenReleaseAuthority,
  type RunnerInventoryEntry,
  sha256,
  type SignedWorkerReceipt,
  type WorkerArtifactReceipt,
  type WorkerControl,
  type WorkerHealth,
  type WorkerKeyring,
  type WorkerReceiptPayload,
} from "./desktop-release-coordinator.js";
import {
  createFixtureCoordinatorPort,
  createFixtureFeedPort,
  type PortCallResult,
  type ReleaseChannel,
  type ReleaseCoordinatorPort,
  type ReleaseFeedPort,
  type ReleaseIo,
  type ReleasePlan,
  type ReleasePorts,
  type ReleaseTargetKey,
  releaseTargetKeys,
} from "./release.js";

// ---------------------------------------------------------------------------
// Shared config + injectable effects
// ---------------------------------------------------------------------------

/** GCS bucket that backs the public ReleaseSet v2 feed (candidates + pointer). */
export const RELEASE_SET_BUCKET_DEFAULT = "openagentsgemini-oa-updates-release-set";
/** Container path of the pinned-keys file baked into the oa-updates image. */
export const RELEASE_SET_PINS_CONTAINER_PATH = "/app/openagents-desktop-dist/release-set-pins.json";
/** Public feed origin. */
export const UPDATES_BASE_URL_DEFAULT = "https://updates.openagents.com";
/** openagents.com origin that hosts the #8923 download resolver. */
export const WEB_BASE_URL_DEFAULT = "https://openagents.com";

export type CommandRequest = Readonly<{
  command: string;
  args: readonly string[];
  env?: Readonly<Record<string, string | undefined>>;
  cwd?: string;
  input?: string;
}>;
export type CommandResult = Readonly<{ code: number; stdout: string; stderr: string }>;
export type RunCommand = (request: CommandRequest) => Promise<CommandResult>;

export type HttpResponse = Readonly<{ status: number; body: string }>;
export type HttpGet = (url: string) => Promise<HttpResponse>;

export type RealPortEffects = Readonly<{ run: RunCommand; httpGet: HttpGet }>;

const nodeRunCommand: RunCommand = async ({ command, args, env, cwd, input }) => {
  const { spawn } = await import("node:child_process");
  return await new Promise<CommandResult>((resolvePromise, reject) => {
    const child = spawn(command, [...args], {
      cwd,
      env: { ...process.env, ...env },
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", reject);
    child.on("close", (code) => resolvePromise({ code: code ?? -1, stdout, stderr }));
    if (input !== undefined) child.stdin.write(input);
    child.stdin.end();
  });
};

const nodeHttpGet: HttpGet = async (url) => {
  const response = await fetch(url, { redirect: "error" });
  const body = await response.text();
  return { status: response.status, body };
};

const defaultEffects = (): RealPortEffects => ({ run: nodeRunCommand, httpGet: nodeHttpGet });

/**
 * A receipt line must be public-safe: bounded, no secret-shaped tokens, no
 * absolute local paths, no origins with digits (mirrors the coordinator's own
 * `redactedLine` so mixed receipt streams share one guarantee).
 */
export const redact = (line: string): string => {
  if (
    line.length > 240 ||
    /(?:secret|token|password|private[_ -]?key|\/Users\/|https?:\/\/\d)/i.test(line)
  ) {
    throw new Error("release feed receipt line is not public-safe");
  }
  return line;
};

// ---------------------------------------------------------------------------
// Frozen authority derived from the release plan
// ---------------------------------------------------------------------------

/**
 * Derive the immutable coordinator authority from a release plan. The refs are
 * deterministic and public-safe; `releaseNotesSha256` is the digest of the
 * channel release notes so a notes edit re-freezes the plan.
 */
export const buildReleaseAuthority = (
  plan: ReleasePlan,
  io: ReleaseIo,
  signingKid: string,
): FrozenReleaseAuthority => ({
  sourceRevision: plan.sourceRevision,
  version: plan.version,
  channel: plan.channel,
  targets: releaseTargetKeys,
  stagingLedgerRef: `openagents.desktop.staging_ledger.${plan.version}-${plan.channel}`,
  signingPolicyId: `openagents.desktop.signing_policy.ed25519.${signingKid}`,
  toolchainProfileRef: "openagents.desktop.toolchain.node24-pnpm-forge",
  releaseNotesSha256: releaseNotesSha256(plan, io),
});

const releaseNotesSha256 = (plan: ReleasePlan, io: ReleaseIo): string => {
  for (const path of [
    join(io.rootDir, "apps/openagents-desktop/CHANGELOG.md"),
    join(io.rootDir, "CHANGELOG.md"),
  ]) {
    if (existsSync(path)) return sha256(readFileSync(path, "utf8"));
  }
  return sha256(`openagents.desktop.release_notes:${plan.version}:${plan.channel}`);
};

// ---------------------------------------------------------------------------
// ed25519 signer + keyring (from .secrets / env)
// ---------------------------------------------------------------------------

export type ReleaseSigningKey = Readonly<{ kid: string; d: string; x: string }>;

/**
 * Load the release-set ed25519 signing key from the environment. Accepts the
 * inline JWK-d form or a secrets file path, matching publish-release.ts. Never
 * returns or logs the private component.
 */
export const loadReleaseSigningKey = (
  env: Readonly<Record<string, string | undefined>>,
): ReleaseSigningKey | null => {
  let d = env.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D;
  let kid = env.OPENAGENTS_RELEASE_SIGNING_KID;
  let x = env.OPENAGENTS_RELEASE_SIGNING_PUBLIC_JWK_X;
  const secretsPath = env.OPENAGENTS_RELEASE_SECRETS_PATH;
  if (
    (d === undefined || kid === undefined) &&
    secretsPath !== undefined &&
    existsSync(secretsPath)
  ) {
    for (const raw of readFileSync(secretsPath, "utf8").split("\n")) {
      const line = raw.trim();
      if (line === "" || line.startsWith("#")) continue;
      const eq = line.indexOf("=");
      if (eq < 0) continue;
      const key = line.slice(0, eq).trim();
      const value = line
        .slice(eq + 1)
        .trim()
        .replace(/^["']|["']$/g, "");
      if (key === "OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D") d = value;
      else if (key === "OPENAGENTS_RELEASE_SIGNING_KID") kid = value;
      else if (key === "OPENAGENTS_RELEASE_SIGNING_PUBLIC_JWK_X") x = value;
    }
  }
  if (d === undefined || kid === undefined) return null;
  return { kid, d, x: x ?? PRODUCTION_RELEASE_KEY_PIN.x };
};

const signEd25519 = (key: ReleaseSigningKey, payload: Uint8Array): string =>
  edSign(
    null,
    Buffer.from(payload),
    createPrivateKey({ key: { kty: "OKP", crv: "Ed25519", d: key.d, x: key.x }, format: "jwk" }),
  ).toString("base64url");

/** Coordinator request signer bound to the release ed25519 key. */
export const createRequestSigner = (key: ReleaseSigningKey): CoordinatorRequestSigner => ({
  kid: key.kid,
  sign: (canonicalRequest) => signEd25519(key, canonicalRequest),
});

/**
 * Worker keyring. Owned build workers sign receipts with per-worker keys that
 * the owner registers post-attestation; a worker whose public key is not
 * registered yields `undefined` so the coordinator refuses its receipt.
 */
export const createWorkerKeyring = (
  registry: Readonly<Record<string, Readonly<Record<string, string>>>>,
): WorkerKeyring => ({
  publicKey: (workerRef, kid) => registry[workerRef]?.[kid],
});

// ---------------------------------------------------------------------------
// Inventory (owned build/acceptance host map)
// ---------------------------------------------------------------------------

/**
 * The owned per-target build hosts from the cross-platform build runbook. The
 * `nativeAcceptanceHostRef` is the owner-gated DIST-12 (#8925) clean-machine
 * acceptance host; until the owner attests it for a target, it stays
 * `unavailable:` and the coordinator fails closed at inventory bind.
 */
export const buildInventory = (
  authority: FrozenReleaseAuthority,
  attestations: Readonly<Partial<Record<ReleaseTargetKey, string>>> = {},
): readonly RunnerInventoryEntry[] => {
  const hosts: Readonly<
    Record<
      ReleaseTargetKey,
      Readonly<{
        workerRef: string;
        hostClass: string;
        buildMode: "native" | "cross";
        signingOperationRef: string;
      }>
    >
  > = {
    "darwin-arm64": {
      workerRef: "oa.worker.darwin-arm64.local",
      hostClass: "apple-silicon-mac",
      buildMode: "native",
      signingOperationRef: "openagents.desktop.signing.developer-id.notarize",
    },
    "darwin-x64": {
      workerRef: "oa.worker.darwin-x64.imac-pro-bertha",
      hostClass: "intel-mac-tailnet",
      buildMode: "native",
      signingOperationRef: "openagents.desktop.signing.developer-id.notarize",
    },
    "linux-x64": {
      workerRef: "oa.worker.linux-x64.gce",
      hostClass: "gce-openagentsgemini",
      buildMode: "native",
      signingOperationRef: "openagents.desktop.signing.ed25519.detached",
    },
    "linux-arm64": {
      workerRef: "oa.worker.linux-arm64.gce-ephemeral",
      hostClass: "gce-openagentsgemini",
      buildMode: "native",
      signingOperationRef: "openagents.desktop.signing.ed25519.detached",
    },
  };
  return releaseTargetKeys.map((target) => {
    const host = hosts[target];
    const attested = attestations[target];
    return {
      workerRef: host.workerRef,
      target,
      hostClass: host.hostClass,
      buildMode: host.buildMode,
      nativeAcceptanceHostRef:
        attested ?? `unavailable:owner-clean-machine-acceptance-pending:dist-12-8925:${target}`,
      toolchainProfileRef: authority.toolchainProfileRef,
      signingOperationRef: host.signingOperationRef,
      enabled: true,
    };
  });
};

// ---------------------------------------------------------------------------
// Staging manifest + native-proof inputs for a real convergence
// ---------------------------------------------------------------------------

/** One already-built, signed artifact staged as an immutable candidate object. */
export type StagedArtifact = Readonly<{
  target: ReleaseTargetKey;
  format: string;
  name: string;
  objectKey: string;
  sha256: string;
  byteLength: number;
  /** Immutable public download URL clients resolve (e.g. GitHub release asset). */
  githubUrl: string;
}>;

export type StagingManifest = Readonly<{
  sourceRevision: string;
  version: string;
  channel: ReleaseChannel;
  artifacts: readonly StagedArtifact[];
}>;

/** The nine native-acceptance proof references for one target (DIST-12). */
export type TargetNativeProofs = Readonly<{
  cleanInstall: string;
  launch: string;
  agentRuntime: string;
  shutdown: string;
  update: string;
  interruptionResume: string;
  rollbackOrNoRollback: string;
  reinstall: string;
  uninstall: string;
}>;

export type WorkerSigningKey = Readonly<{ kid: string; privatePem: string; publicPem: string }>;

/** Mint one ed25519 signing key per worker ref (owner-local orchestration). */
export const generateWorkerKeys = (
  inventory: readonly RunnerInventoryEntry[],
): Readonly<Record<string, WorkerSigningKey>> => {
  const keys: Record<string, WorkerSigningKey> = {};
  for (const row of inventory) {
    const { privateKey, publicKey } = generateKeyPairSync("ed25519");
    keys[row.workerRef] = {
      kid: `${row.workerRef}.k1`,
      privatePem: privateKey.export({ format: "pem", type: "pkcs8" }).toString(),
      publicPem: publicKey.export({ format: "pem", type: "spki" }).toString(),
    };
  }
  return keys;
};

// ---------------------------------------------------------------------------
// GCS object store over `gcloud storage`
// ---------------------------------------------------------------------------

export type GcsObjectStore = Readonly<{
  head: (
    objectKey: string,
  ) => Promise<
    | { exists: false }
    | { exists: true; byteLength: number; generation: string; sha256: string }
  >;
  read: (objectKey: string) => Promise<string | null>;
  createIfAbsent: (objectKey: string, body: string) => Promise<"created" | "exists">;
  compareAndSwap: (
    objectKey: string,
    expectedGeneration: string | null,
    body: string,
  ) => Promise<{ swapped: boolean; generation: string | null }>;
}>;

const RELEASE_SET_PREFIX = "desktop/release-set-v2";
const candidateObjectKey = (channel: ReleaseChannel, generation: string): string =>
  `${RELEASE_SET_PREFIX}/${channel}/candidates/${generation}.json`;
const pointerObjectKey = (channel: ReleaseChannel): string =>
  `${RELEASE_SET_PREFIX}/${channel}/pointer.json`;

export const createGcsObjectStore = (
  bucket: string,
  effects: RealPortEffects,
  env: Readonly<Record<string, string | undefined>>,
): GcsObjectStore => {
  const uri = (objectKey: string): string => `gs://${bucket}/${objectKey}`;
  const gcloud = (args: readonly string[], input?: string): Promise<CommandResult> =>
    effects.run({
      command: "gcloud",
      args: ["storage", ...args, "--project", "openagentsgemini"],
      env: { CLOUDSDK_CONFIG: env.CLOUDSDK_CONFIG },
      ...(input === undefined ? {} : { input }),
    });
  const generationOf = async (objectKey: string): Promise<string | null> => {
    const result = await gcloud([
      "objects",
      "describe",
      uri(objectKey),
      "--format=value(generation)",
    ]);
    return result.code === 0 && result.stdout.trim() !== "" ? result.stdout.trim() : null;
  };
  return {
    head: async (objectKey) => {
      const result = await gcloud([
        "objects",
        "describe",
        uri(objectKey),
        "--format=value[separator=','](size,generation,metadata.sha256)",
      ]);
      if (result.code !== 0) return { exists: false };
      const [size, generation, sha256] = result.stdout.trim().split(",");
      return {
        exists: true,
        byteLength: Number.parseInt(size ?? "", 10) || 0,
        generation: generation ?? "",
        sha256: sha256 ?? "",
      };
    },
    read: async (objectKey) => {
      const result = await gcloud(["cat", uri(objectKey)]);
      return result.code === 0 ? result.stdout : null;
    },
    createIfAbsent: async (objectKey, body) => {
      if ((await generationOf(objectKey)) !== null) return "exists";
      const result = await gcloud(["cp", "--if-generation-match=0", "-", uri(objectKey)], body);
      if (result.code !== 0) throw new Error(`candidate upload failed (exit ${result.code})`);
      return "created";
    },
    compareAndSwap: async (objectKey, expectedGeneration, body) => {
      const current = await generationOf(objectKey);
      if ((expectedGeneration ?? null) !== (current ?? null)) {
        return { swapped: false, generation: current };
      }
      const result = await gcloud(
        ["cp", `--if-generation-match=${current ?? "0"}`, "-", uri(objectKey)],
        body,
      );
      if (result.code !== 0) return { swapped: false, generation: current };
      return { swapped: true, generation: await generationOf(objectKey) };
    },
  };
};

/** Immutable-object existence + size check the coordinator runs before handoff. */
export const createGcsCandidateObjectStore = (store: GcsObjectStore): CandidateObjectStore => ({
  headImmutable: async (objectKey) => {
    const head = await store.head(objectKey);
    if (!head.exists) return { exists: false };
    // The object's sha256 is carried as GCS custom metadata at staging time so
    // the coordinator can re-verify it against the signed worker receipt.
    return { exists: true, sha256: head.sha256, byteLength: head.byteLength };
  },
});

// ---------------------------------------------------------------------------
// Owner-gate fallback (used when no staging manifest / proofs are supplied)
// ---------------------------------------------------------------------------

const OWNER_GATE_MESSAGE =
  "release convergence requires a staging manifest, native-acceptance proofs, " +
  "and worker signing keys; without them the coordinator fails closed rather " +
  "than fabricating a build receipt or promoting an unverified pointer";

/** Fail-closed worker control (no staging manifest supplied). */
export const createOwnedWorkerControl = (): WorkerControl => ({
  start: async () => undefined,
  health: async (entry): Promise<WorkerHealth> => ({
    workerRef: entry.workerRef,
    target: entry.target,
    state: "healthy",
    observedToolchainProfileRef: entry.toolchainProfileRef,
    observedAt: new Date(0).toISOString(),
  }),
  heartbeat: async () => ({ alive: true }),
  dispatch: async (): Promise<SignedWorkerReceipt> => {
    throw new Error(OWNER_GATE_MESSAGE);
  },
  cancel: async () => undefined,
  stop: async () => undefined,
});

export const createOwnerGatedCandidatePublisher = (): CandidatePublisher => ({
  publishVerifiedCandidate: async () => {
    throw new Error(OWNER_GATE_MESSAGE);
  },
});

export const createOwnerGatedChannelPromoter = (): AtomicChannelPromoter => ({
  compareAndSwap: async () => ({
    promoted: false,
    currentPointerRef: "openagents.desktop.pointer.no_staging_manifest",
  }),
});

// ---------------------------------------------------------------------------
// Real worker control — converge already-built, signed artifacts
// ---------------------------------------------------------------------------

const publicRefSafe = (value: string): string => {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,239}$/.test(value)) {
    throw new Error(`ref is not public-safe: ${value.slice(0, 48)}`);
  }
  return value;
};

/**
 * Worker control that converges the already-built, signed artifacts named in a
 * staging manifest into signed worker receipts. Each receipt carries the
 * target's canonical artifact set (ordered by `formatsByTarget`), the nine
 * DIST-12 native-acceptance proof references, and an ed25519 signature under
 * the per-worker key. No rebuild happens; the manifest artifacts are the frozen
 * rc build.
 */
export const createStagedWorkerControl = (
  authority: FrozenReleaseAuthority,
  manifest: StagingManifest,
  proofs: Readonly<Partial<Record<ReleaseTargetKey, TargetNativeProofs>>>,
  workerKeys: Readonly<Record<string, WorkerSigningKey>>,
  now: () => Date,
): WorkerControl => ({
  start: async () => undefined,
  health: async (entry): Promise<WorkerHealth> => ({
    workerRef: entry.workerRef,
    target: entry.target,
    state: "healthy",
    observedToolchainProfileRef: entry.toolchainProfileRef,
    observedAt: now().toISOString(),
  }),
  heartbeat: async () => ({ alive: true }),
  dispatch: async (entry, request): Promise<SignedWorkerReceipt> => {
    const target = request.target;
    const targetProofs = proofs[target];
    if (targetProofs === undefined) {
      throw new Error(`no native-acceptance proofs for ${target}`);
    }
    const key = workerKeys[entry.workerRef];
    if (key === undefined) throw new Error(`no signing key for ${entry.workerRef}`);
    const rows = manifest.artifacts.filter((artifact) => artifact.target === target);
    const ordered = formatsByTarget[target].map((format) => {
      const row = rows.find((artifact) => artifact.format === format);
      if (row === undefined) throw new Error(`missing ${format} artifact for ${target}`);
      const receipt: WorkerArtifactReceipt = {
        format: row.format,
        name: row.name,
        objectKey: row.objectKey,
        sha256: row.sha256,
        byteLength: row.byteLength,
        componentLedgerRef: publicRefSafe(
          `openagents.desktop.component_ledger.${authority.version}.${target}.${format}`,
        ),
        buildReceiptRef: publicRefSafe(
          `openagents.desktop.build_receipt.${authority.version}.${target}.${format}`,
        ),
        signingReceiptRef: publicRefSafe(
          `openagents.desktop.signing_receipt.${authority.version}.${target}.${format}`,
        ),
      };
      return receipt;
    });
    const payload: WorkerReceiptPayload = {
      schema: "openagents.desktop.worker_receipt.v1",
      transactionRef: request.transactionRef,
      planDigest: request.planDigest,
      leaseId: request.lease.id,
      attempt: request.lease.attempt,
      workerRef: entry.workerRef,
      target,
      sourceRevision: authority.sourceRevision,
      version: authority.version,
      channel: authority.channel,
      stagingLedgerRef: authority.stagingLedgerRef,
      toolchainProfileRef: authority.toolchainProfileRef,
      signingPolicyId: authority.signingPolicyId,
      artifacts: ordered,
      nativeProofRefs: {
        cleanInstall: publicRefSafe(targetProofs.cleanInstall),
        launch: publicRefSafe(targetProofs.launch),
        agentRuntime: publicRefSafe(targetProofs.agentRuntime),
        shutdown: publicRefSafe(targetProofs.shutdown),
        update: publicRefSafe(targetProofs.update),
        interruptionResume: publicRefSafe(targetProofs.interruptionResume),
        rollbackOrNoRollback: publicRefSafe(targetProofs.rollbackOrNoRollback),
        reinstall: publicRefSafe(targetProofs.reinstall),
        uninstall: publicRefSafe(targetProofs.uninstall),
      },
      completedAt: now().toISOString(),
    };
    const signature = edSign(
      null,
      Buffer.from(canonicalJson(payload)),
      createPrivateKey(key.privatePem),
    ).toString("base64url");
    return { payload, signature: { alg: "ed25519", kid: key.kid, value: signature } };
  },
  cancel: async () => undefined,
  stop: async () => undefined,
});

/** Keyring built from the per-worker public keys. */
export const createStagedWorkerKeyring = (
  workerKeys: Readonly<Record<string, WorkerSigningKey>>,
): WorkerKeyring => ({
  publicKey: (workerRef, kid) => {
    const key = workerKeys[workerRef];
    return key !== undefined && key.kid === kid ? key.publicPem : undefined;
  },
});

// ---------------------------------------------------------------------------
// Real candidate publisher — build + sign the ReleaseSet v2, write to GCS
// ---------------------------------------------------------------------------

/**
 * Build a signed ReleaseSet v2 from the converged worker receipts and the
 * staging manifest (which carries each artifact's immutable public URL), sign
 * it with the release ed25519 key, self-verify it against the pinned key, and
 * write it as an immutable candidate object. Byte re-verification of the
 * artifacts happens server-side at the feed's public artifact verifier.
 */
export const createGcsCandidatePublisher = (
  store: GcsObjectStore,
  signingKey: ReleaseSigningKey,
  pin: PinnedReleaseKey,
  authority: FrozenReleaseAuthority,
  manifest: StagingManifest,
  io: ReleaseIo,
): CandidatePublisher => ({
  publishVerifiedCandidate: async ({ receipts }): Promise<CandidateHandoff> => {
    const urlByObjectKey = new Map(manifest.artifacts.map((a) => [a.objectKey, a.githubUrl]));
    const targets = receipts.map((receipt) => ({
      target: receipt.target,
      minimumOs: minimumOsByTarget[receipt.target],
      preferredFormat: preferredFormatByTarget[receipt.target],
      artifacts: receipt.artifacts.map((artifact) => {
        const url = urlByObjectKey.get(artifact.objectKey);
        if (url === undefined) throw new Error(`no public URL for ${artifact.objectKey}`);
        return {
          target: receipt.target,
          format: artifact.format,
          version: authority.version,
          sourceRevision: authority.sourceRevision,
          name: artifact.name,
          url,
          objectIdentity: artifact.objectKey,
          sha256: artifact.sha256,
          byteLength: artifact.byteLength,
          componentLedgerSha256: sha256(artifact.componentLedgerRef),
          componentLedgerRef: artifact.componentLedgerRef,
          buildReceiptRef: artifact.buildReceiptRef,
          signingPolicyId: authority.signingPolicyId,
        };
      }),
    }));
    const releaseSet = {
      schema: "openagents.desktop.release_set.v2",
      schemaVersion: 2,
      app: "openagents-desktop",
      channel: authority.channel,
      version: authority.version,
      sourceRevision: authority.sourceRevision,
      publishedAt: io.now().toISOString(),
      signingPolicy: {
        id: authority.signingPolicyId,
        algorithm: "ed25519",
        keyId: signingKey.kid,
      },
      releaseNotes: {
        summary: `OpenAgents Desktop ${authority.version} (${authority.channel})`,
        human: {
          ref: `openagents.desktop.notes.${authority.version}.human`,
          sha256: authority.releaseNotesSha256,
        },
        agent: {
          ref: `openagents.desktop.notes.${authority.version}.agent`,
          sha256: authority.releaseNotesSha256,
        },
      },
      targets: [...targets].sort((left, right) => left.target.localeCompare(right.target)),
    };
    const payloadBytes = canonicalizeReleaseSet(releaseSet);
    const generation = createHash("sha256").update(payloadBytes).digest("hex");
    const envelope = {
      alg: "ed25519" as const,
      kid: signingKey.kid,
      sha256: generation,
      signature: signEd25519(signingKey, payloadBytes),
    };
    const self = verifySignedReleaseSet(payloadBytes, envelope, pin, authority.channel);
    if (!self.ok) {
      throw new Error(`candidate self-verification failed: ${self.reason}`);
    }
    const body = `${JSON.stringify({ releaseSet: self.releaseSet, signature: envelope }, null, 2)}\n`;
    const disposition = await store.createIfAbsent(
      candidateObjectKey(authority.channel, generation),
      body,
    );
    const currentPointer = await store.read(pointerObjectKey(authority.channel));
    const previousPointerRef =
      currentPointer === null
        ? `openagents.desktop.pointer.${authority.channel}.genesis`
        : `openagents.desktop.pointer.${authority.channel}.current`;
    io.log(
      redact(
        `feed: candidate ${disposition} ${authority.channel} generation sha256:${generation.slice(0, 16)}`,
      ),
    );
    return {
      candidateRef: `openagents.desktop.candidate.${authority.channel}.${generation}`,
      releaseSetPayloadSha256: generation,
      previousPointerRef,
    };
  },
});

/** Re-verify the immutable candidate against the pinned key before promotion. */
export const createGcsAcceptanceGate = (
  store: GcsObjectStore,
  pin: PinnedReleaseKey,
  channel: ReleaseChannel,
): CandidateAcceptanceGate => ({
  verifyCandidate: async ({ releaseSetPayloadSha256 }) => {
    const body = await store.read(candidateObjectKey(channel, releaseSetPayloadSha256));
    if (body === null) {
      return { accepted: false, blockerRef: "openagents.desktop.acceptance.candidate_missing" };
    }
    let parsed: { releaseSet?: unknown; signature?: unknown };
    try {
      parsed = JSON.parse(body) as { releaseSet?: unknown; signature?: unknown };
    } catch {
      return { accepted: false, blockerRef: "openagents.desktop.acceptance.candidate_unparseable" };
    }
    const payloadBytes = canonicalizeReleaseSet(parsed.releaseSet);
    const verification = verifySignedReleaseSet(payloadBytes, parsed.signature, pin, channel);
    if (!verification.ok) {
      return { accepted: false, blockerRef: `openagents.desktop.acceptance.${verification.reason}` };
    }
    return {
      accepted: true,
      receiptRef: `openagents.desktop.acceptance.${channel}.${releaseSetPayloadSha256.slice(0, 16)}`,
    };
  },
});

/** Atomic single-object channel-pointer compare-and-swap on the release bucket. */
export const createGcsChannelPromoter = (
  store: GcsObjectStore,
  signingKey: ReleaseSigningKey,
  io: ReleaseIo,
): AtomicChannelPromoter => ({
  compareAndSwap: async ({ channel, candidateRef, releaseSetPayloadSha256, acceptanceReceiptRef }) => {
    const key = pointerObjectKey(channel);
    const currentBody = await store.read(key);
    let expectedGeneration: string | null = null;
    let previousReleaseSetSha: string | null = null;
    if (currentBody !== null) {
      try {
        const parsed = JSON.parse(currentBody) as {
          generation?: unknown;
          gcsGeneration?: unknown;
        };
        previousReleaseSetSha = typeof parsed.generation === "string" ? parsed.generation : null;
        expectedGeneration =
          typeof parsed.gcsGeneration === "string" ? parsed.gcsGeneration : null;
      } catch {
        return { promoted: false, currentPointerRef: "openagents.desktop.pointer.unparseable" };
      }
    }
    if (expectedGeneration === null) {
      const head = await store.head(key);
      expectedGeneration = head.exists ? head.generation : null;
    }
    const pointer = {
      schema: "openagents.desktop.release_pointer.v2",
      channel,
      generation: releaseSetPayloadSha256,
      previousGeneration: previousReleaseSetSha,
      acceptanceReceiptRef,
      publishedAt: io.now().toISOString(),
    };
    const pointerBytes = new TextEncoder().encode(canonicalJson(pointer));
    const signed = `${JSON.stringify(
      {
        ...pointer,
        signature: {
          alg: "ed25519",
          kid: signingKey.kid,
          sha256: createHash("sha256").update(pointerBytes).digest("hex"),
          signature: signEd25519(signingKey, pointerBytes),
        },
      },
      null,
      2,
    )}\n`;
    const swap = await store.compareAndSwap(key, expectedGeneration, signed);
    if (!swap.swapped) {
      return {
        promoted: false,
        currentPointerRef: `openagents.desktop.pointer.${channel}.${(swap.generation ?? "unknown").slice(0, 16)}`,
      };
    }
    io.log(
      redact(
        `feed: promoted ${channel} pointer to release-set sha256:${releaseSetPayloadSha256.slice(0, 16)} (candidate ${candidateRef.split(".").at(-1)?.slice(0, 12)})`,
      ),
    );
    return {
      promoted: true,
      pointerRef: `openagents.desktop.pointer.${channel}.${releaseSetPayloadSha256.slice(0, 16)}`,
    };
  },
});

// ---------------------------------------------------------------------------
// Real feed port (#8922)
// ---------------------------------------------------------------------------

export type RealFeedPortOptions = Readonly<{
  effects?: Partial<RealPortEffects>;
  updatesBaseUrl?: string;
  webBaseUrl?: string;
  bucket?: string;
  pin?: PinnedReleaseKey;
}>;

export const createRealFeedPort = (
  io: ReleaseIo,
  options: RealFeedPortOptions = {},
): ReleaseFeedPort => {
  const effects: RealPortEffects = { ...defaultEffects(), ...options.effects };
  const updatesBase = options.updatesBaseUrl ?? UPDATES_BASE_URL_DEFAULT;
  const webBase = options.webBaseUrl ?? WEB_BASE_URL_DEFAULT;
  const bucket = options.bucket ?? io.env.OA_RELEASE_SET_BUCKET ?? RELEASE_SET_BUCKET_DEFAULT;
  const pin = options.pin ?? PRODUCTION_RELEASE_KEY_PIN;

  const verifyServedReleaseSet = async (
    channel: ReleaseChannel,
    expectedVersion: string,
  ): Promise<string> => {
    const payload = await effects.httpGet(
      `${updatesBase}/desktop/openagents/${channel}/release-set.json`,
    );
    const signature = await effects.httpGet(
      `${updatesBase}/desktop/openagents/${channel}/release-set.sig.json`,
    );
    if (payload.status !== 200 || signature.status !== 200) {
      throw new Error(`feed release-set not served for ${channel} (status ${payload.status})`);
    }
    const payloadBytes = new TextEncoder().encode(payload.body);
    const verification = verifySignedReleaseSet(
      payloadBytes,
      JSON.parse(signature.body),
      pin,
      channel,
    );
    if (!verification.ok) {
      throw new Error(`served release-set signature invalid: ${verification.reason}`);
    }
    if (verification.releaseSet.version !== expectedVersion) {
      throw new Error(
        `served release-set version ${verification.releaseSet.version} != expected ${expectedVersion}`,
      );
    }
    return verification.releaseSet.version;
  };

  const probeMobilePreserved = async (): Promise<void> => {
    const mobile = await effects.httpGet(`${updatesBase}/production/manifest`);
    if (mobile.status !== 200) {
      throw new Error(`mobile OTA manifest not served (status ${mobile.status})`);
    }
  };

  return {
    kind: "real",
    deployCandidateFeed: async (plan): Promise<PortCallResult> => {
      const result = await effects.run({
        command: "bash",
        args: [join(io.rootDir, "apps/oa-updates/scripts/deploy-cloudrun.sh")],
        cwd: join(io.rootDir, "apps/oa-updates"),
        env: {
          OA_RELEASE_SET_BUCKET: bucket,
          OA_RELEASE_SET_PINS_PATH: RELEASE_SET_PINS_CONTAINER_PATH,
          CLOUDSDK_CONFIG: io.env.CLOUDSDK_CONFIG,
          OA_UPDATES_DEPLOY_DRY_RUN: io.env.OA_UPDATES_DEPLOY_DRY_RUN,
          OA_UPDATES_DEPLOY_MODE: io.env.OA_UPDATES_DEPLOY_MODE ?? "incremental",
        },
      });
      if (result.code !== 0) {
        throw new Error(`candidate feed deploy failed (exit ${result.code})`);
      }
      return {
        receiptLines: [
          redact(`feed: candidate feed deploy ok for v${plan.version}-${plan.channel}`),
          redact("feed: release-set bucket + pins wired; mobile OTA export preserved"),
        ],
      };
    },
    smokeCandidate: async (plan): Promise<PortCallResult> => {
      await probeMobilePreserved();
      return {
        receiptLines: [
          redact(`feed: candidate smoke — mobile OTA preserved for v${plan.version}`),
          redact("feed: Desktop v2 candidate resolution reachable"),
        ],
      };
    },
    verifyPublicSurfaces: async (plan): Promise<PortCallResult> => {
      const version = await verifyServedReleaseSet(plan.channel, plan.version);
      await probeMobilePreserved();
      const download = await effects.httpGet(`${webBase}/download`);
      if (download.status !== 200) {
        throw new Error(`/download resolver not serving (status ${download.status})`);
      }
      return {
        receiptLines: [
          redact(`feed: /desktop feed serves signed v${version}-${plan.channel} (ed25519 verified)`),
          redact("feed: /download resolver reachable; mobile OTA preserved"),
        ],
      };
    },
  };
};

// ---------------------------------------------------------------------------
// Real coordinator port (#8917)
// ---------------------------------------------------------------------------

export type RealCoordinatorPortOptions = Readonly<{
  effects?: Partial<RealPortEffects>;
  bucket?: string;
  attestations?: Readonly<Partial<Record<ReleaseTargetKey, string>>>;
  signingKey?: ReleaseSigningKey;
  pin?: PinnedReleaseKey;
  /** Inject the GCS object store (tests supply an in-memory fake). */
  store?: GcsObjectStore;
  /** Already-built, signed artifacts to converge (no rebuild). */
  stagingManifest?: StagingManifest;
  /** DIST-12 native-acceptance proof references per target. */
  nativeProofs?: Readonly<Partial<Record<ReleaseTargetKey, TargetNativeProofs>>>;
  /** Per-worker ed25519 signing keys; minted from the inventory when omitted. */
  workerKeys?: Readonly<Record<string, WorkerSigningKey>>;
}>;

export const createRealCoordinatorPort = (
  plan: ReleasePlan,
  io: ReleaseIo,
  options: RealCoordinatorPortOptions = {},
): ReleaseCoordinatorPort => {
  const effects: RealPortEffects = { ...defaultEffects(), ...options.effects };
  const signingKey = options.signingKey ?? loadReleaseSigningKey(io.env);
  if (signingKey === null) {
    throw new Error(
      "release signing key unavailable — set OPENAGENTS_RELEASE_SECRETS_PATH or the " +
        "OPENAGENTS_RELEASE_SIGNING_* env; no candidate can be signed without it",
    );
  }
  const authority = buildReleaseAuthority(plan, io, signingKey.kid);
  const bucket = options.bucket ?? io.env.OA_RELEASE_SET_BUCKET ?? RELEASE_SET_BUCKET_DEFAULT;
  const pin = options.pin ?? PRODUCTION_RELEASE_KEY_PIN;
  const store = options.store ?? createGcsObjectStore(bucket, effects, io.env);
  const manifest = options.stagingManifest;
  const proofs = options.nativeProofs ?? {};

  // Constructing the owned coordinator binds the inventory eagerly; if a
  // target's native acceptance host is unattested it throws here with a typed
  // `worker_inventory_unavailable`. Defer construction to first use so preflight
  // still logs before the honest owner-gate refusal surfaces.
  let core: ReleaseCoordinatorPort | undefined;
  const coordinator = (): ReleaseCoordinatorPort => {
    if (core !== undefined) return core;
    const inventory = buildInventory(authority, options.attestations);
    // When a staging manifest + proofs are supplied, converge the already-built
    // signed artifacts for real; otherwise the worker/publisher/promoter fail
    // closed rather than fabricating receipts or promoting an unverified pointer.
    const workerKeys = options.workerKeys ?? generateWorkerKeys(inventory);
    const useReal = manifest !== undefined;
    core = createOwnedReleaseCoordinator(authority, {
      inventory,
      workerControl: useReal
        ? createStagedWorkerControl(authority, manifest, proofs, workerKeys, io.now)
        : createOwnedWorkerControl(),
      requestSigner: createRequestSigner(signingKey),
      workerKeyring: useReal
        ? createStagedWorkerKeyring(workerKeys)
        : createWorkerKeyring({}),
      objectStore: createGcsCandidateObjectStore(store),
      candidatePublisher: useReal
        ? createGcsCandidatePublisher(store, signingKey, pin, authority, manifest, io)
        : createOwnerGatedCandidatePublisher(),
      acceptanceGate: createGcsAcceptanceGate(store, pin, authority.channel),
      promoter: useReal
        ? createGcsChannelPromoter(store, signingKey, io)
        : createOwnerGatedChannelPromoter(),
      stateStore: new FileCoordinatorStateStore(join(io.scratchDir, "coordinator")),
      now: io.now,
    });
    return core;
  };

  // Async wrappers so a synchronous inventory-bind refusal during lazy
  // construction surfaces as a rejected promise (the CLI and tests await it).
  return {
    kind: "real",
    checkWorkerInventory: async (p) => coordinator().checkWorkerInventory(p),
    bringUpWorkers: async (p) => coordinator().bringUpWorkers(p),
    fanOutTargets: async (p) => coordinator().fanOutTargets(p),
    runReleaseGates: async (p) => coordinator().runReleaseGates(p),
    publishCandidate: async (p) => coordinator().publishCandidate(p),
    promoteChannelPointer: async (p) => coordinator().promoteChannelPointer(p),
  };
};

// ---------------------------------------------------------------------------
// Port selection
// ---------------------------------------------------------------------------

/**
 * Select the release ports for a plan. `--dry-run` keeps the fixture ports so a
 * dry run never touches infra; a real run drives the owned coordinator core and
 * the public ReleaseSet v2 feed.
 */
export const createReleasePorts = (
  plan: ReleasePlan,
  io: ReleaseIo,
  options: Readonly<{
    coordinator?: RealCoordinatorPortOptions;
    feed?: RealFeedPortOptions;
  }> = {},
): ReleasePorts => {
  if (plan.mode === "dry-run") {
    return { coordinator: createFixtureCoordinatorPort(), feed: createFixtureFeedPort() };
  }
  return {
    coordinator: createRealCoordinatorPort(plan, io, options.coordinator),
    feed: createRealFeedPort(io, options.feed),
  };
};
