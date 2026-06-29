import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";

export const OPENAGENTS_STUDYBENCH_EXTERNAL_CALIBRATION_MANIFEST_SCHEMA_REF =
  "openagents.studybench_external_calibration_manifest.v0" as const;
export const OPENAGENTS_STUDYBENCH_EXTERNAL_DATASET_REF_SCHEMA_REF =
  "openagents.studybench_external_dataset_ref.v0" as const;

export const OpenAgentsStudybenchExternalDatasetProvider = S.Literal("huggingface");
export type OpenAgentsStudybenchExternalDatasetProvider =
  typeof OpenAgentsStudybenchExternalDatasetProvider.Type;

export const OpenAgentsStudybenchExternalStudybenchConfig = S.Literals(["dspy", "openclaw"]);
export type OpenAgentsStudybenchExternalStudybenchConfig =
  typeof OpenAgentsStudybenchExternalStudybenchConfig.Type;

export const OpenAgentsStudybenchExternalDatasetRef = S.Struct({
  config: OpenAgentsStudybenchExternalStudybenchConfig,
  datasetRef: S.String,
  expectedRows: S.Number,
  licenseRefs: S.Array(S.String),
  loaderRef: S.String,
  provider: OpenAgentsStudybenchExternalDatasetProvider,
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_EXTERNAL_DATASET_REF_SCHEMA_REF),
  sourceAttributionRefs: S.Array(S.String),
  split: S.Literal("train"),
});
export type OpenAgentsStudybenchExternalDatasetRef =
  typeof OpenAgentsStudybenchExternalDatasetRef.Type;

export const OpenAgentsStudybenchExternalCalibrationManifest = S.Struct({
  attributionPolicyRef: S.String,
  datasetRefs: S.Array(OpenAgentsStudybenchExternalDatasetRef),
  loaderBoundaryRef: S.String,
  manifestRef: S.String,
  notesRef: S.String,
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_EXTERNAL_CALIBRATION_MANIFEST_SCHEMA_REF),
  sourceBoundary: S.Literal("external_public_calibration_refs_only"),
});
export type OpenAgentsStudybenchExternalCalibrationManifest =
  typeof OpenAgentsStudybenchExternalCalibrationManifest.Type;

export const STATIC_STUDYBENCH_EXTERNAL_CALIBRATION_MANIFEST: OpenAgentsStudybenchExternalCalibrationManifest = {
  attributionPolicyRef: "policy.openagents.studybench_external.cc_by_4_and_upstream_mit_attribution",
  datasetRefs: [
    {
      config: "dspy",
      datasetRef: "hf://jacobli/studybench/dspy",
      expectedRows: 30,
      licenseRefs: [
        "license.studybench.questions_gold_rubrics.cc_by_4_0",
        "license.studybench.embedded_dspy_source.mit",
      ],
      loaderRef: "loader.huggingface.datasets.load_dataset.jacobli_studybench.dspy.train",
      provider: "huggingface",
      schemaRef: OPENAGENTS_STUDYBENCH_EXTERNAL_DATASET_REF_SCHEMA_REF,
      sourceAttributionRefs: [
        "source.studybench.huggingface_dataset_card",
        "source.studybench.dspy.stanfordnlp_dspy.9cdb0aac28b2a04b064e40697ccd301872cf6a43",
      ],
      split: "train",
    },
    {
      config: "openclaw",
      datasetRef: "hf://jacobli/studybench/openclaw",
      expectedRows: 20,
      licenseRefs: [
        "license.studybench.questions_gold_rubrics.cc_by_4_0",
        "license.studybench.embedded_openclaw_source.mit",
      ],
      loaderRef: "loader.huggingface.datasets.load_dataset.jacobli_studybench.openclaw.train",
      provider: "huggingface",
      schemaRef: OPENAGENTS_STUDYBENCH_EXTERNAL_DATASET_REF_SCHEMA_REF,
      sourceAttributionRefs: [
        "source.studybench.huggingface_dataset_card",
        "source.studybench.openclaw.openclaw.da228660306b55a9cce3b973946f3aacfc515848",
      ],
      split: "train",
    },
  ],
  loaderBoundaryRef: "boundary.openagents.studybench_external.runtime_validates_loaded_rows_no_network_fetch",
  manifestRef: "manifest.openagents.studybench_external_calibration.v0",
  notesRef: "docs.research.machine_studying.openagents_studybench.external_calibration",
  schemaRef: OPENAGENTS_STUDYBENCH_EXTERNAL_CALIBRATION_MANIFEST_SCHEMA_REF,
  sourceBoundary: "external_public_calibration_refs_only",
};

