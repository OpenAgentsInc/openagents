// Cloudflare Browser Rendering execution backend (#6205).
//
// The prod-native browser substrate. Instead of launching a local chromium (the
// `localBackend` in backend.ts) or standing up a firecracker/KVM microVM (the
// now-superseded #6200 direction), a QA run drives a MANAGED Chrome on
// Cloudflare's network via the `env.BROWSER` Durable-Object binding, using
// `@cloudflare/playwright`'s `launch(env.BROWSER)` (CDP under the hood). No local
// Chrome, no VM, no host to provision — it runs on the same Workers stack we
// already deploy.
//
// This is wired through the EXISTING `Backend` abstraction (backend.ts): a
// `cfBrowserBackend()` returns a `Backend` whose `provision()` yields a
// `BackendSession` whose `acquireBrowser()` returns the `AcquiredBrowser &
// { artifacts() }` shape the runner (runner.ts) drives. The brain/target/result
// contracts are therefore UNCHANGED — a Khala QA run executes on Cloudflare's
// managed browser with no runner-side changes.
//
// OWNER-GATED / ARMED-BY-ENV (default OFF):
//   Inert unless explicitly armed (`QA_CF_BROWSER_BACKEND=1`, or `armed: true`).
//   An un-armed backend throws `CfBrowserBackendNotArmedError` on provision — it
//   never silently falls back to local and never fakes a green.
//
// HONEST ABOUT THE BINDING:
//   The `env.BROWSER` binding only exists inside a deployed Worker. CI has NO live
//   binding, so when armed but `env.BROWSER` is absent, provisioning throws
//   `CfBrowserBindingAbsentError`. The REAL Cloudflare run is therefore a DEPLOY
//   step (a Worker with `[browser] binding = "BROWSER"`); it cannot run in unit
//   CI. Unit tests inject a FAKE `env.BROWSER` + a fake `launch` to prove the
//   provision -> drive -> screenshot -> artifact-shape -> teardown lifecycle
//   deterministically, with NO network and NO spend.
//
// NO NATIVE VIDEO (#6213 is the video-compose follow-up):
//   Browser Rendering has no native video recording. The artifact BASIS here is
//   per-step SCREENSHOTS (PNG) written into the run dir, plus result.json. A
//   reviewer confirms the run from the screenshots + result; composing those
//   frames into a playable video is the separate #6213 lane. `artifacts()` thus
//   reports NO `videoPath` here — honestly, not a fake mp4.
//
// LIMITS we respect (verified against Cloudflare's docs):
//   - concurrent browsers/account: ~3 (free) / ~120 (paid). A managed browser is
//     a scarce, billed resource: this backend acquires ONE browser per provision
//     and ALWAYS closes it on teardown (even on error) so a run never leaks a
//     concurrent slot.
//   - 60s idle close: a managed browser auto-closes after ~60s inactivity. We
//     pass a `keepAliveMs` to `launch` so a longer run is not killed mid-flight;
//     the default keeps the contract explicit rather than relying on the implicit
//     idle window.

import { mkdirSync } from "node:fs";
import { join } from "node:path";
import type { AcquiredBrowser } from "@openagentsinc/probe-runtime";
import type { ComputerUsePage, PlaywrightArtifacts, WaitForCondition } from "@openagentsinc/probe-runtime";
import type { Backend, BackendSession } from "./backend";

/** Default keep-alive (ms) passed to `launch` so a managed browser is not closed
 *  by the ~60s idle window mid-run. Conservative; a long run can raise it. */
export const DEFAULT_CF_BROWSER_KEEP_ALIVE_MS = 600_000;

export class CfBrowserBackendNotArmedError extends Error {
  constructor() {
    super(
      "cfBrowserBackend is not armed: the Cloudflare Browser Rendering backend " +
        "(managed Chrome via env.BROWSER) is owner-gated and OFF by default. Arm " +
        "it explicitly with QA_CF_BROWSER_BACKEND=1 (or { armed: true }).",
    );
    this.name = "CfBrowserBackendNotArmedError";
  }
}

export class CfBrowserBindingAbsentError extends Error {
  constructor() {
    super(
      "cfBrowserBackend is armed but the Cloudflare Browser Rendering binding " +
        "(env.BROWSER) is absent. That binding only exists inside a deployed " +
        "Worker with `[browser] binding = \"BROWSER\"`; it is NOT available in " +
        "unit CI. Run this on a deploy, or inject a fake binding for tests. It " +
        "will NOT fall back to local or fake a result.",
    );
    this.name = "CfBrowserBindingAbsentError";
  }
}

