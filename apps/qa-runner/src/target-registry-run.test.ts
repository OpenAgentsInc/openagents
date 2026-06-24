// Multi-target run tests (#6190): one scenario across N targets from a single
// definition; per-target result + video; read-only restriction blocks a mutating
// step honestly. Fakes-in-CI: a fake chromium proves the full path with no real
// browser.

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { type BrainStep, scriptedBrain } from "./brain";
import { makeFakeChromium } from "./fake-chromium";
import { loginRegressionCommitments, loginRegressionSteps } from "./scenarios";
import { resolveSelectedTargets } from "./target-registry";
import { runScenarioAcrossTargets } from "./target-registry-run";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-multitarget-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const passingChromium = () =>
  makeFakeChromium({
    pages: { "/login": { text: "Log in to OpenAgents", html: "<form>Log in to OpenAgents</form>" } },
  });

const clock = () => () => new Date("2026-06-24T00:00:00.000Z");

describe("runScenarioAcrossTargets (#6190)", () => {
  test("ONE scenario runs against dev + prod from a single definition (no rewrite)", async () => {
    // dev + prod resolved from the registry (env overrides keep it offline-safe).
    const targets = resolveSelectedTargets(["dev", "prod"], {
      QA_DEV_URL: "https://dev.example.test",
    });
    expect(targets.map((t) => t.name)).toEqual(["dev", "prod"]);

    const outcome = await Effect.runPromise(
      runScenarioAcrossTargets({
        scenario: {
          id: "login-regression",
          title: "/login renders the sign-in form",
          // ONE definition — both targets share these read-only steps.
          brain: () => scriptedBrain(loginRegressionSteps()),
          backend: () => localBackend({ chromium: passingChromium() }),
          commitments: loginRegressionCommitments(),
        },
        targets,
        artifactDir: dir,
        now: clock(),
      }),
    );

    const m = outcome.result;
    expect(m.targetCount).toBe(2);
    expect(m.passCount).toBe(2);
    expect(m.passRate).toBe(1);

    const byName = Object.fromEntries(m.targets.map((t) => [t.targetName, t]));
    // per-target pass/fail
    expect(byName.dev!.status).toBe("pass");
    expect(byName.prod!.status).toBe("pass");
    // per-target video
    expect(byName.dev!.video).toBeDefined();
    expect(byName.prod!.video).toBeDefined();
    // per-target verify verdict (#6192 carried through)
    expect(byName.dev!.verdict).toBe("CONFIRMED");
    expect(byName.prod!.verdict).toBe("CONFIRMED");
    // prod is flagged read-only; dev is not
    expect(byName.prod!.readOnly).toBe(true);
    expect(byName.dev!.readOnly).toBe(false);

    // artifacts really exist per-target on disk (video path is matrix-root relative,
    // i.e. already includes the per-target subdir)
    expect(existsSync(join(dir, byName.dev!.video!))).toBe(true);
    expect(existsSync(join(dir, byName.prod!.video!))).toBe(true);

    // the matrix persisted + is public-safe (no forbidden fields)
    expect(existsSync(outcome.resultPath)).toBe(true);
    const raw = readFileSync(outcome.resultPath, "utf8");
    expect(raw).toContain('"schemaVersion": "openagents.qa_runner.target_matrix.v1"');
  });

  test("a read-only prod target BLOCKS a mutating step (honest failure, not a fake pass)", async () => {
    // Same scenario shape, but it tries to CLICK — a mutating step. Against dev it
    // succeeds; against read-only prod it is refused with a recorded reason.
    const mutatingSteps: ReadonlyArray<BrainStep> = [
      { kind: "navigate", url: "/login", label: "open /login" },
      { kind: "click", selector: "button[type=submit]", label: "submit the form (mutates)" },
    ];

    const targets = resolveSelectedTargets(["dev", "prod"], {
      QA_DEV_URL: "https://dev.example.test",
    });

    const outcome = await Effect.runPromise(
      runScenarioAcrossTargets({
        scenario: {
          id: "mutating-login",
          title: "submit the login form (mutating)",
          brain: () => scriptedBrain(mutatingSteps),
          backend: () => localBackend({ chromium: passingChromium() }),
        },
        targets,
        artifactDir: dir,
        now: clock(),
      }),
    );

    const byName = Object.fromEntries(outcome.result.targets.map((t) => [t.targetName, t]));
    // dev (writable): the click is allowed -> pass
    expect(byName.dev!.status).toBe("pass");
    // prod (read-only): the click is REFUSED -> honest fail with the reason
    expect(byName.prod!.status).toBe("fail");
    expect(byName.prod!.failure).toContain("read-only");
    expect(byName.prod!.failure).toContain("click");

    // the per-target result.json records the refusal as a failed step (not skipped)
    const prodResult = JSON.parse(readFileSync(join(dir, "prod", "result.json"), "utf8"));
    const refused = prodResult.steps.find((s: { kind: string }) => s.kind === "click");
    expect(refused.status).toBe("failed");
    expect(refused.detail.restriction).toBe("read-only");

    // honest aggregate: 1 of 2 passed
    expect(outcome.result.passCount).toBe(1);
    expect(outcome.result.targetCount).toBe(2);
  });

  test("zero targets is an honest error", async () => {
    const exit = await Effect.runPromiseExit(
      runScenarioAcrossTargets({
        scenario: {
          id: "x",
          title: "x",
          brain: () => scriptedBrain([]),
          backend: () => localBackend({ chromium: passingChromium() }),
        },
        targets: [],
        artifactDir: dir,
      }),
    );
    expect(exit._tag).toBe("Failure");
  });
});
