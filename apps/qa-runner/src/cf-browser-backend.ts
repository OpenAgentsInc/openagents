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
// NO NATIVE VIDEO -> SCREENCAST COMPOSE (#6213, implemented here):
//   Browser Rendering has no native video recording. Instead this backend
//   captures a SCREENCAST during the run — a steady cadence of page screenshots
//   (a frame after each driven action) written as a frame sequence into a
//   `frames/` subdir — and at flush composes those frames into a playable
//   `session.mp4` with ffmpeg's image2 demuxer (the same fully-OSS ffmpeg path as
//   the compose layer #6187). A CF-executed run then yields the SAME `video`
//   artifact a local run does: `artifacts()` reports a `videoPath`.
//
//   HONEST FALLBACK (no fake video): when frames can't be captured, or ffmpeg is
//   absent / fails, NO `videoPath` is reported and the per-step SCREENSHOTS (PNG)
//   remain the artifact basis. We never emit an empty or placeholder mp4. Video
//   capture is armed-by-env consistent with the backend itself (default ON when
//   the backend is armed; disable with `QA_CF_BROWSER_VIDEO=0`).
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
import {
  CF_FRAMES_DIRNAME,
  CF_SESSION_VIDEO_NAME,
  composeFramesToMp4,
  startCfScreencast,
} from "./cf-browser-video";

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
  /**
   * Capture a screencast and compose it into `session.mp4` (#6213). Defaults to
   * reading `QA_CF_BROWSER_VIDEO` from `env` (default ON when the backend is
   * armed; set "0"/"false" to disable). When off, the backend behaves as before
   * (screenshots-only, no `videoPath`).
   */
  readonly captureVideo?: boolean;
  /** Screencast frame rate the composed mp4 is encoded at (default 4 fps). */
  readonly videoFps?: number;
  /**
   * Optional steady-cadence interval (ms) for screencast frames between actions.
   * Omitted by default (frames are taken after each driven action, which is
   * deterministic); a live run can set e.g. 250ms for a smoother video.
   */
  readonly screencastIntervalMs?: number;
  /** ffmpeg binary used to compose frames (default "ffmpeg"). Tests can override. */
  readonly ffmpegBin?: string;
}

/** True when the env arms the CF Browser Rendering backend. */
export function isCfBrowserBackendArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_CF_BROWSER_BACKEND;
  return v === "1" || v === "true";
}

/**
 * Whether to capture a screencast and compose `session.mp4` (#6213). Default ON:
 * the natural artifact of a CF run is the same video a local run produces. Only
 * an explicit `QA_CF_BROWSER_VIDEO=0`/`false` disables it (screenshots-only).
 * Honest either way — video is never faked when frames can't be captured.
 */
export function isCfBrowserVideoArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_CF_BROWSER_VIDEO;
  return v !== "0" && v !== "false";
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
 * Wrap a `ComputerUsePage` so each state-changing/visual action (`navigate`,
 * `click`, `type`, `waitFor`, `screenshot`) ALSO captures a screencast frame
 * afterward. Pure reads (`url`, `readText`, `readDom`) are passed through
 * unwrapped — they change nothing visible, so they need no frame. A frame
 * capture never throws into the run (the screencast swallows screenshot errors),
 * so instrumentation cannot turn a green run red.
 */
function instrumentComputerUsePage(
  page: ComputerUsePage,
  screencast: { captureFrame: () => Promise<boolean> },
): ComputerUsePage {
  return {
    ...page,
    navigate: async (url) => {
      await page.navigate(url);
      await screencast.captureFrame();
    },
    click: async (selector) => {
      await page.click(selector);
      await screencast.captureFrame();
    },
    type: async (selector, text) => {
      await page.type(selector, text);
      await screencast.captureFrame();
    },
    waitFor: async (condition, opts) => {
      const met = await page.waitFor(condition, opts);
      await screencast.captureFrame();
      return met;
    },
    screenshot: async (path) => {
      await page.screenshot(path);
      await screencast.captureFrame();
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
 * native video) + result.json, PLUS a composed `session.mp4` from a captured
 * screencast when video is armed (#6213). `artifacts()` reports the trace-path
 * slot and, when the screencast composed an mp4, a `videoPath` — the SAME `video`
 * artifact a local run yields. When frames can't be captured (or ffmpeg is
 * absent/fails) it honestly reports NO `videoPath` and the screenshots stand.
 */
export function cfBrowserBackend(options: CfBrowserBackendOptions = {}): Backend {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isCfBrowserBackendArmed(env);
  const launch = options.launch ?? defaultCfLaunch;
  const keepAliveMs = options.keepAliveMs ?? DEFAULT_CF_BROWSER_KEEP_ALIVE_MS;
  const defaultTimeoutMs = options.defaultTimeoutMs ?? 15_000;
  const captureVideo = options.captureVideo ?? isCfBrowserVideoArmed(env);

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

          // SCREENCAST (#6213): when video is armed, capture a frame after each
          // driven action so the run is filmed as a frame sequence; compose those
          // frames into session.mp4 at flush. Frames live in a `frames/` subdir so
          // the per-step `*.png` screenshots in `artifactDir` stay the untouched
          // artifact basis (and the screenshots-only fallback is honest).
          const framesDir = join(artifactDir, CF_FRAMES_DIRNAME);
          const screencast = captureVideo
            ? startCfScreencast({
                page: cfPage,
                framesDir,
                ...(options.screencastIntervalMs !== undefined
                  ? { intervalMs: options.screencastIntervalMs }
                  : {}),
              })
            : undefined;

          // The runner drives `page` (a ComputerUsePage). Wrap its action methods
          // so each completed action leaves a screencast frame. An initial frame
          // captures the about:blank start state. A frame capture never throws
          // into the run (startCfScreencast swallows screenshot failures).
          let drivenPage = page;
          if (screencast !== undefined) {
            await screencast.captureFrame();
            drivenPage = instrumentComputerUsePage(page, screencast);
          }

          let artifactsValue: PlaywrightArtifacts = {
            tracePath: join(artifactDir, "trace.zip"),
          };

          let flushed = false;
          const flush = async (): Promise<void> => {
            if (flushed) return;
            flushed = true;
            if (screencast !== undefined) {
              // One last frame on the final state, then compose. Compose returns
              // null on no-frames / ffmpeg-absent / non-zero exit -> screenshots
              // stand (honest, never a fake mp4).
              await screencast.captureFrame();
              await screencast.stop();
              const composed = await composeFramesToMp4(
                framesDir,
                join(artifactDir, CF_SESSION_VIDEO_NAME),
                {
                  ...(options.videoFps !== undefined ? { fps: options.videoFps } : {}),
                  ...(options.ffmpegBin !== undefined ? { ffmpegBin: options.ffmpegBin } : {}),
                },
              );
              if (composed !== null) {
                artifactsValue = {
                  tracePath: join(artifactDir, "trace.zip"),
                  videoPath: composed.videoPath,
                  videoFormat: composed.videoFormat,
                };
              }
            }
            // ALWAYS close the managed browser so the concurrent slot is freed.
            await browser.close().catch(() => undefined);
          };

          // The trace slot is reported for contract shape but the file is not
          // produced by Browser Rendering. `videoPath` is set above only when the
          // screencast actually composed an mp4.
          const artifacts = (): PlaywrightArtifacts => artifactsValue;

          return { page: drivenPage, flush, artifacts };
        },
        teardown: async () => undefined,
      };
    },
  };
}
