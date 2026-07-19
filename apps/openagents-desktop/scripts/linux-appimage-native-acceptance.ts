import { createHash, randomBytes } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  inspectAppImage,
  openLinuxAppImageUpdateApplier,
} from "../src/linux-update-applier.ts";

const argv = process.argv.slice(2);
const flag = (name: string): string => {
  const index = argv.indexOf(`--${name}`);
  const value = index >= 0 ? argv[index + 1] : undefined;
  if (value === undefined) throw new Error(`missing --${name}`);
  return value;
};

const architecture = flag("architecture");
if (architecture !== "x64" && architecture !== "arm64") {
  throw new Error("--architecture must be x64 or arm64");
}
const current = path.resolve(flag("current"));
const candidate = path.resolve(flag("candidate"));
const currentSha256 = flag("current-sha256");
const candidateSha256 = flag("candidate-sha256");

const sha256 = (file: string): string =>
  createHash("sha256").update(readFileSync(file)).digest("hex");
const waitFor = async (file: string, expected: string): Promise<string> => {
  const deadline = Date.now() + 15_000;
  let observed = "missing";
  while (Date.now() < deadline) {
    if (existsSync(file)) {
      observed = readFileSync(file, "utf8").trim();
      if (observed === expected) return observed;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${expected} in ${file}; observed ${observed}`);
};
function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}
const selectedTarget = (selected: string): string =>
  path.resolve(path.dirname(selected), readlinkSync(selected));

assert(sha256(current) === currentSha256, "current AppImage digest mismatch");
assert(sha256(candidate) === candidateSha256, "candidate AppImage digest mismatch");
assert(inspectAppImage(current, architecture) === null, "current AppImage identity invalid");
assert(inspectAppImage(candidate, architecture) === null, "candidate AppImage identity invalid");

const runInstall = (root: string) =>
  openLinuxAppImageUpdateApplier({
    root,
    currentImagePath: current,
    installedVersion: "0.1.0-rc.21",
    channel: "rc",
    platform: "linux",
    packaged: true,
    targetArchitecture: architecture,
  });

const rollbackRoot = mkdtempSync(path.join(tmpdir(), `oa-linux-${architecture}-rollback-`));
const rollbackApplier = runInstall(rollbackRoot);
const installed = await rollbackApplier.install(candidate, "0.1.0-rc.23", architecture);
assert(installed.ok && installed.action === "installed", "candidate install failed");
const installedPath = selectedTarget(rollbackApplier.selectedImagePath);
assert(sha256(installedPath) === candidateSha256, "selected candidate bytes drifted");
assert((statSync(installedPath).mode & 0o777) === 0o755, "selected candidate mode is not 0755");
const rollbackWatchdog = rollbackApplier.armFirstLaunchRollback;
assert(rollbackWatchdog !== undefined, "rollback watchdog is unavailable");
const rollbackArmed = await rollbackWatchdog({
  receiptPath: path.join(rollbackRoot, "first-launch.json"),
  expectedVersion: "0.1.0-rc.23",
  transactionRef: randomBytes(16).toString("hex"),
  previousVersion: "0.1.0-rc.21",
  previousArchitecture: architecture,
  deadlineMs: Date.now() - 1_000,
});
assert(rollbackArmed, "rollback watchdog did not arm");
const rollbackStatus = await waitFor(
  path.join(rollbackRoot, "first-launch-watchdog.result"),
  "rolled_back",
);
assert(rollbackStatus === "rolled_back", `unexpected rollback status ${rollbackStatus}`);
assert(
  sha256(selectedTarget(rollbackApplier.selectedImagePath)) === currentSha256,
  "rollback did not restore exact previous bytes",
);

const healthyRoot = mkdtempSync(path.join(tmpdir(), `oa-linux-${architecture}-healthy-`));
const healthyApplier = runInstall(healthyRoot);
const healthyInstall = await healthyApplier.install(candidate, "0.1.0-rc.23", architecture);
assert(healthyInstall.ok && healthyInstall.action === "installed", "healthy install failed");
const transactionRef = randomBytes(16).toString("hex");
const receiptPath = path.join(healthyRoot, "first-launch.json");
writeFileSync(
  receiptPath,
  `${JSON.stringify({
    version: "0.1.0-rc.23",
    transactionRef,
    cleanShutdownAt: new Date().toISOString(),
  })}\n`,
  { mode: 0o600 },
);
chmodSync(receiptPath, 0o600);
const healthyWatchdog = healthyApplier.armFirstLaunchRollback;
assert(healthyWatchdog !== undefined, "healthy watchdog is unavailable");
const healthyArmed = await healthyWatchdog({
  receiptPath,
  expectedVersion: "0.1.0-rc.23",
  transactionRef,
  previousVersion: "0.1.0-rc.21",
  previousArchitecture: architecture,
  deadlineMs: Date.now() + 10_000,
});
assert(healthyArmed, "healthy watchdog did not arm");
const healthyStatus = await waitFor(
  path.join(healthyRoot, "first-launch-watchdog.result"),
  "healthy",
);
assert(healthyStatus === "healthy", `unexpected healthy status ${healthyStatus}`);
assert(
  sha256(selectedTarget(healthyApplier.selectedImagePath)) === candidateSha256,
  "healthy commit did not retain candidate bytes",
);

process.stdout.write(
  `${JSON.stringify({
    schema: "openagents.desktop.linux_appimage_native_acceptance.v1",
    architecture,
    currentSha256,
    candidateSha256,
    install: "pass",
    selectedMode: "0755",
    rollback: "pass",
    healthyCommit: "pass",
  })}\n`,
);
