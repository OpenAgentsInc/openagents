import {
  FORENSIC_COVERAGE_MANIFEST_VERSION,
  FORENSIC_EVIDENCE_RECEIPT_VERSION,
  FORENSIC_SOURCE_BUNDLE_VERSION,
  type ForensicCoverageManifest,
  ForensicCoverageManifestSchema,
  type ForensicEvidenceReceipt,
  ForensicEvidenceReceiptSchema,
  type ForensicSourceBundle,
  ForensicSourceBundleSchema,
  forensicCanonicalJson,
  forensicSha256Digest,
  strictDecode,
} from "@openagentsinc/forensic-contract";
import { Data, Effect, Schema as S } from "effect";
import { gunzipSync } from "node:zlib";

import { generateColdcardBuildConfigV1 } from "./coldcard-build-config-generator.v1";
import type { HttpHeadersDecorator } from "./http/responses";
import {
  BoxV1FacadeError,
  type BoxV1Principal,
  type BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import type { ManagedSandboxBroker } from "./managed-sandbox-broker";

const SourcePath = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(1_024),
  S.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.?\/)(?!.*\/\/)(?!.*\\)[A-Za-z0-9_.@+\/-]+$/u),
);
const Ref = S.String.check(S.isMinLength(3), S.isMaxLength(512));
const Sha256Digest = S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/u));
const CommitSha = S.String.check(S.isPattern(/^[0-9a-f]{40}$/u));
const GeneratedInputKey = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(128),
  S.isPattern(/^[A-Za-z][A-Za-z0-9_.-]*$/u),
);
const GeneratedInputValue = S.String.check(S.isMinLength(1), S.isMaxLength(2_048));
const Timestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0));

export const FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION =
  "openagents.forensic_source_materialization_receipt.v2" as const;
export const FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION =
  "openagents.forensic_source_cleanup_receipt.v2" as const;
export const FORENSIC_SOURCE_AUTHORITY_VERSION = "openagents.forensic_source_authority.v1" as const;
export const FORENSIC_SOURCE_MATERIALIZATION_PATH = "/api/forensics/source-bundles" as const;

export const ForensicGithubRepositorySchema = S.Struct({
  provider: S.Literal("github"),
  owner: S.String.check(S.isPattern(/^[A-Za-z0-9_.-]{1,100}$/u)),
  name: S.String.check(S.isPattern(/^[A-Za-z0-9_.-]{1,100}$/u)),
});
export type ForensicGithubRepository = typeof ForensicGithubRepositorySchema.Type;

export const ForensicToolchainPinSchema = S.Struct({
  toolchainRef: Ref,
  digest: Sha256Digest,
});

export const ForensicSourcePathExpectationSchema = S.Struct({
  bundlePath: SourcePath,
  repositoryPath: SourcePath,
  source: S.Literals(["target", "submodule"]),
  submodulePath: S.optionalKey(SourcePath),
  classification: S.Literals(["target", "dependency"]),
  required: S.Boolean,
  expectedContentDigest: S.optionalKey(Sha256Digest),
  reasonRef: S.optionalKey(Ref),
}).pipe(
  S.check(
    S.makeFilter(
      (expectation) =>
        expectation.source === "target"
          ? expectation.submodulePath === undefined
          : expectation.submodulePath !== undefined,
      {
        message: "target paths cannot name a submodule and submodule paths must name one",
      },
    ),
  ),
);
export type ForensicSourcePathExpectation = typeof ForensicSourcePathExpectationSchema.Type;

export const ForensicSubmodulePlanSchema = S.Union([
  S.Struct({
    path: SourcePath,
    repository: ForensicGithubRepositorySchema,
    disposition: S.Literal("materialize"),
    expectedCommitSha: CommitSha,
    expectedGitTreeSha: CommitSha,
  }),
  S.Struct({
    path: SourcePath,
    repository: ForensicGithubRepositorySchema,
    disposition: S.Literal("exclude"),
    expectedCommitSha: CommitSha,
    expectedGitTreeSha: CommitSha,
    reasonRef: Ref,
  }),
  S.Struct({
    path: SourcePath,
    repository: ForensicGithubRepositorySchema,
    disposition: S.Literal("stale_declaration"),
    reasonRef: Ref,
  }),
]);
export type ForensicSubmodulePlan = typeof ForensicSubmodulePlanSchema.Type;

export const ForensicGeneratedInputPlanSchema = S.Struct({
  path: SourcePath,
  generatorRef: Ref,
  generatorDigest: Sha256Digest,
  input: S.Record(GeneratedInputKey, GeneratedInputValue),
  toolchainPins: S.Array(ForensicToolchainPinSchema).check(S.isMinLength(1), S.isMaxLength(32)),
  expectedContentDigest: Sha256Digest,
  required: S.Boolean,
});
export type ForensicGeneratedInputPlan = typeof ForensicGeneratedInputPlanSchema.Type;

export const ForensicSourceMaterializationRequestSchema = S.Struct({
  targetRef: Ref,
  repositoryRef: Ref,
  repository: ForensicGithubRepositorySchema,
  commitSha: CommitSha,
  expectedGitTreeSha: CommitSha,
  dependencyPolicyRef: Ref,
  authorizationRefs: S.Array(Ref).check(S.isMinLength(1), S.isMaxLength(32)),
  toolchainPins: S.Array(ForensicToolchainPinSchema).check(S.isMaxLength(32)),
  bundleRef: Ref,
  coverageRef: Ref,
  runRef: Ref,
  operationRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  sandboxRef: Ref,
  attachmentGeneration: PositiveInteger,
  resourceGeneration: PositiveInteger,
  capabilityRef: Ref,
  builderRef: Ref,
  submodules: S.Array(ForensicSubmodulePlanSchema).check(S.isMaxLength(128)),
  paths: S.Array(ForensicSourcePathExpectationSchema).check(S.isMinLength(1), S.isMaxLength(4_000)),
  generatedInputs: S.Array(ForensicGeneratedInputPlanSchema).check(S.isMaxLength(256)),
  expectedManifestDigest: Sha256Digest,
  expectedSourceDigest: S.optionalKey(Sha256Digest),
  retentionExpiresAt: Timestamp,
  requestedAt: Timestamp,
});
export type ForensicSourceMaterializationRequest =
  typeof ForensicSourceMaterializationRequestSchema.Type;

const MAX_SELECTED_SOURCE_ENTRIES = 4_000;
const MAX_SELECTED_SOURCE_BYTES = 10 * 1_024 * 1_024;
const MAX_CANONICAL_BUNDLE_BYTES = 16 * 1_024 * 1_024;

export const ForensicSourceMaterializationReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION),
  receiptRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  sandboxRef: Ref,
  attachmentGeneration: PositiveInteger,
  resourceGeneration: PositiveInteger,
  capabilityRef: Ref,
  bundleRef: Ref,
  coverageRef: Ref,
  outcome: S.Literals(["succeeded", "incomplete", "refused"]),
  artifactRef: S.optionalKey(Ref),
  sourceDigest: S.optionalKey(Sha256Digest),
  manifestDigest: Sha256Digest,
  scmReceiptRefs: S.Array(Ref).check(S.isMinLength(1), S.isMaxLength(256)),
  generatedInputReceiptRefs: S.Array(Ref).check(S.isMaxLength(256)),
  artifactWriteReceiptRef: S.optionalKey(Ref),
  artifactReadbackDigest: S.optionalKey(Sha256Digest),
  deliveryReceiptRef: S.optionalKey(Ref),
  postCopyDigest: S.optionalKey(Sha256Digest),
  sourceReadOnly: S.Boolean,
  scratchSeparateAndWritable: S.Boolean,
  guestExternalIpObserved: S.Boolean,
  guestNetworkBytes: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  controlNetworkBytes: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  controlCredentialClass: S.Literals(["none", "owner_github_identity"]),
  scmDirtyState: S.Literal("clean_immutable_archive"),
  credentialMaterializedInGuest: S.Boolean,
  retentionExpiresAt: Timestamp,
  observedAt: Timestamp,
}).pipe(
  S.check(
    S.makeFilter(
      (receipt) =>
        receipt.outcome !== "succeeded" ||
        (receipt.artifactRef !== undefined &&
          receipt.sourceDigest !== undefined &&
          receipt.artifactWriteReceiptRef !== undefined &&
          receipt.artifactReadbackDigest === receipt.sourceDigest &&
          receipt.deliveryReceiptRef !== undefined &&
          receipt.postCopyDigest === receipt.sourceDigest &&
          receipt.sourceReadOnly &&
          receipt.scratchSeparateAndWritable &&
          !receipt.guestExternalIpObserved &&
          receipt.guestNetworkBytes === 0 &&
          !receipt.credentialMaterializedInGuest),
      {
        message: "successful source materialization requires exact artifact and guest proofs",
      },
    ),
  ),
);
export type ForensicSourceMaterializationReceipt =
  typeof ForensicSourceMaterializationReceiptSchema.Type;

export const ForensicSourceAuthoritySchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_AUTHORITY_VERSION),
  authorityRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  runRef: Ref,
  sandboxRef: Ref,
  attachmentGeneration: PositiveInteger,
  resourceGeneration: PositiveInteger,
  capabilityRef: Ref,
  capabilityState: S.Literals(["consumed", "revoked"]),
  bundleRef: Ref,
  coverageRef: Ref,
  coverageDigest: Sha256Digest,
  sourceDigest: S.optionalKey(Sha256Digest),
  artifactRef: S.optionalKey(Ref),
  materializationReceiptRef: Ref,
  status: S.Literals([
    "pending",
    "incomplete",
    "ready",
    "dispatching",
    "cleaning",
    "cleaned",
    "recovery_required",
  ]),
  dispatchLeaseRef: S.optionalKey(Ref),
  dispatchLeaseExpiresAt: S.optionalKey(Timestamp),
  retentionExpiresAt: Timestamp,
  updatedAt: Timestamp,
}).pipe(
  S.check(
    S.makeFilter(
      (authority) =>
        authority.status === "dispatching"
          ? authority.dispatchLeaseRef !== undefined &&
            authority.dispatchLeaseExpiresAt !== undefined
          : authority.dispatchLeaseRef === undefined &&
            authority.dispatchLeaseExpiresAt === undefined,
      { message: "dispatching source authority requires an exact lease" },
    ),
  ),
);
export type ForensicSourceAuthority = typeof ForensicSourceAuthoritySchema.Type;

export const ForensicSourceCleanupReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION),
  cleanupReceiptRef: Ref,
  materializationReceiptRef: Ref,
  authorityRef: Ref,
  artifactRef: S.optionalKey(Ref),
  sandboxRef: Ref,
  resourceGeneration: PositiveInteger,
  scmWorkspaceReleased: S.Boolean,
  artifactDeleted: S.Boolean,
  artifactReadbackAbsent: S.Boolean,
  guestSourceDeleted: S.Boolean,
  guestSourceReadbackAbsent: S.Boolean,
  scratchDeleted: S.Boolean,
  scratchReadbackAbsent: S.Boolean,
  capabilityRevoked: S.Boolean,
  capabilityReadbackRevoked: S.Boolean,
  outcome: S.Literals(["cleaned", "recovery_required"]),
  reason: S.Literals(["explicit", "retention_expired"]),
  observedAt: Timestamp,
});
export type ForensicSourceCleanupReceipt = typeof ForensicSourceCleanupReceiptSchema.Type;

