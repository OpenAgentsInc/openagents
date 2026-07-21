// Real release ports (#8917 coordinator, #8922 feed): port selection, the real
// feed port against injected effects, and the coordinator's honest owner-gate
// refusal at inventory bind versus the happy inventory path once the owner
// attests the native acceptance hosts.
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { ReleaseIo, ReleasePlan, ReleaseTargetKey } from "./release.js";
import { releaseTargetKeys } from "./release.js";
import {
  type CommandResult,
  type HttpResponse,
  createRealCoordinatorPort,
  createRealFeedPort,
  createReleasePorts,
} from "./release-ports-real.js";

const scratches: string[] = [];
afterEach(() => {
  while (scratches.length > 0) rmSync(scratches.pop()!, { recursive: true, force: true });
});

const makeIo = (env: Readonly<Record<string, string | undefined>> = {}): ReleaseIo => {
  const scratchDir = mkdtempSync(join(tmpdir(), "release-ports-"));
  scratches.push(scratchDir);
  return {
    rootDir: join(scratchDir, "root"),
    scratchDir,
    log: () => undefined,
    env,
    now: () => new Date(0),
  };
};

const makePlan = (mode: "dry-run" | "real"): ReleasePlan => ({
  transactionRef: "v0.1.0-rc.99-rc-19700101T000000Z",
  mode,
  version: "0.1.0-rc.99",
  channel: "rc",
  sourceRevision: "0".repeat(40),
  targets: releaseTargetKeys,
  date: "1970-01-01",
  unattended: true,
  approvedGates: [],
  attribution: {
    triggerKind: "owner_direction",
    triggeredBy: "owner (test)",
    releaseActor: "OpenAgents release operator",
    authorityRef: "test",
    releaseUrl: "https://github.com/OpenAgentsInc/openagents/releases/tag/test",
    sourceFeedback: "none recorded",
  },
});

const dummyKey = { kid: "test-kid", d: "", x: "" };
const fullAttestations: Readonly<Record<ReleaseTargetKey, string>> = {
  "darwin-arm64": "openagents.desktop.acceptance.darwin-arm64.attested",
  "darwin-x64": "openagents.desktop.acceptance.darwin-x64.attested",
  "linux-x64": "openagents.desktop.acceptance.linux-x64.attested",
  "linux-arm64": "openagents.desktop.acceptance.linux-arm64.attested",
};

const okRun = async (): Promise<CommandResult> => ({ code: 0, stdout: "", stderr: "" });
const okHttp = async (): Promise<HttpResponse> => ({ status: 200, body: "" });

describe("createReleasePorts", () => {
  test("dry-run selects fixture ports", () => {
    const ports = createReleasePorts(makePlan("dry-run"), makeIo());
    expect(ports.coordinator.kind).toBe("fixture");
    expect(ports.feed.kind).toBe("fixture");
  });

  test("real mode selects real ports", () => {
    const ports = createReleasePorts(makePlan("real"), makeIo(), {
      coordinator: { signingKey: dummyKey, attestations: fullAttestations },
    });
    expect(ports.coordinator.kind).toBe("real");
    expect(ports.feed.kind).toBe("real");
  });

  test("real mode without a signing key is refused", () => {
    expect(() => createReleasePorts(makePlan("real"), makeIo())).toThrow(/signing key unavailable/);
  });
});

describe("createRealFeedPort", () => {
  test("deployCandidateFeed returns public-safe receipt lines on success", async () => {
    let captured: { env?: Record<string, string | undefined> } = {};
    const feed = createRealFeedPort(makeIo({ CLOUDSDK_CONFIG: "/x" }), {
      effects: {
        run: async (request) => {
          captured = { env: { ...request.env } };
          return { code: 0, stdout: "", stderr: "" };
        },
      },
    });
    const result = await feed.deployCandidateFeed(makePlan("real"));
    expect(result.receiptLines.length).toBe(2);
    expect(result.receiptLines[0]).toContain("v0.1.0-rc.99-rc");
    expect(captured.env?.OA_RELEASE_SET_BUCKET).toContain("release-set");
    expect(captured.env?.OA_RELEASE_SET_PINS_PATH).toContain("release-set-pins.json");
  });

  test("deployCandidateFeed fails closed on a non-zero exit", async () => {
    const feed = createRealFeedPort(makeIo(), {
      effects: { run: async () => ({ code: 1, stdout: "", stderr: "boom" }) },
    });
    await expect(feed.deployCandidateFeed(makePlan("real"))).rejects.toThrow(/feed deploy failed/);
  });

  test("smokeCandidate requires the mobile OTA manifest to stay served", async () => {
    const ok = createRealFeedPort(makeIo(), { effects: { run: okRun, httpGet: okHttp } });
    const result = await ok.smokeCandidate(makePlan("real"));
    expect(result.receiptLines.some((line) => line.includes("mobile OTA preserved"))).toBe(true);

    const broken = createRealFeedPort(makeIo(), {
      effects: { run: okRun, httpGet: async () => ({ status: 503, body: "" }) },
    });
    await expect(broken.smokeCandidate(makePlan("real"))).rejects.toThrow(/mobile OTA/);
  });

  test("verifyPublicSurfaces fails closed when the release-set is not served", async () => {
    const feed = createRealFeedPort(makeIo(), {
      effects: { run: okRun, httpGet: async () => ({ status: 404, body: "" }) },
    });
    await expect(feed.verifyPublicSurfaces(makePlan("real"))).rejects.toThrow(/release-set not served/);
  });
});

describe("createRealCoordinatorPort", () => {
  test("checkWorkerInventory fails closed on the owner acceptance gate", async () => {
    const port = createRealCoordinatorPort(makePlan("real"), makeIo(), { signingKey: dummyKey });
    await expect(port.checkWorkerInventory(makePlan("real"))).rejects.toThrow(
      /worker_inventory_unavailable/,
    );
  });

  test("checkWorkerInventory binds the four-target inventory once every host is attested", async () => {
    const plan = makePlan("real");
    const port = createRealCoordinatorPort(plan, makeIo(), {
      signingKey: dummyKey,
      attestations: fullAttestations,
    });
    const result = await port.checkWorkerInventory(plan);
    expect(result.receiptLines.length).toBeGreaterThan(0);
    expect(result.receiptLines[0]).toContain("4-target inventory");
  });
});
