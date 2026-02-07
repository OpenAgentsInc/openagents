import { Effect, Schema } from "effect";

import { sha256IdFromCanonicalJson } from "../hashes.js";
import { canonicalJson } from "../internal/canonicalJson.js";

export type DatasetSplit = "train" | "holdout" | "test" | string;

export type DatasetExample<I, Y> = {
  readonly exampleId: string;
  readonly input: I;
  readonly expected: Y;
  readonly split?: DatasetSplit | undefined;
  readonly tags?: ReadonlyArray<string> | undefined;
  readonly meta?: unknown | undefined;
};

export type Dataset<I, Y> = {
  readonly datasetId: string;
  readonly examples: ReadonlyArray<DatasetExample<I, Y>>;
};

export class DatasetError extends Schema.TaggedError<DatasetError>()("DatasetError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

function normalizeTagList(tags: ReadonlyArray<string> | undefined): ReadonlyArray<string> | undefined {
  if (!tags || tags.length === 0) return undefined;
  const cleaned = tags.map((t) => t.trim()).filter((t) => t.length > 0);
  if (cleaned.length === 0) return undefined;
  cleaned.sort();
  // Dedupe.
  return cleaned.filter((t, i) => (i === 0 ? true : t !== cleaned[i - 1]));
}

function normalizeExample<I, Y>(ex: DatasetExample<I, Y>): DatasetExample<I, Y> {
  return {
    ...ex,
    exampleId: ex.exampleId.trim(),
    ...(ex.split ? { split: ex.split } : {}),
    ...(ex.tags ? { tags: normalizeTagList(ex.tags) } : {})
  };
}

export function make<I, Y>(options: {
  readonly datasetId: string;
  readonly examples: ReadonlyArray<DatasetExample<I, Y>>;
}): Effect.Effect<Dataset<I, Y>, DatasetError> {
  return Effect.gen(function* () {
    const datasetId = options.datasetId.trim();
    if (!datasetId) {
      return yield* Effect.fail(
        DatasetError.make({ message: "datasetId must be non-empty" })
      );
    }

    const normalized = options.examples.map(normalizeExample);

    const ids = normalized.map((e) => e.exampleId);
    if (ids.some((id) => !id)) {
      return yield* Effect.fail(
        DatasetError.make({ message: "All dataset examples must have a non-empty exampleId" })
      );
    }

    const seen = new Set<string>();
    const duplicates: Array<string> = [];
    for (const id of ids) {
      if (seen.has(id)) duplicates.push(id);
      seen.add(id);
    }
    if (duplicates.length > 0) {
      return yield* Effect.fail(
        DatasetError.make({
          message: `Dataset contains duplicate exampleIds: ${duplicates.slice(0, 10).join(", ")}`
        })
      );
    }

    // Stable, deterministic iteration order by exampleId (lexicographic).
    const sorted = [...normalized].sort((a, b) =>
      a.exampleId.localeCompare(b.exampleId)
    );

    return { datasetId, examples: sorted };
  });
}

export type DatasetExportV1 = {
  readonly format: "openagents.dse.dataset";
  readonly formatVersion: 1;
  readonly datasetId: string;
  readonly examples: ReadonlyArray<{
    readonly exampleId: string;
    readonly split?: string | undefined;
    readonly tags?: ReadonlyArray<string> | undefined;
    readonly input: unknown;
    readonly expected: unknown;
  }>;
};

export function exportV1<I, Y>(dataset: Dataset<I, Y>): DatasetExportV1 {
  return {
    format: "openagents.dse.dataset",
    formatVersion: 1,
    datasetId: dataset.datasetId,
    examples: dataset.examples.map((e) => ({
      exampleId: e.exampleId,
      ...(e.split ? { split: e.split } : {}),
      ...(e.tags ? { tags: e.tags } : {}),
      input: e.input as unknown,
      expected: e.expected as unknown
    }))
  };
}

export function datasetHash<I, Y>(dataset: Dataset<I, Y>) {
  return sha256IdFromCanonicalJson(exportV1(dataset));
}

export type DatasetFilter = {
  readonly split?: DatasetSplit | undefined;
  readonly requireAllTags?: ReadonlyArray<string> | undefined;
};

export function filter<I, Y>(
  dataset: Dataset<I, Y>,
  filter: DatasetFilter
): Dataset<I, Y> {
  const split = filter.split;
  const required = normalizeTagList(filter.requireAllTags);
  if (!split && (!required || required.length === 0)) return dataset;

  const out = dataset.examples.filter((e) => {
    if (split && e.split !== split) return false;
    if (!required || required.length === 0) return true;
    const tags = new Set(e.tags ?? []);
    return required.every((t) => tags.has(t));
  });

  return { datasetId: dataset.datasetId, examples: out };
}

function mulberry32(seed: number): () => number {
  let t = seed >>> 0;
  return () => {
    t += 0x6d2b79f5;
    let x = t;
    x = Math.imul(x ^ (x >>> 15), x | 1);
    x ^= x + Math.imul(x ^ (x >>> 7), x | 61);
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
}

export type SamplePlan = { readonly seed: number; readonly n: number };

export function sample<I, Y>(
  dataset: Dataset<I, Y>,
  plan: SamplePlan
): { readonly dataset: Dataset<I, Y>; readonly selectedExampleIds: ReadonlyArray<string> } {
  const n = Math.max(0, Math.floor(plan.n));
  if (n === 0) {
    return { dataset: { datasetId: dataset.datasetId, examples: [] }, selectedExampleIds: [] };
  }

  const all = dataset.examples.map((e) => e.exampleId);
  if (n >= all.length) {
    return { dataset, selectedExampleIds: all };
  }

  const rng = mulberry32(plan.seed);
  const shuffled = [...all];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    const tmp = shuffled[i]!;
    shuffled[i] = shuffled[j]!;
    shuffled[j] = tmp;
  }
  const selectedSet = new Set(shuffled.slice(0, n));
  const selected = dataset.examples
    .filter((e) => selectedSet.has(e.exampleId))
    .map((e) => e.exampleId);

  // Preserve dataset order (already sorted by exampleId).
  const sampled: Dataset<I, Y> = {
    datasetId: dataset.datasetId,
    examples: dataset.examples.filter((e) => selectedSet.has(e.exampleId))
  };

  return { dataset: sampled, selectedExampleIds: selected };
}

export function selectedExampleIdsHash(selectedExampleIds: ReadonlyArray<string>) {
  return sha256IdFromCanonicalJson({ exampleIds: selectedExampleIds });
}

export function prettyPrint<I, Y>(dataset: Dataset<I, Y>): string {
  return canonicalJson(exportV1(dataset));
}