export class ForensicSourceMaterializationError extends Data.TaggedError(
  "ForensicSourceMaterializationError",
)<{
  readonly code:
    | "invalid_request"
    | "source_unavailable"
    | "source_mismatch"
    | "artifact_unavailable"
    | "delivery_failed"
    | "dispatch_refused"
    | "cleanup_incomplete";
  readonly message: string;
  readonly retryable: boolean;
}> {}

type ResolvedFile = Readonly<{
  path: string;
  bytes: Uint8Array;
  contentDigest: string;
}>;
type ResolvedSubmodule = Readonly<{
  path: string;
  url: string;
  gitlinkCommitSha: string | null;
}>;
type ResolvedRepository = Readonly<{
  repository: ForensicGithubRepository;
  commitSha: string;
  gitTreeSha: string;
  files: ReadonlyArray<ResolvedFile>;
  declaredSubmodules: ReadonlyArray<ResolvedSubmodule>;
  receiptRef: string;
  controlNetworkBytes: number;
  controlCredentialClass: "none" | "owner_github_identity";
  workspaceReleased: boolean;
  dirtyState: "clean_immutable_archive";
}>;

export type ForensicScmResolver = Readonly<{
  resolve: (
    input: Readonly<{
      ownerRef: string;
      repository: ForensicGithubRepository;
      commitSha: string;
      expectedGitTreeSha: string;
      authorizationRefs: ReadonlyArray<string>;
      materializeFiles: boolean;
    }>,
  ) => Effect.Effect<ResolvedRepository, ForensicSourceMaterializationError>;
}>;

export type ForensicGeneratedInputResolver = Readonly<{
  generate: (
    input: Readonly<{
      ownerRef: string;
      sourceCommitSha: string;
      plan: ForensicGeneratedInputPlan;
    }>,
  ) => Effect.Effect<
    Readonly<{ bytes: Uint8Array; contentDigest: string; receiptRef: string }>,
    ForensicSourceMaterializationError
  >;
}>;

export type ForensicGeneratedInputRegistration = Readonly<{
  generatorRef: string;
  generatorDigest: string;
  toolchainPins: ReadonlyArray<Readonly<{ toolchainRef: string; digest: string }>>;
  generate: (
    input: Readonly<{
      ownerRef: string;
      sourceCommitSha: string;
      path: string;
      parameters: Readonly<Record<string, string>>;
    }>,
  ) => Effect.Effect<Uint8Array, ForensicSourceMaterializationError>;
}>;

export const makeRegisteredForensicGeneratedInputResolver = (
  registrations: ReadonlyArray<ForensicGeneratedInputRegistration>,
): ForensicGeneratedInputResolver => ({
  generate: ({ ownerRef, sourceCommitSha, plan }) =>
    Effect.gen(function* () {
      const registration = registrations.find(
        (candidate) => candidate.generatorRef === plan.generatorRef,
      );
      if (
        registration === undefined ||
        registration.generatorDigest !== plan.generatorDigest ||
        forensicCanonicalJson(
          [...registration.toolchainPins].sort((left, right) =>
            left.toolchainRef.localeCompare(right.toolchainRef),
          ),
        ) !==
          forensicCanonicalJson(
            [...plan.toolchainPins].sort((left, right) =>
              left.toolchainRef.localeCompare(right.toolchainRef),
            ),
          )
      ) {
        return yield* failure(
          "source_mismatch",
          "generated input has no exact admitted generator and toolchain registration",
        );
      }
      const bytes = yield* registration.generate({
        ownerRef,
        sourceCommitSha,
        path: plan.path,
        parameters: plan.input,
      });
      const contentDigest = yield* Effect.promise(() => forensicSourceBytesDigest(bytes));
      if (contentDigest !== plan.expectedContentDigest) {
        return yield* failure("source_mismatch", "generated input bytes drifted from their pin");
      }
      return {
        bytes,
        contentDigest,
        receiptRef: `receipt.forensic-generated-input.${forensicSha256Digest({ generatorRef: plan.generatorRef, generatorDigest: plan.generatorDigest, toolchainPins: plan.toolchainPins, path: plan.path, contentDigest }).slice(7)}`,
      };
    }),
});

export const COLDCARD_BUILD_CONFIG_GENERATOR_REF = "generator.coldcard.build-config.v1" as const;
export const COLDCARD_BUILD_CONFIG_TOOLCHAIN_PIN = {
  toolchainRef: "toolchain.coldcard.stm32.dockerfile-build",
  digest: "sha256:e1900ca9116bcd97ae95d51c189a5003a22022a16f2aae389096f2a3200eef46",
} as const;
export const COLDCARD_BUILD_CONFIG_GENERATOR_DIGEST =
  "sha256:3b8270cc172286efcdfe05cb86fe71b9c8bde9e61acb48bc6cbcb826a485bc15" as const;

export const coldcardBuildConfigGeneratedInputRegistration: ForensicGeneratedInputRegistration = {
  generatorRef: COLDCARD_BUILD_CONFIG_GENERATOR_REF,
  generatorDigest: COLDCARD_BUILD_CONFIG_GENERATOR_DIGEST,
  toolchainPins: [COLDCARD_BUILD_CONFIG_TOOLCHAIN_PIN],
  generate: ({ sourceCommitSha, path, parameters }) => {
    const bytes = generateColdcardBuildConfigV1({
      sourceCommitSha,
      path,
      parameters,
    });
    if (bytes === undefined) {
      return Effect.fail(
        new ForensicSourceMaterializationError({
          code: "source_mismatch",
          message:
            "Coldcard build configuration inputs do not match the admitted source revision and toolchain",
          retryable: false,
        }),
      );
    }
    return Effect.succeed(bytes);
  },
};

export type ForensicSourceArtifactStore = Readonly<{
  refFor: (input: Readonly<{ instanceRef: string; contentDigest: string }>) => string;
  put: (
    input: Readonly<{
      ownerRef: string;
      instanceRef: string;
      bundleRef: string;
      contentDigest: string;
      bytes: Uint8Array;
      retentionExpiresAt: string;
    }>,
  ) => Effect.Effect<
    Readonly<{ artifactRef: string; receiptRef: string }>,
    ForensicSourceMaterializationError
  >;
  read: (
    input: Readonly<{ ownerRef: string; artifactRef: string }>,
  ) => Effect.Effect<Uint8Array | undefined, ForensicSourceMaterializationError>;
  delete: (
    input: Readonly<{ ownerRef: string; artifactRef: string }>,
  ) => Effect.Effect<boolean, ForensicSourceMaterializationError>;
}>;

export type ForensicSourceAuthorityStore = Readonly<{
  create: (
    authority: ForensicSourceAuthority,
  ) => Effect.Effect<boolean, ForensicSourceMaterializationError>;
  read: (
    input: Readonly<{ ownerRef: string; authorityRef: string }>,
  ) => Effect.Effect<ForensicSourceAuthority | undefined, ForensicSourceMaterializationError>;
  compareAndPut: (
    input: Readonly<{
      expected: ForensicSourceAuthority;
      updated: ForensicSourceAuthority;
    }>,
  ) => Effect.Effect<boolean, ForensicSourceMaterializationError>;
  listExpired: (
    input: Readonly<{ ownerRef: string; observedAt: string }>,
  ) => Effect.Effect<ReadonlyArray<ForensicSourceAuthority>, ForensicSourceMaterializationError>;
}>;

export type ForensicSourceDelivery = Readonly<{
  install: (
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
      sandboxRef: string;
      attachmentGeneration: number;
      resourceGeneration: number;
      capabilityRef: string;
      artifactRef: string;
      artifactBytes: Uint8Array;
      sourceDigest: string;
      retentionExpiresAt: string;
      observedAt: string;
    }>,
  ) => Effect.Effect<
    Readonly<{
      receiptRef: string;
      postCopyDigest: string;
      sourceReadOnly: boolean;
      scratchSeparateAndWritable: boolean;
      guestExternalIpObserved: boolean;
      guestNetworkBytes: number;
      credentialMaterializedInGuest: boolean;
    }>,
    ForensicSourceMaterializationError
  >;
  cleanup: (
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
      sandboxRef: string;
      attachmentGeneration?: number;
      resourceGeneration: number;
      capabilityRef: string;
      sourceDigest: string;
      observedAt: string;
    }>,
  ) => Effect.Effect<
    Readonly<{
      guestSourceDeleted: boolean;
      guestSourceReadbackAbsent: boolean;
      scratchDeleted: boolean;
      scratchReadbackAbsent: boolean;
      capabilityRevoked: boolean;
      capabilityReadbackRevoked: boolean;
    }>,
    ForensicSourceMaterializationError
  >;
  revoke: (
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
      sandboxRef: string;
      attachmentGeneration?: number;
      resourceGeneration: number;
      capabilityRef: string;
      observedAt: string;
    }>,
  ) => Effect.Effect<
    Readonly<{
      capabilityRevoked: boolean;
      capabilityReadbackRevoked: boolean;
    }>,
    ForensicSourceMaterializationError
  >;
}>;

export type ForensicSourceMaterialization = Readonly<{
  bundle: ForensicSourceBundle | null;
  coverage: ForensicCoverageManifest;
  receipt: ForensicSourceMaterializationReceipt;
  authority: ForensicSourceAuthority;
  evidenceReceipt: ForensicEvidenceReceipt;
}>;

const failure = (
  code: ForensicSourceMaterializationError["code"],
  message: string,
  retryable = false,
) => new ForensicSourceMaterializationError({ code, message, retryable });

export const forensicSourceBytesDigest = async (bytes: Uint8Array): Promise<`sha256:${string}`> => {
  const digest = await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer);
  return `sha256:${[...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")}`;
};

export const forensicGitBlobOid = async (bytes: Uint8Array): Promise<string> => {
  const header = new TextEncoder().encode(`blob ${bytes.byteLength}\0`);
  const payload = new Uint8Array(header.byteLength + bytes.byteLength);
  payload.set(header);
  payload.set(bytes, header.byteLength);
  const digest = await crypto.subtle.digest("SHA-1", payload.buffer);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
};

const strict = <A>(schema: S.Decoder<A>, value: unknown, message: string) =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value, { onExcessProperty: "error" }),
    catch: () => failure("invalid_request", message),
  });

const manifestMaterial = (request: ForensicSourceMaterializationRequest) => ({
  targetRef: request.targetRef,
  repositoryRef: request.repositoryRef,
  repository: request.repository,
  commitSha: request.commitSha,
  expectedGitTreeSha: request.expectedGitTreeSha,
  dependencyPolicyRef: request.dependencyPolicyRef,
  toolchainPins: [...request.toolchainPins].sort((left, right) =>
    left.toolchainRef.localeCompare(right.toolchainRef),
  ),
  submodules: [...request.submodules].sort((left, right) => left.path.localeCompare(right.path)),
  paths: [...request.paths].sort((left, right) => left.bundlePath.localeCompare(right.bundlePath)),
  generatedInputs: [...request.generatedInputs].sort((left, right) =>
    left.path.localeCompare(right.path),
  ),
});

export const forensicSourceManifestDigest = (
  request: ForensicSourceMaterializationRequest,
): string => forensicSha256Digest(manifestMaterial(request));

const canonicalBundlePayload = (
  input: Readonly<{
    repositoryRef: string;
    commitSha: string;
    gitTreeSha: string;
    files: ReadonlyArray<ResolvedFile>;
  }>,
) =>
  new TextEncoder().encode(
    forensicCanonicalJson({
      schema: "openagents.forensic_source_bundle_payload.v2",
      repositoryRef: input.repositoryRef,
      commitSha: input.commitSha,
      gitTreeSha: input.gitTreeSha,
      entries: [...input.files]
        .sort((left, right) => left.path.localeCompare(right.path))
        .map((file) => ({
          path: file.path,
          contentDigest: file.contentDigest,
          contentBase64: Buffer.from(file.bytes).toString("base64"),
        })),
    }),
  );

