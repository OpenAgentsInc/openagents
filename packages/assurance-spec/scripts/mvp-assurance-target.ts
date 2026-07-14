import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { canonicalArtifact, sha256Digest } from "../src/index.ts";

export type MvpAssuranceTargetName = "electron" | "native-sdk";

export type MvpAssuranceTargetDescriptor = Readonly<{
  name: MvpAssuranceTargetName;
  targetRef: string;
  repositoryPath: string;
  productSpecPath: string;
  assuranceSpec: Readonly<{ path: string; id: string; revision: number; gateRef: string; reviewId: string; admissionRef: string }>;
  paths: Readonly<{
    proposalFixture: string;
    environment: string;
    adapterLock: string;
    review: string;
    admission: string;
    manifest: string;
    evidenceIndex: string;
    fullGateReceipt: string;
    receiptRoot: string;
    runRoot: string;
  }>;
  criterion: Readonly<{ testPath: string; environmentRef: string; adapterRef: string; adapterSourcePaths: ReadonlyArray<string> }>;
  fullGate: Readonly<{
    argv: readonly [string, ...string[]];
    successMarker: string;
    smokeField: "electron_smoke" | "native_sdk_smoke";
    sourcePaths: ReadonlyArray<string>;
    hostGateFormat?: string;
  }>;
  dependencyLockPath: string;
  companionEvidenceRefs: ReadonlyArray<string>;
  targetSourcePaths: ReadonlyArray<string>;
}>;

export const electronMvpAssuranceTarget: MvpAssuranceTargetDescriptor = {
  name: "electron",
  targetRef: "openagents.desktop.current",
  repositoryPath: "apps/openagents-desktop",
  productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
  assuranceSpec: {
    path: "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
    id: "assurance.openagents.desktop.codex.workroom.mvp",
    revision: 2,
    gateRef: "GATE-MVP-FULL-ASSURANCE",
    reviewId: "review.openagents.desktop.mvp.assurance.2",
    admissionRef: "admission.openagents.desktop.mvp.assurance.2",
  },
  paths: {
    proposalFixture: "packages/assurance-spec/conformance/valid/mvp-proposal.assurance-spec.md",
    environment: "assurance/environments/openagents-desktop-mvp.assurance-environment.json",
    adapterLock: "assurance/openagents-desktop-mvp.adapter-lock.json",
    review: "assurance/openagents-desktop-mvp.assurance-review.json",
    admission: "assurance/openagents-desktop-mvp.assurance-admission.json",
    manifest: "assurance/openagents-desktop-mvp.assurance-manifest.json",
    evidenceIndex: "assurance/openagents-desktop-mvp.evidence-index.json",
    fullGateReceipt: "assurance/openagents-desktop-mvp.full-desktop-gate-receipt.json",
    receiptRoot: "assurance/receipts/openagents-desktop-mvp",
    runRoot: "var/assurance/openagents-desktop-mvp",
  },
  criterion: {
    testPath: "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
    environmentRef: "ENV-OA-DESKTOP-MVP-VITE-PLUS-1",
    adapterRef: "openagents.vite_plus_test.v1",
    adapterSourcePaths: ["packages/assurance-spec/src/vite-plus-test-adapter.ts"],
  },
  fullGate: {
    argv: ["pnpm", "--dir", "apps/openagents-desktop", "run", "verify"],
    successMarker: "[openagents-desktop smoke] OK",
    smokeField: "electron_smoke",
    sourcePaths: [
      "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
      "apps/openagents-desktop/src/renderer/assurance-spec-workspace.ts",
      "apps/openagents-desktop/package.json",
    ],
  },
  dependencyLockPath: "package.json",
  companionEvidenceRefs: [
    "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md",
    "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md",
    "assurance/openagents-desktop-mvp.full-desktop-gate-receipt.json",
  ],
  targetSourcePaths: [
    "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts",
    "apps/openagents-desktop/src/renderer/assurance-spec-workspace.ts",
    "apps/openagents-desktop/package.json",
  ],
};

