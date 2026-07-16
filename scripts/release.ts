#!/usr/bin/env node
// DIST-13 (#8926, slice 1): the one owner release command.
//
// `pnpm run release -- --channel <stable|rc> --version <semver>` maps exactly
// to `node --import tsx scripts/release.ts` (ProductSpec §11.1,
// docs/deploy/openagents-desktop-cross-platform-release.md). The command is a
// typed, idempotent, resumable step graph over the ENTIRE release transaction
// (issue #8926, nine steps):
//
//   1. preflight             REAL   clean-tree/origin-main freeze, version and
//                                   channel resolution against the Desktop
//                                   version authority, toolchain pins,
//                                   signing-credential PRESENCE (names only,
//                                   never values), worker-inventory port check
//   2. worker_bring_up       PORT   ReleaseCoordinatorPort.bringUpWorkers
//   3. fan_out               PORT   ReleaseCoordinatorPort.fanOutTargets
//   4. test_gates            PORT   ReleaseCoordinatorPort.runReleaseGates
//   5. candidate             PORT   ReleaseCoordinatorPort.publishCandidate +
//                                   ReleaseFeedPort.deployCandidateFeed +
//                                   ReleaseFeedPort.smokeCandidate
//   6. changelog             REAL   drives scripts/changelog.ts roll semantics
//                                   (refuses when UNRELEASED is empty)
//   7. promote               PORT   ReleaseCoordinatorPort.promoteChannelPointer
//   8. public_surface_verify PORT   ReleaseFeedPort.verifyPublicSurfaces
//   9. final_receipt         REAL   one public-safe release receipt under
//                                   docs/deploy/receipts/ (dry-run: scratch)
//
// PORT steps execute only against typed integration-point interfaces in this
// slice. The REAL implementations arrive with #8917 (coordinator/workers) and
// #8922 (feed/promotion); until then a fixture port refuses to run outside
// --dry-run (ReleasePortNotImplementedError), which makes it STRUCTURALLY
// impossible for this slice to touch a channel pointer in a real run.
//
// ## ReleaseCoordinatorPort — assumed methods for the #8917 lane
//
// The #8917 coordinator is expected to provide a `kind: "real"` implementation
// of exactly these methods (align or object on the issue):
//
//   - checkWorkerInventory(plan): verify the six-target owned worker registry
//     is healthy and admits the plan's targets (GCE in `openagentsgemini` for
//     linux-x64/linux-arm64/win32-x64, owned Tailnet Macs for darwin-arm64/
//     darwin-x64, the reviewed DIST-04 cross-build + native acceptance host
//     for win32-arm64). Preflight-time; no builds started.
//   - bringUpWorkers(plan): start/verify the owned workers for the plan.
//   - fanOutTargets(plan): build, stage (DIST-03 stage-target descriptors),
//     sign, and verify all six targets/all formats; converge evidence.
//   - runReleaseGates(plan): per-target release gates plus the automatable
//     platform install/update proofs; named native-host steps surface as
//     explicit receipt lines.
//   - publishCandidate(plan): upload immutable candidate artifacts.
//   - promoteChannelPointer(plan): ONE atomic channel-pointer promotion of an
//     already-verified immutable ReleaseSet; never rebuilds artifacts; any
//     failure leaves the current pointer untouched.
//
// ## ReleaseFeedPort — assumed methods for the #8922 lane
//
//   - deployCandidateFeed(plan): deploy the ReleaseSet v2 candidate feed on
//     `oa-updates` while preserving the mobile OTA export.
//   - smokeCandidate(plan): candidate smoke — Desktop feed resolution and the
//     mobile-preservation probe.
//   - verifyPublicSurfaces(plan): post-promotion verification that /download
//     serves the promoted version through the #8923 resolver (no manual page
//     edits), homepage CTAs resolve, /changelog updated, and the mobile feed
//     is preserved.
//
// Every port method takes the frozen ReleasePlan and returns bounded
// public-safe receipt lines. Secrets never enter receipts.
//
// Usage (from the repo root):
//   pnpm run release -- --channel rc --version 0.1.0-rc.18 --dry-run
//   pnpm run release -- --channel rc --version 0.1.0-rc.18 --yes
//   pnpm run release -- --resume <transaction-ref>
//   pnpm run release -- --channel rc --version 0.1.0-rc.18 --dry-run --allow-unfrozen
//
// `--allow-unfrozen` is valid ONLY with --dry-run: it downgrades the
// clean-tree/frozen rows to conspicuous warnings so a dry-run can exercise the
// plan from a development tree. A real run always requires the exact frozen
// clean origin/main.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import {
  CHANGELOG_DIR,
  RELEASE_NOTES_MAX_LENGTH,
  UNRELEASED_FILE,
  rollUnreleased,
  runRoll,
} from "./changelog.js";

// ---------------------------------------------------------------------------
// Mirrored release vocabulary
//
// These constants mirror the Desktop release contracts
// (apps/openagents-desktop/src/update-contract.ts and
// src/release-set-contract.ts). Root scripts deliberately do not import app
// sources; scripts/release.test.ts guards the mirror against drift by reading
// those contract sources.
// ---------------------------------------------------------------------------

export const releaseChannels = ["stable", "rc"] as const;
export type ReleaseChannel = (typeof releaseChannels)[number];

export const RELEASE_VERSION_PATTERN =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/;

export const releaseTargetKeys = [
  "darwin-arm64",
  "darwin-x64",
  "win32-arm64",
  "win32-x64",
  "linux-arm64",
  "linux-x64",
] as const;
export type ReleaseTargetKey = (typeof releaseTargetKeys)[number];

/** The Desktop version authority: the app package manifest. */
export const DESKTOP_PACKAGE_JSON_PATH = "apps/openagents-desktop/package.json";

export const REQUIRED_NODE_MAJOR = 24;

// ---------------------------------------------------------------------------
// Signing-credential PRESENCE checks (names only — values are never read
// beyond existence, never printed, never written to state or receipts).
// The names mirror the documented seams:
//   - apps/openagents-desktop/scripts/macos-gatekeeper.ts (Developer ID +
//     notary credentials)
//   - apps/openagents-desktop/scripts/publish-release.ts (pinned ed25519
//     ReleaseSet signing key seam)
// ---------------------------------------------------------------------------

