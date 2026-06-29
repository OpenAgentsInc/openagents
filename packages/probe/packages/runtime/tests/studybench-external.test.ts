import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  decodeOpenAgentsStudybenchExternalCalibrationManifest,
  loadStaticStudybenchExternalCalibrationManifest,
} from "../src";

describe("StudyBench external calibration manifest", () => {
  test("loads public StudyBench configs by external dataset ref", async () => {
    const manifest = await Effect.runPromise(loadStaticStudybenchExternalCalibrationManifest());

    expect(manifest.sourceBoundary).toBe("external_public_calibration_refs_only");
    expect(manifest.datasetRefs.map((ref) => ref.datasetRef)).toEqual([
      "hf://jacobli/studybench/dspy",
      "hf://jacobli/studybench/openclaw",
    ]);
    expect(manifest.datasetRefs.map((ref) => ref.expectedRows)).toEqual([30, 20]);
    expect(JSON.stringify(manifest)).not.toContain("gold_answer");
  });

  test("preserves StudyBench and upstream license attribution refs", async () => {
    const manifest = await Effect.runPromise(loadStaticStudybenchExternalCalibrationManifest());
    const dspy = manifest.datasetRefs.find((ref) => ref.config === "dspy")!;
    const openclaw = manifest.datasetRefs.find((ref) => ref.config === "openclaw")!;

    expect(dspy.licenseRefs).toContain("license.studybench.questions_gold_rubrics.cc_by_4_0");
    expect(dspy.licenseRefs).toContain("license.studybench.embedded_dspy_source.mit");
    expect(openclaw.licenseRefs).toContain("license.studybench.embedded_openclaw_source.mit");
    expect(dspy.sourceAttributionRefs[1]).toContain("9cdb0aac28b2a04b064e40697ccd301872cf6a43");
    expect(openclaw.sourceAttributionRefs[1]).toContain("da228660306b55a9cce3b973946f3aacfc515848");
  });

  test("rejects vendored upstream row payloads", async () => {
    const manifest = await Effect.runPromise(loadStaticStudybenchExternalCalibrationManifest());

    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchExternalCalibrationManifest({
          ...manifest,
          rows: [
            {
              id: "dspy_example",
              gold_answer: "This should stay in the upstream dataset, not this manifest.",
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchExternalCalibrationManifest.rows",
    });
  });

  test("rejects mismatched dataset refs", async () => {
    const manifest = await Effect.runPromise(loadStaticStudybenchExternalCalibrationManifest());

    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchExternalCalibrationManifest({
          ...manifest,
          datasetRefs: [
            {
              ...manifest.datasetRefs[0],
              datasetRef: "hf://jacobli/studybench/not-dspy",
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchExternalCalibrationManifest.datasetRefs[0].datasetRef",
    });
  });
});
