import { describe, expect, test } from "vite-plus/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  electronMvpAssuranceTarget,
  mvpAssuranceTargetDescriptorDigest,
  mvpAssuranceTargetSourceDigest,
  nativeSdkMvpAssuranceTarget,
  parseMvpAssuranceTargetArgs,
} from "../scripts/mvp-assurance-target.ts";

const root = resolve(import.meta.dirname, "../../..");

describe("MVP assurance execution targets", () => {
  test("defaults byte-compatibly to Electron and accepts only one explicit target", () => {
    expect(parseMvpAssuranceTargetArgs([])).toBe(electronMvpAssuranceTarget);
    expect(parseMvpAssuranceTargetArgs(["--target=electron"])).toBe(electronMvpAssuranceTarget);
    expect(parseMvpAssuranceTargetArgs(["--target=native-sdk"])).toBe(nativeSdkMvpAssuranceTarget);
    expect(() => parseMvpAssuranceTargetArgs(["--target=native-sdk", "extra"])).toThrow();
    expect(() => parseMvpAssuranceTargetArgs(["--target=other"])).toThrow();
  });

  test("keeps Electron and Native writable namespaces disjoint", () => {
    const writablePaths = (target: typeof electronMvpAssuranceTarget) => {
      const { proposalFixture: _sharedReadOnlyFixture, ...paths } = target.paths;
      return [target.assuranceSpec.path, ...Object.values(paths)];
    };
    const electronPaths = new Set(writablePaths(electronMvpAssuranceTarget));
    const nativePaths = writablePaths(nativeSdkMvpAssuranceTarget);
    expect(nativePaths.every((path) => !electronPaths.has(path))).toBe(true);
    expect(nativeSdkMvpAssuranceTarget.criterion.testPath).not.toContain("openagents-desktop/src");
    expect(nativeSdkMvpAssuranceTarget.targetSourcePaths.filter((path) => path.includes("openagents-desktop/src"))).toEqual([
      "apps/openagents-desktop/src/desktop-command-contract.ts",
      "apps/openagents-desktop/src/chat-contract.ts",
      "apps/openagents-desktop/src/desktop-coding-catalog.ts",
      "apps/openagents-desktop/src/renderer/app.css",
      "apps/openagents-desktop/src/renderer/command-registry.ts",
      "apps/openagents-desktop/src/renderer/portable.ts",
      "apps/openagents-desktop/src/renderer/shell.ts",
    ]);
    expect(JSON.stringify(nativeSdkMvpAssuranceTarget)).not.toContain("rc9");
    expect(nativeSdkMvpAssuranceTarget.fullGate.smokeField).toBe("native_sdk_smoke");
  });

  test("binds descriptor policy and every declared target source", () => {
    expect(mvpAssuranceTargetDescriptorDigest(electronMvpAssuranceTarget)).not.toBe(
      mvpAssuranceTargetDescriptorDigest(nativeSdkMvpAssuranceTarget),
    );
    const sourceDigest = mvpAssuranceTargetSourceDigest(root, electronMvpAssuranceTarget);
    const nativeSourceDigest = mvpAssuranceTargetSourceDigest(root, nativeSdkMvpAssuranceTarget);
    const firstPath = electronMvpAssuranceTarget.targetSourcePaths[0]!;
    const current = readFileSync(resolve(root, firstPath), "utf8");
    expect(sourceDigest).toMatch(/^sha256:[a-f0-9]{64}$/u);
    expect(nativeSourceDigest).toMatch(/^sha256:[a-f0-9]{64}$/u);
    expect(nativeSourceDigest).not.toBe(sourceDigest);
    expect(current.length).toBeGreaterThan(0);
  });
});