export const MACOS_SIGNING_CREDENTIAL_NAMES = [
  "OA_DEVELOPER_ID_APPLICATION",
  "ASC_API_PRIVATE_KEY_PATH",
  "ASC_API_KEY_ID",
  "ASC_API_ISSUER_ID",
] as const;

export const RELEASE_SET_SIGNING_ENV_NAMES = [
  "OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D",
  "OPENAGENTS_RELEASE_SIGNING_KID",
] as const;

export const RELEASE_SET_SIGNING_SECRETS_PATH_NAME = "OPENAGENTS_RELEASE_SECRETS_PATH";

// ---------------------------------------------------------------------------
// Step graph
// ---------------------------------------------------------------------------

export const releaseStepIds = [
  "preflight",
  "worker_bring_up",
  "fan_out",
  "test_gates",
  "candidate",
  "changelog",
  "promote",
  "public_surface_verify",
  "final_receipt",
] as const;
export type ReleaseStepId = (typeof releaseStepIds)[number];

export type ReleaseStepKind = "real" | "port";

export type ReleaseStepDefinition = Readonly<{
  id: ReleaseStepId;
  title: string;
  kind: ReleaseStepKind;
  /** Which typed ports the step executes against ("real" steps use none). */
  ports: ReadonlyArray<"coordinator" | "feed">;
  dependsOn: readonly ReleaseStepId[];
}>;

export const RELEASE_STEP_GRAPH: readonly ReleaseStepDefinition[] = [
  {
    id: "preflight",
    title: "Preflight: freeze, version/channel, toolchain, credential presence, worker inventory",
    kind: "real",
    ports: ["coordinator"],
    dependsOn: [],
  },
  {
    id: "worker_bring_up",
    title: "Bring up/verify the owned six-target workers (GCE + Tailnet)",
    kind: "port",
    ports: ["coordinator"],
    dependsOn: ["preflight"],
  },
  {
    id: "fan_out",
    title: "Fan out: build/stage/sign/verify all six targets; converge evidence",
    kind: "port",
    ports: ["coordinator"],
    dependsOn: ["worker_bring_up"],
  },
  {
    id: "test_gates",
    title: "Run per-target release gates and platform install/update proofs",
    kind: "port",
    ports: ["coordinator"],
    dependsOn: ["fan_out"],
  },
  {
    id: "candidate",
    title: "Upload immutable candidates, deploy candidate feed, run candidate smoke",
    kind: "port",
    ports: ["coordinator", "feed"],
    dependsOn: ["test_gates"],
  },
  {
    id: "changelog",
    title: "Generate human + agent changelogs and bounded release notes (DIST-14)",
    kind: "real",
    ports: [],
    dependsOn: ["candidate"],
  },
  {
    id: "promote",
    title: "One atomic channel-pointer promotion",
    kind: "port",
    ports: ["coordinator"],
    dependsOn: ["changelog"],
  },
  {
    id: "public_surface_verify",
    title: "Verify /download, homepage CTAs, /changelog, and mobile preservation",
    kind: "port",
    ports: ["feed"],
    dependsOn: ["promote"],
  },
  {
    id: "final_receipt",
    title: "Write the one final public-safe release receipt",
    kind: "real",
    ports: [],
    dependsOn: ["public_surface_verify"],
  },
];

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

export type ReleaseMode = "dry-run" | "real";

export type ReleasePlan = Readonly<{
  transactionRef: string;
  mode: ReleaseMode;
  version: string;
  channel: ReleaseChannel;
  sourceRevision: string;
  targets: readonly ReleaseTargetKey[];
  /** YYYY-MM-DD (UTC) — changelog roll date and receipt file-name date. */
  date: string;
  /** `--yes`: auto-approve only gates declared safe for unattended use. */
  unattended: boolean;
  /** Gate ids explicitly approved via `--approve <gateId>`. */
  approvedGates: readonly string[];
}>;

export const TRANSACTION_REF_PATTERN = /^v[0-9A-Za-z.-]+-(?:stable|rc)-\d{8}T\d{6}Z$/;

export const newTransactionRef = (version: string, channel: ReleaseChannel, now: Date): string => {
  const stamp = now
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d+Z$/, "Z");
  return `v${version}-${channel}-${stamp}`;
};

// ---------------------------------------------------------------------------
// Owner gates — explicit, named in output before effects start; never a
// silent stall.
// ---------------------------------------------------------------------------

export type ReleaseOwnerGate = Readonly<{
  id: string;
  description: string;
  /** May `--yes` approve it unattended? */
  safeForUnattended: boolean;
  beforeStep: ReleaseStepId;
  appliesTo: (plan: ReleasePlan) => boolean;
}>;

export const RELEASE_OWNER_GATES: readonly ReleaseOwnerGate[] = [
  {
    id: "changelog_human_review",
    description:
      "The rolled human changelog section is a DRAFT and must be reviewed/edited " +
      "for clarity before promotion (DIST-14 semantics: committed artifact is " +
      "reviewed text, not raw generation)",
    safeForUnattended: false,
    beforeStep: "promote",
    appliesTo: () => true,
  },
  {
    id: "first_stable_promotion",
    description: "Stable channel-pointer promotion requires explicit owner approval",
    safeForUnattended: false,
    beforeStep: "promote",
    appliesTo: (plan) => plan.channel === "stable",
  },
  {
    id: "rc_promotion",
    description: "RC channel-pointer promotion (safe to auto-approve with --yes)",
    safeForUnattended: true,
    beforeStep: "promote",
    appliesTo: (plan) => plan.channel === "rc",
  },
];

export class ReleaseGateError extends Error {
  readonly gateId: string;
  constructor(gate: ReleaseOwnerGate) {
    super(
      `owner gate "${gate.id}" is not approved: ${gate.description}. ` +
        (gate.safeForUnattended
          ? "Pass --yes (safe for unattended use) or --approve " + gate.id + "."
          : `This gate is NOT safe for unattended use — pass --approve ${gate.id} explicitly.`),
    );
    this.name = "ReleaseGateError";
    this.gateId = gate.id;
  }
}