export const nativeSdkMvpAssuranceTarget: MvpAssuranceTargetDescriptor = {
  name: "native-sdk",
  targetRef: "openagents.desktop.native-sdk.mvp",
  repositoryPath: "apps/native-sdk-effect-native-spike",
  productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
  assuranceSpec: {
    path: "docs/mvp/openagents-codex-workroom-mvp.native-sdk.assurance-spec.md",
    id: "assurance.openagents.desktop.codex.workroom.mvp.native-sdk",
    revision: 2,
    gateRef: "GATE-MVP-FULL-ASSURANCE-NATIVE-SDK",
    reviewId: "review.openagents.desktop.native-sdk.mvp.assurance.2",
    admissionRef: "admission.openagents.desktop.native-sdk.mvp.assurance.2",
  },
  paths: {
    proposalFixture: "packages/assurance-spec/conformance/valid/mvp-proposal.assurance-spec.md",
    environment: "assurance/environments/openagents-desktop-native-sdk-mvp.assurance-environment.json",
    adapterLock: "assurance/openagents-desktop-native-sdk-mvp.adapter-lock.json",
    review: "assurance/openagents-desktop-native-sdk-mvp.assurance-review.json",
    admission: "assurance/openagents-desktop-native-sdk-mvp.assurance-admission.json",
    manifest: "assurance/openagents-desktop-native-sdk-mvp.assurance-manifest.json",
    evidenceIndex: "assurance/openagents-desktop-native-sdk-mvp.evidence-index.json",
    fullGateReceipt: "assurance/openagents-desktop-native-sdk-mvp.full-native-gate-receipt.json",
    receiptRoot: "assurance/receipts/openagents-desktop-native-sdk-mvp",
    runRoot: "var/assurance/openagents-desktop-native-sdk-mvp",
  },
  criterion: {
    testPath: "apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts",
    environmentRef: "ENV-OA-DESKTOP-NATIVE-SDK-MACOS-1",
    adapterRef: "openagents.native_sdk_assurance.v1",
    adapterSourcePaths: [
      "packages/assurance-spec/src/vite-plus-test-adapter.ts",
      "packages/assurance-spec/src/native-sdk-assurance-adapter.ts",
    ],
  },
  fullGate: {
    argv: ["pnpm", "--dir", "apps/native-sdk-effect-native-spike", "run", "verify"],
    successMarker: "[native-sdk-effect-native-spike smoke] OK",
    smokeField: "native_sdk_smoke",
    hostGateFormat: "openagents.native-sdk.host-gate.v3",
    sourcePaths: [
      "apps/native-sdk-effect-native-spike/scripts/host-gate.ts",
      "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
      "apps/native-sdk-effect-native-spike/src/main.zig",
      "apps/native-sdk-effect-native-spike/frontend/src/program.ts",
      "apps/native-sdk-effect-native-spike/package.json",
      "apps/openagents-desktop/src/renderer/portable.ts",
      "apps/openagents-desktop/src/renderer/shell.ts",
      "apps/openagents-desktop/package.json",
    ],
  },
  dependencyLockPath: "pnpm-lock.yaml",
  companionEvidenceRefs: ["assurance/openagents-desktop-native-sdk-mvp.full-native-gate-receipt.json"],
  targetSourcePaths: [
    "apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts",
    "apps/native-sdk-effect-native-spike/frontend/src/program.ts",
    "apps/native-sdk-effect-native-spike/frontend/src/native-bridge.ts",
    "apps/native-sdk-effect-native-spike/frontend/src/production-command-parity.ts",
    "apps/native-sdk-effect-native-spike/scripts/host-gate.ts",
    "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
    "apps/native-sdk-effect-native-spike/src/main.zig",
    "apps/native-sdk-effect-native-spike/src/tests.zig",
    "apps/native-sdk-effect-native-spike/build.zig.zon",
    "apps/native-sdk-effect-native-spike/package.json",
    "apps/openagents-desktop/src/desktop-command-contract.ts",
    "apps/openagents-desktop/package.json",
    "apps/openagents-desktop/src/chat-contract.ts",
    "apps/openagents-desktop/src/desktop-coding-catalog.ts",
    "apps/openagents-desktop/src/renderer/app.css",
    "apps/openagents-desktop/src/renderer/command-registry.ts",
    "apps/openagents-desktop/src/renderer/portable.ts",
    "apps/openagents-desktop/src/renderer/shell.ts",
  ],
};

export const parseMvpAssuranceTargetArgs = (args: ReadonlyArray<string>): MvpAssuranceTargetDescriptor => {
  if (args.length === 0 || (args.length === 1 && args[0] === "--target=electron")) return electronMvpAssuranceTarget;
  if (args.length === 1 && args[0] === "--target=native-sdk") return nativeSdkMvpAssuranceTarget;
  throw new Error("usage: run-mvp-assurance.ts [--target=electron|native-sdk]");
};

export const mvpAssuranceTargetDescriptorDigest = (target: MvpAssuranceTargetDescriptor): string =>
  canonicalArtifact(target).digest;

export const mvpAssuranceTargetSourceDigest = (repositoryRoot: string, target: MvpAssuranceTargetDescriptor): string =>
  canonicalArtifact([...target.targetSourcePaths].sort().map((path) => ({
    path,
    digest: sha256Digest(readFileSync(resolve(repositoryRoot, path), "utf8")),
  }))).digest;
