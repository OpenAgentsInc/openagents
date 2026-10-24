import assert from "node:assert/strict";
import test from "node:test";
import { referenceCameraLook } from "../packages/renderer/reference-camera-look.ts";
import {
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
} from "../packages/editor-services/protocol.ts";

test("owned camera grades expose usable controls without changing source lighting", () => {
  for (const [id, mode] of [
    ["MT-02", "day"],
    ["MT-17", "day"],
    ["MT-18", "day"],
    ["MT-09", "day"],
    ["MT-16", "day"],
    ["MT-11", "night"],
  ] as const) {
    const grade = referenceCameraLook(id, mode)!;
    const { profile, ...settings } = grade;
    const base = { ...DEFAULT_ENVIRONMENT, mode };
    const accepted = validateEnvironment({ ...base, ...settings });
    assert.equal(profile, "owned-reference-camera-look-v1");
    assert.ok(
      Math.abs(
        accepted.exposure * accepted.skyBackgroundIntensity! -
          (mode === "day" ? 0.28 : 1),
      ) < 1e-12,
    );
    assert.equal(
      accepted.exposure,
      mode === "night" ? 1 : id === "MT-09" || id === "MT-16" ? 4 : 3,
    );
    for (const key of [
      "sunIntensity",
      "moonIntensity",
      "ambientIntensity",
      "skyIntensity",
      "fogDensity",
      "bloomStrength",
    ] as const)
      assert.equal(accepted[key], base[key]);
  }
});

test("reference-only and arbitrary cameras receive no implicit grade", () => {
  for (const id of ["MT-01", "MT-04", "MT-19", "walk", "", "MT-09-extra"])
    assert.equal(referenceCameraLook(id, "day"), undefined);
  assert.notEqual(
    referenceCameraLook("MT-16", "day"),
    referenceCameraLook("MT-16", "day"),
  );
});
