// Cloudflare Browser Rendering video capture (#6213).
//
// Browser Rendering (managed Chrome via `env.BROWSER`, driven over CDP by
// `@cloudflare/playwright`) has screenshots / PDF / CDP — but NO native video
// recording. The local-chromium backend films via Playwright `recordVideo` and
// reports a `videoPath`; the CF backend (#6205) previously could not, so a
// CF-executed run yielded screenshots-only and `artifacts()` reported NO
// `videoPath`.
//
// This module closes that gap WITHOUT faking anything:
//   1) CAPTURE: during a CF session we capture a SCREENCAST — a steady cadence of
//      page screenshots written as a zero-padded frame sequence into a dedicated
//      `frames/` subdir (kept OUT of the run dir's top level so the per-step
//      `*.png` screenshots remain the untouched artifact basis). A frame is taken
//      after each driven action (deterministic, even with no wall clock) and,
//      optionally, on a steady timer between actions.
//   2) COMPOSE: at flush we compose the captured frames into a playable mp4 with
//      ffmpeg's image2 demuxer (`-framerate N -i frame-%06d.png ... libx264`),
//      reusing the SAME fully-OSS ffmpeg path the compose layer (#6187) is built
//      on (no Remotion, no paid license).
//
// HONEST FALLBACK (no fake video):
//   - If NO frames were captured (e.g. every screenshot threw), compose is
//     skipped and `composeFramesToMp4` returns `null`. The backend then reports
//     NO `videoPath` and the screenshots-only artifact stands. We never emit an
//     empty or placeholder mp4.
//   - If ffmpeg is absent or the compose exits non-zero, we likewise return
//     `null` and keep the screenshots. A reviewer still confirms the run from the
//     per-step screenshots + result.json.
//
// DETERMINISM:
//   `buildFramesToMp4Args` is a pure string-in / string[]-out arg builder
//   (mirrors `compose/ffmpeg.ts`'s `buildFfmpegArgs`) so the compose command is
//   unit-asserted without spawning. `countFrames` lets a test prove the
//   capture→compose pipeline deterministically even on a host with no ffmpeg.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync } from "node:fs";
import { join } from "node:path";

/** A page slice that can screenshot to a path — the only capability the
 *  screencast needs. Satisfied by `CfPlaywrightPage` from cf-browser-backend. */
export interface ScreencastCapturablePage {
  screenshot(options: { readonly path: string }): Promise<unknown>;
}

/** The frame-file basename pattern. Zero-padded so lexical sort == capture order
 *  and ffmpeg's image2 demuxer reads them in sequence. */
export const FRAME_PREFIX = "frame-";
const FRAME_PAD = 6;
const FRAME_EXT = ".png";

/** The default screencast frame rate (frames per second) the mp4 is encoded at. */
export const DEFAULT_SCREENCAST_FPS = 4;

/** The output video basename, matching the local backend's `session.mp4`. */
export const CF_SESSION_VIDEO_NAME = "session.mp4";

/** Subdir (under the run's artifact dir) holding the raw screencast frames. */
export const CF_FRAMES_DIRNAME = "frames";

function frameName(index: number): string {
  return `${FRAME_PREFIX}${String(index).padStart(FRAME_PAD, "0")}${FRAME_EXT}`;
}

/** Count the captured frame files in a frames dir (deterministic, no ffmpeg). */
export function countFrames(framesDir: string): number {
  try {
    return readdirSync(framesDir).filter(
      (f) => f.startsWith(FRAME_PREFIX) && f.endsWith(FRAME_EXT),
    ).length;
  } catch {
    return 0;
  }
}

export interface CfScreencastOptions {
  /** The page to screenshot for each frame. */
  readonly page: ScreencastCapturablePage;
  /** Directory the frame sequence is written into (created if absent). */
  readonly framesDir: string;
  /**
   * Optional steady-cadence interval (ms) between auto-captured frames. When
   * provided (> 0), a timer captures a frame every `intervalMs` in addition to
   * the per-action frames. Omitted in unit tests (deterministic = action frames
   * only); a live run can set e.g. 250ms for a smoother screencast.
   */
  readonly intervalMs?: number;
}

/**
 * A running screencast: captures page screenshots into a zero-padded frame
 * sequence. `captureFrame()` is safe to call after each action; the optional
 * timer adds steady-cadence frames between actions. `stop()` clears the timer
 * and resolves once any in-flight capture settles.
 *
 * A capture that throws is SWALLOWED (a single dropped frame must never break the
 * run); the frame index only advances on a successful write, so the sequence
 * stays gapless for ffmpeg's `%06d` pattern.
 */
export interface CfScreencast {
  /** Capture one frame now. Resolves to true if a frame was written. */
  readonly captureFrame: () => Promise<boolean>;
  /** Number of frames written so far. */
  readonly frameCount: () => number;
  /** Stop the steady-cadence timer (if any) and await the last capture. */
  readonly stop: () => Promise<void>;
}

/**
 * Start a screencast against a page. Captures frames into `framesDir`. Call
 * `captureFrame()` after each driven action; `stop()` at flush.
 */
