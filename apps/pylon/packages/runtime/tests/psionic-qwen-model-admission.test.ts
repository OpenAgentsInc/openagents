import { describe, expect, test } from "bun:test";
import {
  admitPsionicQwenModelRows,
  descriptorsFromPsionicModelList,
  selectPsionicQwenModel,
} from "../src";

describe("Psionic Qwen model row admission", () => {
  test("admits 0.8B only with the retained artifact digest", () => {
    const admission = admitPsionicQwenModelRows([
      {
        id: "/Users/local/models/qwen3.5-0.8b-q8_0.gguf",
        artifactDigest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
      },
    ]);

    expect(admission.admittedModelRefs).toEqual(["model.psionic.qwen35.0_8b.q8_0"]);
    expect(admission.observedModelRefs).toEqual(["model.psionic.qwen35.0_8b.q8_0"]);
    expect(selectPsionicQwenModel(admission, "install_smoke")).toMatchObject({
      admitted: true,
      selectedModelRef: "model.psionic.qwen35.0_8b.q8_0",
      blockerRefs: [],
    });
  });

  test("admits 2B only with a public-safe manifest ref", () => {
    const admission = admitPsionicQwenModelRows([
      {
        id: "/home/example/models/qwen3.5-2b-q8_0-registry.gguf",
        artifactManifestRef: "artifact.psionic.qwen35.2b.q8_0.manifest",
      },
    ]);

    expect(admission.admittedModelRefs).toEqual(["model.psionic.qwen35.2b.q8_0"]);
    expect(selectPsionicQwenModel(admission, "coding_agent")).toMatchObject({
      admitted: true,
      selectedModelRef: "model.psionic.qwen35.2b.q8_0",
      blockerRefs: [],
    });
  });

  test("admits both and selects 2B for coding-agent mode", () => {
    const admission = admitPsionicQwenModelRows([
      {
        id: "qwen3.5-0.8b",
        artifactDigest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
      },
      {
        id: "qwen3.5-2b",
        artifactManifestRef: "artifact.psionic.qwen35.2b.q8_0.manifest",
      },
    ]);

    expect(admission.admittedModelRefs).toEqual([
      "model.psionic.qwen35.0_8b.q8_0",
      "model.psionic.qwen35.2b.q8_0",
    ]);
    expect(selectPsionicQwenModel(admission, "coding_agent")).toMatchObject({
      admitted: true,
      selectedModelRef: "model.psionic.qwen35.2b.q8_0",
    });
  });

  test("refuses 2B-required task when only 0.8B is admitted", () => {
    const admission = admitPsionicQwenModelRows([
      {
        id: "qwen35:0.8b-q8_0",
        artifactDigest: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
      },
    ]);

    expect(selectPsionicQwenModel(admission, "requires_2b")).toEqual({
      admitted: false,
      mode: "requires_2b",
      selectedModelRef: null,
      blockerRefs: ["blocker.psionic_qwen35.model_2b_missing"],
    });
  });

  test("does not admit observed model refs without digest or manifest verification", () => {
    const admission = admitPsionicQwenModelRows([
      {
        id: "qwen3.5-2b",
      },
    ]);

    expect(admission.observedModelRefs).toEqual(["model.psionic.qwen35.2b.q8_0"]);
    expect(admission.admittedModelRefs).toEqual([]);
    expect(admission.blockerRefs).toContain("blocker.psionic_qwen35.artifact_digest_unverified");
  });

  test("decodes model-list metadata without returning raw GGUF paths as refs", () => {
    const descriptors = descriptorsFromPsionicModelList({
      data: [
        {
          id: "/Users/christopherdavid/models/qwen3.5-2b-q8_0.gguf",
          metadata: {
            artifact_manifest_ref: "artifact.psionic.qwen35.2b.q8_0.manifest",
          },
        },
      ],
    });
    const admission = admitPsionicQwenModelRows(descriptors);

    expect(admission.admittedModelRefs).toEqual(["model.psionic.qwen35.2b.q8_0"]);
    expect(JSON.stringify(admission.admittedModelRefs)).not.toContain("/Users/");
    expect(JSON.stringify(admission.rows.map((row) => row.verificationRef))).not.toContain("/Users/");
  });
});
