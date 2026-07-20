import { Schema as S } from "effect";

import { canonicalDigest } from "../internal/canonical.js";
import { DatasetId, DatasetRevisionId, ExampleId, Sha256Hex, datasetRevisionId } from "./refs.js";

/**
 * Immutable datasets and their train / validation / holdout splits.
 *
 * A dataset revision has an immutable identity: its digest covers the canonical
 * bytes of every example, so a changed example is a new revision. A split binds
 * one revision and separates train, validation, and holdout example identity.
 * The split builder is where two audit corrections become mechanical: a missing
 * holdout fails, and train can never silently become holdout.
 */

export const DATASET_REVISION_SCHEMA_LITERAL = "openagents.dse.dataset_revision.v1" as const;
export const DATASET_SPLIT_SCHEMA_LITERAL = "openagents.dse.dataset_split.v1" as const;

export const DatasetSplitName = S.Literals(["train", "validation", "holdout"]);
export type DatasetSplitName = typeof DatasetSplitName.Type;

/** A labeled example: a signature input, its reference output, and tags. */
export const LabeledExample = S.Struct({
  exampleId: ExampleId,
  input: S.Json,
  expected: S.Json,
  tags: S.Array(S.String.check(S.isMaxLength(64))).check(S.isMaxLength(16)),
});
export type LabeledExample = typeof LabeledExample.Type;

/** An immutable dataset revision, content-addressed by its example digest. */
export const DatasetRevision = S.Struct({
  schema: S.Literal(DATASET_REVISION_SCHEMA_LITERAL),
  datasetId: DatasetId,
  revisionId: DatasetRevisionId,
  examples: S.Array(LabeledExample).check(S.isMinLength(1), S.isMaxLength(10000)),
  digest: Sha256Hex,
});
export type DatasetRevision = typeof DatasetRevision.Type;

/** A split over one revision. Every list is example identity, never example bytes. */
export const DatasetSplit = S.Struct({
  schema: S.Literal(DATASET_SPLIT_SCHEMA_LITERAL),
  revisionId: DatasetRevisionId,
  train: S.Array(ExampleId).check(S.isMinLength(1)),
  validation: S.Array(ExampleId).check(S.isMinLength(1)),
  holdout: S.Array(ExampleId).check(S.isMinLength(1)),
});
export type DatasetSplit = typeof DatasetSplit.Type;

const decodeRevision = S.decodeUnknownSync(DatasetRevision);
const decodeSplit = S.decodeUnknownSync(DatasetSplit);

/**
 * Build an immutable dataset revision. The digest covers the canonical bytes of
 * the ordered examples, so the revision identity changes whenever any example
 * changes. Duplicate example identity is rejected.
 */
export const makeDatasetRevision = (args: {
  readonly datasetId: DatasetId;
  readonly examples: ReadonlyArray<typeof LabeledExample.Type>;
}): DatasetRevision => {
  const seen = new Set<string>();
  for (const example of args.examples) {
    if (seen.has(example.exampleId)) {
      throw new Error(`DSE dataset has a duplicate example identity: ${example.exampleId}`);
    }
    seen.add(example.exampleId);
  }
  const digest = canonicalDigest({ datasetId: args.datasetId, examples: args.examples });
  return decodeRevision({
    schema: DATASET_REVISION_SCHEMA_LITERAL,
    datasetId: args.datasetId,
    revisionId: datasetRevisionId(`dset:${args.datasetId}:${digest.slice(0, 16)}`),
    examples: args.examples,
    digest,
  });
};

export type DatasetSplitResult =
  | { readonly ok: true; readonly split: DatasetSplit }
  | { readonly ok: false; readonly reason: DatasetSplitFailure };

export type DatasetSplitFailure =
  | "missing_holdout"
  | "missing_train"
  | "missing_validation"
  | "unknown_example"
  | "contaminated_holdout"
  | "contaminated_validation";

/**
 * Build a split and fail closed. The holdout must be present and disjoint from
 * both train and validation, every referenced example must exist in the
 * revision, and no list may be empty. A caller can never reuse train rows as
 * holdout by omission — the builder returns a typed failure instead.
 */
export const buildDatasetSplit = (args: {
  readonly revision: DatasetRevision;
  readonly train: ReadonlyArray<typeof ExampleId.Type>;
  readonly validation: ReadonlyArray<typeof ExampleId.Type>;
  readonly holdout: ReadonlyArray<typeof ExampleId.Type>;
}): DatasetSplitResult => {
  if (args.holdout.length === 0) return { ok: false, reason: "missing_holdout" };
  if (args.train.length === 0) return { ok: false, reason: "missing_train" };
  if (args.validation.length === 0) return { ok: false, reason: "missing_validation" };

  const known = new Set<string>(args.revision.examples.map((example) => example.exampleId));
  for (const id of [...args.train, ...args.validation, ...args.holdout]) {
    if (!known.has(id)) return { ok: false, reason: "unknown_example" };
  }

  const trainSet = new Set<string>(args.train);
  const validationSet = new Set<string>(args.validation);
  for (const id of args.holdout) {
    if (trainSet.has(id) || validationSet.has(id))
      return { ok: false, reason: "contaminated_holdout" };
  }
  for (const id of args.validation) {
    if (trainSet.has(id)) return { ok: false, reason: "contaminated_validation" };
  }

  return {
    ok: true,
    split: decodeSplit({
      schema: DATASET_SPLIT_SCHEMA_LITERAL,
      revisionId: args.revision.revisionId,
      train: args.train,
      validation: args.validation,
      holdout: args.holdout,
    }),
  };
};

/** Index a revision's examples by identity for evaluation lookups. */
export const indexExamples = (revision: DatasetRevision): ReadonlyMap<string, LabeledExample> =>
  new Map(revision.examples.map((example) => [example.exampleId, example]));