export const gatesForPlan = (plan: ReleasePlan): readonly ReleaseOwnerGate[] =>
  RELEASE_OWNER_GATES.filter((gate) => gate.appliesTo(plan));

export const isGateApproved = (gate: ReleaseOwnerGate, plan: ReleasePlan): boolean =>
  plan.approvedGates.includes(gate.id) || (plan.unattended && gate.safeForUnattended);

// ---------------------------------------------------------------------------
// Typed integration ports (#8917 coordinator, #8922 feed)
// ---------------------------------------------------------------------------

export type PortCallResult = Readonly<{ receiptLines: readonly string[] }>;

export type ReleaseCoordinatorPort = Readonly<{
  /** "fixture" refuses to execute outside --dry-run. #8917 lands "real". */
  kind: "fixture" | "real";
  checkWorkerInventory: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  bringUpWorkers: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  fanOutTargets: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  runReleaseGates: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  publishCandidate: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  promoteChannelPointer: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
}>;

export type ReleaseFeedPort = Readonly<{
  /** "fixture" refuses to execute outside --dry-run. #8922 lands "real". */
  kind: "fixture" | "real";
  deployCandidateFeed: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  smokeCandidate: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
  verifyPublicSurfaces: (plan: ReleasePlan) => Promise<PortCallResult> | PortCallResult;
}>;

export type ReleasePorts = Readonly<{
  coordinator: ReleaseCoordinatorPort;
  feed: ReleaseFeedPort;
}>;

export class ReleasePortNotImplementedError extends Error {
  constructor(portName: string, method: string) {
    super(
      `${portName}.${method} has no real implementation in this slice — it lands with ` +
        `${portName === "ReleaseCoordinatorPort" ? "#8917" : "#8922"}. ` +
        "Only --dry-run may execute fixture ports; a real release run is refused " +
        "fail-closed (no channel pointer can be touched).",
    );
    this.name = "ReleasePortNotImplementedError";
  }
}

const callPort = async (
  plan: ReleasePlan,
  portKind: "fixture" | "real",
  portName: "ReleaseCoordinatorPort" | "ReleaseFeedPort",
  method: string,
  invoke: () => Promise<PortCallResult> | PortCallResult,
): Promise<PortCallResult> => {
  if (plan.mode !== "dry-run" && portKind !== "real") {
    throw new ReleasePortNotImplementedError(portName, method);
  }
  return await invoke();
};

export type FixturePortOptions = Readonly<{
  /** Method name that fails exactly once (resume-after-failure tests). */
  failOnceOn?: string;
}>;

export type FixtureCoordinatorPort = ReleaseCoordinatorPort & { readonly calls: string[] };
export type FixtureFeedPort = ReleaseFeedPort & { readonly calls: string[] };

const fixtureCall = (
  calls: string[],
  failState: { failed: boolean },
  options: FixturePortOptions,
  method: string,
  lines: readonly string[],
): PortCallResult => {
  calls.push(method);
  if (options.failOnceOn === method && !failState.failed) {
    failState.failed = true;
    throw new Error(`fixture failure injected on ${method}`);
  }
  return { receiptLines: lines };
};

export const createFixtureCoordinatorPort = (
  options: FixturePortOptions = {},
): FixtureCoordinatorPort => {
  const calls: string[] = [];
  const failState = { failed: false };
  return {
    kind: "fixture",
    calls,
    checkWorkerInventory: (plan) =>
      fixtureCall(calls, failState, options, "checkWorkerInventory", [
        `fixture: worker inventory healthy for ${plan.targets.length} targets`,
        "fixture: darwin-arm64/darwin-x64 = owned Tailnet Macs; linux-x64/linux-arm64/win32-x64 = GCE openagentsgemini; win32-arm64 = DIST-04 cross-build + native host",
      ]),
    bringUpWorkers: (plan) =>
      fixtureCall(calls, failState, options, "bringUpWorkers", [
        `fixture: ${plan.targets.length}/6 workers up (no cloud spend in dry-run)`,
      ]),
    fanOutTargets: (plan) =>
      fixtureCall(
        calls,
        failState,
        options,
        "fanOutTargets",
        plan.targets.map((target) => `fixture: ${target} built/staged/signed/verified`),
      ),
    runReleaseGates: (plan) =>
      fixtureCall(calls, failState, options, "runReleaseGates", [
        `fixture: release gates green for ${plan.targets.join(", ")}`,
        "fixture: native-host interaction steps surfaced as named receipts",
      ]),
    publishCandidate: (plan) =>
      fixtureCall(calls, failState, options, "publishCandidate", [
        `fixture: immutable candidate v${plan.version}-${plan.channel} uploaded`,
      ]),
    promoteChannelPointer: (plan) =>
      fixtureCall(calls, failState, options, "promoteChannelPointer", [
        `fixture: SIMULATED atomic ${plan.channel} pointer promotion to v${plan.version} — no real pointer exists in this slice`,
      ]),
  };
};

export const createFixtureFeedPort = (options: FixturePortOptions = {}): FixtureFeedPort => {
  const calls: string[] = [];
  const failState = { failed: false };
  return {
    kind: "fixture",
    calls,
    deployCandidateFeed: (plan) =>
      fixtureCall(calls, failState, options, "deployCandidateFeed", [
        `fixture: candidate feed deployed for v${plan.version}-${plan.channel}; mobile OTA export preserved`,
      ]),
    smokeCandidate: (plan) =>
      fixtureCall(calls, failState, options, "smokeCandidate", [
        `fixture: candidate smoke green (Desktop resolution + mobile-preservation probe) for v${plan.version}`,
      ]),
    verifyPublicSurfaces: (plan) =>
      fixtureCall(calls, failState, options, "verifyPublicSurfaces", [
        `fixture: /download serves v${plan.version}-${plan.channel} via the #8923 resolver`,
        "fixture: homepage CTAs verified; /changelog updated; mobile feed preserved",
      ]),
  };
};

// ---------------------------------------------------------------------------
// Preflight (REAL) — pure checks over gathered inputs
// ---------------------------------------------------------------------------

export type PreflightSeverity = "refusal" | "warning" | "ok";

