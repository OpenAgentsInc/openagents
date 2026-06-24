// CF Browser Rendering backend tests.
//
// FAKE-TESTED (deterministic, no network, no spend): the backend drives a scenario
// through the REAL runner against a FAKE `env.BROWSER` binding + a FAKE
// `@cloudflare/playwright` `launch`, producing result.json + per-step screenshots.
// armed/unarmed + binding-absent honest-error are all covered.
//
// NOT tested here (needs a live CF deploy): the actual managed Chrome over CDP.
// The `env.BROWSER` binding only exists inside a deployed Worker — there is no
// live binding in CI — so the real Cloudflare run is a DEPLOY step, asserted by
// inspecting result.json + screenshots from that run.

import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import {
  CfBrowserBackendNotArmedError,
  CfBrowserBindingAbsentError,
  cfBrowserBackend,
  isCfBrowserBackendArmed,
  type CfPlaywrightBrowser,
  type CfPlaywrightLaunch,
  type CfPlaywrightPage,
} from "./cf-browser-backend";
import { scriptedBrain, type BrainStep } from "./brain";
import { decodeQaRunResult } from "./result";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-cf-browser-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "cf-fake", baseUrl: "https://example.test" });

/**
 * A fake managed page driven over a fake `launch`. Records navigation + writes a
 * tiny PNG on screenshot (so the artifact-shape path is exercised honestly), and
 * matches `text=`/selectors against configured page text.
 */
function makeFakeCfLaunch(config: {
  readonly text?: Record<string, string>;
  /** Sentinel proving the injected binding was the one passed to launch. */
  readonly onLaunch?: (binding: unknown) => void;
  /** Record close() so teardown can be asserted (no leaked browser slot). */
  readonly onClose?: () => void;
}): CfPlaywrightLaunch {
  return async (binding, _opts) => {
    config.onLaunch?.(binding);
    let currentUrl = "about:blank";
    const resolve = (url: string) =>
      /^https?:/.test(url) ? url : `https://example.test${url}`;
    const textForUrl = () => {
      for (const [k, v] of Object.entries(config.text ?? {})) {
        if (currentUrl.endsWith(k)) return v;
      }
      return "";
    };
    const page: CfPlaywrightPage = {
      goto: async (url) => {
        currentUrl = resolve(url);
        return null;
      },
      url: () => currentUrl,
      click: async () => undefined,
      fill: async () => undefined,
      innerText: async () => textForUrl(),
      content: async () => `<html>${textForUrl()}</html>`,
      screenshot: async ({ path }) => {
        // Real bun runtime: write a tiny PNG-ish file so the runner's screenshot
        // list picks it up (the artifact basis — no video on Browser Rendering).
        const { writeFileSync } = await import("node:fs");
        writeFileSync(path, Buffer.from("fake-png-bytes"));
        return null;
      },
      waitForURL: async (pred, _o) => {
        if (!pred(currentUrl)) throw new Error("url predicate not met");
      },
      waitForSelector: async (selector, _o) => {
        const t = textForUrl();
        const needle = selector.startsWith("text=") ? selector.slice("text=".length) : selector;
        if (!t.includes(needle)) throw new Error(`selector not visible: ${selector}`);
        return null;
      },
    };
    const browser: CfPlaywrightBrowser = {
      newPage: async () => page,
      close: async () => {
        config.onClose?.();
      },
    };
    return browser;
  };
}

describe("cfBrowserBackend arming", () => {
  test("isCfBrowserBackendArmed reads the env flag", () => {
    expect(isCfBrowserBackendArmed({})).toBe(false);
    expect(isCfBrowserBackendArmed({ QA_CF_BROWSER_BACKEND: "1" })).toBe(true);
    expect(isCfBrowserBackendArmed({ QA_CF_BROWSER_BACKEND: "true" })).toBe(true);
    expect(isCfBrowserBackendArmed({ QA_CF_BROWSER_BACKEND: "0" })).toBe(false);
  });

  test("un-armed provision throws CfBrowserBackendNotArmedError (no fake green)", async () => {
    const backend = cfBrowserBackend({ env: {}, browser: {} });
    expect(backend.name).toBe("cf-browser");
    await expect(backend.provision({ target, artifactDir: dir })).rejects.toBeInstanceOf(
      CfBrowserBackendNotArmedError,
    );
  });

  test("armed but binding ABSENT throws CfBrowserBindingAbsentError (honest CI error)", async () => {
    const backend = cfBrowserBackend({ armed: true });
    await expect(backend.provision({ target, artifactDir: dir })).rejects.toBeInstanceOf(
      CfBrowserBindingAbsentError,
    );
  });
});

