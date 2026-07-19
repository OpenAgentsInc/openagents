import { afterEach, describe, expect, test } from "vite-plus/test";
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  inspectAppImage,
  LINUX_UPDATE_TRANSACTION_SCHEMA,
  openLinuxAppImageUpdateApplier,
} from "./linux-update-applier.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

const appImage = (file: string, architecture: "arm64" | "x64", marker: string): void => {
  const bytes = Buffer.alloc(128);
  bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1, 0, 0x41, 0x49, 2]);
  bytes.writeUInt16LE(architecture === "arm64" ? 183 : 62, 18);
  bytes.write(marker, 32);
  writeFileSync(file, bytes, { mode: 0o755 });
};

const fixture = (architecture: "arm64" | "x64" = "x64") => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-linux-update-"));
  roots.push(root);
  const current = path.join(root, "OpenAgents-0.1.0-rc.20.AppImage");
  const candidate = path.join(root, "OpenAgents-0.1.0-rc.21.AppImage");
  appImage(current, architecture, "current");
  appImage(candidate, architecture, "candidate");
  const updates = path.join(root, "updates");
  const spawns: Array<{ executable: string; args: ReadonlyArray<string> }> = [];
  const open = (version = "0.1.0-rc.20") =>
    openLinuxAppImageUpdateApplier({
      root: updates,
      currentImagePath: current,
      installedVersion: version,
      channel: "rc",
      platform: "linux",
      packaged: true,
      targetArchitecture: architecture,
      spawnWatchdog: (executable, args) => {
        spawns.push({ executable, args });
        return true;
      },
    });
  return { root, current, candidate, updates, spawns, open };
};

