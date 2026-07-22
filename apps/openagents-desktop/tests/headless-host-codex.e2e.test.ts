/**
 * LIVE #9161 real-provider acceptance: run one real owner-local Codex turn
 * through the PRODUCTION headless host (`createHeadlessHost` -> the real
 * provider-lane dispatcher -> the live Codex lane) with no renderer, no
 * Playwright, no DOM. Gated — spends real capacity:
 *
 *   HEADLESS_HOST_LIVE=1 pnpm --dir apps/openagents-desktop exec \
 *     vitest run tests/headless-host-codex.e2e.test.ts
 *
 * Proves: ordered typed frames + a durable thread + zero Full Auto records
 * for an ordinary identity question, driven only through the programmatic
 * host interface.
 */

import { describe, expect, test } from "vite-plus/test";
import { existsSync, mkdtempSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { makeCodexHeadlessLane } from "../src/codex-headless-lane.ts";
import { createHeadlessHost } from "../src/desktop-headless-host.ts";
import { deriveHeadlessReceipts } from "../src/desktop-headless-receipt.ts";

const live = process.env.HEADLESS_HOST_LIVE === "1";
const model = process.env.HEADLESS_HOST_MODEL ?? "gpt-5.6-terra";

const codexBinary = [
  join(homedir(), ".local", "bin", "codex"),
  "/Applications/ChatGPT.app/Contents/Resources/codex",
  "/opt/homebrew/bin/codex",
].find((path) => existsSync(path));

describe.skipIf(!live)("headless host — LIVE codex ordinary turn (#9161)", () => {
  test(
    "an identity question runs through the production host and creates no Full Auto record",
    { timeout: 360_000 },
    async () => {
      expect(codexBinary).toBeDefined();
      const root = mkdtempSync(join(tmpdir(), "oa-hh-root-"));
      const workspace = mkdtempSync(join(tmpdir(), "oa-hh-work-"));
      const host = createHeadlessHost({ root });
      const lane = makeCodexHeadlessLane({ workspace, model, timeoutMs: 300_000 });
      const thread = host.createThread("live identity");

      const result = await host.submitOrdinaryTurn({
        lane,
        threadRef: thread.id,
        turnRef: "turn-1",
        message: "hey who are you",
      });

      console.log(`dispatch.ok=${result.dispatch.ok}`);
      console.log(`frames: ${result.frames.map((frame) => frame.event.kind).join(" ")}`);
      console.log(`fullAutoRecordCount=${result.fullAutoRecordCount}`);

      expect(result.dispatch.ok).toBe(true);
      const kinds = result.frames.map((frame) => frame.event.kind);
      expect(kinds).toContain("turn_started");
      expect(kinds).toContain("turn_completed");
      // The ordinary turn created no Full Auto authority.
      expect(result.fullAutoRecordCount).toBe(0);
      expect(host.fullAutoRuns()).toHaveLength(0);
      // The turn persisted to the durable thread.
      expect(result.thread?.notes.length ?? 0).toBeGreaterThan(0);
    },
  );
});
