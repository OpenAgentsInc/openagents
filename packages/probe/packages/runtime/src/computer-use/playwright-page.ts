// Real-chromium adapter for the browser tool seam.
//
// This module is the ONLY place that imports Playwright. It is deliberately NOT
// referenced by unit tests (which inject a fake `ComputerUsePage`) so unit CI
// never launches chromium or touches the network. The qa-runner (#6176/#6177)
// drives this for the real-chromium proof.
//
// It mirrors executor's `e2e/src/surfaces/browser.ts`:
//   - dark mode, fixed viewport
//   - Playwright trace (screenshots+snapshots+sources)
//   - `recordVideo` (webm) flushed on context close, then transcoded to mp4
//     with ffmpeg if available (else the webm is kept and reported)
//   - the flush is wired into the `AcquiredBrowser.flush()` hook so the
//     browser-surface `acquireUseRelease` release block closes + persists even
//     on interruption.

import { execFile } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { promisify } from "node:util";
import type { AcquiredBrowser } from "./browser";
import type { ComputerUsePage, WaitForCondition } from "./page";

const execFileAsync = promisify(execFile);

export interface PlaywrightBrowserOptions {
  /** Base URL navigations resolve against (the run target). */
  readonly baseUrl?: string;
  /** Directory artifacts are written to (trace.zip, session.webm/mp4). */
  readonly artifactDir: string;
  /** Run headed (for filming). Defaults to headless. */
  readonly headed?: boolean;
  /** Default wait deadline in ms. */
  readonly defaultTimeoutMs?: number;
  /** Override the playwright module (kept tiny; the import stays dynamic). */
  readonly chromium?: unknown;
}

export interface PlaywrightArtifacts {
  readonly tracePath: string;
  /** mp4 if ffmpeg transcoded the recording, else the raw webm. */
  readonly videoPath?: string;
  readonly videoFormat?: "mp4" | "webm";
}

/**
 * Acquire a real chromium-backed browser. Returns the `AcquiredBrowser` the
 * browser surface drives, plus an `artifacts()` accessor resolved after flush.
 */
export async function acquirePlaywrightBrowser(
  options: PlaywrightBrowserOptions,
): Promise<AcquiredBrowser & { artifacts: () => PlaywrightArtifacts }> {
  mkdirSync(options.artifactDir, { recursive: true });
  const videoTmp = join(options.artifactDir, ".video-tmp");
  mkdirSync(videoTmp, { recursive: true });

  // Dynamic import keeps Playwright out of the unit-test module graph.
  const pw = (options.chromium ?? (await import("playwright")).chromium) as typeof import("playwright").chromium;
  const browser = await pw.launch(options.headed ? { headless: false } : {});
  const context = await browser.newContext({
    colorScheme: "dark",
    viewport: { width: 1280, height: 800 },
    recordVideo: { dir: videoTmp, size: { width: 1280, height: 800 } },
    ...(options.baseUrl ? { baseURL: options.baseUrl } : {}),
  });
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  const pwPage = await context.newPage();
  const defaultTimeoutMs = options.defaultTimeoutMs ?? 15_000;

  const page: ComputerUsePage = {
    navigate: async (url) => {
      await pwPage.goto(url, { waitUntil: "load" });
    },
    url: async () => pwPage.url(),
    click: async (selector) => {
      await pwPage.click(selector, { timeout: defaultTimeoutMs });
    },
    type: async (selector, text) => {
      await pwPage.fill(selector, text, { timeout: defaultTimeoutMs });
    },
    readText: async (selector) => {
      if (selector) return (await pwPage.locator(selector).first().innerText()).trim();
      return (await pwPage.locator("body").innerText()).trim();
    },
    readDom: async (selector) => {
      if (selector) return await pwPage.locator(selector).first().evaluate((el) => (el as Element).outerHTML);
      return await pwPage.content();
    },
    waitFor: async (condition: WaitForCondition, opts) => {
      const timeout = opts?.timeoutMs ?? defaultTimeoutMs;
      try {
        switch (condition.kind) {
          case "url-includes":
            await pwPage.waitForURL((u) => u.toString().includes(condition.value), { timeout });
            return true;
          case "text-visible":
            await pwPage.getByText(condition.value).first().waitFor({ state: "visible", timeout });
            return true;
          case "selector-visible":
            await pwPage.locator(condition.selector).first().waitFor({ state: "visible", timeout });
            return true;
        }
      } catch {
        return false;
      }
    },
    screenshot: async (path) => {
      await pwPage.screenshot({ path });
    },
  };

  let artifacts: PlaywrightArtifacts = { tracePath: join(options.artifactDir, "trace.zip") };
  let flushed = false;

  const flush = async (): Promise<void> => {
    if (flushed) return;
    flushed = true;
    const tracePath = join(options.artifactDir, "trace.zip");
    await context.tracing.stop({ path: tracePath }).catch(() => undefined);
    const video = pwPage.video();
    await context.close().catch(() => undefined); // flushes the recording
    await browser.close().catch(() => undefined);
    const recordedPath = await video?.path().catch(() => undefined);
    let videoPath: string | undefined;
    let videoFormat: "mp4" | "webm" | undefined;
    if (recordedPath && existsSync(recordedPath)) {
      const mp4 = join(options.artifactDir, "session.mp4");
      try {
        await execFileAsync("ffmpeg", [
          "-y",
          "-i",
          recordedPath,
          "-c:v",
          "libx264",
          "-preset",
          "veryfast",
          "-crf",
          "26",
          "-pix_fmt",
          "yuv420p",
          "-movflags",
          "+faststart",
          mp4,
        ]);
        videoPath = mp4;
        videoFormat = "mp4";
      } catch {
        // ffmpeg missing or failed: keep the webm as-is and say so.
        const webm = join(options.artifactDir, "session.webm");
        copyFileSync(recordedPath, webm);
        videoPath = webm;
        videoFormat = "webm";
      }
    }
    rmSync(videoTmp, { recursive: true, force: true });
    artifacts = {
      tracePath,
      ...(videoPath ? { videoPath } : {}),
      ...(videoFormat ? { videoFormat } : {}),
    };
  };

  return { page, flush, artifacts: () => artifacts };
}