describe("Linux AppImage retained-slot update applier", () => {
  test("verifies native AppImage identity, retains one previous image, and atomically selects the candidate", async () => {
    const h = fixture();
    const applier = h.open();

    expect(await applier.install(h.candidate, "0.1.0-rc.21", "x64")).toEqual({
      ok: true,
      action: "installed",
      installedVersion: "0.1.0-rc.21",
      previousVersion: "0.1.0-rc.20",
    });
    expect(applier.target).toBe("linux-x64");
    expect(applier.format).toBe("appimage");
    expect(applier.rollbackClaim).toBe("retained_slot");
    expect(lstatSync(applier.selectedImagePath).isSymbolicLink()).toBe(true);
    expect(readlinkSync(applier.selectedImagePath)).toContain(
      "OpenAgents-0.1.0-rc.21-x64.AppImage",
    );
    expect(readFileSync(applier.selectedImagePath).subarray(32, 41).toString()).toBe("candidate");
    expect(lstatSync(applier.selectedImagePath).mode & 0o111).toBe(0o111);
    expect(applier.rollbackAvailable()).toBe(true);
    expect(applier.rollbackVersion()).toBe("0.1.0-rc.20");
    expect(
      JSON.parse(readFileSync(path.join(h.updates, "linux-appimage-transaction.json"), "utf8")),
    ).toMatchObject({
      schema: LINUX_UPDATE_TRANSACTION_SCHEMA,
      status: "installed",
      previousVersion: "0.1.0-rc.20",
      installedVersion: "0.1.0-rc.21",
      architecture: "x64",
    });
  });

  test("restart discovers exactly one retained slot and rollback atomically reselects it", async () => {
    const h = fixture("arm64");
    const applier = h.open();
    expect((await applier.install(h.candidate, "0.1.0-rc.21", "arm64")).ok).toBe(true);

    const restarted = openLinuxAppImageUpdateApplier({
      root: h.updates,
      currentImagePath: applier.selectedImagePath,
      installedVersion: "0.1.0-rc.21",
      channel: "rc",
      platform: "linux",
      packaged: true,
      targetArchitecture: "arm64",
    });
    expect(restarted.target).toBe("linux-arm64");
    expect(restarted.rollbackVersion()).toBe("0.1.0-rc.20");
    expect(await restarted.rollback()).toEqual({
      ok: true,
      action: "rolled_back",
      installedVersion: "0.1.0-rc.20",
      previousVersion: null,
    });
    expect(readlinkSync(restarted.selectedImagePath)).toContain(
      "OpenAgents-0.1.0-rc.20-arm64.AppImage",
    );
    expect(restarted.rollbackAvailable()).toBe(false);
    expect(restarted.rollbackCompletionStatus?.()).toBe("rolled_back");
    expect(await restarted.rollback()).toEqual({ ok: false, reason: "rollback_unavailable" });
  });

  test("rejects foreign architecture, malformed bytes, symlinks, downgrade, dev mode, and non-AppImage installs", async () => {
    const h = fixture();
    const arm = path.join(h.root, "foreign.AppImage");
    const malformed = path.join(h.root, "malformed.AppImage");
    const link = path.join(h.root, "linked.AppImage");
    appImage(arm, "arm64", "foreign");
    writeFileSync(malformed, "not an appimage");
    symlinkSync(h.candidate, link);

    expect(inspectAppImage(arm, "x64")).toBe("architecture_mismatch");
    expect(inspectAppImage(malformed, "x64")).toBe("appimage_invalid");
    expect(inspectAppImage(link, "x64")).toBe("artifact_not_regular");
    expect(await h.open().install(arm, "0.1.0-rc.21", "x64")).toEqual({
      ok: false,
      reason: "architecture_mismatch",
    });
    expect(await h.open().install(h.candidate, "0.1.0-rc.19", "x64")).toEqual({
      ok: false,
      reason: "candidate_not_monotonic",
    });
    expect(
      await openLinuxAppImageUpdateApplier({
        root: h.updates,
        currentImagePath: h.current,
        installedVersion: "0.1.0-rc.20",
        channel: "rc",
        platform: "darwin",
        packaged: true,
        targetArchitecture: "x64",
      }).install(h.candidate, "0.1.0-rc.21", "x64"),
    ).toEqual({ ok: false, reason: "unsupported_platform" });
    expect(
      await openLinuxAppImageUpdateApplier({
        root: h.updates,
        currentImagePath: h.current,
        installedVersion: "0.1.0-rc.20",
        channel: "rc",
        platform: "linux",
        packaged: false,
        targetArchitecture: "x64",
      }).install(h.candidate, "0.1.0-rc.21", "x64"),
    ).toEqual({ ok: false, reason: "not_packaged" });
    expect(
      await openLinuxAppImageUpdateApplier({
        root: h.updates,
        currentImagePath: null,
        installedVersion: "0.1.0-rc.20",
        channel: "rc",
        platform: "linux",
        packaged: true,
        targetArchitecture: "x64",
      }).install(h.candidate, "0.1.0-rc.21", "x64"),
    ).toEqual({ ok: false, reason: "current_image_unavailable" });
    expect(existsSync(path.join(h.updates, "appimages", "current.AppImage"))).toBe(false);
  });

  test("staged downloads become executable only inside the verified immutable slot", async () => {
    const h = fixture();
    chmodSync(h.candidate, 0o600);
    const applier = h.open();
    expect((await applier.install(h.candidate, "0.1.0-rc.21", "x64")).ok).toBe(true);
    expect(lstatSync(h.candidate).mode & 0o111).toBe(0);
    expect(lstatSync(applier.selectedImagePath).mode & 0o111).toBe(0o111);
  });

  test("arms a bounded first-launch watchdog bound to exact version, transaction, and retained image", async () => {
    const h = fixture();
    const applier = h.open();
    expect((await applier.install(h.candidate, "0.1.0-rc.21", "x64")).ok).toBe(true);
    const receiptPath = path.join(h.updates, "launch-receipt.json");
    expect(
      await applier.armFirstLaunchRollback?.({
        receiptPath,
        expectedVersion: "0.1.0-rc.21",
        transactionRef: "a".repeat(32),
        previousVersion: "0.1.0-rc.20",
        previousArchitecture: "x64",
        deadlineMs: 2_000_000_000_000,
      }),
    ).toBe(true);
    expect(h.spawns).toHaveLength(1);
    expect(h.spawns[0]!.executable).toBe("/bin/sh");
    const script = readFileSync(
      path.join(h.updates, "linux-appimage-first-launch-watchdog.sh"),
      "utf8",
    );
    expect(script).toContain('grep -Fq \'"transactionRef":"\'"$transaction_ref"\'"\'');
    expect(script).toContain('mv -Tf "$selected.tmp" "$selected"');
    expect(script).toContain(`"schema":"${LINUX_UPDATE_TRANSACTION_SCHEMA}"`);
    expect(script).toContain('nohup "$previous"');
    expect(
      await applier.armFirstLaunchRollback?.({
        receiptPath,
        expectedVersion: "0.1.0-rc.999",
        transactionRef: "b".repeat(32),
        previousVersion: "0.1.0-rc.20",
        previousArchitecture: "x64",
        deadlineMs: 2_000_000_000_000,
      }),
    ).toBe(false);
  });

  test("fails closed when the current image has been replaced by a foreign or malformed file", async () => {
    const h = fixture();
    appImage(h.current, "arm64", "foreign");
    expect(await h.open().install(h.candidate, "0.1.0-rc.21", "x64")).toEqual({
      ok: false,
      reason: "current_image_unavailable",
    });
    expect(existsSync(path.join(h.updates, "appimages", "current.AppImage"))).toBe(false);

    mkdirSync(path.dirname(h.current), { recursive: true });
  });
});
