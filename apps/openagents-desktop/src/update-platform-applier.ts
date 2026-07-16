import type { DesktopArtifactFormat, DesktopTargetKey } from "./release-staging-contract.ts";

export const childRuntimeKinds = [
  "agent",
  "pty",
  "local_server",
  "helper",
  "window",
  "wsl",
] as const;
export type ChildRuntimeKind = (typeof childRuntimeKinds)[number];

export type ChildRuntimeDrainReceipt = Readonly<{
  ok: boolean;
  drained: ReadonlyArray<ChildRuntimeKind>;
  timedOut: ReadonlyArray<ChildRuntimeKind>;
  elapsedMs: number;
}>;

export type PlatformUpdateResult =
  | Readonly<{
      ok: true;
      action: "installed" | "rolled_back";
      installedVersion: string;
      previousVersion: string | null;
    }>
  | Readonly<{ ok: false; reason: string }>;

/**
 * Common update-host boundary. Platform implementations own native signature,
 * executable-architecture, replacement, and retained-slot mechanics only.
 * Feed trust, monotonicity, lifecycle state, and renderer projection remain in
 * Electron main's common host.
 */
export type DesktopPlatformUpdateApplier = Readonly<{
  target: DesktopTargetKey;
  format: DesktopArtifactFormat;
  rollbackClaim: "retained_slot" | "none";
  rollbackAvailable: () => boolean;
  rollbackVersion: () => string | null;
  rollbackCompletionStatus?: () => "rolled_back" | null;
  armFirstLaunchRollback?: (
    input: Readonly<{
      receiptPath: string;
      expectedVersion: string;
      transactionRef: string;
      previousVersion: string;
      previousArchitecture: "arm64" | "x64";
      deadlineMs: number;
    }>,
  ) => Promise<boolean>;
  install: (
    artifactPath: string,
    candidateVersion: string,
    expectedApplicationArchitecture: "arm64" | "x64",
  ) => Promise<PlatformUpdateResult>;
  rollback: () => Promise<PlatformUpdateResult>;
}>;