export type PreflightCheckResult = Readonly<{
  id: string;
  severity: PreflightSeverity;
  detail: string;
}>;

export type PreflightInput = Readonly<{
  mode: ReleaseMode;
  /** `--allow-unfrozen` — valid only with --dry-run. */
  allowUnfrozen: boolean;
  statusPorcelain: string;
  headSha: string;
  originMainSha: string;
  /** Version from the Desktop version authority (app package.json). */
  desktopPackageVersion: string;
  cliVersion: string;
  channel: ReleaseChannel;
  nodeVersion: string;
  /** Root package.json `packageManager` pin. */
  packageManagerPin: string | undefined;
  /** Presence-only view of the environment. Values are never surfaced. */
  env: Readonly<Record<string, string | undefined>>;
}>;

const freezeSeverity = (input: PreflightInput): PreflightSeverity =>
  input.mode === "dry-run" && input.allowUnfrozen ? "warning" : "refusal";

export const runPreflightChecks = (input: PreflightInput): readonly PreflightCheckResult[] => {
  const checks: PreflightCheckResult[] = [];

  const dirty = input.statusPorcelain.trim().length > 0;
  checks.push(
    dirty
      ? {
          id: "clean_tree",
          severity: freezeSeverity(input),
          detail:
            "working tree has uncommitted changes — a release runs only from a clean tree" +
            (freezeSeverity(input) === "warning"
              ? " (DRY-RUN --allow-unfrozen: reported only)"
              : ""),
        }
      : { id: "clean_tree", severity: "ok", detail: "working tree clean" },
  );

  const head = input.headSha.trim();
  const originMain = input.originMainSha.trim();
  const frozen = head.length > 0 && head === originMain;
  checks.push(
    frozen
      ? {
          id: "frozen_at_origin_main",
          severity: "ok",
          detail: `frozen at origin/main ${head.slice(0, 10)}`,
        }
      : {
          id: "frozen_at_origin_main",
          severity: freezeSeverity(input),
          detail:
            `HEAD ${head.slice(0, 10) || "<none>"} is not origin/main ${originMain.slice(0, 10) || "<none>"}` +
            (freezeSeverity(input) === "warning"
              ? " (DRY-RUN --allow-unfrozen: reported only)"
              : ""),
        },
  );

  if (!RELEASE_VERSION_PATTERN.test(input.cliVersion)) {
    checks.push({
      id: "version_resolution",
      severity: "refusal",
      detail: `--version "${input.cliVersion}" is not a valid release version (X.Y.Z or X.Y.Z-rc.N)`,
    });
  } else if (input.cliVersion !== input.desktopPackageVersion) {
    checks.push({
      id: "version_resolution",
      severity: "refusal",
      detail:
        `--version ${input.cliVersion} does not match the Desktop version authority ` +
        `${DESKTOP_PACKAGE_JSON_PATH} (${input.desktopPackageVersion})`,
    });
  } else if (input.channel === "stable" && input.cliVersion.includes("-rc.")) {
    checks.push({
      id: "version_resolution",
      severity: "refusal",
      detail: `pre-release ${input.cliVersion} may not release on the stable channel`,
    });
  } else if (input.channel === "rc" && !input.cliVersion.includes("-rc.")) {
    checks.push({
      id: "version_resolution",
      severity: "refusal",
      detail: `rc-channel releases require an -rc.N version; got ${input.cliVersion}`,
    });
  } else {
    checks.push({
      id: "version_resolution",
      severity: "ok",
      detail: `version ${input.cliVersion} on channel ${input.channel} matches the version authority`,
    });
  }

  const nodeMajor = Number.parseInt(input.nodeVersion.split(".")[0] ?? "", 10);
  const pnpmPinned = input.packageManagerPin?.startsWith("pnpm@") === true;
  if (nodeMajor !== REQUIRED_NODE_MAJOR || !pnpmPinned) {
    checks.push({
      id: "toolchain_pins",
      severity: "refusal",
      detail:
        `toolchain policy: node ${input.nodeVersion} (require major ${REQUIRED_NODE_MAJOR}); ` +
        `packageManager pin ${input.packageManagerPin ?? "<missing>"} (require pnpm@…)`,
    });
  } else {
    checks.push({
      id: "toolchain_pins",
      severity: "ok",
      detail: `node ${input.nodeVersion} and ${input.packageManagerPin} match the toolchain policy`,
    });
  }

  // PRESENCE only. Values are never read beyond truthiness, never printed.
  const missingMac = MACOS_SIGNING_CREDENTIAL_NAMES.filter((name) => !input.env[name]);
  const releaseSetPresent =
    RELEASE_SET_SIGNING_ENV_NAMES.every((name) => Boolean(input.env[name])) ||
    Boolean(input.env[RELEASE_SET_SIGNING_SECRETS_PATH_NAME]);
  const missingNames = [
    ...missingMac,
    ...(releaseSetPresent
      ? []
      : [`${RELEASE_SET_SIGNING_ENV_NAMES.join("+")} or ${RELEASE_SET_SIGNING_SECRETS_PATH_NAME}`]),
  ];
  if (missingNames.length === 0) {
    checks.push({
      id: "signing_credentials_present",
      severity: "ok",
      detail: "signing credentials present (presence only — values never inspected)",
    });
  } else {
    checks.push({
      id: "signing_credentials_present",
      severity: input.mode === "dry-run" ? "warning" : "refusal",
      detail:
        `missing signing credentials: ${missingNames.join(", ")} — ` +
        (input.mode === "dry-run"
          ? "a real run would REFUSE fail-closed before any build"
          : "REFUSING fail-closed before any build (no unsigned release fallback)"),
    });
  }

  return checks;
};

export class ReleasePreflightError extends Error {
  readonly refusals: readonly PreflightCheckResult[];
  constructor(refusals: readonly PreflightCheckResult[]) {
    super(
      `preflight refused: ${refusals.map((check) => `${check.id} (${check.detail})`).join("; ")}`,
    );
    this.name = "ReleasePreflightError";
    this.refusals = refusals;
  }
}

// ---------------------------------------------------------------------------
// Durable, resumable transaction state
// ---------------------------------------------------------------------------

