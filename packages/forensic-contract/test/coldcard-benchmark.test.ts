import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ColdcardBenchmarkManifestSchema,
  ColdcardHistoricalImportSchema,
  ColdcardReproductionManifestSchema,
  ColdcardSuiteManifestSchema,
  ForensicPath,
  ForensicRef,
  ShortText,
  forensicSha256Digest,
  strictDecode,
} from "../src/index.ts";

const readJson = (name: string): unknown =>
  JSON.parse(
    readFileSync(new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url), "utf8"),
  );

const ArmFixtureSchema = S.Struct({
  armRef: ForensicRef,
  sourceRefs: S.Array(ForensicRef),
  presentDependencyPaths: S.Array(ForensicPath),
  missingDependencyPaths: S.Array(ForensicPath),
  variantRefs: S.Array(ForensicRef),
  controlCaseRefs: S.Array(ForensicRef),
});
const VariantSchema = S.Struct({
  variantRef: ForensicRef,
  transformation: ShortText,
  forbiddenShortcut: ShortText,
});
const CleanControlSchema = S.Struct({
  controlCaseRef: ForensicRef,
  expectedDisposition: S.Literals(["historical_finding_absent", "not_proven"]),
  description: ShortText,
});
const ArmFixturesSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_arm_fixtures.v1"),
  fixtures: S.Array(ArmFixtureSchema).check(S.isMinLength(5), S.isMaxLength(5)),
  structuralVariants: S.Array(VariantSchema).check(S.isMinLength(5), S.isMaxLength(5)),
  cleanControls: S.Array(CleanControlSchema).check(S.isMinLength(3), S.isMaxLength(3)),
});
const DatasetSplitsSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_dataset_splits.v1"),
  splits: S.Array(
    S.Struct({
      split: S.Literals(["train", "development", "holdout", "clean_holdout"]),
      ownerRef: ForensicRef,
      privateManifestRef: ForensicRef,
      benchmarkArmRefs: S.Array(ForensicRef),
      optimizerVisibility: S.Literals(["optimizer_visible", "evaluator_only"]),
    }),
  ).check(S.isMinLength(4), S.isMaxLength(4)),
});

const benchmark = strictDecode(
  ColdcardBenchmarkManifestSchema,
  readJson("benchmark-manifest.v1.json"),
);
const reproduction = strictDecode(
  ColdcardReproductionManifestSchema,
  readJson("reproduction-manifest.v1.json"),
);
const historicalImport = strictDecode(
  ColdcardHistoricalImportSchema,
  readJson("historical-import.v1.json"),
);
const armFixtures = strictDecode(ArmFixturesSchema, readJson("arm-fixtures.v1.json"));
const datasetSplits = strictDecode(DatasetSplitsSchema, readJson("dataset-splits.v1.json"));
const suiteNames = [
  "suite-code-to-artifact.v1.json",
  "suite-generator-owned-fixture.v1.json",
  "suite-historical-chain-fingerprint.v1.json",
  "suite-evidence-graph.v1.json",
] as const;
const suites = suiteNames.map((name) => strictDecode(ColdcardSuiteManifestSchema, readJson(name)));

