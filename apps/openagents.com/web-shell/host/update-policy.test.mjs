import test from "node:test";
import assert from "node:assert/strict";

import {
  compareBuildIds,
  evaluateBuildSkew,
  parseCompatibilityManifest,
} from "./update-policy.js";

test("compareBuildIds handles timestamp build ids", () => {
  assert.equal(compareBuildIds("20260221T120000Z", "20260221T130000Z") < 0, true);
  assert.equal(compareBuildIds("20260221T130000Z", "20260221T120000Z") > 0, true);
  assert.equal(compareBuildIds("", "20260221T120000Z"), 0);
});

test("parseCompatibilityManifest extracts compatibility window", () => {
  const parsed = parseCompatibilityManifest({
    buildId: "20260221T130000Z",
    compatibility: {
      minClientBuildId: "20260221T120000Z",
      maxClientBuildId: "20260221T140000Z",
    },
  });

  assert.deepEqual(parsed, {
    buildId: "20260221T130000Z",
    minClientBuildId: "20260221T120000Z",
    maxClientBuildId: "20260221T140000Z",
  });
});

test("evaluateBuildSkew returns none for compatible client", () => {
  const decision = evaluateBuildSkew({
    currentBuildId: "20260221T130000Z",
    serverBuildId: "20260221T130000Z",
    minClientBuildId: "20260221T120000Z",
    maxClientBuildId: "20260221T140000Z",
  });

  assert.deepEqual(decision, {
    skewDetected: false,
    reason: "none",
  });
});

test("evaluateBuildSkew detects below-min client", () => {
  const decision = evaluateBuildSkew({
    currentBuildId: "20260221T110000Z",
    serverBuildId: "20260221T130000Z",
    minClientBuildId: "20260221T120000Z",
    maxClientBuildId: "",
  });

  assert.deepEqual(decision, {
    skewDetected: true,
    reason: "below_min_client",
  });
});

test("evaluateBuildSkew detects build mismatch", () => {
  const decision = evaluateBuildSkew({
    currentBuildId: "20260221T130000Z",
    serverBuildId: "20260221T140000Z",
    minClientBuildId: "20260221T120000Z",
    maxClientBuildId: "",
  });

  assert.deepEqual(decision, {
    skewDetected: true,
    reason: "build_id_mismatch",
  });
});