export function startCfScreencast(options: CfScreencastOptions): CfScreencast {
  mkdirSync(options.framesDir, { recursive: true });
  let index = 0;
  let inFlight: Promise<unknown> = Promise.resolve();
  let stopped = false;

  const captureFrame = async (): Promise<boolean> => {
    if (stopped) return false;
    const path = join(options.framesDir, frameName(index));
    try {
      const p = options.page.screenshot({ path });
      inFlight = p;
      await p;
      // Only advance on a real write so the sequence has no gaps.
      index += 1;
      return true;
    } catch {
      // A dropped frame is honest: the screencast just has one fewer frame. The
      // run continues; the index does not advance over a missing file.
      return false;
    }
  };

  let timer: ReturnType<typeof setInterval> | undefined;
  if (options.intervalMs !== undefined && options.intervalMs > 0) {
    timer = setInterval(() => {
      void captureFrame();
    }, options.intervalMs);
    // Do not keep the event loop alive solely for the screencast timer.
    (timer as { unref?: () => void }).unref?.();
  }

  const stop = async (): Promise<void> => {
    stopped = true;
    if (timer !== undefined) {
      clearInterval(timer);
      timer = undefined;
    }
    await inFlight.catch(() => undefined);
  };

  return { captureFrame, frameCount: () => index, stop };
}

/**
 * Wrap a screenshot-capable page so that every call to a wrapped action method
 * ALSO captures a screencast frame afterward. This is how the CF backend films
 * deterministically: the runner drives `navigate`/`click`/`type`/`screenshot`
 * and each driven action leaves a frame, with no separate runner changes.
 *
 * The listed methods are wrapped in place: the original method is preserved and
 * re-invoked, then a frame is captured. Methods not present on the page (or not
 * functions) are skipped.
 */
export function instrumentPageWithScreencast<P extends Record<string, unknown>>(
  page: P,
  screencast: CfScreencast,
  methods: ReadonlyArray<keyof P>,
): P {
  for (const key of methods) {
    const original = page[key];
    if (typeof original !== "function") continue;
    const fn = original as (...args: unknown[]) => unknown;
    (page as Record<string, unknown>)[key as string] = async (
      ...args: unknown[]
    ): Promise<unknown> => {
      const out = await fn.apply(page, args);
      await screencast.captureFrame();
      return out;
    };
  }
  return page;
}

export interface FramesToMp4Options {
  /** Frame rate the image sequence is encoded at. Default `DEFAULT_SCREENCAST_FPS`. */
  readonly fps?: number;
}

/**
 * Build the ffmpeg argv that composes a zero-padded PNG frame sequence into a
 * playable mp4. Pure: string in / string[] out, no spawning, no I/O. Mirrors
 * `compose/ffmpeg.ts` (same `-pix_fmt yuv420p` + `+faststart` for broad
 * playability), but reads an image2 sequence instead of a clip input.
 */
export function buildFramesToMp4Args(
  framesDir: string,
  outPath: string,
  options: FramesToMp4Options = {},
): string[] {
  const fps = options.fps ?? DEFAULT_SCREENCAST_FPS;
  const pattern = join(framesDir, `${FRAME_PREFIX}%0${FRAME_PAD}d${FRAME_EXT}`);
  return [
    "-y",
    "-framerate",
    String(fps),
    "-start_number",
    "0",
    "-i",
    pattern,
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
    outPath,
  ];
}

export interface ComposeFramesResult {
  /** Absolute path to the composed mp4. */
  readonly videoPath: string;
  readonly videoFormat: "mp4";
  /** Number of frames that fed the compose. */
  readonly frameCount: number;
  readonly args: string[];
}

/**
 * Compose the captured frame sequence in `framesDir` into `outPath` (an mp4)
 * with ffmpeg. HONEST FALLBACK: returns `null` (no video, screenshots stand)
 * when there are no frames, when ffmpeg is missing, or when the compose exits
 * non-zero — never an empty or placeholder file.
 */
export function composeFramesToMp4(
  framesDir: string,
  outPath: string,
  options: FramesToMp4Options & { readonly ffmpegBin?: string } = {},
): Promise<ComposeFramesResult | null> {
  const ffmpegBin = options.ffmpegBin ?? "ffmpeg";
  const frameCount = countFrames(framesDir);
  if (frameCount === 0) {
    // No frames captured -> no fake video. Screenshots remain the artifact basis.
    return Promise.resolve(null);
  }
  const args = buildFramesToMp4Args(framesDir, outPath, options);
  return new Promise((resolvePromise) => {
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn(ffmpegBin, args, { stdio: ["ignore", "ignore", "pipe"] });
    } catch {
      resolvePromise(null);
      return;
    }
    let stderr = "";
    child.stderr?.on("data", (d: Buffer) => {
      stderr += d.toString();
      if (stderr.length > 8000) stderr = stderr.slice(-8000);
    });
    // ffmpeg missing on PATH surfaces as a spawn 'error' (ENOENT): fall back.
    child.on("error", () => resolvePromise(null));
    child.on("close", (code) => {
      if (code === 0 && existsSync(outPath)) {
        resolvePromise({ videoPath: outPath, videoFormat: "mp4", frameCount, args });
      } else {
        // Non-zero exit -> honest fallback, keep screenshots.
        resolvePromise(null);
      }
    });
  });
}
