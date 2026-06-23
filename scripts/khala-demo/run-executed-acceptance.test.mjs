// Regression proof for the executed-acceptance harness (EPIC #6017, M8 #6016).
//
// The whole point of M8's "verified must mean we ran it" upgrade: the preserved
// north-star crossy-road artifact (reconstructed from the verified prod SSE stream)
// must FAIL the EXECUTED acceptance suite — its earlier `verified:true` came only from
// the M2 static regex pre-screen, which never ran the game. This test runs the REAL
// headless suite against that exact preserved artifact and asserts the honest red.
//
// Requires a real headless chromium (Playwright):
//   bunx playwright install chromium
// If chromium is unavailable the real-execution case throws (it does NOT silently
// pass) so a missing browser can never disguise a fake green.

import { readFileSync } from "node:fs";

import { describe, expect, test } from "bun:test";

import {
  DEFAULT_ARTIFACT,
  runExecutedAcceptance,
} from "./run-executed-acceptance.mjs";

describe("executed acceptance — preserved north-star crossy-road artifact", () => {
  test(
    "the verified prod artifact FAILS the EXECUTED suite (its verified:true was a static pre-screen)",
    async () => {
      const html = readFileSync(DEFAULT_ARTIFACT, "utf8");
      const { acceptanceVerdict, khalaCodeVerdict } =
        await runExecutedAcceptance(html);

      // It really ran in a browser.
      expect(acceptanceVerdict.executed).toBe(true);
      expect(khalaCodeVerdict.executed).toBe(true);

      // Honest red: NOT verified, zero passing checks.
      expect(acceptanceVerdict.verified).toBe(false);
      expect(acceptanceVerdict.passedChecks.length).toBe(0);
      expect(acceptanceVerdict.scalarReward).toBe(0);

      // The khala-code verifier, fed the EXECUTED verdict, downgrades to `failed`
      // (executed) — NOT `test_passed`, NOT the static-prescreen `unverified`.
      expect(khalaCodeVerdict.verification).toBe("failed");
      expect(khalaCodeVerdict.verified).toBe(false);
      expect(khalaCodeVerdict.scalarReward).toBe(0);
    },
    120_000,
  );
});