export const RELEASE_TRANSACTION_SCHEMA = "openagents.desktop.release_transaction.v1";
export const RELEASE_SCRATCH_DIR = ".release";

export type ReleaseStepStatus = "pending" | "succeeded" | "failed";

export type ReleaseStepRecord = {
  status: ReleaseStepStatus;
  receiptLines: string[];
  startedAt?: string;
  finishedAt?: string;
  failure?: string;
};

export type ReleaseTransactionState = {
  schema: typeof RELEASE_TRANSACTION_SCHEMA;
  transactionRef: string;
  mode: ReleaseMode;
  version: string;
  channel: ReleaseChannel;
  sourceRevision: string;
  date: string;
  createdAt: string;
  updatedAt: string;
  steps: Record<ReleaseStepId, ReleaseStepRecord>;
};

export const createTransactionState = (plan: ReleasePlan, now: Date): ReleaseTransactionState => ({
  schema: RELEASE_TRANSACTION_SCHEMA,
  transactionRef: plan.transactionRef,
  mode: plan.mode,
  version: plan.version,
  channel: plan.channel,
  sourceRevision: plan.sourceRevision,
  date: plan.date,
  createdAt: now.toISOString(),
  updatedAt: now.toISOString(),
  steps: Object.fromEntries(
    releaseStepIds.map((id) => [id, { status: "pending", receiptLines: [] }]),
  ) as unknown as Record<ReleaseStepId, ReleaseStepRecord>,
});

export const transactionFilePath = (scratchDir: string, transactionRef: string): string =>
  join(scratchDir, "transactions", `${transactionRef}.json`);

export const saveTransactionState = (
  scratchDir: string,
  state: ReleaseTransactionState,
  now: Date,
): string => {
  state.updatedAt = now.toISOString();
  const filePath = transactionFilePath(scratchDir, state.transactionRef);
  mkdirSync(join(scratchDir, "transactions"), { recursive: true });
  writeFileSync(filePath, `${JSON.stringify(state, null, 2)}\n`);
  return filePath;
};

export const loadTransactionState = (
  scratchDir: string,
  transactionRef: string,
): ReleaseTransactionState => {
  const filePath = transactionFilePath(scratchDir, transactionRef);
  if (!existsSync(filePath)) {
    throw new Error(`no transaction state at ${filePath} — nothing to resume`);
  }
  const state = JSON.parse(readFileSync(filePath, "utf8")) as ReleaseTransactionState;
  if (state.schema !== RELEASE_TRANSACTION_SCHEMA) {
    throw new Error(
      `transaction ${transactionRef} has schema "${state.schema}"; expected ${RELEASE_TRANSACTION_SCHEMA}`,
    );
  }
  for (const id of releaseStepIds) {
    if (state.steps[id] === undefined) {
      throw new Error(`transaction ${transactionRef} is missing step record "${id}"`);
    }
  }
  return state;
};

// ---------------------------------------------------------------------------
// Bounded public-safe receipt lines
// ---------------------------------------------------------------------------

export const RECEIPT_LINE_MAX_LENGTH = 240;
export const RECEIPT_MAX_LINES_PER_STEP = 16;

export const boundReceiptLine = (line: string): string => {
  const flat = line.replace(/\s+/g, " ").trim();
  return flat.length <= RECEIPT_LINE_MAX_LENGTH
    ? flat
    : `${flat.slice(0, RECEIPT_LINE_MAX_LENGTH - 1)}…`;
};

/**
 * Refuse receipt lines that carry secret material: any value of a present
 * signing-credential variable, or private-key markers. Checked before a line
 * enters state, stdout, or a receipt file.
 */
export const assertPublicSafeReceiptLine = (
  line: string,
  env: Readonly<Record<string, string | undefined>>,
): string => {
  if (/PRIVATE KEY|BEGIN (?:RSA|EC|OPENSSH)/i.test(line)) {
    throw new Error("receipt line rejected: private-key material detected");
  }
  const secretNames = [...MACOS_SIGNING_CREDENTIAL_NAMES, ...RELEASE_SET_SIGNING_ENV_NAMES];
  for (const name of secretNames) {
    const value = env[name];
    if (value !== undefined && value.length >= 6 && line.includes(value)) {
      throw new Error(`receipt line rejected: contains the value of ${name}`);
    }
  }
  return line;
};

export const boundReceiptLines = (
  lines: readonly string[],
  env: Readonly<Record<string, string | undefined>>,
): string[] => {
  const bounded = lines
    .slice(0, RECEIPT_MAX_LINES_PER_STEP)
    .map((line) => assertPublicSafeReceiptLine(boundReceiptLine(line), env));
  if (lines.length > RECEIPT_MAX_LINES_PER_STEP) {
    bounded.push(`… ${lines.length - RECEIPT_MAX_LINES_PER_STEP} more lines truncated`);
  }
  return bounded;
};

// ---------------------------------------------------------------------------
// Final release receipt (public-safe)
// ---------------------------------------------------------------------------

export const RELEASE_RECEIPT_SCHEMA = "openagents.desktop.release_receipt.v1";
export const RECEIPTS_DIR = "docs/deploy/receipts";

export type ReleaseReceipt = Readonly<{
  schema: typeof RELEASE_RECEIPT_SCHEMA;
  transactionRef: string;
  mode: ReleaseMode;
  version: string;
  channel: ReleaseChannel;
  sourceRevision: string;
  date: string;
  generatedAt: string;
  steps: ReadonlyArray<
    Readonly<{
      id: ReleaseStepId;
      title: string;
      kind: ReleaseStepKind;
      status: ReleaseStepStatus;
      receiptLines: readonly string[];
    }>
  >;
  gates: ReadonlyArray<
    Readonly<{ id: string; description: string; safeForUnattended: boolean; approved: boolean }>
  >;
}>;

/** ProductSpec §11.1 receipt name: YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md */
export const releaseReceiptFileName = (
  date: string,
  version: string,
  channel: ReleaseChannel,
): string => `${date}-openagents-desktop-v${version}-${channel}.md`;

