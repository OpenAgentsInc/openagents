// Backend tests: localBackend provisions a real (here, fake-injected) browser
// session and tears down; cloudVmBackend is owner-gated and inert without an
// injected provisioner.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import { CloudVmBackendNotArmedError, cloudVmBackend, localBackend } from "./backend";
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
});
