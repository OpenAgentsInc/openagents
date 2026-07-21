// promote-tag: manifest validation, plan construction, and proof/attestation
// derivation. The live coordinator path this command drives is covered by the
// end-to-end convergence test in release-ports-real.test.ts.
import { describe, expect, test } from "vite-plus/test";

import {
  buildTagAttestations,
  buildTagNativeProofs,
  buildTagPlan,
  PromoteTagUsageError,
  validateStagingManifest,
} from "./promote-tag.js";
import { releaseTargetKeys } from "./release.js";

const validArtifacts = () => {
  const formats: Record<string, readonly string[]> = {
    "darwin-arm64": ["dmg", "zip"],
    "darwin-x64": ["dmg", "zip"],
    "linux-arm64": ["appimage", "deb", "rpm"],
    "linux-x64": ["appimage", "deb", "rpm"],
  };
  const out: unknown[] = [];
  for (const target of releaseTargetKeys) {
    for (const format of formats[target]!) {
      out.push({
        target,
        format,
        name: `OpenAgents-0.1.0-rc.9-rc-${target}.${format}`,
        objectKey: `desktop/candidate/0.1.0-rc.9/${target}/x.${format}`,
        sha256: "0".repeat(64),
        byteLength: 100,
        githubUrl: "https://github.com/OpenAgentsInc/openagents/releases/download/t/x",
      });
    }
  }
  return out;
};

const validManifest = () => ({
  version: "0.1.0-rc.9",
  channel: "rc",
  sourceRevision: "a".repeat(40),
  artifacts: validArtifacts(),
});

describe("validateStagingManifest", () => {
  test("accepts a full ten-artifact manifest", () => {
    const manifest = validateStagingManifest(validManifest());
    expect(manifest.artifacts.length).toBe(10);
  });

  test("rejects a short artifact set", () => {
    const bad = { ...validManifest(), artifacts: validArtifacts().slice(0, 9) };
    expect(() => validateStagingManifest(bad)).toThrow(PromoteTagUsageError);
  });

  test("rejects a non-40-hex source revision", () => {
    const bad = { ...validManifest(), sourceRevision: "nope" };
    expect(() => validateStagingManifest(bad)).toThrow(/sourceRevision/);
  });

  test("rejects an unknown channel", () => {
    const bad = { ...validManifest(), channel: "beta" };
    expect(() => validateStagingManifest(bad)).toThrow(PromoteTagUsageError);
  });
});

describe("buildTagPlan", () => {
  test("builds a real rc plan from the manifest", () => {
    const manifest = validateStagingManifest(validManifest());
    const plan = buildTagPlan(manifest, new Date(0), ["rc_promotion"]);
    expect(plan.mode).toBe("real");
    expect(plan.version).toBe("0.1.0-rc.9");
    expect(plan.channel).toBe("rc");
    expect(plan.targets.length).toBe(releaseTargetKeys.length);
    expect(plan.approvedGates).toContain("rc_promotion");
    expect(plan.transactionRef).toMatch(/^v0\.1\.0-rc\.9-rc-\d{8}T\d{6}Z$/);
  });
});

describe("proofs + attestations", () => {
  test("nine unique proof refs per target, all public-safe", () => {
    const proofs = buildTagNativeProofs("0.1.0-rc.9");
    for (const target of releaseTargetKeys) {
      const refs = Object.values(proofs[target]);
      expect(refs.length).toBe(9);
      expect(new Set(refs).size).toBe(9);
      for (const ref of refs) expect(ref).toMatch(/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,239}$/);
    }
  });

  test("one attestation per target, all public-safe", () => {
    const attestations = buildTagAttestations("0.1.0-rc.9");
    for (const target of releaseTargetKeys) {
      expect(attestations[target]).toMatch(/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,239}$/);
    }
  });
});
