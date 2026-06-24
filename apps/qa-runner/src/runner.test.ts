// Runner unit tests — fakes-in-CI: a fake chromium proves the full
// provision -> capture -> teardown -> artifact-shape path with NO real browser.
// The real-chromium path is exercised by run-once / demo:login.

import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend, cloudVmBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { makeFakeChromium } from "./fake-chromium";
import { decodeQaRunResult } from "./result";
import {
  loginRedirectClaimCommitments,
  loginRegressionCommitments,
  loginRegressionSteps,
  loginRegressionStepsWrong,
} from "./scenarios";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-runner-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = () => makeTarget({ name: "fake-target", baseUrl: "https://example.test" });

const passingChromium = () =>
  makeFakeChromium({
    pages: { "/login": { text: "Log in to OpenAgents", html: "<form>Log in to OpenAgents</form>" } },
  });

describe("runQaSession (fake chromium)", () => {
  test("provisions, captures artifacts, tears down, writes a PASS result.json", async () => {
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: localBackend({ chromium: passingChromium() }),
        artifactDir: dir,
        now: () => new Date("2026-06-24T00:00:00.000Z"),
      }),
    );
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.brain).toBe("scripted");
    expect(outcome.result.backend).toBe("local");
    expect(outcome.result.target.name).toBe("fake-target");

    // artifact shape
    expect(outcome.result.artifacts.trace).toBe("trace.zip");
    expect(outcome.result.artifacts.video).toBeDefined();
    expect(outcome.result.artifacts.screenshots.length).toBeGreaterThan(0);

    // artifacts really exist on disk
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
    expect(existsSync(join(dir, outcome.result.artifacts.video!))).toBe(true);
    expect(existsSync(outcome.resultPath)).toBe(true);

    // result.json round-trips through the schema
    const parsed = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(parsed.status).toBe("pass");
    expect(parsed.steps.every((s) => s.status === "ok")).toBe(true);
  });

  test("FAILS honestly when an assertion is wrong (no fabricated success)", async () => {
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionStepsWrong()),
        backend: localBackend({ chromium: passingChromium() }),
        artifactDir: dir,
      }),
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toBeDefined();
    // a failed step is recorded
    expect(outcome.result.steps.some((s) => s.status === "failed")).toBe(true);
    // and the video/trace were STILL flushed despite the failure
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
  });

  test("FAILS when the page is missing the expected text (broken build)", async () => {
    const brokenChromium = makeFakeChromium({
      pages: { "/login": { text: "Internal Server Error", html: "<h1>500</h1>" } },
    });
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: localBackend({ chromium: brokenChromium }),
        artifactDir: dir,
      }),
    );
    expect(outcome.result.status).toBe("fail");
  });

  test("records a redirect-to-home as a failure (the exact regression)", async () => {
    const redirectingChromium = makeFakeChromium({
      pages: { "/": { text: "Home" } },
      redirectTo: { "/login": "https://example.test/" },
    });
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: localBackend({ chromium: redirectingChromium }),
        artifactDir: dir,
      }),
    );
    expect(outcome.result.status).toBe("fail");
  });
});

describe("verify stage (#6192): commitments -> verdict in result.json", () => {
  test("an honest run that satisfies its commitments -> CONFIRMED", async () => {
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: localBackend({ chromium: passingChromium() }),
        artifactDir: dir,
        commitments: loginRegressionCommitments(),
      }),
    );
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.verify).toBeDefined();
    expect(outcome.result.verify!.verdict).toBe("CONFIRMED");
    expect(outcome.result.verify!.observed).toBe(true);

    // it persisted into result.json and round-trips through the schema
    const parsed = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(parsed.verify!.verdict).toBe("CONFIRMED");
    expect(parsed.verify!.findings.length).toBe(2);
  });

  test("a run whose CLAIM is FALSE -> REFUTED (NOT a fake pass), with contradicting evidence", async () => {
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionStepsWrong()),
        backend: localBackend({ chromium: passingChromium() }),
        artifactDir: dir,
        // the run CLAIMS /login redirects away — it does not.
        commitments: loginRedirectClaimCommitments(),
      }),
    );
    // honest red run
    expect(outcome.result.status).toBe("fail");
    // and the verdict is REFUTED, not CONFIRMED
    expect(outcome.result.verify!.verdict).toBe("REFUTED");
    const finding = outcome.result.verify!.findings[0]!;
    expect(finding.verdict).toBe("REFUTED");
    expect(finding.evidenceSummary).toContain("contradicting evidence");

    // result.json carries the refuted verdict + the inline contradiction
    const raw = readFileSync(outcome.resultPath, "utf8");
    expect(raw).toContain("REFUTED");
    expect(raw).not.toContain('"verdict": "CONFIRMED"');
  });

  test("no commitments -> no verify field (additive, opt-in)", async () => {
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: localBackend({ chromium: passingChromium() }),
        artifactDir: dir,
      }),
    );
    expect(outcome.result.verify).toBeUndefined();
    // existing fields are untouched
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.steps.length).toBeGreaterThan(0);
  });
});

describe("cloudVmBackend (owner-gated)", () => {
  test("throws 'not armed' without an injected provisioner", async () => {
    const exit = await Effect.runPromiseExit(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend: cloudVmBackend(), // no provisioner
        artifactDir: dir,
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("uses an injected provisioner when armed", async () => {
    let provisioned = false;
    let toreDown = false;
    const backend = cloudVmBackend({
      provisioner: {
        provision: async ({ artifactDir }) => {
          provisioned = true;
          const acquired = await localBackend({ chromium: passingChromium() }).provision({
            target: target(),
            artifactDir,
          });
          return {
            acquireBrowser: acquired.acquireBrowser,
            teardown: async () => {
              toreDown = true;
            },
          };
        },
      },
    });
    const outcome = await Effect.runPromise(
      runQaSession({
        target: target(),
        brain: scriptedBrain(loginRegressionSteps()),
        backend,
        artifactDir: dir,
      }),
    );
    expect(provisioned).toBe(true);
    expect(toreDown).toBe(true);
    expect(outcome.result.backend).toBe("cloud-vm");
    expect(outcome.result.status).toBe("pass");
  });
});
