import {
  chmodSync,
  closeSync,
  copyFileSync,
  existsSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";

import { isMonotonicUpgrade, type UpdateChannel } from "./update-contract.ts";
import type {
  DesktopPlatformUpdateApplier,
  PlatformUpdateResult,
} from "./update-platform-applier.ts";

export const LINUX_UPDATE_TRANSACTION_SCHEMA =
  "openagents.desktop.linux_appimage_update_transaction.v1" as const;

type LinuxArchitecture = "arm64" | "x64";
type LinuxTransaction = Readonly<{
  schema: typeof LINUX_UPDATE_TRANSACTION_SCHEMA;
  status: "prepared" | "installed" | "rollback_prepared" | "rolled_back" | "healthy";
  previousVersion: string | null;
  installedVersion: string;
  previousImage: string | null;
  installedImage: string;
  channel: UpdateChannel;
  architecture: LinuxArchitecture;
}>;

export type LinuxUpdateFailureReason =
  | "unsupported_platform"
  | "not_packaged"
  | "current_image_unavailable"
  | "artifact_missing"
  | "artifact_not_regular"
  | "appimage_invalid"
  | "architecture_mismatch"
  | "version_mismatch"
  | "candidate_not_monotonic"
  | "backup_failed"
  | "install_failed"
  | "watchdog_failed"
  | "rollback_unavailable"
  | "rollback_failed";

export type LinuxAppImageUpdateApplier = DesktopPlatformUpdateApplier &
  Readonly<{
    selectedImagePath: string;
    install: (
      artifactPath: string,
      candidateVersion: string,
      expectedApplicationArchitecture?: LinuxArchitecture,
    ) => Promise<PlatformUpdateResult>;
  }>;

const safeVersion = (value: unknown): value is string =>
  typeof value === "string" &&
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/.test(value);

const fsyncPath = (value: string): void => {
  const descriptor = openSync(value, "r");
  try {
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
};

const writeAtomic = (file: string, value: string, mode: number): void => {
  const temporary = `${file}.tmp`;
  writeFileSync(temporary, value, { encoding: "utf8", mode });
  chmodSync(temporary, mode);
  fsyncPath(temporary);
  renameSync(temporary, file);
  fsyncPath(path.dirname(file));
};

const readTransaction = (file: string): LinuxTransaction | null => {
  try {
    const value = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
    if (
      value.schema !== LINUX_UPDATE_TRANSACTION_SCHEMA ||
      !["prepared", "installed", "rollback_prepared", "rolled_back", "healthy"].includes(
        String(value.status),
      ) ||
      (value.channel !== "stable" && value.channel !== "rc") ||
      (value.architecture !== "arm64" && value.architecture !== "x64") ||
      !safeVersion(value.installedVersion) ||
      (value.previousVersion !== null && !safeVersion(value.previousVersion)) ||
      typeof value.installedImage !== "string" ||
      (value.previousImage !== null && typeof value.previousImage !== "string")
    ) {
      return null;
    }
    return value as LinuxTransaction;
  } catch {
    return null;
  }
};

const expectedElfMachine = (architecture: LinuxArchitecture): number =>
  architecture === "arm64" ? 183 : 62;

export const inspectAppImage = (
  file: string,
  architecture: LinuxArchitecture,
): LinuxUpdateFailureReason | null => {
  if (!existsSync(file)) return "artifact_missing";
  let stat;
  try {
    stat = lstatSync(file);
  } catch {
    return "artifact_missing";
  }
  if (!stat.isFile() || stat.isSymbolicLink()) return "artifact_not_regular";
  const descriptor = openSync(file, "r");
  try {
    const header = Buffer.alloc(20);
    if (readSync(descriptor, header, 0, header.length, 0) !== header.length)
      return "appimage_invalid";
    if (
      header[0] !== 0x7f ||
      header[1] !== 0x45 ||
      header[2] !== 0x4c ||
      header[3] !== 0x46 ||
      header[4] !== 2 ||
      header[5] !== 1 ||
      header[8] !== 0x41 ||
      header[9] !== 0x49 ||
      header[10] !== 2
    ) {
      return "appimage_invalid";
    }
    return header.readUInt16LE(18) === expectedElfMachine(architecture)
      ? null
      : "architecture_mismatch";
  } finally {
    closeSync(descriptor);
  }
};

const copyImageAtomically = (source: string, destination: string): boolean => {
  try {
    mkdirSync(path.dirname(destination), { recursive: true, mode: 0o700 });
    const temporary = `${destination}.tmp`;
    rmSync(temporary, { force: true });
    copyFileSync(source, temporary);
    chmodSync(temporary, 0o755);
    fsyncPath(temporary);
    renameSync(temporary, destination);
    fsyncPath(path.dirname(destination));
    return true;
  } catch {
    return false;
  }
};

const selectImageAtomically = (selectedImage: string, image: string): boolean => {
  try {
    const parent = path.dirname(selectedImage);
    mkdirSync(parent, { recursive: true, mode: 0o700 });
    const temporary = `${selectedImage}.tmp`;
    rmSync(temporary, { force: true });
    symlinkSync(path.relative(parent, image), temporary);
    renameSync(temporary, selectedImage);
    fsyncPath(parent);
    return true;
  } catch {
    return false;
  }
};

export const openLinuxAppImageUpdateApplier = (
  input: Readonly<{
    root: string;
    currentImagePath: string | null;
    installedVersion: string;
    channel: UpdateChannel;
    platform?: NodeJS.Platform;
    packaged: boolean;
    targetArchitecture?: LinuxArchitecture;
    spawnWatchdog?: (executable: string, args: ReadonlyArray<string>) => boolean;
  }>,
): LinuxAppImageUpdateApplier => {
  const platform = input.platform ?? process.platform;
  const architecture = input.targetArchitecture ?? (process.arch === "arm64" ? "arm64" : "x64");
  const target = architecture === "arm64" ? "linux-arm64" : "linux-x64";
  const imagesRoot = path.join(input.root, "appimages");
  const selectedImagePath = path.join(imagesRoot, "current.AppImage");
  const transactionFile = path.join(input.root, "linux-appimage-transaction.json");
  const watchdogResult = path.join(input.root, "first-launch-watchdog.result");
  let transaction = readTransaction(transactionFile);

  const writeTransaction = (value: LinuxTransaction): boolean => {
    try {
      mkdirSync(input.root, { recursive: true, mode: 0o700 });
      chmodSync(input.root, 0o700);
      writeAtomic(transactionFile, `${JSON.stringify(value)}\n`, 0o600);
      transaction = value;
      return true;
    } catch {
      return false;
    }
  };

  const supported = (): LinuxUpdateFailureReason | null =>
    platform !== "linux"
      ? "unsupported_platform"
      : !input.packaged
        ? "not_packaged"
        : input.currentImagePath === null || !path.isAbsolute(input.currentImagePath)
          ? "current_image_unavailable"
          : null;

  const rollbackAvailable = (): boolean =>
    supported() === null &&
    transaction !== null &&
    (transaction.status === "installed" ||
      transaction.status === "prepared" ||
      transaction.status === "rollback_prepared") &&
    transaction.channel === input.channel &&
    transaction.previousVersion !== null &&
    transaction.previousImage !== null &&
    existsSync(transaction.previousImage) &&
    inspectAppImage(transaction.previousImage, transaction.architecture) === null;

  const rollbackVersion = (): string | null =>
    rollbackAvailable() ? (transaction?.previousVersion ?? null) : null;

  const rollbackCompletionStatus = (): "rolled_back" | null =>
    transaction?.status === "rolled_back" ? "rolled_back" : null;

  const rollback = async (): Promise<PlatformUpdateResult> => {
    const current = transaction;
    if (
      !rollbackAvailable() ||
      current === null ||
      current.previousVersion === null ||
      current.previousImage === null
    ) {
      return { ok: false, reason: "rollback_unavailable" };
    }
    const previousVersion = current.previousVersion;
    const previousImage = current.previousImage;
    const prepared: LinuxTransaction = { ...current, status: "rollback_prepared" };
    if (!writeTransaction(prepared) || !selectImageAtomically(selectedImagePath, previousImage)) {
      return { ok: false, reason: "rollback_failed" };
    }
    const rolledBack: LinuxTransaction = {
      ...prepared,
      status: "rolled_back",
      previousVersion: null,
      previousImage: null,
      installedVersion: previousVersion,
      installedImage: previousImage,
    };
    if (!writeTransaction(rolledBack)) return { ok: false, reason: "rollback_failed" };
    return {
      ok: true,
      action: "rolled_back",
      installedVersion: previousVersion,
      previousVersion: null,
    };
  };

  const install = async (
    artifactPath: string,
    candidateVersion: string,
    expectedApplicationArchitecture = architecture,
  ): Promise<PlatformUpdateResult> => {
    const unavailable = supported();
    if (unavailable !== null) return { ok: false, reason: unavailable };
    if (!safeVersion(candidateVersion)) return { ok: false, reason: "version_mismatch" };
    if (!isMonotonicUpgrade(input.installedVersion, candidateVersion, input.channel).admissible) {
      return { ok: false, reason: "candidate_not_monotonic" };
    }
    const candidateProblem = inspectAppImage(artifactPath, expectedApplicationArchitecture);
    if (candidateProblem !== null) return { ok: false, reason: candidateProblem };
    const currentImage = input.currentImagePath!;
    const currentProblem = inspectAppImage(currentImage, architecture);
    if (currentProblem !== null) return { ok: false, reason: "current_image_unavailable" };

    const previousImage = path.join(
      imagesRoot,
      `OpenAgents-${input.installedVersion}-${architecture}.AppImage`,
    );
    const candidateImage = path.join(
      imagesRoot,
      `OpenAgents-${candidateVersion}-${architecture}.AppImage`,
    );
    if (!copyImageAtomically(currentImage, previousImage))
      return { ok: false, reason: "backup_failed" };
    if (!copyImageAtomically(artifactPath, candidateImage))
      return { ok: false, reason: "install_failed" };
    const prepared: LinuxTransaction = {
      schema: LINUX_UPDATE_TRANSACTION_SCHEMA,
      status: "prepared",
      previousVersion: input.installedVersion,
      installedVersion: candidateVersion,
      previousImage,
      installedImage: candidateImage,
      channel: input.channel,
      architecture,
    };
    if (!writeTransaction(prepared) || !selectImageAtomically(selectedImagePath, candidateImage)) {
      selectImageAtomically(selectedImagePath, previousImage);
      return { ok: false, reason: "install_failed" };
    }
    if (!writeTransaction({ ...prepared, status: "installed" })) {
      selectImageAtomically(selectedImagePath, previousImage);
      return { ok: false, reason: "install_failed" };
    }
    return {
      ok: true,
      action: "installed",
      installedVersion: candidateVersion,
      previousVersion: input.installedVersion,
    };
  };

  const armFirstLaunchRollback = async (
    receipt: Readonly<{
      receiptPath: string;
      expectedVersion: string;
      transactionRef: string;
      previousVersion: string;
      previousArchitecture: LinuxArchitecture;
      deadlineMs: number;
    }>,
  ): Promise<boolean> => {
    if (
      !rollbackAvailable() ||
      transaction === null ||
      transaction.installedVersion !== receipt.expectedVersion ||
      transaction.previousVersion !== receipt.previousVersion ||
      transaction.architecture !== receipt.previousArchitecture ||
      !/^[a-f0-9]{32}$/.test(receipt.transactionRef)
    ) {
      return false;
    }
    const script = path.join(input.root, "linux-appimage-first-launch-watchdog.sh");
    const resultFile = watchdogResult;
    const deadlineSeconds = Math.floor(receipt.deadlineMs / 1000);
    const expectedVersion = receipt.expectedVersion.replaceAll("'", "");
    const transactionRef = receipt.transactionRef;
    const previousImage = transaction.previousImage!;
    const installedImage = transaction.installedImage;
    const body = `#!/bin/sh
set -eu
receipt=$1
deadline=$2
transaction=$3
selected=$4
previous=$5
installed=$6
result=$7
expected_version=$8
transaction_ref=$9
previous_version=\${10}
sync_file() {
  sync -f "$1"
  sync -f "$(dirname "$1")"
}
while [ "$(date +%s)" -le "$deadline" ]; do
  if [ -f "$receipt" ] && grep -Fq '"version":"'"$expected_version"'"' "$receipt" && grep -Fq '"transactionRef":"'"$transaction_ref"'"' "$receipt" && grep -Fq '"cleanShutdownAt":"' "$receipt"; then
    printf '{"schema":"${LINUX_UPDATE_TRANSACTION_SCHEMA}","status":"healthy","previousVersion":null,"installedVersion":"%s","previousImage":null,"installedImage":"%s","channel":"${input.channel}","architecture":"${architecture}"}\n' "$expected_version" "$installed" > "$transaction.tmp"
    chmod 600 "$transaction.tmp"
    sync_file "$transaction.tmp"
    mv -f "$transaction.tmp" "$transaction"
    sync_file "$transaction"
    printf '%s\n' healthy > "$result.tmp"
    chmod 600 "$result.tmp"
    sync_file "$result.tmp"
    mv -f "$result.tmp" "$result"
    sync_file "$result"
    rm -f "$previous"
    exit 0
  fi
  sleep 2
done
printf '%s\n' rollback_prepared > "$result.tmp"
chmod 600 "$result.tmp"
sync_file "$result.tmp"
mv -f "$result.tmp" "$result"
sync_file "$result"
printf '{"schema":"${LINUX_UPDATE_TRANSACTION_SCHEMA}","status":"rollback_prepared","previousVersion":"%s","installedVersion":"%s","previousImage":"%s","installedImage":"%s","channel":"${input.channel}","architecture":"${architecture}"}\n' "$previous_version" "$expected_version" "$previous" "$installed" > "$transaction.tmp"
chmod 600 "$transaction.tmp"
sync_file "$transaction.tmp"
mv -f "$transaction.tmp" "$transaction"
sync_file "$transaction"
rm -f "$selected.tmp"
ln -s "$previous" "$selected.tmp"
mv -Tf "$selected.tmp" "$selected"
sync_file "$selected"
printf '{"schema":"${LINUX_UPDATE_TRANSACTION_SCHEMA}","status":"rolled_back","previousVersion":null,"installedVersion":"%s","previousImage":null,"installedImage":"%s","channel":"${input.channel}","architecture":"${architecture}"}\n' "$previous_version" "$previous" > "$transaction.tmp"
chmod 600 "$transaction.tmp"
sync_file "$transaction.tmp"
mv -f "$transaction.tmp" "$transaction"
sync_file "$transaction"
printf '%s\n' rolled_back > "$result.tmp"
chmod 600 "$result.tmp"
sync_file "$result.tmp"
mv -f "$result.tmp" "$result"
sync_file "$result"
chmod 755 "$previous"
nohup "$previous" >/dev/null 2>&1 &
`;
    try {
      writeAtomic(script, body, 0o700);
      rmSync(resultFile, { force: true });
      const args = [
        script,
        receipt.receiptPath,
        String(deadlineSeconds),
        transactionFile,
        selectedImagePath,
        previousImage,
        installedImage,
        resultFile,
        expectedVersion,
        transactionRef,
        receipt.previousVersion,
      ];
      if (input.spawnWatchdog !== undefined) return input.spawnWatchdog("/bin/sh", args);
      const child = spawn("/bin/sh", args, { detached: true, stdio: "ignore" });
      child.unref();
      return true;
    } catch {
      return false;
    }
  };

  return {
    target,
    format: "appimage",
    rollbackClaim: "retained_slot",
    selectedImagePath,
    rollbackAvailable,
    rollbackVersion,
    rollbackCompletionStatus,
    armFirstLaunchRollback,
    install,
    rollback,
  };
};
