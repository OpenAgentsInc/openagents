import {
  FORENSIC_COVERAGE_MANIFEST_VERSION,
  FORENSIC_EVIDENCE_RECEIPT_VERSION,
  FORENSIC_SOURCE_BUNDLE_VERSION,
  type ForensicCoverageManifest,
  type ForensicEvidenceReceipt,
  type ForensicSourceBundle,
  ForensicCoverageManifestSchema,
  ForensicEvidenceReceiptSchema,
  ForensicSourceBundleSchema,
  ForensicTargetSnapshotSchema,
  forensicCanonicalJson,
  forensicSha256Digest,
  strictDecode,
} from "@openagentsinc/forensic-contract";
import { Data, Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import type { ManagedSandboxBroker } from "./managed-sandbox-broker";
import {
  type BoxV1Principal,
  type BoxV1Runtime,
  BoxV1FacadeError,
} from "./managed-sandbox-box-v1-routes";

const SourcePath = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(1_024),
  S.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.?\/)(?!.*\/\/)(?!.*\\)[A-Za-z0-9_.@+\/-]+$/u),
);
const Ref = S.String.check(S.isMinLength(1), S.isMaxLength(512));
const Base64Bytes = S.String.check(S.isMaxLength(1_398_104));
const Sha256Digest = S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/u));
const CommitSha = S.String.check(S.isPattern(/^[0-9a-f]{40}$/u));
const Timestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0));

export const FORENSIC_SOURCE_OBJECT_VERSION = "openagents.forensic_source_object.v1" as const;
export const FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION =
  "openagents.forensic_source_materialization_receipt.v1" as const;
export const FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION =
  "openagents.forensic_source_cleanup_receipt.v1" as const;
export const FORENSIC_SOURCE_MATERIALIZATION_PATH = "/api/forensics/source-bundles" as const;

export const ForensicSourceObjectSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_OBJECT_VERSION),
  sourceObjectRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  repositoryRef: Ref,
  commitSha: CommitSha,
  treeDigest: Sha256Digest,
  path: SourcePath,
  classification: S.Literals(["target", "dependency", "generated"]),
  submodulePath: S.optionalKey(SourcePath),
  generatedByRef: S.optionalKey(Ref),
  toolchainRefs: S.Array(Ref).check(S.isMaxLength(128)),
  contentDigest: Sha256Digest,
  contentBase64: Base64Bytes,
});
export type ForensicSourceObject = typeof ForensicSourceObjectSchema.Type;

export const ForensicSourceExpectationSchema = S.Struct({
  path: SourcePath,
  classification: S.Literals(["target", "dependency", "generated", "excluded", "oversized"]),
  required: S.Boolean,
  sourceObjectRef: S.optionalKey(Ref),
  expectedContentDigest: S.optionalKey(Sha256Digest),
  submodulePath: S.optionalKey(SourcePath),
  generatedByRef: S.optionalKey(Ref),
  reasonRef: S.optionalKey(Ref),
});
export type ForensicSourceExpectation = typeof ForensicSourceExpectationSchema.Type;

export const ForensicSourceMaterializationRequestSchema = S.Struct({
  target: ForensicTargetSnapshotSchema,
  bundleRef: Ref,
  coverageRef: Ref,
  runRef: Ref,
  operationRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  sandboxRef: Ref,
  resourceGeneration: PositiveInteger,
  capabilityRef: Ref,
  builderRef: Ref,
  targetTreeDigest: Sha256Digest,
  declaredSubmodules: S.Array(
    S.Struct({ path: SourcePath, commitSha: CommitSha, treeDigest: Sha256Digest }),
  ).check(S.isMaxLength(128)),
  expectations: S.Array(ForensicSourceExpectationSchema).check(
    S.isMinLength(1),
    S.isMaxLength(20_000),
  ),
  expectedDependencyManifestDigest: Sha256Digest,
  expectedSourceDigest: S.optionalKey(Sha256Digest),
  retentionExpiresAt: Timestamp,
  requestedAt: Timestamp,
});
export type ForensicSourceMaterializationRequest =
  typeof ForensicSourceMaterializationRequestSchema.Type;

export const ForensicSourceMaterializationReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION),
  receiptRef: Ref,
  ownerRef: Ref,
  tenantRef: Ref,
  workUnitRef: Ref,
  sandboxRef: Ref,
  resourceGeneration: PositiveInteger,
  capabilityRef: Ref,
  bundleRef: Ref,
  coverageRef: Ref,
  outcome: S.Literals(["succeeded", "incomplete", "refused"]),
  artifactRef: S.optionalKey(Ref),
  sourceDigest: S.optionalKey(Sha256Digest),
  dependencyManifestDigest: Sha256Digest,
  deliveryReceiptRefs: S.Array(Ref).check(S.isMaxLength(20_000)),
  externalIpUsed: S.Literal(false),
  ambientEgressUsed: S.Literal(false),
  scmCredentialMaterialized: S.Literal(false),
  providerCredentialMaterialized: S.Literal(false),
  networkBytes: S.Literal(0),
  retentionExpiresAt: Timestamp,
  observedAt: Timestamp,
});
export type ForensicSourceMaterializationReceipt =
  typeof ForensicSourceMaterializationReceiptSchema.Type;

export const ForensicSourceCleanupReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION),
  cleanupReceiptRef: Ref,
  materializationReceiptRef: Ref,
  artifactRef: Ref,
  sandboxRef: Ref,
  resourceGeneration: PositiveInteger,
  artifactDeleted: S.Boolean,
  guestSourceDeleted: S.Boolean,
  scratchDeleted: S.Boolean,
  grantsRevoked: S.Boolean,
  outcome: S.Literals(["cleaned", "recovery_required"]),
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
    | "cleanup_incomplete";
  readonly message: string;
  readonly retryable: boolean;
}> {}

export type ForensicSourceObjectStore = Readonly<{
  read: (
    input: Readonly<{
      sourceObjectRef: string;
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
    }>,
  ) => Effect.Effect<unknown | undefined, ForensicSourceMaterializationError>;
  put: (input: ForensicSourceObject) => Effect.Effect<void, ForensicSourceMaterializationError>;
}>;

export type ForensicSourceArtifactStore = Readonly<{
  put: (
    input: Readonly<{
      ownerRef: string;
      bundleRef: string;
      contentDigest: string;
      bytes: Uint8Array;
      retentionExpiresAt: string;
    }>,
  ) => Effect.Effect<string, ForensicSourceMaterializationError>;
  delete: (
    input: Readonly<{ ownerRef: string; artifactRef: string }>,
  ) => Effect.Effect<boolean, ForensicSourceMaterializationError>;
}>;

export type ForensicSourceDelivery = Readonly<{
  install: (
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      workUnitRef: string;
      sandboxRef: string;
      resourceGeneration: number;
      capabilityRef: string;
      entries: ReadonlyArray<
        Readonly<{ path: string; contentBase64: string; contentDigest: string }>
      >;
    }>,
  ) => Effect.Effect<ReadonlyArray<string>, ForensicSourceMaterializationError>;
  cleanup: (
    input: Readonly<{
      sandboxRef: string;
      resourceGeneration: number;
      capabilityRef: string;
    }>,
  ) => Effect.Effect<
    Readonly<{ guestSourceDeleted: boolean; scratchDeleted: boolean; grantsRevoked: boolean }>,
    ForensicSourceMaterializationError
  >;
}>;

export type ForensicSourceMaterialization = Readonly<{
  bundle: ForensicSourceBundle | null;
  coverage: ForensicCoverageManifest;
  receipt: ForensicSourceMaterializationReceipt;
  evidenceReceipt: ForensicEvidenceReceipt;
}>;

const failure = (
  code: ForensicSourceMaterializationError["code"],
  message: string,
  retryable = false,
) => new ForensicSourceMaterializationError({ code, message, retryable });

const decodeBase64 = (value: string): Uint8Array => {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
};

const base64ByteLength = (value: string): number =>
  Math.floor((value.length * 3) / 4) - (value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0);

export const forensicSourceBytesDigest = async (bytes: Uint8Array): Promise<`sha256:${string}`> => {
  const digest = await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer);
  return `sha256:${[...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")}`;
};

const strict = <A>(schema: S.Decoder<A>, value: unknown, message: string) =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value, { onExcessProperty: "error" }),
    catch: () => failure("invalid_request", message),
  });

const sameRefs = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean =>
  forensicCanonicalJson([...left].sort()) === forensicCanonicalJson([...right].sort());

const expectationManifest = (
  request: ForensicSourceMaterializationRequest,
): Readonly<Record<string, unknown>> => ({
  targetCommitSha: request.target.commitSha,
  targetTreeDigest: request.targetTreeDigest,
  declaredSubmodules: [...request.declaredSubmodules].sort((left, right) =>
    left.path.localeCompare(right.path),
  ),
  expectations: [...request.expectations].sort((left, right) =>
    left.path.localeCompare(right.path),
  ),
  toolchainRefs: [...request.target.toolchainRefs].sort(),
});

