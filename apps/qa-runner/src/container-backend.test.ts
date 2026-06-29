// Containerized execution backend tests.
//
// All deterministic + NO Docker required (a fake `ContainerRuntime` is injected):
//   - armed + engine-available: full provision -> exec -> extract -> teardown,
//     artifacts copied OUT to the host, public-safe result.json round-trips.
//   - UN-armed (default): refuses with ContainerBackendNotArmedError; the engine
//     is never even touched.
//   - armed but Docker ABSENT: refuses honestly with
//     ContainerEngineUnavailableError; no fallback, no fake green.
//   - exec failure still tears the container down (no leak).
//   - provisionContainerVm: handle exec + teardown drive the runtime; the
//     un-supported acquireBrowser throws rather than faking.
//
// A real-Docker proof, when Docker is present on the host, runs ONE actual
// container session (skipped otherwise).

import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import {
  ContainerBackendNotArmedError,
  ContainerEngineUnavailableError,
  CONTAINER_ARTIFACT_DIR,
  defaultProbeSessionCommand,
  isContainerBackendArmed,
  provisionContainerVm,
  runContainerSession,
} from "./container-backend";
import { dockerContainerRuntime, type ContainerRuntime } from "./container-runtime";
import { decodeQaRunResult } from "./result";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-container-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "container-target", baseUrl: "https://example.test" });

// ── A deterministic fake container engine (no Docker, no network) ─────────────
//
// It records the lifecycle calls, and on copyOut writes a synthetic artifact set
// to the host dir (the same shape the in-container session would have written),
// so the extract step is proven end-to-end.
interface FakeRuntimeLog {
  readonly events: string[];
  removed: boolean;
}

function makeFakeRuntime(opts: {
  readonly available?: boolean;
  /** Exec result the fake returns (default success). */
  readonly exec?: { code: number; output: string };
  /** When set, copyOut writes this result.json to the host dir on extraction. */
  readonly artifactResult?: unknown;
  readonly log?: FakeRuntimeLog;
}): ContainerRuntime {
  const log = opts.log ?? { events: [], removed: false };
  return {
    name: "fake-engine",
    available: async () => opts.available ?? true,
    run: async (o) => {
      log.events.push(`run:${o.image}:${o.command.join(" ")}`);
      return { id: `fake-${o.name}` };
    },
    exec: async (id, command) => {
      log.events.push(`exec:${id}:${command.join(" ")}`);
      return opts.exec ?? { code: 0, output: "ok" };
    },
    copyOut: async (id, containerPath, hostPath) => {
      log.events.push(`cp:${id}:${containerPath}->${hostPath}`);
      // Simulate `docker cp <id>:/qa/artifacts/. <hostPath>` materializing files.
      mkdirSync(hostPath, { recursive: true });
      const resultBody =
        opts.artifactResult ?? {
          schemaVersion: "openagents.qa_runner.result.v1",
          status: "pass",
          target: { name: target.name, baseUrl: target.baseUrl },
          brain: "container-probe",
          backend: "container",
          startedAt: "2026-06-24T00:00:00Z",
          endedAt: "2026-06-24T00:00:00Z",
          durationMs: 0,
          steps: [{ index: 0, kind: "exec", label: "in-container probe", status: "ok" }],
          artifacts: { screenshots: ["snapshot.txt"] },
        };
      writeFileSync(join(hostPath, "result.json"), `${JSON.stringify(resultBody, null, 2)}\n`);
      writeFileSync(join(hostPath, "snapshot.txt"), "in-container snapshot\n");
    },
    remove: async (id) => {
      log.events.push(`rm:${id}`);
      log.removed = true;
    },
  };
}

describe("isContainerBackendArmed", () => {
  test("off by default; on only for explicit 1/true", () => {
    expect(isContainerBackendArmed({})).toBe(false);
    expect(isContainerBackendArmed({ QA_CONTAINER_BACKEND: "0" })).toBe(false);
    expect(isContainerBackendArmed({ QA_CONTAINER_BACKEND: "1" })).toBe(true);
    expect(isContainerBackendArmed({ QA_CONTAINER_BACKEND: "true" })).toBe(true);
  });
});

describe("runContainerSession (fake engine)", () => {
  test("armed: provision -> exec -> extract -> teardown; public-safe result extracted", async () => {
    const log: FakeRuntimeLog = { events: [], removed: false };
    const runtime = makeFakeRuntime({ available: true, log });
    const outcome = await runContainerSession(
      { target, artifactDir: dir, os: "linux" },
      { armed: true, runtime, now: () => 12345 },
    );

    // lifecycle order: run -> exec -> cp -> rm
    expect(log.events[0]).toContain("run:");
    expect(log.events.some((e) => e.startsWith("exec:"))).toBe(true);
    expect(log.events.some((e) => e.startsWith("cp:"))).toBe(true);
    expect(log.removed).toBe(true);
    expect(outcome.containerId).toBe("fake-qa-runner-12345");
    expect(outcome.os).toBe("linux");

    // artifacts were extracted to the host run dir, dereferenceable with no container.
    const resultPath = join(dir, "result.json");
    expect(existsSync(resultPath)).toBe(true);
    expect(existsSync(join(dir, "snapshot.txt"))).toBe(true);

    // result.json round-trips through the SHARED public-safe schema.
    const decoded = decodeQaRunResult(JSON.parse(readFileSync(resultPath, "utf8")));
    expect(decoded.status).toBe("pass");
    expect(decoded.backend).toBe("container");
  });

  test("default probe session command writes under the in-container artifact dir", () => {
    const cmd = defaultProbeSessionCommand(target);
    expect(cmd[0]).toBe("sh");
    expect(cmd[1]).toBe("-c");
    expect(cmd[2]).toContain(CONTAINER_ARTIFACT_DIR);
    expect(cmd[2]).toContain("result.json");
  });

  test("teardown still runs when exec fails (no container leak)", async () => {
    const log: FakeRuntimeLog = { events: [], removed: false };
    const runtime = makeFakeRuntime({ available: true, exec: { code: 1, output: "boom" }, log });
    const outcome = await runContainerSession(
      { target, artifactDir: dir },
      { armed: true, runtime },
    );
    // exec returned non-zero; outcome still carries the transcript, container removed.
    expect(outcome.exec.code).toBe(1);
    expect(log.removed).toBe(true);
  });

  test("non-linux OS tier is accepted but recorded as container-runs-linux", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const outcome = await runContainerSession(
      { target, artifactDir: dir, os: "windows" },
      { armed: true, runtime },
    );
    expect(outcome.os).toContain("windows");
    expect(outcome.os).toContain("linux");
  });
});

