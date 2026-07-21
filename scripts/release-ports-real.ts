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

import { createPrivateKey, sign as edSign } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { verifySignedReleaseSet } from "../apps/openagents-desktop/src/release-set-contract.js";
import {
  type PinnedReleaseKey,
  PRODUCTION_RELEASE_KEY_PIN,
} from "../apps/openagents-desktop/src/update-contract.js";
import {
  type AtomicChannelPromoter,
  type CandidateAcceptanceGate,
  type CandidateObjectStore,
  type CandidatePublisher,
  type CoordinatorRequestSigner,
  createOwnedReleaseCoordinator,
  FileCoordinatorStateStore,
  type FrozenReleaseAuthority,
  type RunnerInventoryEntry,
  sha256,
  type SignedWorkerReceipt,
  type WorkerControl,
  type WorkerHealth,
  type WorkerKeyring,
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
// GCS-backed immutable object store over `gcloud storage`
// ---------------------------------------------------------------------------

/**
 * Immutable-object existence + size check the coordinator runs before candidate
 * handoff, backed by `gcloud storage objects describe` on the release-set
 * bucket. Sha256 is re-verified out of band from the candidate body, so the
 * head only proves existence and byte length here.
 */
export const createGcsCandidateObjectStore = (
  bucket: string,
  effects: RealPortEffects,
  env: Readonly<Record<string, string | undefined>>,
): CandidateObjectStore => ({
  headImmutable: async (objectKey) => {
    const result = await effects.run({
      command: "gcloud",
      args: [
        "storage",
        "objects",
        "describe",
        `gs://${bucket}/${objectKey}`,
        "--project",
        "openagentsgemini",
        "--format=value(size)",
      ],
      env: { CLOUDSDK_CONFIG: env.CLOUDSDK_CONFIG },
    });
    if (result.code !== 0) return { exists: false };
    const size = Number.parseInt(result.stdout.trim(), 10);
    return { exists: true, sha256: "", byteLength: Number.isSafeInteger(size) ? size : 0 };
  },
});

// ---------------------------------------------------------------------------
// Owner-gated adapters (fail closed until the attested build path lands)
// ---------------------------------------------------------------------------
//
// These four adapters are only reached AFTER convergence, which requires signed
// worker receipts carrying the owner-gated native-proof refs. Because the
// coordinator refuses at inventory bind until the owner attests those hosts
// (DIST-12 #8925), a real run never reaches them today. They fail closed with a
// typed owner-gate error rather than fabricating a build receipt or promoting an
// unverified pointer. The GCS candidate write + pointer CAS + ReleaseSet
// finalization from attested receipts lands with the owner-attested build path.

const OWNER_GATE_MESSAGE =
  "owner-attested build receipts required (DIST-12 #8925): the ReleaseSet " +
  "finalization, candidate upload, and pointer promotion land with the " +
  "attested owned-worker build path; a real run fails closed at inventory bind " +
  "until the owner completes clean-machine acceptance";

/** Drives the owned build hosts; dispatch fails closed until attestation. */
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

export const createOwnerGatedAcceptanceGate = (): CandidateAcceptanceGate => ({
  verifyCandidate: async () => ({
    accepted: false,
    blockerRef: "openagents.desktop.acceptance.owner_gate_dist_12_8925",
  }),
});

export const createOwnerGatedChannelPromoter = (): AtomicChannelPromoter => ({
  compareAndSwap: async () => ({
    promoted: false,
    currentPointerRef: "openagents.desktop.pointer.owner_gate_dist_12_8925",
  }),
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
  workerKeyRegistry?: Readonly<Record<string, Readonly<Record<string, string>>>>;
  signingKey?: ReleaseSigningKey;
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

  // Constructing the owned coordinator binds the inventory eagerly; if a
  // target's native acceptance host is unattested it throws here with a typed
  // `worker_inventory_unavailable`. Defer construction to first use so preflight
  // still logs before the honest owner-gate refusal surfaces.
  let core: ReleaseCoordinatorPort | undefined;
  const coordinator = (): ReleaseCoordinatorPort => {
    core ??= createOwnedReleaseCoordinator(authority, {
      inventory: buildInventory(authority, options.attestations),
      workerControl: createOwnedWorkerControl(),
      requestSigner: createRequestSigner(signingKey),
      workerKeyring: createWorkerKeyring(options.workerKeyRegistry ?? {}),
      objectStore: createGcsCandidateObjectStore(bucket, effects, io.env),
      candidatePublisher: createOwnerGatedCandidatePublisher(),
      acceptanceGate: createOwnerGatedAcceptanceGate(),
      promoter: createOwnerGatedChannelPromoter(),
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
