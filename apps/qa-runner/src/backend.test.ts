// Backend tests: localBackend provisions a real (here, fake-injected) browser
// session and tears down; cloudVmBackend is owner-gated and inert without an
// injected provisioner.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import {
  CloudVmBackendNotArmedError,
  cloudVmBackend,
  inertCloudVmProvisioner,
  localBackend,
  nativeDesktopDriver,
  NativeDesktopDriverNotImplementedError,
} from "./backend";
import { makeFakeChromium } from "./fake-chromium";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-backend-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "t", baseUrl: "https://example.test" });

describe("localBackend", () => {
  test("provisions a session that can acquire + flush a browser", async () => {
    const backend = localBackend({ chromium: makeFakeChromium() });
    expect(backend.name).toBe("local");
    const session = await backend.provision({ target, artifactDir: dir });
    const acquired = await session.acquireBrowser();
    expect(typeof acquired.flush).toBe("function");
    await acquired.flush();
    await session.teardown();
  });
});

describe("cloudVmBackend", () => {
  test("provision throws CloudVmBackendNotArmedError without a provisioner", async () => {
    const backend = cloudVmBackend();
    expect(backend.name).toBe("cloud-vm");
    await expect(backend.provision({ target, artifactDir: dir })).rejects.toBeInstanceOf(
      CloudVmBackendNotArmedError,
    );
  });

  test("inert provisioner errors honestly on both provision and provisionVm", async () => {
    const provisioner = inertCloudVmProvisioner();
    await expect(provisioner.provision({ target, artifactDir: dir })).rejects.toBeInstanceOf(
      CloudVmBackendNotArmedError,
    );
    await expect(
      provisioner.provisionVm({ target, artifactDir: dir, os: "linux" }),
    ).rejects.toBeInstanceOf(CloudVmBackendNotArmedError);
  });

  test("cloudVmBackend wired with the inert provisioner still errors honestly (no fake green)", async () => {
    const backend = cloudVmBackend({ provisioner: inertCloudVmProvisioner() });
    await expect(backend.provision({ target, artifactDir: dir })).rejects.toBeInstanceOf(
      CloudVmBackendNotArmedError,
    );
  });
});

describe("nativeDesktopDriver (spec-only stub)", () => {
  test("throws NativeDesktopDriverNotImplementedError — not a working driver", () => {
    expect(() => nativeDesktopDriver("macos")).toThrow(NativeDesktopDriverNotImplementedError);
    expect(() => nativeDesktopDriver("windows")).toThrow(NativeDesktopDriverNotImplementedError);
  });
});