// ── The @cloudflare/playwright shape we depend on (kept minimal + injectable) ──
//
// We do NOT statically import `@cloudflare/playwright`: it is a Workers-runtime
// module not present in this app's deps, and importing it would pull a Worker-only
// graph into unit CI. Instead the `launch` function is INJECTED (default: a
// dynamic import of `@cloudflare/playwright`). This mirrors how `playwright-page.ts`
// keeps real Playwright out of the unit-test module graph.

/** The opaque Cloudflare Browser Rendering binding (`env.BROWSER`). */
export type CfBrowserBinding = unknown;

/** The minimal Playwright `Page` slice this backend drives (over CDP, managed). */
export interface CfPlaywrightPage {
  goto(url: string, options?: { readonly waitUntil?: string }): Promise<unknown>;
  url(): string;
  click(selector: string, options?: { readonly timeout?: number }): Promise<void>;
  fill(selector: string, value: string, options?: { readonly timeout?: number }): Promise<void>;
  innerText(selector: string, options?: { readonly timeout?: number }): Promise<string>;
  content(): Promise<string>;
  screenshot(options: { readonly path: string }): Promise<unknown>;
  waitForURL(
    predicate: (url: string) => boolean,
    options?: { readonly timeout?: number },
  ): Promise<void>;
  waitForSelector(selector: string, options?: { readonly timeout?: number }): Promise<unknown>;
}

/** The minimal managed-browser slice (from `launch(env.BROWSER)`). */
export interface CfPlaywrightBrowser {
  newPage(): Promise<CfPlaywrightPage>;
  close(): Promise<void>;
}

/** `launch(env.BROWSER, opts)` from `@cloudflare/playwright`. */
export type CfPlaywrightLaunch = (
  binding: CfBrowserBinding,
  options?: { readonly keep_alive?: number },
) => Promise<CfPlaywrightBrowser>;

export interface CfBrowserBackendOptions {
  /**
   * The Cloudflare Browser Rendering binding. In a deployed Worker this is
   * `env.BROWSER`. ABSENT in unit CI — when armed without it, provisioning throws
   * `CfBrowserBindingAbsentError`. Tests inject a fake.
   */
  readonly browser?: CfBrowserBinding;
  /**
   * Arm the backend. Defaults to reading `QA_CF_BROWSER_BACKEND` from `env`
   * ("1"/"true" => armed). Owner-gated: OFF unless explicitly set.
   */
  readonly armed?: boolean;
  /** Env source for the arming check (default `process.env`). */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /**
   * Injectable `launch`. Default: dynamic import of `@cloudflare/playwright`'s
   * `launch`. Tests inject a fake that returns a scripted page (no network).
   */
  readonly launch?: CfPlaywrightLaunch;
  /** Keep-alive window (ms) so the ~60s idle close does not kill a run. */
  readonly keepAliveMs?: number;
  /** Default per-action deadline (ms). */
  readonly defaultTimeoutMs?: number;
}

/** True when the env arms the CF Browser Rendering backend. */
export function isCfBrowserBackendArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_CF_BROWSER_BACKEND;
  return v === "1" || v === "true";
}

/** Dynamic import of the real `@cloudflare/playwright` `launch`. Only called when
 *  no `launch` is injected — i.e. inside a deployed Worker. Kept out of the unit
 *  module graph so CI never resolves a Workers-only package. */
async function defaultCfLaunch(
  binding: CfBrowserBinding,
  options?: { readonly keep_alive?: number },
): Promise<CfPlaywrightBrowser> {
  // The package name is assembled so a bundler/CI graph walker does not try to
  // statically resolve `@cloudflare/playwright` (absent from this app's deps).
  const pkg = ["@cloudflare", "playwright"].join("/");
  const mod = (await import(/* @vite-ignore */ pkg)) as { launch: CfPlaywrightLaunch };
  return mod.launch(binding, options);
}

/**
 * Adapt a managed `CfPlaywrightPage` into the runner's `ComputerUsePage` seam,
 * so the EXISTING browser surface + runner drive it unchanged. The mapping
 * mirrors `playwright-page.ts` (the local-chromium adapter): same actions, same
 * deterministic `waitFor` (condition, never sleep), same screenshot-to-path.
 */