describe("cfBrowserBackend run (fake env.BROWSER, deterministic)", () => {
  const scenario: ReadonlyArray<BrainStep> = [
    { kind: "navigate", url: "/welcome", label: "go to welcome" },
    { kind: "wait-for", condition: { kind: "text-visible", value: "Welcome" }, label: "welcome shows" },
    { kind: "screenshot", label: "welcome" },
    { kind: "assert", check: { kind: "url-includes", value: "/welcome" }, label: "on welcome url" },
    {
      kind: "assert",
      check: { kind: "text-contains", value: "Welcome", selector: "body" },
      label: "page greets",
    },
  ];

  test("drives the runner end-to-end -> result.json (pass) + screenshots", async () => {
    let launchedBinding: unknown;
    let closed = 0;
    const fakeBinding = { __brand: "fake-env-BROWSER" };
    const backend = cfBrowserBackend({
      armed: true,
      browser: fakeBinding,
      // This test pins the provision->drive->screenshot->teardown lifecycle and
      // the screenshots artifact basis; the screencast->mp4 video path (#6213)
      // is exercised by cf-browser-video.test.ts. Keep video OFF here so the
      // assertion is deterministic regardless of whether ffmpeg is installed.
      captureVideo: false,
      launch: makeFakeCfLaunch({
        text: { "/welcome": "Welcome to OpenAgents" },
        onLaunch: (b) => {
          launchedBinding = b;
        },
        onClose: () => {
          closed += 1;
        },
      }),
    });

    const outcome = await Effect.runPromise(
      runQaSession({
        target,
        brain: scriptedBrain(scenario),
        backend,
        artifactDir: dir,
        now: () => new Date("2026-06-24T00:00:00.000Z"),
      }),
    );

    // The injected fake binding was the one handed to launch.
    expect(launchedBinding).toBe(fakeBinding);
    // The managed browser was ALWAYS closed (no leaked concurrent slot).
    expect(closed).toBe(1);

    // result.json: passing, on the cf-browser backend, schema-valid + public-safe.
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("cf-browser");
    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(onDisk.status).toBe("pass");
    expect(onDisk.backend).toBe("cf-browser");
    // Video capture is OFF for this lifecycle test -> no video artifact.
    expect(onDisk.artifacts.video).toBeUndefined();

    // Screenshots are the artifact basis: at least one PNG was written.
    const pngs = readdirSync(dir).filter((f) => f.endsWith(".png"));
    expect(pngs.length).toBeGreaterThanOrEqual(1);
    expect(onDisk.artifacts.screenshots.length).toBeGreaterThanOrEqual(1);
    expect(existsSync(join(dir, onDisk.artifacts.screenshots[0]!))).toBe(true);
  });

  test("a real red is a real red: a failed assertion -> status fail", async () => {
    const backend = cfBrowserBackend({
      armed: true,
      browser: {},
      launch: makeFakeCfLaunch({ text: { "/welcome": "Welcome" } }),
    });

    const wrong: ReadonlyArray<BrainStep> = [
      { kind: "navigate", url: "/welcome", label: "go to welcome" },
      // WRONG on purpose: the page never says "Goodbye".
      {
        kind: "assert",
        check: { kind: "text-contains", value: "Goodbye", selector: "body" },
        label: "page says goodbye (intentionally wrong)",
      },
    ];

    const outcome = await Effect.runPromise(
      runQaSession({
        target,
        brain: scriptedBrain(wrong),
        backend,
        artifactDir: dir,
        now: () => new Date("2026-06-24T00:00:00.000Z"),
      }),
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toBeDefined();
  });
});