describe("owner-gating + Docker-availability honesty", () => {
  test("UN-armed (default env): refuses, engine never touched", async () => {
    const log: FakeRuntimeLog = { events: [], removed: false };
    const runtime = makeFakeRuntime({ available: true, log });
    await expect(
      runContainerSession({ target, artifactDir: dir }, { armed: false, runtime }),
    ).rejects.toBeInstanceOf(ContainerBackendNotArmedError);
    // engine was never invoked
    expect(log.events.length).toBe(0);
  });

  test("UN-armed via env (no QA_CONTAINER_BACKEND) refuses", async () => {
    const runtime = makeFakeRuntime({ available: true });
    await expect(
      runContainerSession({ target, artifactDir: dir }, { runtime, env: {} }),
    ).rejects.toBeInstanceOf(ContainerBackendNotArmedError);
  });

  test("armed via env QA_CONTAINER_BACKEND=1 + fake engine: runs", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const outcome = await runContainerSession(
      { target, artifactDir: dir },
      { runtime, env: { QA_CONTAINER_BACKEND: "1" } },
    );
    expect(existsSync(join(outcome.extractedTo, "result.json"))).toBe(true);
  });

  test("armed but Docker ABSENT: refuses honestly, no fallback, no fake green", async () => {
    const log: FakeRuntimeLog = { events: [], removed: false };
    const runtime = makeFakeRuntime({ available: false, log });
    await expect(
      runContainerSession({ target, artifactDir: dir }, { armed: true, runtime }),
    ).rejects.toBeInstanceOf(ContainerEngineUnavailableError);
    // never started a container
    expect(log.events.length).toBe(0);
    // no artifacts fabricated
    expect(existsSync(join(dir, "result.json"))).toBe(false);
  });
});

describe("provisionContainerVm (CloudVmHandle over the engine)", () => {
  test("armed: handle exec + teardown drive the runtime", async () => {
    const log: FakeRuntimeLog = { events: [], removed: false };
    const runtime = makeFakeRuntime({ available: true, exec: { code: 0, output: "uname-out" }, log });
    const handle = await provisionContainerVm(
      { target, artifactDir: dir, os: "linux" },
      { armed: true, runtime, now: () => 999 },
    );
    expect(handle.id).toBe("fake-qa-runner-vm-999");
    expect(handle.os).toBe("linux");

    const r = await handle.exec("uname", ["-a"]);
    expect(r.code).toBe(0);
    expect(r.output).toBe("uname-out");
    expect(log.events.some((e) => e.includes("uname -a"))).toBe(true);

    await handle.teardown();
    expect(log.removed).toBe(true);
  });

  test("acquireBrowser over the handle throws (not faked)", async () => {
    const runtime = makeFakeRuntime({ available: true });
    const handle = await provisionContainerVm(
      { target, artifactDir: dir, os: "linux" },
      { armed: true, runtime },
    );
    expect(() => handle.acquireBrowser()).toThrow(/not supported/);
    await handle.teardown();
  });

  test("UN-armed: refuses", async () => {
    const runtime = makeFakeRuntime({ available: true });
    await expect(
      provisionContainerVm({ target, artifactDir: dir, os: "linux" }, { armed: false, runtime }),
    ).rejects.toBeInstanceOf(ContainerBackendNotArmedError);
  });

  test("armed but Docker absent: refuses honestly", async () => {
    const runtime = makeFakeRuntime({ available: false });
    await expect(
      provisionContainerVm({ target, artifactDir: dir, os: "linux" }, { armed: true, runtime }),
    ).rejects.toBeInstanceOf(ContainerEngineUnavailableError);
  });
});

// ── REAL Docker proof (one container session) — skipped if Docker is absent ───
describe("runContainerSession (real Docker, one session)", () => {
  test("provisions a real container, execs the probe, extracts artifacts, tears down", async () => {
    const runtime = dockerContainerRuntime();
    const available = await runtime.available();
    if (!available) {
      // Honest skip: Docker not available on this host. The fake-runtime tests
      // above prove the lifecycle; this would prove it against a real engine.
      console.log("[skip-live] Docker engine not available; skipping real-container proof");
      return;
    }
    // Use a tiny image with a shell so the run is fast + does not need the
    // heavyweight playwright image just to prove the exec+extract lifecycle.
    const outcome = await runContainerSession(
      { target, artifactDir: dir, os: "linux" },
      { armed: true, runtime, image: "alpine:3" },
    );
    const resultPath = join(outcome.extractedTo, "result.json");
    expect(existsSync(resultPath)).toBe(true);
    const decoded = decodeQaRunResult(JSON.parse(readFileSync(resultPath, "utf8")));
    expect(decoded.status).toBe("pass");
    expect(decoded.backend).toBe("container");
    expect(existsSync(join(outcome.extractedTo, "snapshot.txt"))).toBe(true);
  }, 120_000);
});