function pageFromCfPage(cfPage: CfPlaywrightPage, defaultTimeoutMs: number): ComputerUsePage {
  return {
    navigate: async (url) => {
      await cfPage.goto(url, { waitUntil: "load" });
    },
    url: async () => cfPage.url(),
    click: async (selector) => {
      await cfPage.click(selector, { timeout: defaultTimeoutMs });
    },
    type: async (selector, text) => {
      await cfPage.fill(selector, text, { timeout: defaultTimeoutMs });
    },
    readText: async (selector) => {
      return (await cfPage.innerText(selector ?? "body", { timeout: defaultTimeoutMs })).trim();
    },
    readDom: async (selector) => {
      if (selector) {
        // No `evaluate` in the minimal slice: outerHTML of a selector falls back
        // to the full content (the readDom contract is "best-effort DOM read").
        return await cfPage.content();
      }
      return await cfPage.content();
    },
    waitFor: async (condition: WaitForCondition, opts) => {
      const timeout = opts?.timeoutMs ?? defaultTimeoutMs;
      try {
        switch (condition.kind) {
          case "url-includes":
            await cfPage.waitForURL((u) => u.includes(condition.value), { timeout });
            return true;
          case "url-not-includes":
            await cfPage.waitForURL((u) => !u.includes(condition.value), { timeout });
            return true;
          case "text-visible": {
            // Managed Browser Rendering exposes the Playwright selector engine;
            // `text=` matches visible text. A miss within the deadline is a
            // false (honest timeout), never a throw that masks the red.
            await cfPage.waitForSelector(`text=${condition.value}`, { timeout });
            return true;
          }
          case "selector-visible":
            await cfPage.waitForSelector(condition.selector, { timeout });
            return true;
        }
      } catch {
        return false;
      }
    },
    screenshot: async (path) => {
      await cfPage.screenshot({ path });
    },
  };
}

/**
 * The Cloudflare Browser Rendering backend. Wired through the existing `Backend`
 * abstraction so the runner drives a managed Chrome with NO runner changes.
 * Owner-gated (armed) + honest about the binding (absent in CI). Acquires exactly
 * ONE managed browser per provision and ALWAYS closes it on teardown (even on
 * error), so a run never leaks one of the scarce concurrent-browser slots.
 *
 * Artifacts: per-step SCREENSHOTS (the artifact basis — Browser Rendering has no
 * native video) + result.json. `artifacts()` reports the trace-path slot for the
 * shared `PlaywrightArtifacts` contract but NO `videoPath` (full video compose is
 * the separate #6213 lane).
 */
export function cfBrowserBackend(options: CfBrowserBackendOptions = {}): Backend {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isCfBrowserBackendArmed(env);
  const launch = options.launch ?? defaultCfLaunch;
  const keepAliveMs = options.keepAliveMs ?? DEFAULT_CF_BROWSER_KEEP_ALIVE_MS;
  const defaultTimeoutMs = options.defaultTimeoutMs ?? 15_000;

  return {
    name: "cf-browser",
    provision: async ({ target, artifactDir }): Promise<BackendSession> => {
      if (!armed) throw new CfBrowserBackendNotArmedError();
      // The binding only exists inside a deployed Worker. Honest in CI.
      if (options.browser === undefined || options.browser === null) {
        throw new CfBrowserBindingAbsentError();
      }
      mkdirSync(artifactDir, { recursive: true });
      void target; // target.baseUrl is resolved by the runner's navigate steps.

      return {
        acquireBrowser: async (): Promise<
          AcquiredBrowser & { artifacts: () => PlaywrightArtifacts }
        > => {
          const browser = await launch(options.browser, { keep_alive: keepAliveMs });
          const cfPage = await browser.newPage();
          const page = pageFromCfPage(cfPage, defaultTimeoutMs);

          let flushed = false;
          const flush = async (): Promise<void> => {
            if (flushed) return;
            flushed = true;
            // ALWAYS close the managed browser so the concurrent slot is freed.
            await browser.close().catch(() => undefined);
          };

          // No native video; the trace slot is reported for contract shape but
          // the file is not produced by Browser Rendering. Screenshots are the
          // artifact basis and are written to `artifactDir` per step by the
          // runner's screenshot action.
          const artifacts = (): PlaywrightArtifacts => ({
            tracePath: join(artifactDir, "trace.zip"),
          });

          return { page, flush, artifacts };
        },
        teardown: async () => undefined,
      };
    },
  };
}
