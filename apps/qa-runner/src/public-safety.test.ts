// Public-safety tripwire tests: a result.json (and any artifact metadata it
// carries) must never leak secrets/tokens/prompts/cookies/credentials. The
// runner calls `assertPublicSafeResult` before writing; these tests pin that
// contract directly and via a full run.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { makeFakeChromium } from "./fake-chromium";
import { assertPublicSafeResult, PublicSafetyViolation } from "./result";
import { loginRegressionSteps } from "./scenarios";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";

describe("assertPublicSafeResult", () => {
  test("passes a clean result", () => {
    expect(() =>
      assertPublicSafeResult({ status: "pass", steps: [{ label: "open /login" }] }),
    ).not.toThrow();
  });

  for (const key of ["token", "accessToken", "secret", "password", "cookie", "authorization", "bearer", "apiKey", "api_key", "prompt", "credential"]) {
    test(`rejects a forbidden key: ${key}`, () => {
      expect(() => assertPublicSafeResult({ ok: true, [key]: "x" })).toThrow(PublicSafetyViolation);
    });
  }

  test("rejects forbidden keys nested deep", () => {
    expect(() =>
      assertPublicSafeResult({ steps: [{ detail: { sessionToken: "leak" } }] }),
    ).toThrow(PublicSafetyViolation);
  });
});

describe("full run produces a public-safe result.json", () => {
  test("no forbidden fields, even when a credential is typed", async () => {
    const dir = mkdtempSync(join(tmpdir(), "qa-runner-safety-"));
    try {
      const chromium = makeFakeChromium({
        pages: { "/login": { text: "Log in to OpenAgents", html: "<form/>" } },
      });
      const steps = [
        ...loginRegressionSteps(),
        // type a secret: it must NOT appear anywhere in result.json
        { kind: "type" as const, selector: "input[name=password]", text: "hunter2-TOP-SECRET", label: "enter password" },
      ];
      const outcome = await Effect.runPromise(
        runQaSession({
          target: makeTarget({ name: "t", baseUrl: "https://example.test" }),
          brain: scriptedBrain(steps),
          backend: localBackend({ chromium }),
          artifactDir: dir,
        }),
      );
      const raw = readFileSync(outcome.resultPath, "utf8");
      expect(raw).not.toContain("hunter2-TOP-SECRET");
      expect(raw.toLowerCase()).not.toContain("password\":");
      // the result is structurally public-safe
      expect(() => assertPublicSafeResult(outcome.result)).not.toThrow();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