describe("Coldcard forensic benchmark", () => {
  it("pins every real revision and reproducible git tree digest", () => {
    expect(benchmark.treeDigestAlgorithm).toBe("sha256_git_ls_tree_r_z_v1");
    expect(
      reproduction.pinnedRevisions.map(({ role, commitSha, treeDigest }) => ({
        role,
        commitSha,
        treeDigest,
      })),
    ).toEqual([
      {
        role: "vulnerable_target",
        commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
        treeDigest: "sha256:c1b3fe958af8e6110589ce8772bf3064e4f40ea529c8b0d345e5881ba4b9df5d",
      },
      {
        role: "fixed_target",
        commitSha: "ca72463709f4e3f8964952039d5caf955f566a87",
        treeDigest: "sha256:015039b7f92bcc34b042e091037818f7e364e141891053120243977f02d373f6",
      },
      {
        role: "libngu",
        commitSha: "537519a829259622ea6b0334fbafd6cae852852f",
        treeDigest: "sha256:66061a40108a0ea4184cf83aeecc15cf3008e083e98e668fd926279b791ea448",
      },
      {
        role: "micropython",
        commitSha: "4107246f8a080807b62c3b4838e71e812ea68b6f",
        treeDigest: "sha256:7c041d0be166a91edc46fa20e6f737451f3a9c9fe44ba9144e2354b25fbbc86b",
      },
      {
        role: "ckcc_protocol",
        commitSha: "3d1dfa858beb58b8dac37d8c66d7aed2909812f2",
        treeDigest: "sha256:f6c3a1d546f45b1c402d0b0a03306c88ee7c70e096e15acd8033cb5acc28f19b",
      },
      {
        role: "mpy_qr",
        commitSha: "11347d83f4eb325b10676a4eb8e17deccfe0df44",
        treeDigest: "sha256:f422cdf157e9daef6b73bb97ab30b95e882b79046352c7888912295409962ec3",
      },
      {
        role: "postmortem",
        commitSha: "47d8f5543812c8244fa95ed90db957ddcc05200c",
        treeDigest: "sha256:c665e24e736138701b1f4ad05c7ec838c466f29ab10912fc9363f48be2b076a7",
      },
    ]);
  });

  it("binds every checked-in arm, split, suite, import, evaluator, and rubric digest", () => {
    expect(benchmark.reproductionManifestDigest).toBe(forensicSha256Digest(reproduction));
    expect(benchmark.historicalImportDigest).toBe(forensicSha256Digest(historicalImport));
    for (const arm of benchmark.arms) {
      const fixture = armFixtures.fixtures.find((candidate) => candidate.armRef === arm.armRef);
      expect(fixture).toBeDefined();
      expect(arm.fixtureDigest).toBe(forensicSha256Digest(fixture));
    }
    for (const split of benchmark.datasetSplits) {
      const splitFixture = datasetSplits.splits.find(
        (candidate) => candidate.split === split.split,
      );
      expect(splitFixture).toBeDefined();
      expect(split.manifestDigest).toBe(forensicSha256Digest(splitFixture));
    }
    for (const suite of suites) {
      const binding = benchmark.suites.find((candidate) => candidate.suite === suite.suite);
      expect(binding).toBeDefined();
      expect(binding?.manifestRef).toBe(suite.suiteRef);
      expect(binding?.manifestDigest).toBe(forensicSha256Digest(suite));
      const { evaluatorDigest, ...evaluatorInputs } = suite;
      expect(evaluatorDigest).toBe(forensicSha256Digest(evaluatorInputs));
    }
    const { rubricDigest, ...rubricInputs } = benchmark.rubric;
    expect(rubricDigest).toBe(forensicSha256Digest(rubricInputs));
    expect(reproduction.evaluatorDigest).toBe(rubricDigest);
  });

  it("keeps Arm A incomplete and imports Episode 264 without invented usage", () => {
    const arm = benchmark.arms.find((candidate) => candidate.kind === "incomplete_clone");
    const run = historicalImport.runs.find(
      (candidate) => candidate.armRef === "arm.coldcard.incomplete",
    );
    expect(arm).toMatchObject({
      expectedRunState: "completed_incomplete",
      expectedVerdict: "not_scored_incomplete",
      modelCallPolicy: "blocked_before_inference",
    });
    expect(arm?.missingDependencyPaths).toEqual([
      "external/libngu",
      "external/micropython",
      "external/ckcc-protocol",
      "external/mpy-qr",
    ]);
    expect(run).toMatchObject({
      inputCompleteness: "incomplete",
      importedRunState: "completed_incomplete",
      verdict: "miss",
      findingCount: 12,
      qualifiedFindingRefs: [],
      wallDurationMilliseconds: { exactness: "unavailable" },
      tokenUsage: { exactness: "unavailable" },
    });
    expect(run?.wallDurationMilliseconds).not.toHaveProperty("value");
    expect(run?.tokenUsage).not.toHaveProperty("value");
  });

  it("imports Arm B only as an unverified source hit with the full frozen chain", () => {
    const arm = benchmark.arms.find((candidate) => candidate.kind === "complete_vulnerable");
    const run = historicalImport.runs.find(
      (candidate) => candidate.armRef === "arm.coldcard.complete-vulnerable",
    );
    expect(arm).toMatchObject({
      expectedVerdict: "source_hit_unverified",
      expectedEvidenceTier: "source_observed",
      finalLinkOutcome: "not_proven",
    });
    expect(arm?.requiredCausalLinkRefs).toEqual(
      benchmark.rubric.causalLinks.map((link) => link.causalLinkRef),
    );
    expect(run).toMatchObject({
      verdict: "hit",
      evidenceTier: "source_observed",
      verificationState: "disabled",
      findingCount: 22,
      qualifiedFindingRefs: ["finding.loupe.152", "finding.loupe.161", "finding.loupe.148"],
    });
  });

  it("freezes fixed, semantic-variant, and clean-control behavior", () => {
    expect(benchmark.arms.find((arm) => arm.kind === "fixed_clone")).toMatchObject({
      expectedVerdict: "historical_finding_absent",
    });
    const structural = benchmark.arms.find((arm) => arm.kind === "structural_variants");
    expect(structural?.exactMatchResistanceRefs).toEqual(
      armFixtures.structuralVariants.map((variant) => variant.variantRef),
    );
    expect(armFixtures.structuralVariants.map((variant) => variant.forbiddenShortcut)).toEqual([
      "Exact MICROPY_HW_ENABLE_RNG token match",
      "Exact rng_get symbol match",
      "Exact Coldcard repository path match",
      "Exact #ifndef syntax match",
      "Exact MicroPython dependency match",
    ]);
    const clean = benchmark.arms.find((arm) => arm.kind === "clean_controls");
    expect(clean?.controlCaseRefs).toEqual(
      armFixtures.cleanControls.map((control) => control.controlCaseRef),
    );
    expect(armFixtures.cleanControls.map((control) => control.expectedDisposition)).toEqual([
      "historical_finding_absent",
      "not_proven",
      "historical_finding_absent",
    ]);
  });

  it("keeps postmortem outputs out of derivation and evaluator inputs", () => {
    expect(
      reproduction.expectedComparisonRefs.some((reference) =>
        reproduction.rawInputRefs.includes(reference),
      ),
    ).toBe(false);
    for (const suite of suites) {
      const inputRefs = suite.inputs.map((input) => input.inputRef);
      for (const comparisonRef of suite.expectedComparisonRefs) {
        expect(inputRefs).not.toContain(comparisonRef);
        expect(suite.evaluatorInputRefs).not.toContain(comparisonRef);
      }
    }
  });

  it("keeps Coldcard in development and holdouts separate, private, and evaluator-only", () => {
    const development = benchmark.datasetSplits.find((split) => split.split === "development");
    expect(development?.benchmarkArmRefs).toEqual(benchmark.arms.map((arm) => arm.armRef));
    for (const splitName of ["holdout", "clean_holdout"] as const) {
      const split = benchmark.datasetSplits.find((candidate) => candidate.split === splitName);
      expect(split).toMatchObject({ optimizerVisibility: "evaluator_only", benchmarkArmRefs: [] });
    }
    expect(new Set(benchmark.datasetSplits.map((split) => split.ownerRef)).size).toBe(4);
    expect(new Set(benchmark.datasetSplits.map((split) => split.manifestDigest)).size).toBe(4);
  });

  it("rejects benchmark false greens and comparison-data contamination", () => {
    const incomplete = benchmark.arms.find((arm) => arm.kind === "incomplete_clone");
    const structural = benchmark.arms.find((arm) => arm.kind === "structural_variants");
    const clean = benchmark.arms.find((arm) => arm.kind === "clean_controls");
    const codeSuite = suites.find((suite) => suite.suite === "code_to_artifact");
    const comparisonRef = codeSuite?.expectedComparisonRefs[0];
    const reproductionComparisonRef = reproduction.expectedComparisonRefs[0];
    if (
      incomplete === undefined ||
      structural === undefined ||
      clean === undefined ||
      codeSuite === undefined ||
      comparisonRef === undefined ||
      reproductionComparisonRef === undefined
    ) {
      throw new Error("Coldcard benchmark fixtures are incomplete");
    }
    expect(() =>
      strictDecode(ColdcardBenchmarkManifestSchema, {
        ...benchmark,
        arms: benchmark.arms.map((arm) =>
          arm.kind === "incomplete_clone" ? { ...arm, expectedRunState: "completed" } : arm,
        ),
      }),
    ).toThrow();
    expect(() =>
      strictDecode(ColdcardBenchmarkManifestSchema, {
        ...benchmark,
        arms: benchmark.arms.map((arm) =>
          arm.kind === "structural_variants"
            ? { ...arm, exactMatchResistanceRefs: structural.exactMatchResistanceRefs.slice(0, 4) }
            : arm,
        ),
      }),
    ).toThrow();
    expect(() =>
      strictDecode(ColdcardBenchmarkManifestSchema, {
        ...benchmark,
        arms: benchmark.arms.map((arm) =>
          arm.kind === "clean_controls"
            ? { ...arm, controlCaseRefs: clean.controlCaseRefs.slice(0, 2) }
            : arm,
        ),
      }),
    ).toThrow();
    expect(() =>
      strictDecode(ColdcardSuiteManifestSchema, {
        ...codeSuite,
        evaluatorInputRefs: [...codeSuite.evaluatorInputRefs, comparisonRef],
      }),
    ).toThrow();
    expect(() =>
      strictDecode(ColdcardReproductionManifestSchema, {
        ...reproduction,
        rawInputRefs: [...reproduction.rawInputRefs, reproductionComparisonRef],
      }),
    ).toThrow();
    const incompleteRun = historicalImport.runs.find(
      (run) => run.armRef === "arm.coldcard.incomplete",
    );
    expect(incompleteRun).toBeDefined();
    expect(() =>
      strictDecode(ColdcardHistoricalImportSchema, {
        ...historicalImport,
        runs: historicalImport.runs.map((run) =>
          run.armRef === "arm.coldcard.incomplete"
            ? { ...run, importedRunState: "completed" }
            : run,
        ),
      }),
    ).toThrow();
  });
});