export const buildReleaseReceipt = (
  state: ReleaseTransactionState,
  plan: ReleasePlan,
  now: Date,
): ReleaseReceipt => ({
  schema: RELEASE_RECEIPT_SCHEMA,
  transactionRef: state.transactionRef,
  mode: state.mode,
  version: state.version,
  channel: state.channel,
  sourceRevision: state.sourceRevision,
  date: state.date,
  generatedAt: now.toISOString(),
  steps: RELEASE_STEP_GRAPH.map((definition) => ({
    id: definition.id,
    title: definition.title,
    kind: definition.kind,
    status: state.steps[definition.id].status,
    receiptLines: state.steps[definition.id].receiptLines,
  })),
  gates: gatesForPlan(plan).map((gate) => ({
    id: gate.id,
    description: gate.description,
    safeForUnattended: gate.safeForUnattended,
    approved: isGateApproved(gate, plan),
  })),
});

export const renderReleaseReceiptMarkdown = (receipt: ReleaseReceipt): string => {
  const lines: string[] = [];
  lines.push(`# OpenAgents Desktop release receipt — v${receipt.version} (${receipt.channel})`);
  lines.push("");
  if (receipt.mode === "dry-run") {
    lines.push("> **DRY-RUN — NOT A RELEASE RECEIPT.** Fixture ports only; no artifacts were");
    lines.push("> built, uploaded, or promoted, and no channel pointer was touched.");
    lines.push("");
  }
  lines.push(`- schema: ${receipt.schema}`);
  lines.push(`- transaction: ${receipt.transactionRef}`);
  lines.push(`- mode: ${receipt.mode}`);
  lines.push(`- version: ${receipt.version}`);
  lines.push(`- channel: ${receipt.channel}`);
  lines.push(`- source-revision: ${receipt.sourceRevision}`);
  lines.push(`- date: ${receipt.date}`);
  lines.push(`- generated-at: ${receipt.generatedAt}`);
  lines.push("");
  lines.push("## Owner gates");
  lines.push("");
  for (const gate of receipt.gates) {
    lines.push(
      `- ${gate.id}: ${gate.approved ? "approved" : "NOT approved"}` +
        `${gate.safeForUnattended ? " (safe for unattended)" : " (owner-explicit)"} — ${gate.description}`,
    );
  }
  lines.push("");
  lines.push("## Steps");
  lines.push("");
  for (const [index, step] of receipt.steps.entries()) {
    lines.push(`### ${index + 1}. ${step.id} — ${step.status} (${step.kind})`);
    lines.push("");
    lines.push(step.title);
    lines.push("");
    for (const receiptLine of step.receiptLines) {
      lines.push(`- ${receiptLine}`);
    }
    lines.push("");
  }
  return `${lines.join("\n").trimEnd()}\n`;
};

// ---------------------------------------------------------------------------
// Step execution
// ---------------------------------------------------------------------------

export type ReleaseIo = Readonly<{
  rootDir: string;
  scratchDir: string;
  log: (line: string) => void;
  env: Readonly<Record<string, string | undefined>>;
  now: () => Date;
}>;

export type ReleaseRunContext = Readonly<{
  plan: ReleasePlan;
  preflightInput: PreflightInput;
  ports: ReleasePorts;
  io: ReleaseIo;
  state: ReleaseTransactionState;
}>;

const executePreflight = async (ctx: ReleaseRunContext): Promise<string[]> => {
  const checks = runPreflightChecks(ctx.preflightInput);
  const refusals = checks.filter((check) => check.severity === "refusal");
  if (refusals.length > 0) throw new ReleasePreflightError(refusals);
  const lines = checks.map(
    (check) => `${check.severity === "ok" ? "ok" : "WARNING"}: ${check.id} — ${check.detail}`,
  );
  const inventory = await callPort(
    ctx.plan,
    ctx.ports.coordinator.kind,
    "ReleaseCoordinatorPort",
    "checkWorkerInventory",
    () => ctx.ports.coordinator.checkWorkerInventory(ctx.plan),
  );
  return [...lines, ...inventory.receiptLines];
};

const executeChangelog = (ctx: ReleaseRunContext): string[] => {
  const { plan, io } = ctx;
  const rollInput = { version: plan.version, channel: plan.channel, date: plan.date };
  if (plan.mode === "dry-run") {
    const unreleasedText = readFileSync(join(io.rootDir, CHANGELOG_DIR, UNRELEASED_FILE), "utf8");
    // Pure roll: refuses when UNRELEASED is empty, writes NOTHING in dry-run.
    const rolled = rollUnreleased({ ...rollInput, unreleasedText });
    return [
      `dry-run: would write ${CHANGELOG_DIR}/${rolled.releaseFileName} and reset ${UNRELEASED_FILE}`,
      `dry-run: bounded release notes ${rolled.releaseNotes.length}/${RELEASE_NOTES_MAX_LENGTH} chars for the signed ReleaseSet payload`,
      "dry-run: docs/changelog was NOT modified",
    ];
  }
  const rolled = runRoll(io.rootDir, rollInput);
  return [
    `wrote ${CHANGELOG_DIR}/${rolled.releaseFileName} and reset ${UNRELEASED_FILE}`,
    `bounded release notes ${rolled.releaseNotes.length}/${RELEASE_NOTES_MAX_LENGTH} chars for the signed ReleaseSet payload`,
    "REVIEW REQUIRED: the human changelog section is a draft (owner gate changelog_human_review)",
  ];
};

const executeFinalReceipt = (ctx: ReleaseRunContext): string[] => {
  const { plan, io, state } = ctx;
  const fileName = releaseReceiptFileName(plan.date, plan.version, plan.channel);
  const targetPath =
    plan.mode === "dry-run"
      ? join(io.scratchDir, "receipts", `DRY-RUN-${fileName}`)
      : join(io.rootDir, RECEIPTS_DIR, fileName);
  // Mark this step succeeded pre-render so the receipt is self-consistent.
  state.steps.final_receipt.status = "succeeded";
  state.steps.final_receipt.receiptLines = [
    `${plan.mode === "dry-run" ? "DRY-RUN receipt" : "receipt"} written to ${targetPath.replace(`${io.rootDir}/`, "")}`,
  ];
  const receipt = buildReleaseReceipt(state, plan, io.now());
  mkdirSync(dirname(targetPath), { recursive: true });
  writeFileSync(targetPath, renderReleaseReceiptMarkdown(receipt));
  return [...state.steps.final_receipt.receiptLines];
};