const repositoryUrl = (repository: ForensicGithubRepository): string =>
  `https://github.com/${repository.owner}/${repository.name}.git`;

const parseGitmodules = (
  bytes: Uint8Array,
): ReadonlyArray<Readonly<{ path: string; url: string }>> => {
  const text = new TextDecoder().decode(bytes);
  const modules: Array<{ path: string; url: string }> = [];
  let currentPath: string | undefined;
  let currentUrl: string | undefined;
  const flush = () => {
    if (currentPath !== undefined && currentUrl !== undefined) {
      modules.push({ path: currentPath, url: currentUrl });
    }
    currentPath = undefined;
    currentUrl = undefined;
  };
  for (const line of text.split(/\r?\n/u)) {
    if (/^\s*\[submodule\s+/u.test(line)) {
      flush();
      continue;
    }
    const pathMatch = /^\s*path\s*=\s*(.+?)\s*$/u.exec(line);
    if (pathMatch?.[1] !== undefined) currentPath = pathMatch[1];
    const urlMatch = /^\s*url\s*=\s*(.+?)\s*$/u.exec(line);
    if (urlMatch?.[1] !== undefined) currentUrl = urlMatch[1];
  }
  flush();
  return modules.sort((left, right) => left.path.localeCompare(right.path));
};

const parseTarOctal = (bytes: Uint8Array): number => {
  const value = new TextDecoder().decode(bytes).replace(/\0.*$/u, "").trim();
  const parsed = Number.parseInt(value || "0", 8);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error("invalid tar size");
  return parsed;
};

const tarText = (bytes: Uint8Array): string =>
  new TextDecoder().decode(bytes).replace(/\0.*$/u, "");

const MAX_SCM_JSON_BYTES = 8 * 1_024 * 1_024;
const MAX_SCM_ARCHIVE_BYTES = 32 * 1_024 * 1_024;
const MAX_SCM_EXPANDED_BYTES = 128 * 1_024 * 1_024;
const MAX_SCM_ARCHIVE_ENTRIES = 100_000;
const MAX_SCM_FILE_BYTES = 24 * 1_024 * 1_024;

export const extractGithubTarball = (
  compressed: Uint8Array,
): ReadonlyArray<Readonly<{ path: string; bytes: Uint8Array }>> => {
  if (compressed.byteLength > MAX_SCM_ARCHIVE_BYTES)
    throw new Error("compressed archive exceeds source bound");
  const archive = gunzipSync(compressed, {
    maxOutputLength: MAX_SCM_EXPANDED_BYTES,
  });
  const files: Array<{ path: string; bytes: Uint8Array }> = [];
  let offset = 0;
  let entryCount = 0;
  while (offset + 512 <= archive.byteLength) {
    entryCount += 1;
    if (entryCount > MAX_SCM_ARCHIVE_ENTRIES)
      throw new Error("archive entry count exceeds source bound");
    const header = archive.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const name = tarText(header.subarray(0, 100));
    const prefix = tarText(header.subarray(345, 500));
    const fullName = prefix === "" ? name : `${prefix}/${name}`;
    const size = parseTarOctal(header.subarray(124, 136));
    if (size > MAX_SCM_FILE_BYTES) throw new Error("archive file exceeds source bound");
    const type = String.fromCharCode(header[156] ?? 0);
    const contentStart = offset + 512;
    const contentEnd = contentStart + size;
    if (contentEnd > archive.byteLength) throw new Error("truncated tar entry");
    const slash = fullName.indexOf("/");
    const relative = slash < 0 ? "" : fullName.slice(slash + 1);
    if (
      relative !== "" &&
      (type === "\0" || type === "0") &&
      !relative.startsWith("/") &&
      !relative.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
    ) {
      files.push({
        path: relative,
        bytes: Uint8Array.from(archive.subarray(contentStart, contentEnd)),
      });
    }
    offset = contentStart + Math.ceil(size / 512) * 512;
  }
  return files;
};

export const makeGithubForensicScmResolver = (
  input: Readonly<{
    fetchImpl?: typeof fetch;
    readAccessToken: (ownerRef: string) => Effect.Effect<string | undefined, unknown>;
  }>,
): ForensicScmResolver => ({
  resolve: (request) =>
    Effect.gen(function* () {
      const fetchImpl = input.fetchImpl ?? fetch;
      const token = yield* input
        .readAccessToken(request.ownerRef)
        .pipe(
          Effect.mapError(() =>
            failure("source_unavailable", "SCM authorization is unavailable", true),
          ),
        );
      const headers = new Headers({ accept: "application/vnd.github+json" });
      if (token !== undefined && token.trim() !== "")
        headers.set("authorization", `Bearer ${token}`);
      const apiRoot = `https://api.github.com/repos/${request.repository.owner}/${request.repository.name}`;
      let networkBytes = 0;
      const fetchBytes = (url: string, accept?: string, maxBytes = MAX_SCM_JSON_BYTES) =>
        Effect.tryPromise({
          try: async () => {
            const requestHeaders = new Headers(headers);
            if (accept !== undefined) requestHeaders.set("accept", accept);
            const response = await fetchImpl(url, {
              headers: requestHeaders,
            });
            if (!response.ok) throw new Error(`SCM response ${response.status}`);
            const declaredLength = Number(response.headers.get("content-length"));
            if (Number.isFinite(declaredLength) && declaredLength > maxBytes)
              throw new Error("SCM response exceeds source bound");
            if (response.body === null) throw new Error("SCM response body is unavailable");
            const reader = response.body.getReader();
            const chunks: Uint8Array[] = [];
            let byteLength = 0;
            while (true) {
              const next = await reader.read();
              if (next.done) break;
              byteLength += next.value.byteLength;
              if (byteLength > maxBytes) {
                await reader.cancel();
                throw new Error("SCM response exceeds source bound");
              }
              chunks.push(next.value);
            }
            const bytes = new Uint8Array(byteLength);
            let offset = 0;
            for (const chunk of chunks) {
              bytes.set(chunk, offset);
              offset += chunk.byteLength;
            }
            networkBytes += bytes.byteLength;
            return bytes;
          },
          catch: () => failure("source_unavailable", "pinned SCM source is unavailable", true),
        });
      const commitBytes = yield* fetchBytes(`${apiRoot}/git/commits/${request.commitSha}`);
      const commit = yield* Effect.try({
        try: (): unknown => JSON.parse(new TextDecoder().decode(commitBytes)),
        catch: () => failure("source_mismatch", "SCM commit response is invalid"),
      });
      const observedCommit =
        typeof commit === "object" && commit !== null ? Reflect.get(commit, "sha") : undefined;
      const treeValue =
        typeof commit === "object" && commit !== null ? Reflect.get(commit, "tree") : undefined;
      const observedTree =
        typeof treeValue === "object" && treeValue !== null
          ? Reflect.get(treeValue, "sha")
          : undefined;
      if (observedCommit !== request.commitSha || observedTree !== request.expectedGitTreeSha) {
        return yield* failure(
          "source_mismatch",
          "SCM commit or tree does not match the immutable pin",
        );
      }
      const recursiveTreeBytes = yield* fetchBytes(
        `${apiRoot}/git/trees/${observedTree}?recursive=1`,
      );
      const recursive = yield* Effect.try({
        try: (): unknown => JSON.parse(new TextDecoder().decode(recursiveTreeBytes)),
        catch: () => failure("source_mismatch", "SCM tree response is invalid"),
      });
      if (
        typeof recursive !== "object" ||
        recursive === null ||
        Reflect.get(recursive, "sha") !== observedTree ||
        Reflect.get(recursive, "truncated") === true
      ) {
        return yield* failure("source_mismatch", "SCM tree listing is incomplete");
      }
      const treeEntries = Reflect.get(recursive, "tree");
      if (!Array.isArray(treeEntries))
        return yield* failure("source_mismatch", "SCM tree listing is invalid");
      const gitlinks = new Map<string, string>();
      const blobs = new Map<string, string>();
      for (const entry of treeEntries) {
        if (
          typeof entry === "object" &&
          entry !== null &&
          Reflect.get(entry, "mode") === "160000" &&
          typeof Reflect.get(entry, "path") === "string" &&
          typeof Reflect.get(entry, "sha") === "string"
        ) {
          gitlinks.set(Reflect.get(entry, "path"), Reflect.get(entry, "sha"));
        } else if (
          typeof entry === "object" &&
          entry !== null &&
          Reflect.get(entry, "type") === "blob" &&
          typeof Reflect.get(entry, "path") === "string" &&
          typeof Reflect.get(entry, "sha") === "string"
        ) {
          blobs.set(Reflect.get(entry, "path"), Reflect.get(entry, "sha"));
        }
      }
      const files = request.materializeFiles
        ? yield* Effect.gen(function* () {
            const tarball = yield* fetchBytes(
              `${apiRoot}/tarball/${request.commitSha}`,
              "application/vnd.github+json",
              MAX_SCM_ARCHIVE_BYTES,
            );
            return yield* Effect.tryPromise({
              try: async () =>
                Promise.all(
                  extractGithubTarball(tarball).map(async (file) => {
                    const expectedBlob = blobs.get(file.path);
                    if (
                      expectedBlob === undefined ||
                      (await forensicGitBlobOid(file.bytes)) !== expectedBlob
                    )
                      throw new Error("archive blob does not match Git tree");
                    return {
                      ...file,
                      contentDigest: await forensicSourceBytesDigest(file.bytes),
                    };
                  }),
                ),
              catch: () =>
                failure("source_mismatch", "SCM archive failed deterministic extraction"),
            });
          })
        : [];
      const gitmodules = files.find((file) => file.path === ".gitmodules");
      const declared = gitmodules === undefined ? [] : parseGitmodules(gitmodules.bytes);
      const declaredPaths = new Set(declared.map((module) => module.path));
      if ([...gitlinks.keys()].some((path) => !declaredPaths.has(path))) {
        return yield* failure(
          "source_mismatch",
          "SCM tree contains a gitlink without a .gitmodules declaration",
        );
      }
      return {
        repository: request.repository,
        commitSha: request.commitSha,
        gitTreeSha: observedTree,
        files,
        declaredSubmodules: declared.map((module) => ({
          ...module,
          gitlinkCommitSha: gitlinks.get(module.path) ?? null,
        })),
        receiptRef: `receipt.forensic-scm.${forensicSha256Digest({ repository: request.repository, commitSha: request.commitSha, gitTreeSha: observedTree }).slice(7)}`,
        controlNetworkBytes: networkBytes,
        controlCredentialClass:
          token === undefined || token.trim() === "" ? "none" : "owner_github_identity",
        workspaceReleased: true,
        dirtyState: "clean_immutable_archive",
      };
    }),
});

export const makeForensicSourceMaterializer = (
  input: Readonly<{
    scm: ForensicScmResolver;
    generated: ForensicGeneratedInputResolver;
    artifacts: ForensicSourceArtifactStore;
    authorities: ForensicSourceAuthorityStore;
    delivery: ForensicSourceDelivery;
    now?: () => Date;
  }>,
) => {
  const materialize = (
    rawRequest: unknown,
  ): Effect.Effect<ForensicSourceMaterialization, ForensicSourceMaterializationError> =>
    Effect.gen(function* () {
      const decodedRequest = yield* strict(
        ForensicSourceMaterializationRequestSchema,
        rawRequest,
        "source materialization request failed strict validation",
      );
      const controllerObservedAt = (input.now ?? (() => new Date()))();
      if (!Number.isFinite(controllerObservedAt.getTime()))
        return yield* failure("invalid_request", "controller time is invalid");
      const request = {
        ...decodedRequest,
        requestedAt: controllerObservedAt.toISOString(),
      };
      const manifestDigest = forensicSourceManifestDigest(request);
      if (manifestDigest !== request.expectedManifestDigest) {
        return yield* failure("source_mismatch", "source manifest digest does not match");
      }
      if (Date.parse(request.retentionExpiresAt) <= controllerObservedAt.getTime()) {
        return yield* failure(
          "invalid_request",
          "source retention must expire after materialization",
        );
      }
      const uniqueBundlePaths = new Set<string>();
      for (const path of [
        ...request.paths.map((value) => value.bundlePath),
        ...request.generatedInputs.map((value) => value.path),
      ]) {
        if (uniqueBundlePaths.has(path))
          return yield* failure("source_mismatch", "bundle paths must be unique");
        uniqueBundlePaths.add(path);
      }
      const uniqueRepositoryPaths = new Set<string>();
      for (const path of request.paths) {
        const identity = `${path.source}\0${path.submodulePath ?? ""}\0${path.repositoryPath}`;
        if (uniqueRepositoryPaths.has(identity))
          return yield* failure("source_mismatch", "repository source paths must be unique");
        uniqueRepositoryPaths.add(identity);
      }
      const uniqueSubmodulePaths = new Set<string>();
      for (const plan of request.submodules) {
        if (uniqueSubmodulePaths.has(plan.path))
          return yield* failure("source_mismatch", "submodule plans must be unique");
        uniqueSubmodulePaths.add(plan.path);
      }
      const uniqueToolchainRefs = new Set<string>();
      for (const pin of request.toolchainPins) {
        if (uniqueToolchainRefs.has(pin.toolchainRef)) {
          return yield* failure("source_mismatch", "toolchain pins must be unique");
        }
        uniqueToolchainRefs.add(pin.toolchainRef);
      }
      for (const expectation of request.paths) {
        if (expectation.source !== "submodule") continue;
        const plan = request.submodules.find(
          (candidate) => candidate.path === expectation.submodulePath,
        );
        if (plan === undefined || plan.disposition !== "materialize") {
          return yield* failure(
            "source_mismatch",
            "dependency paths require a materialized pinned submodule",
          );
        }
      }
      const target = yield* input.scm.resolve({
        ownerRef: request.ownerRef,
        repository: request.repository,
        commitSha: request.commitSha,
        expectedGitTreeSha: request.expectedGitTreeSha,
        authorizationRefs: request.authorizationRefs,
        materializeFiles: true,
      });
      const plans = new Map(request.submodules.map((plan) => [plan.path, plan]));
      const declared = new Map(target.declaredSubmodules.map((module) => [module.path, module]));
      if (
        request.submodules.length !== target.declaredSubmodules.length ||
        request.submodules.some((plan) => !declared.has(plan.path))
      ) {
        return yield* failure(
          "source_mismatch",
          "every declared submodule requires an explicit plan",
        );
      }
      const resolvedSubmodules = new Map<string, ResolvedRepository>();
      const scmReceiptRefs = [target.receiptRef];
      let controlNetworkBytes = target.controlNetworkBytes;
      let controlCredentialClass = target.controlCredentialClass;
      let scmWorkspaceReleased = target.workspaceReleased;
      const coverageEntries: Array<{
        path: string;
        classification: "target" | "dependency" | "generated" | "excluded" | "oversized";
        presence: "present" | "absent" | "not_applicable";
        required: boolean;
        contentDigest?: string;
        reasonRef?: string;
      }> = [];
      const incompleteReasonRefs = new Set<string>();
      for (const module of target.declaredSubmodules) {
        const plan = plans.get(module.path);
        if (plan === undefined) {
          return yield* failure("source_mismatch", "declared submodule is missing its plan");
        }
        if (repositoryUrl(plan.repository).toLowerCase() !== module.url.toLowerCase()) {
          return yield* failure(
            "source_mismatch",
            "submodule repository does not match .gitmodules",
          );
        }
        if (plan.disposition === "stale_declaration") {
          if (module.gitlinkCommitSha !== null)
            return yield* failure(
              "source_mismatch",
              "stale submodule declaration unexpectedly has a gitlink",
            );
          coverageEntries.push({
            path: module.path,
            classification: "excluded",
            presence: "not_applicable",
            required: false,
            reasonRef: plan.reasonRef,
          });
          continue;
        }
        if (module.gitlinkCommitSha !== plan.expectedCommitSha)
          return yield* failure("source_mismatch", "submodule gitlink does not match its pin");
        const dependency = yield* input.scm.resolve({
          ownerRef: request.ownerRef,
          repository: plan.repository,
          commitSha: plan.expectedCommitSha,
          expectedGitTreeSha: plan.expectedGitTreeSha,
          authorizationRefs: request.authorizationRefs,
          materializeFiles: plan.disposition === "materialize",
        });
        if (dependency.declaredSubmodules.length > 0) {
          return yield* failure(
            "source_mismatch",
            "nested submodules require an explicit recursive dependency plan",
          );
        }
        scmReceiptRefs.push(dependency.receiptRef);
        controlNetworkBytes += dependency.controlNetworkBytes;
        if (dependency.controlCredentialClass === "owner_github_identity")
          controlCredentialClass = "owner_github_identity";
        scmWorkspaceReleased = scmWorkspaceReleased && dependency.workspaceReleased;
        if (plan.disposition === "exclude") {
          coverageEntries.push({
            path: module.path,
            classification: "excluded",
            presence: "not_applicable",
            required: false,
            reasonRef: plan.reasonRef,
          });
          continue;
        }
        resolvedSubmodules.set(plan.path, dependency);
      }
      const selectedFiles: ResolvedFile[] = [];
      let selectedSourceBytes = 0;
      const addSelectedFile = (selected: ResolvedFile) => {
        selectedSourceBytes += selected.bytes.byteLength;
        if (
          selectedFiles.length >= MAX_SELECTED_SOURCE_ENTRIES ||
          selectedSourceBytes > MAX_SELECTED_SOURCE_BYTES
        )
          return false;
        selectedFiles.push(selected);
        return true;
      };
      for (const expectation of request.paths) {
        const repository =
          expectation.source === "target"
            ? target
            : expectation.submodulePath === undefined
              ? undefined
              : resolvedSubmodules.get(expectation.submodulePath);
        const file = repository?.files.find(
          (candidate) => candidate.path === expectation.repositoryPath,
        );
        if (file === undefined) {
          const reasonRef = expectation.reasonRef ?? "reason.forensic_source.required_input_absent";
          coverageEntries.push({
            path: expectation.bundlePath,
            classification: expectation.classification,
            presence: "absent",
            required: expectation.required,
            reasonRef,
          });
          if (expectation.required) incompleteReasonRefs.add(reasonRef);
          continue;
        }
        if (
          expectation.expectedContentDigest !== undefined &&
          expectation.expectedContentDigest !== file.contentDigest
        ) {
          return yield* failure(
            "source_mismatch",
            "resolved source bytes do not match the expected digest",
          );
        }
        const selected = {
          path: expectation.bundlePath,
          bytes: file.bytes,
          contentDigest: file.contentDigest,
        };
        if (!addSelectedFile(selected))
          return yield* failure(
            "source_mismatch",
            "selected source exceeds aggregate entry or byte bounds",
          );
        coverageEntries.push({
          path: selected.path,
          classification: expectation.classification,
          presence: "present",
          required: expectation.required,
          contentDigest: selected.contentDigest,
        });
      }
      const generatedInputReceiptRefs: string[] = [];
      for (const plan of request.generatedInputs) {
        const observed = yield* input.generated.generate({
          ownerRef: request.ownerRef,
          sourceCommitSha: request.commitSha,
          plan,
        });
        if (
          observed.contentDigest !== plan.expectedContentDigest ||
          observed.contentDigest !==
            (yield* Effect.promise(() => forensicSourceBytesDigest(observed.bytes)))
        ) {
          return yield* failure(
            "source_mismatch",
            "generated input bytes do not match generator/toolchain pin",
          );
        }
        if (
          !addSelectedFile({
            path: plan.path,
            bytes: observed.bytes,
            contentDigest: observed.contentDigest,
          })
        )
          return yield* failure(
            "source_mismatch",
            "selected source exceeds aggregate entry or byte bounds",
          );
        generatedInputReceiptRefs.push(observed.receiptRef);
        coverageEntries.push({
          path: plan.path,
          classification: "generated",
          presence: "present",
          required: plan.required,
          contentDigest: observed.contentDigest,
        });
      }
      if (!scmWorkspaceReleased) {
        return yield* failure("source_mismatch", "SCM resolver did not release its workspace");
      }
      const coverage = strictDecode(ForensicCoverageManifestSchema, {
        schema: FORENSIC_COVERAGE_MANIFEST_VERSION,
        coverageRef: request.coverageRef,
        bundleRef: request.bundleRef,
        status: incompleteReasonRefs.size === 0 ? "complete" : "incomplete",
        entries: coverageEntries.sort((left, right) => left.path.localeCompare(right.path)),
        incompleteReasonRefs: [...incompleteReasonRefs].sort(),
        generatedAt: request.requestedAt,
      });
      const authorityRef = `authority.forensic-source.${forensicSha256Digest({ ownerRef: request.ownerRef, sandboxRef: request.sandboxRef, resourceGeneration: request.resourceGeneration, bundleRef: request.bundleRef }).slice(7)}`;
      const coverageDigest = forensicSha256Digest(coverage);
      const receiptBase = {
        ownerRef: request.ownerRef,
        tenantRef: request.tenantRef,
        workUnitRef: request.workUnitRef,
        sandboxRef: request.sandboxRef,
        attachmentGeneration: request.attachmentGeneration,
        resourceGeneration: request.resourceGeneration,
        capabilityRef: request.capabilityRef,
        bundleRef: request.bundleRef,
        coverageRef: request.coverageRef,
        manifestDigest,
        scmReceiptRefs,
        generatedInputReceiptRefs,
        controlNetworkBytes,
        controlCredentialClass,
        scmDirtyState: target.dirtyState,
        retentionExpiresAt: request.retentionExpiresAt,
        observedAt: request.requestedAt,
      };
      if (coverage.status !== "complete") {
        const receipt = strictDecode(ForensicSourceMaterializationReceiptSchema, {
          schema: FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION,
          receiptRef: `receipt.forensic-source.${forensicSha256Digest({ ...receiptBase, outcome: "incomplete" }).slice(7)}`,
          ...receiptBase,
          outcome: "incomplete",
          sourceReadOnly: false,
          scratchSeparateAndWritable: false,
          guestExternalIpObserved: false,
          guestNetworkBytes: 0,
          credentialMaterializedInGuest: false,
        });
        const authority = strictDecode(ForensicSourceAuthoritySchema, {
          schema: FORENSIC_SOURCE_AUTHORITY_VERSION,
          authorityRef,
          ownerRef: request.ownerRef,
          tenantRef: request.tenantRef,
          workUnitRef: request.workUnitRef,
          runRef: request.runRef,
          sandboxRef: request.sandboxRef,
          attachmentGeneration: request.attachmentGeneration,
          resourceGeneration: request.resourceGeneration,
          capabilityRef: request.capabilityRef,
          capabilityState: "consumed",
          bundleRef: request.bundleRef,
          coverageRef: request.coverageRef,
          coverageDigest,
          materializationReceiptRef: receipt.receiptRef,
          status: "incomplete",
          retentionExpiresAt: request.retentionExpiresAt,
          updatedAt: request.requestedAt,
        });
        if (!(yield* input.authorities.create(authority)))
          return yield* failure(
            "artifact_unavailable",
            "source authority already exists for this materialization identity",
            true,
          );
        const authorityReadback = yield* input.authorities.read({
          ownerRef: request.ownerRef,
          authorityRef,
        });
        if (
          authorityReadback === undefined ||
          forensicCanonicalJson(authorityReadback) !== forensicCanonicalJson(authority)
        )
          return yield* failure(
            "artifact_unavailable",
            "incomplete source authority readback drifted",
            true,
          );
        const resultDigest = forensicSha256Digest({
          coverage,
          receipt,
          authority,
        });
        return {
          bundle: null,
          coverage,
          receipt,
          authority,
          evidenceReceipt: strictDecode(ForensicEvidenceReceiptSchema, {
            schema: FORENSIC_EVIDENCE_RECEIPT_VERSION,
            receiptRef: `receipt.evidence.${resultDigest.slice(7)}`,
            runRef: request.runRef,
            operationRef: request.operationRef,
            commandDigest: forensicSha256Digest(request),
            inputDigests: [manifestDigest],
            outcome: "inconclusive",
            resultDigest,
            artifactDigests: [],
            environmentDigest: forensicSha256Digest(receiptBase),
            evidenceRefs: [coverage.coverageRef, receipt.receiptRef, authority.authorityRef],
            observedAt: request.requestedAt,
          }),
        };
      }
      const payloadBytes = canonicalBundlePayload({
        repositoryRef: request.repositoryRef,
        commitSha: request.commitSha,
        gitTreeSha: request.expectedGitTreeSha,
        files: selectedFiles,
      });
      if (payloadBytes.byteLength > MAX_CANONICAL_BUNDLE_BYTES)
        return yield* failure(
          "source_mismatch",
          "canonical source bundle exceeds its aggregate byte bound",
        );
      const sourceDigest = yield* Effect.promise(() => forensicSourceBytesDigest(payloadBytes));
      if (
        request.expectedSourceDigest !== undefined &&
        request.expectedSourceDigest !== sourceDigest
      ) {
        return yield* failure("source_mismatch", "materialized source bundle bytes drifted");
      }
      const prospectiveReceiptRef = `receipt.forensic-source.${forensicSha256Digest({ ...receiptBase, sourceDigest }).slice(7)}`;
      const pendingAuthority = strictDecode(ForensicSourceAuthoritySchema, {
        schema: FORENSIC_SOURCE_AUTHORITY_VERSION,
        authorityRef,
        ownerRef: request.ownerRef,
        tenantRef: request.tenantRef,
        workUnitRef: request.workUnitRef,
        runRef: request.runRef,
        sandboxRef: request.sandboxRef,
        attachmentGeneration: request.attachmentGeneration,
        resourceGeneration: request.resourceGeneration,
        capabilityRef: request.capabilityRef,
        capabilityState: "consumed",
        bundleRef: request.bundleRef,
        coverageRef: request.coverageRef,
        coverageDigest,
        sourceDigest,
        materializationReceiptRef: prospectiveReceiptRef,
        status: "pending",
        retentionExpiresAt: request.retentionExpiresAt,
        updatedAt: request.requestedAt,
      });
      const artifactRef = input.artifacts.refFor({
        instanceRef: authorityRef,
        contentDigest: sourceDigest,
      });
      const stagedAuthority = strictDecode(ForensicSourceAuthoritySchema, {
        ...pendingAuthority,
        artifactRef,
      });
      yield* input.authorities.create(pendingAuthority);
      const pendingReadback = yield* input.authorities.read({
        ownerRef: request.ownerRef,
        authorityRef,
      });
      const resumesPending =
        pendingReadback !== undefined &&
        forensicCanonicalJson(pendingReadback) === forensicCanonicalJson(pendingAuthority);
      const resumesStaged =
        pendingReadback !== undefined &&
        forensicCanonicalJson(pendingReadback) === forensicCanonicalJson(stagedAuthority);
      if (!resumesPending && !resumesStaged)
        return yield* failure(
          "artifact_unavailable",
          "source authority already exists with a different materialization state",
          true,
        );
      if (
        resumesPending &&
        !(yield* input.authorities.compareAndPut({ expected: pendingAuthority, updated: stagedAuthority }))
      )
        return yield* failure(
          "artifact_unavailable",
          "pending source authority changed before artifact binding",
          true,
        );
      const stagedReadback = yield* input.authorities.read({
        ownerRef: request.ownerRef,
        authorityRef,
      });
      if (
        stagedReadback === undefined ||
        forensicCanonicalJson(stagedReadback) !== forensicCanonicalJson(stagedAuthority)
      ) {
        return yield* failure(
          "artifact_unavailable",
          "staged source authority readback drifted",
          true,
        );
      }
      const stored = yield* input.artifacts.put({
        ownerRef: request.ownerRef,
        instanceRef: authorityRef,
        bundleRef: request.bundleRef,
        contentDigest: sourceDigest,
        bytes: payloadBytes,
        retentionExpiresAt: request.retentionExpiresAt,
      });
      if (stored.artifactRef !== artifactRef) {
        yield* cleanupAuthority(stagedAuthority, request.requestedAt, "explicit", true);
        return yield* failure(
          "artifact_unavailable",
          "source artifact identity drifted from its staged authority",
          true,
        );
      }
      const readback = yield* input.artifacts.read({
        ownerRef: request.ownerRef,
        artifactRef,
      });
      if (
        readback === undefined ||
        (yield* Effect.promise(() => forensicSourceBytesDigest(readback))) !== sourceDigest
      ) {
        yield* cleanupAuthority(stagedAuthority, request.requestedAt, "explicit", true);
        return yield* failure(
          "artifact_unavailable",
          "source artifact readback did not match stored bytes",
          true,
        );
      }
      const delivery = yield* input.delivery
        .install({
          ownerRef: request.ownerRef,
          tenantRef: request.tenantRef,
          workUnitRef: request.workUnitRef,
          sandboxRef: request.sandboxRef,
          attachmentGeneration: request.attachmentGeneration,
          resourceGeneration: request.resourceGeneration,
          capabilityRef: request.capabilityRef,
          artifactRef: stored.artifactRef,
          artifactBytes: readback,
          sourceDigest,
          retentionExpiresAt: request.retentionExpiresAt,
          observedAt: request.requestedAt,
        })
        .pipe(
          Effect.catch((error) =>
            cleanupAuthority(stagedAuthority, request.requestedAt, "explicit", true).pipe(
              Effect.flatMap(() => Effect.fail(error)),
            ),
          ),
        );
      if (
        delivery.postCopyDigest !== sourceDigest ||
        !delivery.sourceReadOnly ||
        !delivery.scratchSeparateAndWritable ||
        delivery.guestExternalIpObserved ||
        delivery.guestNetworkBytes !== 0 ||
        delivery.credentialMaterializedInGuest
      ) {
        yield* cleanupAuthority(stagedAuthority, request.requestedAt, "explicit", true);
        return yield* failure(
          "delivery_failed",
          "guest source delivery did not prove the exact isolated read-only bundle",
        );
      }
      const receipt = strictDecode(ForensicSourceMaterializationReceiptSchema, {
        schema: FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION,
        receiptRef: prospectiveReceiptRef,
        ...receiptBase,
        outcome: "succeeded",
        artifactRef: stored.artifactRef,
        sourceDigest,
        artifactWriteReceiptRef: stored.receiptRef,
        artifactReadbackDigest: sourceDigest,
        deliveryReceiptRef: delivery.receiptRef,
        postCopyDigest: delivery.postCopyDigest,
        sourceReadOnly: delivery.sourceReadOnly,
        scratchSeparateAndWritable: delivery.scratchSeparateAndWritable,
        guestExternalIpObserved: delivery.guestExternalIpObserved,
        guestNetworkBytes: delivery.guestNetworkBytes,
        credentialMaterializedInGuest: delivery.credentialMaterializedInGuest,
      });
      const declaredSubmodules = yield* Effect.forEach(
        [...resolvedSubmodules.entries()],
        ([path, repository]) =>
          Effect.gen(function* () {
            const plan = plans.get(path);
            if (plan === undefined || plan.disposition !== "materialize") {
              return yield* failure(
                "source_mismatch",
                "resolved submodule is missing its materialization plan",
              );
            }
            const digest = yield* Effect.promise(() =>
              forensicSourceBytesDigest(
                canonicalBundlePayload({
                  repositoryRef: `${plan.repository.owner}/${plan.repository.name}`,
                  commitSha: plan.expectedCommitSha,
                  gitTreeSha: plan.expectedGitTreeSha,
                  files: repository.files,
                }),
              ),
            );
            return {
              path,
              commitSha: plan.expectedCommitSha,
              treeDigest: digest,
            };
          }),
      );
      const bundle = strictDecode(ForensicSourceBundleSchema, {
        schema: FORENSIC_SOURCE_BUNDLE_VERSION,
        bundleRef: request.bundleRef,
        targetRef: request.targetRef,
        repositoryRef: request.repositoryRef,
        commitSha: request.commitSha,
        treeDigest: yield* Effect.promise(() =>
          forensicSourceBytesDigest(
            canonicalBundlePayload({
              repositoryRef: request.repositoryRef,
              commitSha: request.commitSha,
              gitTreeSha: request.expectedGitTreeSha,
              files: target.files,
            }),
          ),
        ),
        sourceDigest,
        declaredSubmodules,
        dependencyManifestDigest: manifestDigest,
        artifactRef: stored.artifactRef,
        builderRef: request.builderRef,
        retentionExpiresAt: request.retentionExpiresAt,
        materializationReceiptRef: receipt.receiptRef,
        createdAt: request.requestedAt,
      });
      const authority = strictDecode(ForensicSourceAuthoritySchema, {
        schema: FORENSIC_SOURCE_AUTHORITY_VERSION,
        authorityRef,
        ownerRef: request.ownerRef,
        tenantRef: request.tenantRef,
        workUnitRef: request.workUnitRef,
        runRef: request.runRef,
        sandboxRef: request.sandboxRef,
        attachmentGeneration: request.attachmentGeneration,
        resourceGeneration: request.resourceGeneration,
        capabilityRef: request.capabilityRef,
        capabilityState: "consumed",
        bundleRef: request.bundleRef,
        coverageRef: request.coverageRef,
        coverageDigest,
        sourceDigest,
        artifactRef: stored.artifactRef,
        materializationReceiptRef: receipt.receiptRef,
        status: "ready",
        retentionExpiresAt: request.retentionExpiresAt,
        updatedAt: request.requestedAt,
      });
      if (
        !(yield* input.authorities.compareAndPut({
          expected: stagedAuthority,
          updated: authority,
        }))
      ) {
        return yield* failure(
          "artifact_unavailable",
          "staged source authority changed before ready transition",
          true,
        );
      }
      const authorityReadback = yield* input.authorities.read({
        ownerRef: request.ownerRef,
        authorityRef,
      });
      if (
        authorityReadback === undefined ||
        forensicCanonicalJson(authorityReadback) !== forensicCanonicalJson(authority)
      ) {
        yield* cleanupAuthority(authority, request.requestedAt, "explicit", true);
        return yield* failure(
          "artifact_unavailable",
          "durable source authority readback drifted",
          true,
        );
      }
      const resultDigest = forensicSha256Digest({
        bundle,
        coverage,
        receipt,
        authority,
      });
      return {
        bundle,
        coverage,
        receipt,
        authority,
        evidenceReceipt: strictDecode(ForensicEvidenceReceiptSchema, {
          schema: FORENSIC_EVIDENCE_RECEIPT_VERSION,
          receiptRef: `receipt.evidence.${resultDigest.slice(7)}`,
          runRef: request.runRef,
          operationRef: request.operationRef,
          commandDigest: forensicSha256Digest(request),
          inputDigests: [manifestDigest, sourceDigest],
          outcome: "succeeded",
          resultDigest,
          artifactDigests: [sourceDigest],
          environmentDigest: forensicSha256Digest(receiptBase),
          evidenceRefs: [
            bundle.bundleRef,
            coverage.coverageRef,
            receipt.receiptRef,
            authority.authorityRef,
          ],
          observedAt: request.requestedAt,
        }),
      };
    });

  const cleanupAuthority = (
    authority: ForensicSourceAuthority,
    observedAt: string,
    reason: "explicit" | "retention_expired",
    allowPendingCleanup = false,
  ) =>
    Effect.gen(function* () {
      if (
        authority.status === "cleaned" ||
        ![
          "pending",
          "incomplete",
          "ready",
          "dispatching",
          "cleaning",
          "recovery_required",
        ].includes(authority.status) ||
        (authority.status === "dispatching" &&
          authority.dispatchLeaseExpiresAt !== undefined &&
          Date.parse(authority.dispatchLeaseExpiresAt) > Date.parse(observedAt)) ||
        (authority.status === "cleaning" &&
          Date.parse(authority.updatedAt) + 5 * 60_000 > Date.parse(observedAt)) ||
        (authority.status === "pending" &&
          !allowPendingCleanup &&
          Date.parse(authority.updatedAt) + 5 * 60_000 > Date.parse(observedAt))
      ) {
        return yield* failure("cleanup_incomplete", "source authority is not eligible for cleanup");
      }
      const {
        dispatchLeaseRef: _dispatchLeaseRef,
        dispatchLeaseExpiresAt: _dispatchLeaseExpiresAt,
        ...unleasedAuthority
      } = authority;
      const cleaningAuthority = strictDecode(ForensicSourceAuthoritySchema, {
        ...unleasedAuthority,
        status: "cleaning",
        updatedAt: observedAt,
      });
      if (
        !(yield* input.authorities.compareAndPut({
          expected: authority,
          updated: cleaningAuthority,
        }))
      )
        return yield* failure(
          "cleanup_incomplete",
          "source authority changed before cleanup lease acquisition",
          true,
        );
      const artifactDeleted =
        authority.artifactRef === undefined
          ? true
          : yield* input.artifacts
              .delete({
                ownerRef: authority.ownerRef,
                artifactRef: authority.artifactRef,
              })
              .pipe(
                Effect.catch(() =>
                  Effect.logWarning("forensic source artifact deletion failed").pipe(
                    Effect.map(() => false),
                  ),
                ),
              );
      const artifactReadbackAbsent =
        authority.artifactRef === undefined
          ? true
          : yield* input.artifacts
              .read({
                ownerRef: authority.ownerRef,
                artifactRef: authority.artifactRef,
              })
              .pipe(
                Effect.map((value) => value === undefined),
                Effect.catch(() =>
                  Effect.logWarning("forensic source artifact absence readback failed").pipe(
                    Effect.map(() => false),
                  ),
                ),
              );
      const deliveryScope = {
        ownerRef: authority.ownerRef,
        tenantRef: authority.tenantRef,
        workUnitRef: authority.workUnitRef,
        sandboxRef: authority.sandboxRef,
        resourceGeneration: authority.resourceGeneration,
        capabilityRef: authority.capabilityRef,
        observedAt,
      };
      const guest = yield* (
        authority.sourceDigest === undefined
          ? input.delivery.revoke(deliveryScope).pipe(
              Effect.map((revocation) => ({
                guestSourceDeleted: true,
                guestSourceReadbackAbsent: true,
                scratchDeleted: true,
                scratchReadbackAbsent: true,
                ...revocation,
              })),
            )
          : input.delivery.cleanup({
              ...deliveryScope,
              sourceDigest: authority.sourceDigest,
            })
      ).pipe(
        Effect.catch(() =>
          Effect.logWarning("forensic guest cleanup proof failed").pipe(
            Effect.map(() => ({
              guestSourceDeleted: false,
              guestSourceReadbackAbsent: false,
              scratchDeleted: false,
              scratchReadbackAbsent: false,
              capabilityRevoked: false,
              capabilityReadbackRevoked: false,
            })),
          ),
        ),
      );
      const cleaned =
        artifactDeleted && artifactReadbackAbsent && Object.values(guest).every(Boolean);
      const updated = strictDecode(ForensicSourceAuthoritySchema, {
        ...cleaningAuthority,
        capabilityState: guest.capabilityReadbackRevoked ? "revoked" : authority.capabilityState,
        status: cleaned ? "cleaned" : "recovery_required",
        updatedAt: observedAt,
      });
      const updatedStored = yield* input.authorities.compareAndPut({
        expected: cleaningAuthority,
        updated,
      });
      const readback = yield* input.authorities.read({
        ownerRef: authority.ownerRef,
        authorityRef: authority.authorityRef,
      });
      const authorityReadbackMatches =
        updatedStored &&
        readback !== undefined &&
        readback.status === updated.status &&
        readback.capabilityState === updated.capabilityState;
      return strictDecode(ForensicSourceCleanupReceiptSchema, {
        schema: FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION,
        cleanupReceiptRef: `receipt.forensic-source-cleanup.${forensicSha256Digest({ authorityRef: authority.authorityRef, observedAt, reason }).slice(7)}`,
        materializationReceiptRef: authority.materializationReceiptRef,
        authorityRef: authority.authorityRef,
        ...(authority.artifactRef === undefined ? {} : { artifactRef: authority.artifactRef }),
        sandboxRef: authority.sandboxRef,
        resourceGeneration: authority.resourceGeneration,
        scmWorkspaceReleased: true,
        artifactDeleted,
        artifactReadbackAbsent,
        ...guest,
        capabilityReadbackRevoked: guest.capabilityReadbackRevoked && authorityReadbackMatches,
        outcome: cleaned && authorityReadbackMatches ? "cleaned" : "recovery_required",
        reason,
        observedAt,
      });
    });

  const cleanup = (
    inputValue: Readonly<{
      ownerRef: string;
      authorityRef: string;
      observedAt: string;
    }>,
  ) =>
    Effect.gen(function* () {
      const observedAt = (input.now ?? (() => new Date()))().toISOString();
      const authority = yield* input.authorities.read({
        ownerRef: inputValue.ownerRef,
        authorityRef: inputValue.authorityRef,
      });
      if (authority === undefined || authority.ownerRef !== inputValue.ownerRef)
        return yield* failure("cleanup_incomplete", "source authority is unavailable");
      return yield* cleanupAuthority(authority, observedAt, "explicit");
    });

  const sweepExpired = (inputValue: Readonly<{ ownerRef: string; observedAt: string }>) =>
    Effect.gen(function* () {
      const observedAt = (input.now ?? (() => new Date()))().toISOString();
      const expired = yield* input.authorities.listExpired({
        ownerRef: inputValue.ownerRef,
        observedAt,
      });
      return yield* Effect.forEach(expired, (authority) =>
        cleanupAuthority(authority, observedAt, "retention_expired"),
      );
    });

  return { materialize, cleanup, sweepExpired };
};

export type ForensicSourceDispatchBinding = Readonly<{
  ownerRef: string;
  tenantRef: string;
  workUnitRef: string;
  runRef: string;
  sandboxRef: string;
  attachmentGeneration: number;
  resourceGeneration: number;
  authorityRef: string;
  bundleRef: string;
  coverageRef: string;
  coverageDigest: string;
  sourceDigest: string;
  materializationReceiptRef: string;
}>;

export type ForensicSourceDispatchLease = Readonly<{
  expiresAt: string;
  release: Effect.Effect<void, ForensicSourceMaterializationError>;
}>;

export const makeForensicSourceDispatchAuthority = (
  input: Readonly<{
    authorities: ForensicSourceAuthorityStore;
    artifacts: ForensicSourceArtifactStore;
    delivery: ForensicSourceDelivery;
    now?: () => Date;
  }>,
) => ({
  assertReady: (
    binding: ForensicSourceDispatchBinding,
  ): Effect.Effect<ForensicSourceDispatchLease, ForensicSourceMaterializationError> =>
    Effect.gen(function* () {
      const observedAt = (input.now ?? (() => new Date()))();
      const authority = yield* input.authorities.read({
        ownerRef: binding.ownerRef,
        authorityRef: binding.authorityRef,
      });
      if (
        authority === undefined ||
        authority.status !== "ready" ||
        authority.ownerRef !== binding.ownerRef ||
        authority.tenantRef !== binding.tenantRef ||
        authority.workUnitRef !== binding.workUnitRef ||
        authority.runRef !== binding.runRef ||
        authority.sandboxRef !== binding.sandboxRef ||
        authority.attachmentGeneration !== binding.attachmentGeneration ||
        authority.resourceGeneration !== binding.resourceGeneration ||
        authority.bundleRef !== binding.bundleRef ||
        authority.coverageRef !== binding.coverageRef ||
        authority.coverageDigest !== binding.coverageDigest ||
        authority.sourceDigest !== binding.sourceDigest ||
        authority.artifactRef === undefined ||
        authority.materializationReceiptRef !== binding.materializationReceiptRef ||
        Date.parse(authority.retentionExpiresAt) <= observedAt.getTime()
      ) {
        return yield* failure(
          "dispatch_refused",
          "forensic dispatch requires exact durable complete source authority",
        );
      }
      const dispatchLeaseRef = `lease.forensic-source-dispatch.${forensicSha256Digest({ authorityRef: authority.authorityRef, observedAt: observedAt.toISOString() }).slice(7)}`;
      const dispatchLeaseExpiresAt = new Date(
        Math.min(observedAt.getTime() + 30 * 60_000, Date.parse(authority.retentionExpiresAt)),
      ).toISOString();
      const leased = strictDecode(ForensicSourceAuthoritySchema, {
        ...authority,
        status: "dispatching",
        dispatchLeaseRef,
        dispatchLeaseExpiresAt,
        updatedAt: observedAt.toISOString(),
      });
      if (
        !(yield* input.authorities.compareAndPut({
          expected: authority,
          updated: leased,
        }))
      )
        return yield* failure(
          "dispatch_refused",
          "forensic source authority changed before dispatch lease acquisition",
        );
      const release = input.authorities
        .compareAndPut({
          expected: leased,
          updated: strictDecode(ForensicSourceAuthoritySchema, {
            ...authority,
            updatedAt: (input.now ?? (() => new Date()))().toISOString(),
          }),
        })
        .pipe(
          Effect.flatMap((released) =>
            released
              ? Effect.void
              : Effect.fail(
                  failure(
                    "cleanup_incomplete",
                    "forensic source dispatch lease release conflicted",
                    true,
                  ),
                ),
          ),
        );
      const releaseAfterFailure = release.pipe(
        Effect.catch(() =>
          Effect.logError(
            "forensic source dispatch lease release failed during pre-dispatch verification",
          ),
        ),
      );
      const artifactBytes = yield* input.artifacts
        .read({
          ownerRef: authority.ownerRef,
          artifactRef: authority.artifactRef,
        })
        .pipe(Effect.onError(() => releaseAfterFailure));
      if (
        artifactBytes === undefined ||
        (yield* Effect.promise(() => forensicSourceBytesDigest(artifactBytes))) !==
          authority.sourceDigest
      ) {
        yield* release;
        return yield* failure(
          "dispatch_refused",
          "forensic source artifact is absent or drifted before dispatch",
        );
      }
      const verified = yield* input.delivery
        .install({
          ownerRef: authority.ownerRef,
          tenantRef: authority.tenantRef,
          workUnitRef: authority.workUnitRef,
          sandboxRef: authority.sandboxRef,
          attachmentGeneration: authority.attachmentGeneration,
          resourceGeneration: authority.resourceGeneration,
          capabilityRef: authority.capabilityRef,
          artifactRef: authority.artifactRef,
          artifactBytes,
          sourceDigest: authority.sourceDigest,
          retentionExpiresAt: authority.retentionExpiresAt,
          observedAt: observedAt.toISOString(),
        })
        .pipe(Effect.onError(() => releaseAfterFailure));
      if (
        verified.postCopyDigest !== authority.sourceDigest ||
        !verified.sourceReadOnly ||
        !verified.scratchSeparateAndWritable ||
        verified.guestExternalIpObserved ||
        verified.guestNetworkBytes !== 0 ||
        verified.credentialMaterializedInGuest
      ) {
        yield* release;
        return yield* failure(
          "dispatch_refused",
          "forensic source guest readback is not exact before dispatch",
        );
      }
      return { expiresAt: dispatchLeaseExpiresAt, release };
    }),
});

const sourceGuestLimits = (maxArtifactBytes: number) => ({
  workspaceRootRef: "workspace.managed-sandbox",
  maxFileBytes: Math.min(maxArtifactBytes, 24 * 1_024 * 1_024),
  maxArtifactBytes: Math.min(maxArtifactBytes, 24 * 1_024 * 1_024),
  maxOutputBytes: 131_072,
  maxDurationMillis: 120_000,
  maxCpuMillis: 120_000,
  maxProcesses: 1,
  maxNetworkBytes: 0,
  networkPolicyRef: "network-policy.managed-sandbox.deny-all",
});

const deliveryFailure = (message: string, retryable = false) =>
  failure("delivery_failed", message, retryable);

export const makeManagedSandboxForensicSourceDelivery = (
  input: Readonly<{
    broker: ManagedSandboxBroker;
    runtime: BoxV1Runtime;
    principal: BoxV1Principal;
  }>,
): ForensicSourceDelivery => {
  const exactResource = (
    request: Readonly<{
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
      sandboxRef: string;
      attachmentGeneration?: number;
      resourceGeneration: number;
      capabilityRef: string;
      observedAt: string;
      retentionExpiresAt?: string;
    }>,
  ) =>
    Effect.gen(function* () {
      const resources = yield* input.broker
        .list()
        .pipe(
          Effect.mapError(() =>
            deliveryFailure("managed source resource authority is unavailable", true),
          ),
        );
      const resource = resources.find((candidate) => candidate.sandboxRef === request.sandboxRef);
      const capability = resource?.capabilities.find(
        (candidate) => candidate.capabilityRef === request.capabilityRef,
      );
      if (
        resource === undefined ||
        resource.ownerRef !== request.ownerRef ||
        resource.tenantRef !== request.tenantRef ||
        resource.workUnitRef !== request.workUnitRef ||
        (request.attachmentGeneration !== undefined &&
          resource.attachmentGeneration !== request.attachmentGeneration) ||
        resource.resourceGeneration !== request.resourceGeneration ||
        resource.facts.lifecycle !== "ready" ||
        capability === undefined ||
        capability.kind !== "forensic_source_delivery" ||
        capability.state !== "active" ||
        Date.parse(capability.expiresAt) <= Date.parse(request.observedAt)
      ) {
        return yield* deliveryFailure(
          "forensic source delivery requires the exact active durable generation and capability",
        );
      }
      return { resource, capability };
    });

  const install: ForensicSourceDelivery["install"] = (request) =>
    Effect.gen(function* () {
      const { resource, capability } = yield* exactResource({
        ...request,
        observedAt: request.observedAt,
      });
      if (input.runtime.probe === undefined) {
        return yield* deliveryFailure("native sandbox isolation probe is unavailable", true);
      }
      const operationRef = `operation.forensic-source.install.${request.sourceDigest.slice(7, 39)}`;
      const probe = yield* input.runtime
        .probe({
          principal: input.principal,
          resource,
          operationRef: `${operationRef}.probe`,
          idempotencyRef: `${operationRef}.probe.${request.resourceGeneration}`,
        })
        .pipe(
          Effect.mapError(() => deliveryFailure("native sandbox isolation probe failed", true)),
        );
      if (
        probe.action !== "probe" ||
        probe.phase !== "ready" ||
        probe.generation !== resource.resourceGeneration ||
        probe.readinessProof === undefined ||
        !Object.values(probe.readinessProof).every(Boolean)
      ) {
        return yield* deliveryFailure("native sandbox isolation proof is incomplete", true);
      }
      const installed = yield* input.runtime
        .installForensicSource({
          principal: input.principal,
          resource,
          operationRef,
          idempotencyRef: `${operationRef}.${request.resourceGeneration}`,
          capabilityRef: capability.capabilityRef,
          capabilityState: "active",
          capabilityExpiresAt: capability.expiresAt,
          requestedAt: request.observedAt,
          limits: sourceGuestLimits(resource.budget.maxArtifactBytes),
          artifactRef: request.artifactRef,
          artifactBytes: request.artifactBytes,
          sourceDigest: request.sourceDigest,
        })
        .pipe(
          Effect.mapError(() =>
            deliveryFailure("dedicated guest source installation failed", true),
          ),
        );
      if (
        installed.receipt.outcome !== "succeeded" ||
        installed.receipt.networkBytes !== 0 ||
        !installed.receipt.egressDenied ||
        installed.receipt.secretScan !== "clean" ||
        installed.receipt.symlinkTraversal ||
        !installed.receipt.processTerminated ||
        installed.receipt.descendantsRemaining !== 0 ||
        installed.postCopyDigest !== request.sourceDigest ||
        !installed.sourceReadOnly ||
        !installed.sourceReadbackVerified ||
        !installed.scratchSeparateAndWritable
      ) {
        return yield* deliveryFailure("dedicated guest source receipt is incomplete");
      }
      return {
        receiptRef: installed.receipt.receiptRef,
        postCopyDigest: installed.postCopyDigest,
        sourceReadOnly: installed.sourceReadOnly,
        scratchSeparateAndWritable: installed.scratchSeparateAndWritable,
        guestExternalIpObserved: !probe.readinessProof.noExternalIp,
        guestNetworkBytes: installed.receipt.networkBytes,
        credentialMaterializedInGuest: !probe.readinessProof.noGuestServiceAccount,
      };
    });

  const revoke: ForensicSourceDelivery["revoke"] = (request) =>
    Effect.gen(function* () {
      const resources = yield* input.broker
        .list()
        .pipe(
          Effect.mapError(() =>
            deliveryFailure("source capability authority is unavailable", true),
          ),
        );
      const existing = resources.find((candidate) => candidate.sandboxRef === request.sandboxRef);
      const existingCapability = existing?.capabilities.find(
        (candidate) => candidate.capabilityRef === request.capabilityRef,
      );
      if (
        existing?.ownerRef === request.ownerRef &&
        existing.tenantRef === request.tenantRef &&
        existing.workUnitRef === request.workUnitRef &&
        existing.resourceGeneration === request.resourceGeneration &&
        existingCapability?.state === "revoked"
      ) {
        return {
          capabilityRevoked: true,
          capabilityReadbackRevoked: true,
        };
      }
      const { resource, capability } = yield* exactResource(request);
      const operationRef = `operation.forensic-source.revoke.${forensicSha256Digest({ sandboxRef: request.sandboxRef, resourceGeneration: request.resourceGeneration, capabilityRef: request.capabilityRef }).slice(7, 39)}`;
      const updatedCapabilities = resource.capabilities.map((candidate) =>
        candidate.capabilityRef === capability.capabilityRef
          ? { ...candidate, state: "revoked" as const }
          : candidate,
      );
      const revoked = yield* input.broker
        .execute({
          _tag: "Update",
          schema: "openagents.managed_sandbox_command.v1",
          commandRef: operationRef,
          requestedByRef: input.principal.actorRef,
          ownerRef: resource.ownerRef,
          tenantRef: resource.tenantRef,
          idempotencyRef: `${operationRef}.${resource.version}`,
          requestedAt: request.observedAt,
          sandboxRef: resource.sandboxRef,
          expectedVersion: resource.version,
          capabilities: updatedCapabilities,
        })
        .pipe(
          Effect.mapError(() =>
            deliveryFailure("source delivery capability revocation failed", true),
          ),
        );
      const readbackCapability = revoked.resource.capabilities.find(
        (candidate) => candidate.capabilityRef === capability.capabilityRef,
      );
      const revokedExactly =
        revoked.resource.ownerRef === request.ownerRef &&
        revoked.resource.tenantRef === request.tenantRef &&
        revoked.resource.workUnitRef === request.workUnitRef &&
        revoked.resource.resourceGeneration === request.resourceGeneration &&
        readbackCapability?.state === "revoked";
      return {
        capabilityRevoked: revokedExactly,
        capabilityReadbackRevoked: revokedExactly,
      };
    });

  const cleanup: ForensicSourceDelivery["cleanup"] = (request) =>
    Effect.gen(function* () {
      const resources = yield* input.broker
        .list()
        .pipe(
          Effect.mapError(() =>
            deliveryFailure("managed source cleanup authority is unavailable", true),
          ),
        );
      const existing = resources.find((candidate) => candidate.sandboxRef === request.sandboxRef);
      const existingCapability = existing?.capabilities.find(
        (candidate) => candidate.capabilityRef === request.capabilityRef,
      );
      if (
        existing !== undefined &&
        existing.ownerRef === request.ownerRef &&
        existing.tenantRef === request.tenantRef &&
        existing.workUnitRef === request.workUnitRef &&
        existing.resourceGeneration >= request.resourceGeneration &&
        existing.facts.lifecycle === "deleted"
      ) {
        const deleted =
          existing.facts.guestState === "absent" && existing.facts.filesystemState === "deleted";
        const revoked = existingCapability?.state === "revoked";
        return {
          guestSourceDeleted: deleted,
          guestSourceReadbackAbsent: deleted,
          scratchDeleted: deleted,
          scratchReadbackAbsent: deleted,
          capabilityRevoked: revoked,
          capabilityReadbackRevoked: revoked,
        };
      }
      const { resource, capability } = yield* exactResource({
        ...request,
        observedAt: request.observedAt,
      });
      const operationRef = `operation.forensic-source.remove.${request.sourceDigest.slice(7, 39)}`;
      const removed = yield* input.runtime
        .removeForensicSource({
          principal: input.principal,
          resource,
          operationRef,
          idempotencyRef: `${operationRef}.${request.resourceGeneration}`,
          capabilityRef: capability.capabilityRef,
          capabilityState: "active",
          capabilityExpiresAt: capability.expiresAt,
          requestedAt: request.observedAt,
          limits: sourceGuestLimits(resource.budget.maxArtifactBytes),
          expectedSourceDigest: request.sourceDigest,
        })
        .pipe(
          Effect.map((value) => value as typeof value | undefined),
          Effect.catch(() =>
            Effect.logWarning(
              "dedicated guest source cleanup failed before capability revocation",
            ).pipe(Effect.as(undefined)),
          ),
        );
      if (
        removed === undefined ||
        !removed.guestSourceDeleted ||
        !removed.guestSourceReadbackAbsent ||
        !removed.scratchDeleted ||
        !removed.scratchReadbackAbsent
      ) {
        return {
          guestSourceDeleted: removed?.guestSourceDeleted ?? false,
          guestSourceReadbackAbsent: removed?.guestSourceReadbackAbsent ?? false,
          scratchDeleted: removed?.scratchDeleted ?? false,
          scratchReadbackAbsent: removed?.scratchReadbackAbsent ?? false,
          capabilityRevoked: false,
          capabilityReadbackRevoked: false,
        };
      }
      const revoked = yield* revoke(request);
      return {
        guestSourceDeleted: removed.guestSourceDeleted,
        guestSourceReadbackAbsent: removed.guestSourceReadbackAbsent,
        scratchDeleted: removed.scratchDeleted,
        scratchReadbackAbsent: removed.scratchReadbackAbsent,
        ...revoked,
      };
    });

  return { install, cleanup, revoke };
};

const safeObjectSegment = (value: string): string => {
  const readable = value
    .replace(/[^A-Za-z0-9_.-]+/gu, "_")
    .replace(/^_+|_+$/gu, "")
    .slice(0, 120);
  return `${readable || "ref"}-${forensicSha256Digest(value).slice(7, 39)}`;
};
const sourceBundleKey = (ownerRef: string, instanceDigest: string, contentDigest: string): string =>
  `private/forensics/source-bundles/${safeObjectSegment(ownerRef)}/${instanceDigest}/${contentDigest.slice(7)}.json`;
const sourceAuthorityKey = (ownerRef: string, authorityRef: string): string =>
  `private/forensics/source-authorities/${safeObjectSegment(ownerRef)}/${safeObjectSegment(authorityRef)}.json`;

export const makeGcsForensicSourceStores = (
  bucket: R2Bucket,
): Readonly<{
  artifacts: ForensicSourceArtifactStore;
  authorities: ForensicSourceAuthorityStore;
}> => ({
  artifacts: {
    refFor: ({ instanceRef, contentDigest }) =>
      `artifact.forensic-source.${forensicSha256Digest(instanceRef).slice(7)}.${contentDigest.slice(7)}`,
    put: (request) =>
      Effect.tryPromise({
        try: async () => {
          const instanceDigest = forensicSha256Digest(request.instanceRef).slice(7);
          const key = sourceBundleKey(request.ownerRef, instanceDigest, request.contentDigest);
          const existing = await bucket.get(key);
          if (existing !== null) {
            const bytes = new Uint8Array(await existing.arrayBuffer());
            if ((await forensicSourceBytesDigest(bytes)) !== request.contentDigest)
              throw failure("source_mismatch", "existing source artifact bytes drifted");
          } else {
            await bucket.put(key, request.bytes, {
              customMetadata: {
                bundleRef: request.bundleRef,
                contentDigest: request.contentDigest,
                ownerRef: request.ownerRef,
                retentionExpiresAt: request.retentionExpiresAt,
                visibility: "operator_only",
              },
              httpMetadata: {
                cacheControl: "private, no-store",
                contentType: "application/vnd.openagents.forensic-source-bundle+json",
              },
              sha256: request.contentDigest.slice(7),
            });
          }
          return {
            artifactRef: `artifact.forensic-source.${instanceDigest}.${request.contentDigest.slice(7)}`,
            receiptRef: `receipt.forensic-source-artifact.${instanceDigest}.${request.contentDigest.slice(7)}`,
          };
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure(
                "artifact_unavailable",
                "private source artifact storage is unavailable",
                true,
              ),
      }),
    read: (request) =>
      Effect.tryPromise({
        try: async () => {
          const match = /^artifact\.forensic-source\.([0-9a-f]{64})\.([0-9a-f]{64})$/u.exec(
            request.artifactRef,
          );
          if (match?.[1] === undefined || match[2] === undefined)
            throw failure("source_mismatch", "source artifact ref is invalid");
          const object = await bucket.get(
            sourceBundleKey(request.ownerRef, match[1], `sha256:${match[2]}`),
          );
          return object === null ? undefined : new Uint8Array(await object.arrayBuffer());
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure(
                "artifact_unavailable",
                "private source artifact readback is unavailable",
                true,
              ),
      }),
    delete: (request) =>
      Effect.tryPromise({
        try: async () => {
          const match = /^artifact\.forensic-source\.([0-9a-f]{64})\.([0-9a-f]{64})$/u.exec(
            request.artifactRef,
          );
          if (match?.[1] === undefined || match[2] === undefined)
            throw failure("source_mismatch", "source artifact ref is invalid");
          const key = sourceBundleKey(request.ownerRef, match[1], `sha256:${match[2]}`);
          await bucket.delete(key);
          return (await bucket.head(key)) === null;
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure(
                "artifact_unavailable",
                "private source artifact deletion is unavailable",
                true,
              ),
      }),
  },
  authorities: {
    create: (authority) =>
      Effect.tryPromise({
        try: async () => {
          const bytes = new TextEncoder().encode(forensicCanonicalJson(authority));
          const written = await bucket.put(
            sourceAuthorityKey(authority.ownerRef, authority.authorityRef),
            bytes,
            {
              onlyIf: { etagDoesNotMatch: "*" },
              customMetadata: {
                ownerRef: authority.ownerRef,
                status: authority.status,
                retentionExpiresAt: authority.retentionExpiresAt,
                visibility: "operator_only",
              },
              httpMetadata: {
                cacheControl: "private, no-store",
                contentType: "application/vnd.openagents.forensic-source-authority+json",
              },
            },
          );
          return written !== null;
        },
        catch: () =>
          failure("artifact_unavailable", "durable source authority create is unavailable", true),
      }),
    read: (request) =>
      Effect.tryPromise({
        try: async () => {
          const object = await bucket.get(
            sourceAuthorityKey(request.ownerRef, request.authorityRef),
          );
          if (object === null) return undefined;
          return strictDecode(ForensicSourceAuthoritySchema, await object.json());
        },
        catch: () =>
          failure("artifact_unavailable", "durable source authority read is unavailable", true),
      }),
    compareAndPut: (request) =>
      Effect.tryPromise({
        try: async () => {
          const key = sourceAuthorityKey(request.expected.ownerRef, request.expected.authorityRef);
          const current = await bucket.get(key);
          if (current === null) return false;
          const currentAuthority = strictDecode(
            ForensicSourceAuthoritySchema,
            await current.json(),
          );
          if (forensicCanonicalJson(currentAuthority) !== forensicCanonicalJson(request.expected))
            return false;
          const written = await bucket.put(
            key,
            new TextEncoder().encode(forensicCanonicalJson(request.updated)),
            {
              onlyIf: { etagMatches: current.etag },
              customMetadata: {
                ownerRef: request.updated.ownerRef,
                status: request.updated.status,
                retentionExpiresAt: request.updated.retentionExpiresAt,
                visibility: "operator_only",
              },
              httpMetadata: {
                cacheControl: "private, no-store",
                contentType: "application/vnd.openagents.forensic-source-authority+json",
              },
            },
          );
          return written !== null;
        },
        catch: () =>
          failure(
            "artifact_unavailable",
            "durable source authority compare-and-set is unavailable",
            true,
          ),
      }),
    listExpired: (request) =>
      Effect.tryPromise({
        try: async () => {
          const prefix = `private/forensics/source-authorities/${safeObjectSegment(request.ownerRef)}/`;
          const values: ForensicSourceAuthority[] = [];
          let cursor: string | undefined;
          do {
            const page = await bucket.list({
              prefix,
              ...(cursor === undefined ? {} : { cursor }),
            });
            for (const listed of page.objects) {
              const object = await bucket.get(listed.key);
              if (object === null) continue;
              const authority = strictDecode(ForensicSourceAuthoritySchema, await object.json());
              if (
                authority.status !== "cleaned" &&
                Date.parse(authority.retentionExpiresAt) <= Date.parse(request.observedAt)
              )
                values.push(authority);
            }
            cursor = page.truncated ? page.cursor : undefined;
          } while (cursor !== undefined);
          return values;
        },
        catch: () =>
          failure("artifact_unavailable", "expired source authority scan is unavailable", true),
      }),
  },
});

const ForensicSourceRouteRequestSchema = S.TaggedUnion({
  Materialize: { request: ForensicSourceMaterializationRequestSchema },
  Cleanup: { authorityRef: Ref, observedAt: Timestamp },
  SweepExpired: { observedAt: Timestamp },
});

type ForensicSourceAuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ForensicSourceRouteDependencies<Bindings> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    context: ExecutionContext,
  ) => Promise<ForensicSourceAuthenticatedOwner | undefined>;
  enabled: (env: Bindings) => boolean;
  materializer: (
    env: Bindings,
    ownerRef: string,
  ) => Effect.Effect<ReturnType<typeof makeForensicSourceMaterializer>, BoxV1FacadeError>;
}>;

const sourceRouteJson = (
  body: unknown,
  init: ResponseInit = {},
  decorate?: HttpHeadersDecorator,
): Response => {
  const headers = new Headers(init.headers);
  headers.set("content-type", "application/json; charset=utf-8");
  headers.set("cache-control", "no-store");
  decorate?.(headers);
  return new Response(JSON.stringify(body), { ...init, headers });
};

export const makeForensicSourceRoutes = <Bindings>(
  dependencies: ForensicSourceRouteDependencies<Bindings>,
) => ({
  handle: (request: Request, env: Bindings, context: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST")
        return sourceRouteJson({ error: "method_not_allowed" }, { status: 405 });
      const owner = yield* Effect.tryPromise({
        try: () => dependencies.authenticateOwner(request, env, context),
        catch: () =>
          new BoxV1FacadeError({
            code: "authentication_required",
            status: 401,
            message: "forensic source owner authentication is unavailable",
            retryable: false,
          }),
      });
      if (owner === undefined) return sourceRouteJson({ error: "unauthorized" }, { status: 401 });
      if (!dependencies.enabled(env))
        return sourceRouteJson({ error: "runtime_not_admitted" }, { status: 503 });
      const body = yield* Effect.tryPromise({
        try: async () =>
          S.decodeUnknownSync(ForensicSourceRouteRequestSchema)(await request.json(), {
            onExcessProperty: "error",
          }),
        catch: () => failure("invalid_request", "request failed the forensic source schema"),
      });
      if (
        body._tag === "Materialize" &&
        (body.request.ownerRef !== owner.userId || body.request.tenantRef !== owner.userId)
      ) {
        return sourceRouteJson({ error: "owner_scope_mismatch" }, { status: 403 });
      }
      const materializer = yield* dependencies.materializer(env, owner.userId);
      const result =
        body._tag === "Materialize"
          ? yield* materializer.materialize(body.request)
          : body._tag === "Cleanup"
            ? yield* materializer.cleanup({
                ownerRef: owner.userId,
                authorityRef: body.authorityRef,
                observedAt: body.observedAt,
              })
            : yield* materializer.sweepExpired({
                ownerRef: owner.userId,
                observedAt: body.observedAt,
              });
      return sourceRouteJson({ result }, {}, owner.decorateResponseHeaders);
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed(
          sourceRouteJson(
            {
              error: error instanceof ForensicSourceMaterializationError ? error.code : error.code,
              message: error.message,
              retryable: error.retryable,
            },
            {
              status: error instanceof ForensicSourceMaterializationError ? 409 : error.status,
            },
          ),
        ),
      ),
    ),
});