export const forensicDependencyManifestDigest = (
  request: ForensicSourceMaterializationRequest,
): string => forensicSha256Digest(expectationManifest(request));

export const makeForensicSourceMaterializer = (
  input: Readonly<{
    objects: ForensicSourceObjectStore;
    artifacts: ForensicSourceArtifactStore;
    delivery: ForensicSourceDelivery;
  }>,
) => {
  const materialize = (
    rawRequest: unknown,
  ): Effect.Effect<ForensicSourceMaterialization, ForensicSourceMaterializationError> =>
    Effect.gen(function* () {
      const request = yield* strict(
        ForensicSourceMaterializationRequestSchema,
        rawRequest,
        "source materialization request failed strict validation",
      );
      const paths = new Set<string>();
      for (const expectation of request.expectations) {
        if (paths.has(expectation.path)) {
          return yield* failure("source_mismatch", "source expectation paths must be unique");
        }
        paths.add(expectation.path);
      }
      const dependencyManifestDigest = forensicDependencyManifestDigest(request);
      if (dependencyManifestDigest !== request.expectedDependencyManifestDigest) {
        return yield* failure("source_mismatch", "dependency manifest digest does not match");
      }

      const declaredSubmodulePaths = new Set<string>();
      for (const pin of request.declaredSubmodules) {
        if (declaredSubmodulePaths.has(pin.path)) {
          return yield* failure("source_mismatch", "declared submodule paths must be unique");
        }
        declaredSubmodulePaths.add(pin.path);
        const isMaterialized = request.expectations.some(
          (expectation) =>
            expectation.classification === "dependency" &&
            expectation.submodulePath === pin.path &&
            expectation.required,
        );
        if (!isMaterialized) {
          return yield* failure(
            "source_mismatch",
            "every declared submodule requires a materialized dependency object",
          );
        }
      }

      const pinsByPath = new Map(request.declaredSubmodules.map((pin) => [pin.path, pin]));
      const coverageEntries: Array<{
        path: string;
        classification: ForensicSourceExpectation["classification"];
        presence: "present" | "absent" | "not_applicable";
        required: boolean;
        contentDigest?: string;
        reasonRef?: string;
      }> = [];
      const bundleEntries: Array<{
        path: string;
        classification: "target" | "dependency" | "generated";
        contentDigest: string;
        contentBase64: string;
      }> = [];
      const incompleteReasonRefs = new Set<string>();

      for (const expectation of request.expectations) {
        if (["excluded", "oversized"].includes(expectation.classification)) {
          if (expectation.reasonRef === undefined) {
            return yield* failure(
              "source_mismatch",
              "excluded and oversized paths require a reason",
            );
          }
          coverageEntries.push({
            path: expectation.path,
            classification: expectation.classification,
            presence: "not_applicable",
            required: expectation.required,
            reasonRef: expectation.reasonRef,
          });
          if (expectation.required) incompleteReasonRefs.add(expectation.reasonRef);
          continue;
        }
        if (expectation.sourceObjectRef === undefined) {
          const reasonRef = expectation.reasonRef ?? "reason.forensic_source.required_input_absent";
          coverageEntries.push({
            path: expectation.path,
            classification: expectation.classification,
            presence: "absent",
            required: expectation.required,
            reasonRef,
          });
          if (expectation.required) incompleteReasonRefs.add(reasonRef);
          continue;
        }
        const unknownObject = yield* input.objects.read({
          sourceObjectRef: expectation.sourceObjectRef,
          ownerRef: request.ownerRef,
          tenantRef: request.tenantRef,
          workUnitRef: request.workUnitRef,
        });
        if (unknownObject === undefined) {
          const reasonRef = expectation.reasonRef ?? "reason.forensic_source.object_unavailable";
          coverageEntries.push({
            path: expectation.path,
            classification: expectation.classification,
            presence: "absent",
            required: expectation.required,
            reasonRef,
          });
          if (expectation.required) incompleteReasonRefs.add(reasonRef);
          continue;
        }
        const object = yield* strict(
          ForensicSourceObjectSchema,
          unknownObject,
          "source object failed strict validation",
        );
        if (
          object.sourceObjectRef !== expectation.sourceObjectRef ||
          object.ownerRef !== request.ownerRef ||
          object.tenantRef !== request.tenantRef ||
          object.workUnitRef !== request.workUnitRef ||
          object.repositoryRef !== request.target.repositoryRef ||
          object.path !== expectation.path ||
          object.classification !== expectation.classification ||
          (expectation.expectedContentDigest !== undefined &&
            object.contentDigest !== expectation.expectedContentDigest)
        ) {
          return yield* failure("source_mismatch", "source object does not bind its expectation");
        }
        if (object.classification === "target") {
          if (
            object.commitSha !== request.target.commitSha ||
            object.treeDigest !== request.targetTreeDigest
          ) {
            return yield* failure(
              "source_mismatch",
              "target object does not bind the pinned commit and tree",
            );
          }
        } else if (object.classification === "dependency") {
          const submodulePath = expectation.submodulePath;
          const pin = submodulePath === undefined ? undefined : pinsByPath.get(submodulePath);
          if (
            pin === undefined ||
            object.submodulePath !== submodulePath ||
            object.commitSha !== pin.commitSha ||
            object.treeDigest !== pin.treeDigest
          ) {
            return yield* failure(
              "source_mismatch",
              "dependency object does not bind its submodule pin",
            );
          }
        } else if (
          object.commitSha !== request.target.commitSha ||
          object.treeDigest !== request.targetTreeDigest ||
          object.generatedByRef !== expectation.generatedByRef ||
          object.generatedByRef === undefined ||
          !sameRefs(object.toolchainRefs, request.target.toolchainRefs)
        ) {
          return yield* failure(
            "source_mismatch",
            "generated object does not bind its generator and toolchain",
          );
        }
        const bytes = yield* Effect.tryPromise({
          try: async () => decodeBase64(object.contentBase64),
          catch: () => failure("source_mismatch", "source object content is not valid base64"),
        });
        const observedDigest = yield* Effect.tryPromise({
          try: () => forensicSourceBytesDigest(bytes),
          catch: () => failure("source_mismatch", "source object digest could not be computed"),
        });
        if (observedDigest !== object.contentDigest) {
          return yield* failure(
            "source_mismatch",
            "source object bytes do not match content digest",
          );
        }
        coverageEntries.push({
          path: expectation.path,
          classification: expectation.classification,
          presence: "present",
          required: expectation.required,
          contentDigest: object.contentDigest,
        });
        bundleEntries.push({
          path: object.path,
          classification: object.classification,
          contentDigest: object.contentDigest,
          contentBase64: object.contentBase64,
        });
      }

      const coverage = strictDecode(ForensicCoverageManifestSchema, {
        schema: FORENSIC_COVERAGE_MANIFEST_VERSION,
        coverageRef: request.coverageRef,
        bundleRef: request.bundleRef,
        status: incompleteReasonRefs.size === 0 ? "complete" : "incomplete",
        entries: coverageEntries,
        incompleteReasonRefs: [...incompleteReasonRefs].sort(),
        generatedAt: request.requestedAt,
      });
      const receiptMaterial = {
        ownerRef: request.ownerRef,
        tenantRef: request.tenantRef,
        workUnitRef: request.workUnitRef,
        sandboxRef: request.sandboxRef,
        resourceGeneration: request.resourceGeneration,
        capabilityRef: request.capabilityRef,
        bundleRef: request.bundleRef,
        coverageRef: request.coverageRef,
        dependencyManifestDigest,
      };
      if (coverage.status !== "complete") {
        const receipt = strictDecode(ForensicSourceMaterializationReceiptSchema, {
          schema: FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION,
          receiptRef: `receipt.forensic-source.${forensicSha256Digest(receiptMaterial).slice(7)}`,
          ...receiptMaterial,
          outcome: "incomplete",
          deliveryReceiptRefs: [],
          externalIpUsed: false,
          ambientEgressUsed: false,
          scmCredentialMaterialized: false,
          providerCredentialMaterialized: false,
          networkBytes: 0,
          retentionExpiresAt: request.retentionExpiresAt,
          observedAt: request.requestedAt,
        });
        const resultDigest = forensicSha256Digest({ coverage, receipt });
        return {
          bundle: null,
          coverage,
          receipt,
          evidenceReceipt: strictDecode(ForensicEvidenceReceiptSchema, {
            schema: FORENSIC_EVIDENCE_RECEIPT_VERSION,
            receiptRef: `receipt.evidence.${resultDigest.slice(7)}`,
            runRef: request.runRef,
            operationRef: request.operationRef,
            commandDigest: forensicSha256Digest(request),
            inputDigests: [dependencyManifestDigest],
            outcome: "inconclusive",
            resultDigest,
            artifactDigests: [],
            environmentDigest: forensicSha256Digest(receiptMaterial),
            evidenceRefs: [coverage.coverageRef, receipt.receiptRef],
            observedAt: request.requestedAt,
          }),
        };
      }

      const payload = {
        schema: "openagents.forensic_source_bundle_payload.v1",
        repositoryRef: request.target.repositoryRef,
        commitSha: request.target.commitSha,
        treeDigest: request.targetTreeDigest,
        declaredSubmodules: [...request.declaredSubmodules].sort((left, right) =>
          left.path.localeCompare(right.path),
        ),
        entries: bundleEntries.sort((left, right) => left.path.localeCompare(right.path)),
      };
      const sourceDigest = forensicSha256Digest(payload);
      if (
        request.expectedSourceDigest !== undefined &&
        request.expectedSourceDigest !== sourceDigest
      ) {
        return yield* failure(
          "source_mismatch",
          "materialized bundle bytes do not match expected digest",
        );
      }
      const bytes = new TextEncoder().encode(forensicCanonicalJson(payload));
      const artifactRef = yield* input.artifacts.put({
        ownerRef: request.ownerRef,
        bundleRef: request.bundleRef,
        contentDigest: sourceDigest,
        bytes,
        retentionExpiresAt: request.retentionExpiresAt,
      });
      const deleteArtifactAfterDeliveryFailure = (
        deliveryError: ForensicSourceMaterializationError,
      ) =>
        input.artifacts
          .delete({ ownerRef: request.ownerRef, artifactRef })
          .pipe(
            Effect.flatMap((deleted) =>
              Effect.fail(
                deleted
                  ? deliveryError
                  : failure(
                      "cleanup_incomplete",
                      "failed source delivery left an artifact requiring recovery",
                      true,
                    ),
              ),
            ),
          );
      const deliveryReceiptRefs = yield* input.delivery
        .install({
          ownerRef: request.ownerRef,
          tenantRef: request.tenantRef,
          workUnitRef: request.workUnitRef,
          sandboxRef: request.sandboxRef,
          resourceGeneration: request.resourceGeneration,
          capabilityRef: request.capabilityRef,
          entries: bundleEntries,
        })
        .pipe(Effect.catch(deleteArtifactAfterDeliveryFailure));
      if (deliveryReceiptRefs.length !== bundleEntries.length) {
        return yield* deleteArtifactAfterDeliveryFailure(
          failure("delivery_failed", "source delivery did not receipt every entry", true),
        );
      }
      const receipt = strictDecode(ForensicSourceMaterializationReceiptSchema, {
        schema: FORENSIC_SOURCE_MATERIALIZATION_RECEIPT_VERSION,
        receiptRef: `receipt.forensic-source.${forensicSha256Digest({ ...receiptMaterial, sourceDigest }).slice(7)}`,
        ...receiptMaterial,
        outcome: "succeeded",
        artifactRef,
        sourceDigest,
        deliveryReceiptRefs,
        externalIpUsed: false,
        ambientEgressUsed: false,
        scmCredentialMaterialized: false,
        providerCredentialMaterialized: false,
        networkBytes: 0,
        retentionExpiresAt: request.retentionExpiresAt,
        observedAt: request.requestedAt,
      });
      const bundle = strictDecode(ForensicSourceBundleSchema, {
        schema: FORENSIC_SOURCE_BUNDLE_VERSION,
        bundleRef: request.bundleRef,
        targetRef: request.target.targetRef,
        repositoryRef: request.target.repositoryRef,
        commitSha: request.target.commitSha,
        treeDigest: request.targetTreeDigest,
        sourceDigest,
        declaredSubmodules: request.declaredSubmodules,
        dependencyManifestDigest,
        artifactRef,
        builderRef: request.builderRef,
        retentionExpiresAt: request.retentionExpiresAt,
        materializationReceiptRef: receipt.receiptRef,
        createdAt: request.requestedAt,
      });
      const resultDigest = forensicSha256Digest({ bundle, coverage, receipt });
      return {
        bundle,
        coverage,
        receipt,
        evidenceReceipt: strictDecode(ForensicEvidenceReceiptSchema, {
          schema: FORENSIC_EVIDENCE_RECEIPT_VERSION,
          receiptRef: `receipt.evidence.${resultDigest.slice(7)}`,
          runRef: request.runRef,
          operationRef: request.operationRef,
          commandDigest: forensicSha256Digest(request),
          inputDigests: [dependencyManifestDigest, sourceDigest],
          outcome: "succeeded",
          resultDigest,
          artifactDigests: [sourceDigest],
          environmentDigest: forensicSha256Digest(receiptMaterial),
          evidenceRefs: [bundle.bundleRef, coverage.coverageRef, receipt.receiptRef],
          observedAt: request.requestedAt,
        }),
      };
    });

  const stage = (
    rawObject: unknown,
  ): Effect.Effect<ForensicSourceObject, ForensicSourceMaterializationError> =>
    Effect.gen(function* () {
      const object = yield* strict(
        ForensicSourceObjectSchema,
        rawObject,
        "source object failed strict validation",
      );
      const bytes = yield* Effect.try({
        try: () => decodeBase64(object.contentBase64),
        catch: () => failure("source_mismatch", "source object content is not valid base64"),
      });
      const observedDigest = yield* Effect.tryPromise({
        try: () => forensicSourceBytesDigest(bytes),
        catch: () => failure("source_mismatch", "source object digest could not be computed"),
      });
      if (observedDigest !== object.contentDigest || bytes.byteLength > 1_048_576) {
        return yield* failure(
          "source_mismatch",
          "source object bytes do not match the bounded content digest",
        );
      }
      yield* input.objects.put(object);
      return object;
    });

  const cleanup = (
    inputValue: Readonly<{
      receipt: ForensicSourceMaterializationReceipt;
      observedAt: string;
    }>,
  ): Effect.Effect<ForensicSourceCleanupReceipt, ForensicSourceMaterializationError> =>
    Effect.gen(function* () {
      const receipt = yield* strict(
        ForensicSourceMaterializationReceiptSchema,
        inputValue.receipt,
        "cleanup requires a valid source materialization receipt",
      );
      if (receipt.outcome !== "succeeded" || receipt.artifactRef === undefined) {
        return yield* failure(
          "cleanup_incomplete",
          "only a succeeded materialization can be cleaned",
        );
      }
      const artifactDeleted = yield* input.artifacts.delete({
        ownerRef: receipt.ownerRef,
        artifactRef: receipt.artifactRef,
      });
      const guest = yield* input.delivery.cleanup({
        sandboxRef: receipt.sandboxRef,
        resourceGeneration: receipt.resourceGeneration,
        capabilityRef: receipt.capabilityRef,
      });
      const cleaned =
        artifactDeleted && guest.guestSourceDeleted && guest.scratchDeleted && guest.grantsRevoked;
      return strictDecode(ForensicSourceCleanupReceiptSchema, {
        schema: FORENSIC_SOURCE_CLEANUP_RECEIPT_VERSION,
        cleanupReceiptRef: `receipt.forensic-source-cleanup.${forensicSha256Digest({ receiptRef: receipt.receiptRef, observedAt: inputValue.observedAt }).slice(7)}`,
        materializationReceiptRef: receipt.receiptRef,
        artifactRef: receipt.artifactRef,
        sandboxRef: receipt.sandboxRef,
        resourceGeneration: receipt.resourceGeneration,
        artifactDeleted,
        ...guest,
        outcome: cleaned ? "cleaned" : "recovery_required",
        observedAt: inputValue.observedAt,
      });
    });

  return { stage, materialize, cleanup };
};