const stepExecutors: Record<ReleaseStepId, (ctx: ReleaseRunContext) => Promise<string[]>> = {
  preflight: executePreflight,
  worker_bring_up: async (ctx) => [
    ...(
      await callPort(
        ctx.plan,
        ctx.ports.coordinator.kind,
        "ReleaseCoordinatorPort",
        "bringUpWorkers",
        () => ctx.ports.coordinator.bringUpWorkers(ctx.plan),
      )
    ).receiptLines,
  ],
  fan_out: async (ctx) => [
    ...(
      await callPort(
        ctx.plan,
        ctx.ports.coordinator.kind,
        "ReleaseCoordinatorPort",
        "fanOutTargets",
        () => ctx.ports.coordinator.fanOutTargets(ctx.plan),
      )
    ).receiptLines,
  ],
  test_gates: async (ctx) => [
    ...(
      await callPort(
        ctx.plan,
        ctx.ports.coordinator.kind,
        "ReleaseCoordinatorPort",
        "runReleaseGates",
        () => ctx.ports.coordinator.runReleaseGates(ctx.plan),
      )
    ).receiptLines,
  ],
  candidate: async (ctx) => {
    const upload = await callPort(
      ctx.plan,
      ctx.ports.coordinator.kind,
      "ReleaseCoordinatorPort",
      "publishCandidate",
      () => ctx.ports.coordinator.publishCandidate(ctx.plan),
    );
    const feed = await callPort(
      ctx.plan,
      ctx.ports.feed.kind,
      "ReleaseFeedPort",
      "deployCandidateFeed",
      () => ctx.ports.feed.deployCandidateFeed(ctx.plan),
    );
    const smoke = await callPort(
      ctx.plan,
      ctx.ports.feed.kind,
      "ReleaseFeedPort",
      "smokeCandidate",
      () => ctx.ports.feed.smokeCandidate(ctx.plan),
    );
    return [...upload.receiptLines, ...feed.receiptLines, ...smoke.receiptLines];
  },
  changelog: async (ctx) => executeChangelog(ctx),
  promote: async (ctx) => [
    ...(
      await callPort(
        ctx.plan,
        ctx.ports.coordinator.kind,
        "ReleaseCoordinatorPort",
        "promoteChannelPointer",
        () => ctx.ports.coordinator.promoteChannelPointer(ctx.plan),
      )
    ).receiptLines,
  ],
  public_surface_verify: async (ctx) => [
    ...(
      await callPort(ctx.plan, ctx.ports.feed.kind, "ReleaseFeedPort", "verifyPublicSurfaces", () =>
        ctx.ports.feed.verifyPublicSurfaces(ctx.plan),
      )
    ).receiptLines,
  ],
  final_receipt: async (ctx) => executeFinalReceipt(ctx),
};

const enforceGatesBeforeStep = (
  stepId: ReleaseStepId,
  plan: ReleasePlan,
  log: (line: string) => void,
): void => {
  for (const gate of gatesForPlan(plan)) {
    if (gate.beforeStep !== stepId) continue;
    if (plan.mode === "dry-run") {
      log(`  GATE ${gate.id}: dry-run — approval not required (${gate.description})`);
      continue;
    }
    if (isGateApproved(gate, plan)) {
      log(`  GATE ${gate.id}: approved`);
      continue;
    }
    throw new ReleaseGateError(gate);
  }
};

export type ReleaseRunResult = Readonly<{
  state: ReleaseTransactionState;
  receiptPath: string;
}>;

/**
 * Run (or resume) the release transaction. Executes steps in graph order;
 * skips already-succeeded steps idempotently; persists durable state after
 * every transition so any failure is resumable via `--resume`.
 */
export const runRelease = async (options: {
  plan: ReleasePlan;
  preflightInput: PreflightInput;
  ports: ReleasePorts;
  io: ReleaseIo;
  /** Existing state when resuming; a fresh transaction otherwise. */
  resumeState?: ReleaseTransactionState;
}): Promise<ReleaseRunResult> => {
  const { plan, ports, io } = options;
  const state = options.resumeState ?? createTransactionState(plan, io.now());
  saveTransactionState(io.scratchDir, state, io.now());

  io.log(`release transaction ${state.transactionRef} (${plan.mode})`);
  io.log(
    `  version v${plan.version} channel ${plan.channel} source ${plan.sourceRevision.slice(0, 10)}`,
  );
  const gates = gatesForPlan(plan);
  io.log(`  owner gates for this run (named up front, never a silent stall):`);
  for (const gate of gates) {
    io.log(
      `    - ${gate.id} before ${gate.beforeStep}` +
        `${gate.safeForUnattended ? " (safe for --yes)" : " (requires --approve " + gate.id + ")"}` +
        `${isGateApproved(gate, plan) ? " [approved]" : ""}`,
    );
  }

  const ctx: ReleaseRunContext = {
    plan,
    preflightInput: options.preflightInput,
    ports,
    io,
    state,
  };

  for (const [index, definition] of RELEASE_STEP_GRAPH.entries()) {
    const record = state.steps[definition.id];
    const label = `[${index + 1}/${RELEASE_STEP_GRAPH.length}] ${definition.id}`;
    if (record.status === "succeeded") {
      io.log(`${label}: already succeeded — skipped (idempotent resume)`);
      continue;
    }
    for (const dependency of definition.dependsOn) {
      if (state.steps[dependency].status !== "succeeded") {
        throw new Error(
          `step ${definition.id} cannot run: dependency ${dependency} is ${state.steps[dependency].status}`,
        );
      }
    }
    io.log(`${label}: ${definition.title}`);
    record.startedAt = io.now().toISOString();
    delete record.failure;
    try {
      // Gate failure is a recorded, resumable step failure — never a silent
      // stall, and always before the step's effects start.
      enforceGatesBeforeStep(definition.id, plan, io.log);
      // eslint-disable-next-line no-await-in-loop -- the release transaction is strictly sequential by design.
      const lines = boundReceiptLines(await stepExecutors[definition.id](ctx), io.env);
      record.status = "succeeded";
      record.receiptLines = definition.id === "final_receipt" ? record.receiptLines : lines;
      record.finishedAt = io.now().toISOString();
      saveTransactionState(io.scratchDir, state, io.now());
      for (const line of record.receiptLines) io.log(`  ${line}`);
    } catch (error) {
      record.status = "failed";
      record.failure = boundReceiptLine(error instanceof Error ? error.message : String(error));
      record.finishedAt = io.now().toISOString();
      const statePath = saveTransactionState(io.scratchDir, state, io.now());
      io.log(`${label}: FAILED — ${record.failure}`);
      io.log(`  state saved to ${statePath}`);
      io.log(
        `  resume with: pnpm run release -- --resume ${state.transactionRef}${plan.mode === "dry-run" ? " --dry-run" : ""}`,
      );
      io.log("  no channel pointer was touched; the transaction is resumable");
      throw error;
    }
  }

  const receiptFileName = releaseReceiptFileName(plan.date, plan.version, plan.channel);
  const receiptPath =
    plan.mode === "dry-run"
      ? join(io.scratchDir, "receipts", `DRY-RUN-${receiptFileName}`)
      : join(io.rootDir, RECEIPTS_DIR, receiptFileName);
  return { state, receiptPath };
};