export function decodeOpenAgentsStudybenchExternalCalibrationManifest(
  value: unknown,
): Effect.Effect<
  OpenAgentsStudybenchExternalCalibrationManifest,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchExternalCalibrationManifest");
    yield* validateNoVendoredStudybenchRows(value, "studybenchExternalCalibrationManifest");
    const manifest = yield* S.decodeUnknownEffect(OpenAgentsStudybenchExternalCalibrationManifest)(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "studybenchExternalCalibrationManifest",
            reason: String(error),
          }),
      ),
    );
    yield* validateOpenAgentsStudybenchExternalCalibrationManifest(manifest);
    return manifest;
  });
}

export function loadStaticStudybenchExternalCalibrationManifest(): Effect.Effect<
  OpenAgentsStudybenchExternalCalibrationManifest,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return decodeOpenAgentsStudybenchExternalCalibrationManifest(STATIC_STUDYBENCH_EXTERNAL_CALIBRATION_MANIFEST);
}

function validateOpenAgentsStudybenchExternalCalibrationManifest(
  manifest: OpenAgentsStudybenchExternalCalibrationManifest,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(manifest.manifestRef, "studybenchExternalCalibrationManifest.manifestRef");
    yield* requireNonEmpty(manifest.attributionPolicyRef, "studybenchExternalCalibrationManifest.attributionPolicyRef");
    yield* requireNonEmpty(manifest.loaderBoundaryRef, "studybenchExternalCalibrationManifest.loaderBoundaryRef");

    if (manifest.datasetRefs.length === 0) {
      return yield* externalStudybenchError(
        "studybenchExternalCalibrationManifest.datasetRefs",
        "must include at least one external calibration dataset ref",
      );
    }

    const datasetRefs = new Set<string>();

    for (const [index, dataset] of manifest.datasetRefs.entries()) {
      const path = `studybenchExternalCalibrationManifest.datasetRefs[${index}]`;
      yield* requireNonEmpty(dataset.datasetRef, `${path}.datasetRef`);
      yield* requireNonEmpty(dataset.loaderRef, `${path}.loaderRef`);
      yield* requireNonEmptyRefs(dataset.licenseRefs, `${path}.licenseRefs`);
      yield* requireNonEmptyRefs(dataset.sourceAttributionRefs, `${path}.sourceAttributionRefs`);

      if (datasetRefs.has(dataset.datasetRef)) {
        return yield* externalStudybenchError(`${path}.datasetRef`, "must be unique in the manifest");
      }

      datasetRefs.add(dataset.datasetRef);

      if (dataset.datasetRef !== `hf://jacobli/studybench/${dataset.config}`) {
        return yield* externalStudybenchError(`${path}.datasetRef`, "must match the configured StudyBench dataset ref");
      }

      if (!Number.isInteger(dataset.expectedRows) || dataset.expectedRows <= 0) {
        return yield* externalStudybenchError(`${path}.expectedRows`, "must be a positive integer row count");
      }

      if (!dataset.licenseRefs.some((ref) => ref.includes("cc_by_4_0"))) {
        return yield* externalStudybenchError(`${path}.licenseRefs`, "must preserve StudyBench CC-BY-4.0 attribution");
      }

      if (!dataset.sourceAttributionRefs.some((ref) => ref.includes("studybench"))) {
        return yield* externalStudybenchError(`${path}.sourceAttributionRefs`, "must include StudyBench source attribution");
      }
    }
  });
}

function validateNoVendoredStudybenchRows(
  value: unknown,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (value === null || value === undefined || typeof value !== "object") {
    return Effect.void;
  }

  if (Array.isArray(value)) {
    return Effect.all(value.map((entry, index) => validateNoVendoredStudybenchRows(entry, `${path}[${index}]`))).pipe(
      Effect.asVoid,
    );
  }

  return Effect.gen(function* () {
    for (const [key, entry] of Object.entries(value)) {
      const normalized = key.replace(/[_-]/g, "").toLowerCase();

      if (normalized === "rows" || normalized === "taskrows" || normalized === "goldanswer") {
        return yield* externalStudybenchError(
          `${path}.${key}`,
          "external calibration manifests must reference StudyBench rows instead of vendoring row payloads",
        );
      }

      yield* validateNoVendoredStudybenchRows(entry, `${path}.${key}`);
    }
  });
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? externalStudybenchError(path, "must be a non-empty string")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return externalStudybenchError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : externalStudybenchError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function externalStudybenchError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
