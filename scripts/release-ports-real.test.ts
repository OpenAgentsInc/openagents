// Real release ports (#8917 coordinator, #8922 feed): port selection, the real
// feed port against injected effects, and the coordinator's honest owner-gate
// refusal at inventory bind versus the happy inventory path once the owner
// attests the native acceptance hosts.
import { generateKeyPairSync } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { verifySignedReleaseSet } from "../apps/openagents-desktop/src/release-set-contract.js";
import type { PinnedReleaseKey } from "../apps/openagents-desktop/src/update-contract.js";
import type { ReleaseIo, ReleasePlan, ReleaseTargetKey } from "./release.js";
import { releaseTargetKeys } from "./release.js";
import {
  type CommandResult,
  createRealCoordinatorPort,
  createRealFeedPort,
  createReleasePorts,
  type GcsObjectStore,
  type HttpResponse,
  type ReleaseSigningKey,
  type StagedArtifact,
  type StagingManifest,
  type TargetNativeProofs,
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

// A test ed25519 signing key whose public component IS the pin.
const makeSigningKeyAndPin = (): { key: ReleaseSigningKey; pin: PinnedReleaseKey } => {
  const { privateKey } = generateKeyPairSync("ed25519");
  const jwk = privateKey.export({ format: "jwk" }) as { d: string; x: string };
  const kid = "test-release-key";
  return { key: { kid, d: jwk.d, x: jwk.x }, pin: { alg: "ed25519", kid, x: jwk.x } };
};

const FORMATS: Record<ReleaseTargetKey, readonly string[]> = {
  "darwin-arm64": ["dmg", "zip"],
  "darwin-x64": ["dmg", "zip"],
  "linux-arm64": ["appimage", "deb", "rpm"],
  "linux-x64": ["appimage", "deb", "rpm"],
};

const artifactName = (version: string, target: ReleaseTargetKey, format: string): string => {
  const [platform, arch] = target.split("-");
  if (platform === "darwin") return `OpenAgents-${version}-rc-darwin-${arch}.${format}`;
  const ext = format === "appimage" ? "AppImage" : format;
  return `OpenAgents-${version}-rc-linux-${arch}.${ext}`;
};

const buildStagingManifest = (version: string): StagingManifest => {
  const artifacts: StagedArtifact[] = [];
  let seed = 1;
  for (const target of releaseTargetKeys) {
    for (const format of FORMATS[target]) {
      const name = artifactName(version, target, format);
      artifacts.push({
        target,
        format,
        name,
        objectKey: `desktop/candidate/${version}/${target}/${name}`,
        sha256: seed.toString(16).padStart(64, "0"),
        byteLength: 1000 + seed,
        githubUrl: `https://github.com/OpenAgentsInc/openagents/releases/download/openagents-desktop-v${version}/${name}`,
      });
      seed += 1;
    }
  }
  return { sourceRevision: "0".repeat(40), version, channel: "rc", artifacts };
};

const buildProofs = (): Record<ReleaseTargetKey, TargetNativeProofs> => {
  const keys = [
    "cleanInstall",
    "launch",
    "agentRuntime",
    "shutdown",
    "update",
    "interruptionResume",
    "rollbackOrNoRollback",
    "reinstall",
    "uninstall",
  ] as const;
  return Object.fromEntries(
    releaseTargetKeys.map((target) => [
      target,
      Object.fromEntries(keys.map((k) => [k, `oa.proof.${target}.${k}`])) as TargetNativeProofs,
    ]),
  ) as Record<ReleaseTargetKey, TargetNativeProofs>;
};

// In-memory GcsObjectStore: pre-seeded artifact identity (sha256 + size) for
// headImmutable, plus a blob map for candidate/pointer read/write/CAS.
const makeFakeStore = (
  artifacts: ReadonlyMap<string, { sha256: string; byteLength: number }>,
): GcsObjectStore => {
  const blobs = new Map<string, { body: string; gen: string }>();
  let counter = 1;
  return {
    head: async (key) => {
      const blob = blobs.get(key);
      if (blob !== undefined)
        return { exists: true, byteLength: blob.body.length, generation: blob.gen, sha256: "" };
      const artifact = artifacts.get(key);
      if (artifact !== undefined)
        return {
          exists: true,
          byteLength: artifact.byteLength,
          generation: "artifact",
          sha256: artifact.sha256,
        };
      return { exists: false };
    },
    read: async (key) => blobs.get(key)?.body ?? null,
    createIfAbsent: async (key, body) => {
      if (blobs.has(key)) return "exists";
      blobs.set(key, { body, gen: String(counter++) });
      return "created";
    },
    compareAndSwap: async (key, expected, body) => {
      const current = blobs.get(key)?.gen ?? null;
      if ((expected ?? null) !== (current ?? null)) return { swapped: false, generation: current };
      blobs.set(key, { body, gen: String(counter++) });
      return { swapped: true, generation: blobs.get(key)!.gen };
    },
    _blobs: blobs,
  } as GcsObjectStore & { _blobs: Map<string, { body: string; gen: string }> };
};

describe("createRealCoordinatorPort — real convergence + promotion", () => {
  test("converges all four targets, publishes a signed candidate, and promotes the pointer", async () => {
    const version = "0.1.0-rc.99";
    const { key, pin } = makeSigningKeyAndPin();
    const manifest = buildStagingManifest(version);
    const sizes = new Map(
      manifest.artifacts.map((a) => [a.objectKey, { sha256: a.sha256, byteLength: a.byteLength }]),
    );
    const store = makeFakeStore(sizes) as GcsObjectStore & {
      _blobs: Map<string, { body: string; gen: string }>;
    };
    const plan = makePlan("real");
    const port = createRealCoordinatorPort(plan, makeIo(), {
      signingKey: key,
      pin,
      store,
      attestations: fullAttestations,
      stagingManifest: manifest,
      nativeProofs: buildProofs(),
    });

    await port.checkWorkerInventory(plan);
    await port.bringUpWorkers(plan);
    await port.fanOutTargets(plan);
    await port.runReleaseGates(plan);
    const published = await port.publishCandidate(plan);
    expect(published.receiptLines.length).toBeGreaterThan(0);
    const promoted = await port.promoteChannelPointer(plan);
    expect(promoted.receiptLines.length).toBeGreaterThan(0);

    // The promoted pointer object exists and names a real candidate; the
    // candidate release-set verifies against the pinned key.
    const pointerBody = store._blobs.get("desktop/release-set-v2/rc/pointer.json");
    expect(pointerBody).toBeDefined();
    const pointer = JSON.parse(pointerBody!.body) as { generation: string };
    const candidateBody = store._blobs.get(
      `desktop/release-set-v2/rc/candidates/${pointer.generation}.json`,
    );
    expect(candidateBody).toBeDefined();
    const candidate = JSON.parse(candidateBody!.body) as { releaseSet: unknown; signature: unknown };
    const { canonicalizeReleaseSet } = await import(
      "../apps/openagents-desktop/src/release-set-contract.js"
    );
    const verification = verifySignedReleaseSet(
      canonicalizeReleaseSet(candidate.releaseSet),
      candidate.signature,
      pin,
      "rc",
    );
    expect(verification.ok).toBe(true);
    if (verification.ok) {
      expect(verification.releaseSet.version).toBe(version);
      expect(verification.releaseSet.targets.length).toBe(4);
    }
  });
});