// ---------------------------------------------------------------------------
// CLI wrapper — gathers real inputs; fixture ports until #8917/#8922 land.
// ---------------------------------------------------------------------------

const gitOutput = (rootDir: string, ...args: string[]): string =>
  execFileSync("git", args, { cwd: rootDir, encoding: "utf8" });

export const gatherPreflightInput = (options: {
  rootDir: string;
  mode: ReleaseMode;
  allowUnfrozen: boolean;
  cliVersion: string;
  channel: ReleaseChannel;
}): PreflightInput => {
  const desktopManifest = JSON.parse(
    readFileSync(join(options.rootDir, DESKTOP_PACKAGE_JSON_PATH), "utf8"),
  ) as { version?: string };
  const rootManifest = JSON.parse(readFileSync(join(options.rootDir, "package.json"), "utf8")) as {
    packageManager?: string;
  };
  return {
    mode: options.mode,
    allowUnfrozen: options.allowUnfrozen,
    statusPorcelain: gitOutput(options.rootDir, "status", "--porcelain"),
    headSha: gitOutput(options.rootDir, "rev-parse", "HEAD"),
    originMainSha: gitOutput(options.rootDir, "rev-parse", "origin/main"),
    desktopPackageVersion: desktopManifest.version ?? "",
    cliVersion: options.cliVersion,
    channel: options.channel,
    nodeVersion: process.versions.node,
    packageManagerPin: rootManifest.packageManager,
    env: process.env,
  };
};

const argValue = (args: readonly string[], flag: string): string | null => {
  const index = args.indexOf(flag);
  const value = args[index + 1];
  return index === -1 || value === undefined ? null : value;
};

const collectApprovals = (args: readonly string[]): string[] => {
  const approvals: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--approve" && args[index + 1] !== undefined) {
      approvals.push(args[index + 1]!);
    }
  }
  return approvals;
};

const main = async (): Promise<void> => {
  const rootDir = resolve(import.meta.dirname, "..");
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  const unattended = args.includes("--yes");
  const allowUnfrozen = args.includes("--allow-unfrozen");
  const resumeRef = argValue(args, "--resume");
  const mode: ReleaseMode = dryRun ? "dry-run" : "real";
  const scratchDir = join(rootDir, RELEASE_SCRATCH_DIR);
  const now = new Date();
  const io: ReleaseIo = {
    rootDir,
    scratchDir,
    log: (line) => console.log(line),
    env: process.env,
    now: () => new Date(),
  };

  if (allowUnfrozen && !dryRun) {
    throw new Error("--allow-unfrozen is valid only with --dry-run");
  }

  let version: string;
  let channel: ReleaseChannel;
  let transactionRef: string;
  let resumeState: ReleaseTransactionState | undefined;

  if (resumeRef !== null) {
    resumeState = loadTransactionState(scratchDir, resumeRef);
    if (resumeState.mode !== mode) {
      throw new Error(
        `transaction ${resumeRef} was started in ${resumeState.mode} mode; ` +
          `resume it with the same mode`,
      );
    }
    version = resumeState.version;
    channel = resumeState.channel;
    transactionRef = resumeState.transactionRef;
  } else {
    const versionArg = argValue(args, "--version");
    const channelArg = argValue(args, "--channel");
    if (versionArg === null || channelArg === null) {
      throw new Error(
        "usage: pnpm run release -- --channel <stable|rc> --version <semver> " +
          "[--dry-run] [--yes] [--approve <gateId>] [--resume <transaction-ref>] [--allow-unfrozen]",
      );
    }
    if (!(releaseChannels as readonly string[]).includes(channelArg)) {
      throw new Error(`unknown --channel ${channelArg}; expected ${releaseChannels.join("|")}`);
    }
    version = versionArg;
    channel = channelArg as ReleaseChannel;
    transactionRef = newTransactionRef(version, channel, now);
  }

  const plan: ReleasePlan = {
    transactionRef,
    mode,
    version,
    channel,
    sourceRevision: gitOutput(rootDir, "rev-parse", "HEAD").trim(),
    targets: releaseTargetKeys,
    date: now.toISOString().slice(0, 10),
    unattended,
    approvedGates: collectApprovals(args),
  };

  const preflightInput = gatherPreflightInput({
    rootDir,
    mode,
    allowUnfrozen,
    cliVersion: version,
    channel,
  });

  const ports: ReleasePorts = {
    coordinator: createFixtureCoordinatorPort(),
    feed: createFixtureFeedPort(),
  };

  const result = await runRelease({
    plan,
    preflightInput,
    ports,
    io,
    ...(resumeState === undefined ? {} : { resumeState }),
  });
  io.log("");
  io.log(`release transaction ${result.state.transactionRef} complete (${mode})`);
  io.log(`receipt: ${result.receiptPath}`);
};

if (import.meta.main) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
