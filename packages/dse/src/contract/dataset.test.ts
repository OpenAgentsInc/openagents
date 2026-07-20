import { describe, expect, test } from "vite-plus/test";

import { buildDatasetSplit, makeDatasetRevision } from "./dataset.js";
import { datasetId, exampleId } from "./refs.js";

const example = (slug: string) => ({
  exampleId: exampleId(`ex:${slug}`),
  input: { conversation: slug },
  expected: { reply: slug, claimedActions: [] as ReadonlyArray<string> },
  tags: [] as ReadonlyArray<string>,
});

const revision = () =>
  makeDatasetRevision({
    datasetId: datasetId("test/ds"),
    examples: ["t1", "t2", "v1", "h1"].map(example),
  });

describe("dataset revision immutability", () => {
  test("the revision identity changes when any example changes", () => {
    const first = makeDatasetRevision({
      datasetId: datasetId("test/ds"),
      examples: [example("a")],
    });
    const second = makeDatasetRevision({
      datasetId: datasetId("test/ds"),
      examples: [example("b")],
    });
    expect(first.digest).not.toBe(second.digest);
    expect(first.revisionId).not.toBe(second.revisionId);
  });

  test("the same examples produce the same immutable identity", () => {
    expect(revision().digest).toBe(revision().digest);
    expect(revision().revisionId).toBe(revision().revisionId);
  });

  test("a duplicate example identity is rejected", () => {
    expect(() =>
      makeDatasetRevision({
        datasetId: datasetId("test/ds"),
        examples: [example("a"), example("a")],
      }),
    ).toThrow();
  });
});

describe("split builder fails closed", () => {
  test("a valid disjoint split is accepted", () => {
    const result = buildDatasetSplit({
      revision: revision(),
      train: [exampleId("ex:t1"), exampleId("ex:t2")],
      validation: [exampleId("ex:v1")],
      holdout: [exampleId("ex:h1")],
    });
    expect(result.ok).toBe(true);
  });

  test("a missing holdout fails", () => {
    const result = buildDatasetSplit({
      revision: revision(),
      train: [exampleId("ex:t1")],
      validation: [exampleId("ex:v1")],
      holdout: [],
    });
    expect(result).toEqual({ ok: false, reason: "missing_holdout" });
  });

  test("train reused as holdout is contamination, not silent fallback", () => {
    const result = buildDatasetSplit({
      revision: revision(),
      train: [exampleId("ex:t1"), exampleId("ex:t2")],
      validation: [exampleId("ex:v1")],
      holdout: [exampleId("ex:t1")],
    });
    expect(result).toEqual({ ok: false, reason: "contaminated_holdout" });
  });

  test("an unknown example is rejected", () => {
    const result = buildDatasetSplit({
      revision: revision(),
      train: [exampleId("ex:t1")],
      validation: [exampleId("ex:v1")],
      holdout: [exampleId("ex:missing")],
    });
    expect(result).toEqual({ ok: false, reason: "unknown_example" });
  });
});