const safeObjectSegment = (value: string): string => {
  const readable = value
    .replace(/[^A-Za-z0-9_.-]+/gu, "_")
    .replace(/^_+|_+$/gu, "")
    .slice(0, 120);
  return `${readable || "ref"}-${forensicSha256Digest(value).slice(7, 39)}`;
};

const sourceObjectKey = (ownerRef: string, sourceObjectRef: string): string =>
  `private/forensics/source-objects/${safeObjectSegment(ownerRef)}/${safeObjectSegment(sourceObjectRef)}.json`;

const sourceBundleKey = (ownerRef: string, digest: string): string =>
  `private/forensics/source-bundles/${safeObjectSegment(ownerRef)}/${digest.slice(7)}.json`;

export const makeR2ForensicSourceStores = (
  bucket: R2Bucket,
): Readonly<{
  objects: ForensicSourceObjectStore;
  artifacts: ForensicSourceArtifactStore;
}> => ({
  objects: {
    read: (request) =>
      Effect.tryPromise({
        try: async () => {
          const object = await bucket.get(
            sourceObjectKey(request.ownerRef, request.sourceObjectRef),
          );
          if (object === null) return undefined;
          const value: unknown = await object.json();
          if (
            typeof value !== "object" ||
            value === null ||
            Reflect.get(value, "tenantRef") !== request.tenantRef ||
            Reflect.get(value, "workUnitRef") !== request.workUnitRef
          ) {
            throw failure("source_mismatch", "source object is outside the requested scope");
          }
          return value;
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure("source_unavailable", "private source object is unavailable", true),
      }),
    put: (sourceObject) =>
      Effect.tryPromise({
        try: async () => {
          const key = sourceObjectKey(sourceObject.ownerRef, sourceObject.sourceObjectRef);
          const bytes = new TextEncoder().encode(forensicCanonicalJson(sourceObject));
          const existing = await bucket.get(key);
          if (existing !== null) {
            const existingBytes = new Uint8Array(await existing.arrayBuffer());
            if (
              (await forensicSourceBytesDigest(existingBytes)) !==
              (await forensicSourceBytesDigest(bytes))
            ) {
              throw failure("source_mismatch", "existing private source object drifted");
            }
            return;
          }
          await bucket.put(key, bytes, {
            customMetadata: {
              contentDigest: sourceObject.contentDigest,
              ownerRef: sourceObject.ownerRef,
              tenantRef: sourceObject.tenantRef,
              workUnitRef: sourceObject.workUnitRef,
              visibility: "operator_only",
            },
            httpMetadata: {
              cacheControl: "private, no-store",
              contentType: "application/vnd.openagents.forensic-source-object+json",
            },
          });
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure("artifact_unavailable", "private source object storage is unavailable", true),
      }),
  },
  artifacts: {
    put: (request) =>
      Effect.tryPromise({
        try: async () => {
          const key = sourceBundleKey(request.ownerRef, request.contentDigest);
          const existing = await bucket.get(key);
          if (existing !== null) {
            const existingBytes = new Uint8Array(await existing.arrayBuffer());
            if ((await forensicSourceBytesDigest(existingBytes)) !== request.contentDigest) {
              throw failure("source_mismatch", "existing source bundle artifact bytes drifted");
            }
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
          return `artifact.forensic-source.${request.contentDigest.slice(7)}`;
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure("artifact_unavailable", "private source bundle storage is unavailable", true),
      }),
    delete: (request) =>
      Effect.tryPromise({
        try: async () => {
          const match = /^artifact\.forensic-source\.([0-9a-f]{64})$/u.exec(request.artifactRef);
          if (match?.[1] === undefined) {
            throw failure("source_mismatch", "source artifact ref is invalid");
          }
          const key = sourceBundleKey(request.ownerRef, `sha256:${match[1]}`);
          await bucket.delete(key);
          return (await bucket.head(key)) === null;
        },
        catch: (error) =>
          error instanceof ForensicSourceMaterializationError
            ? error
            : failure("artifact_unavailable", "source artifact deletion is unavailable", true),
      }),
  },
});

export const makeManagedSandboxForensicSourceDelivery = (
  input: Readonly<{
    broker: ManagedSandboxBroker;
    runtime: BoxV1Runtime;
    principal: BoxV1Principal;
    now?: (() => Date) | undefined;
  }>,
): ForensicSourceDelivery => {
  const now = input.now ?? (() => new Date());
  return {
    install: (request) =>
      Effect.gen(function* () {
        const resources = yield* input.broker
          .list()
          .pipe(
            Effect.mapError(() =>
              failure("delivery_failed", "managed sandbox inspection failed", true),
            ),
          );
        const resource = resources.find((candidate) => candidate.sandboxRef === request.sandboxRef);
        const capability = resource?.capabilities.find(
          (candidate) =>
            candidate.capabilityRef === request.capabilityRef &&
            candidate.kind === "file_write" &&
            candidate.state === "active" &&
            Date.parse(candidate.expiresAt) > now().getTime(),
        );
        if (
          resource === undefined ||
          resource.ownerRef !== request.ownerRef ||
          resource.tenantRef !== request.tenantRef ||
          resource.workUnitRef !== request.workUnitRef ||
          resource.resourceGeneration !== request.resourceGeneration ||
          resource.facts.lifecycle !== "ready" ||
          resource.facts.ingressState !== "closed" ||
          capability === undefined
        ) {
          return yield* failure("delivery_failed", "source delivery capability is not admitted");
        }
        const totalBytes = request.entries.reduce(
          (total, entry) => total + base64ByteLength(entry.contentBase64),
          0,
        );
        if (
          totalBytes > resource.budget.maxArtifactBytes ||
          request.entries.some((entry) => base64ByteLength(entry.contentBase64) > 1_048_576)
        ) {
          return yield* failure(
            "delivery_failed",
            "source delivery exceeds the admitted byte budget",
          );
        }
        return yield* Effect.forEach(request.entries, (entry) =>
          input.runtime
            .writeFile({
              principal: input.principal,
              resource,
              operationRef: `operation.forensic-source.${entry.contentDigest.slice(7, 39)}`,
              idempotencyRef: `idempotency.forensic-source.${entry.contentDigest.slice(7, 39)}`,
              capabilityRef: capability.capabilityRef,
              capabilityState: "active",
              capabilityExpiresAt: capability.expiresAt,
              requestedAt: now().toISOString(),
              limits: {
                workspaceRootRef: "workspace.managed-sandbox",
                maxFileBytes: Math.min(1_048_576, resource.budget.maxArtifactBytes),
                maxArtifactBytes: Math.min(16 * 1_024 * 1_024, resource.budget.maxArtifactBytes),
                maxOutputBytes: 131_072,
                maxDurationMillis: Math.min(60_000, resource.budget.maxLifetimeSeconds * 1_000),
                maxCpuMillis: Math.min(60_000, resource.budget.maxCpuMillis),
                maxProcesses: 1,
                maxNetworkBytes: 0,
                networkPolicyRef: "network-policy.managed-sandbox.deny-all",
              },
              path: `workspace/source/${entry.path}`,
              encoding: "base64",
              content: entry.contentBase64,
            })
            .pipe(
              Effect.flatMap((result) => {
                if (
                  result.receipt.outcome !== "succeeded" ||
                  result.receipt.networkBytes !== 0 ||
                  !result.receipt.egressDenied ||
                  result.receipt.secretScan !== "clean" ||
                  result.receipt.symlinkTraversal ||
                  result.size !== base64ByteLength(entry.contentBase64)
                ) {
                  return Effect.fail(
                    failure("delivery_failed", "guest source receipt is incomplete"),
                  );
                }
                return Effect.succeed(result.receipt.receiptRef);
              }),
              Effect.mapError(() =>
                failure("delivery_failed", "private guest source write failed", true),
              ),
            ),
        );
      }),
    cleanup: (request) =>
      Effect.gen(function* () {
        const resources = yield* input.broker
          .list()
          .pipe(
            Effect.mapError(() =>
              failure("delivery_failed", "managed sandbox cleanup inspection failed", true),
            ),
          );
        const resource = resources.find((candidate) => candidate.sandboxRef === request.sandboxRef);
        const cleaned =
          resource !== undefined &&
          resource.resourceGeneration === request.resourceGeneration &&
          resource.facts.lifecycle === "deleted" &&
          resource.facts.filesystemState === "deleted" &&
          resource.facts.cleanupComplete &&
          resource.capabilities.every((capability) => capability.state !== "active");
        return {
          guestSourceDeleted: cleaned,
          scratchDeleted: cleaned,
          grantsRevoked: cleaned,
        };
      }),
  };
};

const ForensicSourceRouteRequestSchema = S.TaggedUnion({
  StageObject: { object: ForensicSourceObjectSchema },
  Materialize: { request: ForensicSourceMaterializationRequestSchema },
  Cleanup: {
    receipt: ForensicSourceMaterializationReceiptSchema,
    observedAt: Timestamp,
  },
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
      if (request.method !== "POST") {
        return sourceRouteJson({ error: "method_not_allowed" }, { status: 405 });
      }
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
      if (!dependencies.enabled(env)) {
        return sourceRouteJson({ error: "runtime_not_admitted" }, { status: 503 });
      }
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
      if (
        body._tag === "StageObject" &&
        (body.object.ownerRef !== owner.userId || body.object.tenantRef !== owner.userId)
      ) {
        return sourceRouteJson({ error: "owner_scope_mismatch" }, { status: 403 });
      }
      if (body._tag === "Cleanup" && body.receipt.ownerRef !== owner.userId) {
        return sourceRouteJson({ error: "owner_scope_mismatch" }, { status: 403 });
      }
      const materializer = yield* dependencies.materializer(env, owner.userId);
      if (body._tag === "StageObject") {
        const staged = yield* materializer.stage(body.object);
        return sourceRouteJson(
          {
            result: {
              sourceObjectRef: staged.sourceObjectRef,
              contentDigest: staged.contentDigest,
            },
          },
          {},
          owner.decorateResponseHeaders,
        );
      }
      const result =
        body._tag === "Materialize"
          ? yield* materializer.materialize(body.request)
          : yield* materializer.cleanup({ receipt: body.receipt, observedAt: body.observedAt });
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
            { status: error instanceof ForensicSourceMaterializationError ? 409 : error.status },
          ),
        ),
      ),
    ),
});
