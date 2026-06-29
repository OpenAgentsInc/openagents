// CF Browser Rendering video capture tests (#6213).
//
// Proves the screencast -> compose pipeline WITHOUT a live Cloudflare binding and
// WITHOUT assuming native video:
//   - the pure ffmpeg arg builder is deterministic + well-formed (no spawning),
//   - `startCfScreencast` captures a gapless frame sequence (and counts it),
//   - `composeFramesToMp4` honestly returns null on no-frames / ffmpeg-failure,
//   - an end-to-end fake CF run with VALID PNG frames composes a real, playable
//     mp4 (ffprobe-verified when ffmpeg is present; otherwise the frame pipeline
//     + compose plan are asserted deterministically),
//   - the screenshots-only fallback (video disabled) reports NO video.

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import {
  cfBrowserBackend,
  isCfBrowserVideoArmed,
  type CfPlaywrightBrowser,
  type CfPlaywrightLaunch,
  type CfPlaywrightPage,
} from "./cf-browser-backend";
import {
  buildFramesToMp4Args,
  composeFramesToMp4,
  countFrames,
  CF_FRAMES_DIRNAME,
  CF_SESSION_VIDEO_NAME,
  DEFAULT_SCREENCAST_FPS,
  FRAME_PREFIX,
  startCfScreencast,
} from "./cf-browser-video";
import { scriptedBrain, type BrainStep } from "./brain";
import { decodeQaRunResult } from "./result";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";

// A valid 16x16 black RGB PNG (even dims so libx264/yuv420p accepts it). Written
// for every screencast frame so a real ffmpeg compose actually succeeds.
const VALID_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAEElEQVR4nGNgGAWjYBTAAAADEAABPywr7AAAAABJRU5ErkJggg==",
  "base64",
);

const target = makeTarget({ name: "cf-fake", baseUrl: "https://example.test" });

/** True if ffmpeg + ffprobe are on PATH (gates the real-render assertions). */
function ffmpegPresent(): boolean {
  try {
    return (
      spawnSync("ffmpeg", ["-version"], { stdio: "ignore" }).status === 0 &&
      spawnSync("ffprobe", ["-version"], { stdio: "ignore" }).status === 0
    );
  } catch {
    return false;
  }
}

/** ffprobe a video file -> { codec, frames } (only called when ffmpeg present). */
function ffprobe(path: string): { codec: string; frames: number } {
  const out = spawnSync(
    "ffprobe",
    [
      "-v",
      "error",
      "-select_streams",
      "v:0",
      "-count_frames",
      "-show_entries",
      "stream=codec_name,nb_read_frames",
      "-of",
      "default=nw=1",
      path,
    ],
    { encoding: "utf8" },
  );
  const text = `${out.stdout ?? ""}`;
  const codec = /codec_name=(\S+)/.exec(text)?.[1] ?? "";
  const frames = Number(/nb_read_frames=(\d+)/.exec(text)?.[1] ?? "0");
  return { codec, frames };
}

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-cf-video-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("isCfBrowserVideoArmed (default ON, env can disable)", () => {
  test("defaults ON; only 0/false disable", () => {
    expect(isCfBrowserVideoArmed({})).toBe(true);
    expect(isCfBrowserVideoArmed({ QA_CF_BROWSER_VIDEO: "1" })).toBe(true);
    expect(isCfBrowserVideoArmed({ QA_CF_BROWSER_VIDEO: "true" })).toBe(true);
    expect(isCfBrowserVideoArmed({ QA_CF_BROWSER_VIDEO: "0" })).toBe(false);
    expect(isCfBrowserVideoArmed({ QA_CF_BROWSER_VIDEO: "false" })).toBe(false);
  });
});

describe("buildFramesToMp4Args (pure, deterministic)", () => {
  test("reads the zero-padded image2 sequence into a yuv420p faststart mp4", () => {
    const args = buildFramesToMp4Args("/runs/x/frames", "/runs/x/session.mp4");
    expect(args[0]).toBe("-y");
    expect(args).toContain("-framerate");
    expect(args[args.indexOf("-framerate") + 1]).toBe(String(DEFAULT_SCREENCAST_FPS));
    // image2 pattern with the frame prefix + %06d
    expect(args.some((a) => a.endsWith(`${FRAME_PREFIX}%06d.png`))).toBe(true);
    expect(args.some((a) => a.includes("/runs/x/frames/"))).toBe(true);
    expect(args).toContain("libx264");
    expect(args).toContain("yuv420p");
    expect(args).toContain("+faststart");
    expect(args[args.length - 1]).toBe("/runs/x/session.mp4");
  });

  test("fps override flows into -framerate; identical input => identical args", () => {
    const a = buildFramesToMp4Args("/f", "/o.mp4", { fps: 8 });
    const b = buildFramesToMp4Args("/f", "/o.mp4", { fps: 8 });
    expect(a[a.indexOf("-framerate") + 1]).toBe("8");
    expect(a).toEqual(b);
  });
});

describe("startCfScreencast (gapless frame capture)", () => {
  test("captureFrame writes a zero-padded sequence; countFrames matches", async () => {
    const framesDir = join(dir, "frames");
    const page = {
      screenshot: async ({ path }: { path: string }) => {
        const { writeFileSync } = await import("node:fs");
        writeFileSync(path, VALID_PNG);
        return null;
      },
    };
    const sc = startCfScreencast({ page, framesDir });
    expect(await sc.captureFrame()).toBe(true);
    expect(await sc.captureFrame()).toBe(true);
    expect(await sc.captureFrame()).toBe(true);
    await sc.stop();

    expect(sc.frameCount()).toBe(3);
    expect(countFrames(framesDir)).toBe(3);
    const names = readdirSync(framesDir).sort();
    expect(names).toEqual([
      "frame-000000.png",
      "frame-000001.png",
      "frame-000002.png",
    ]);
  });

  test("a screenshot that throws drops the frame without advancing the index", async () => {
    const framesDir = join(dir, "frames");
    let calls = 0;
    const page = {
      screenshot: async ({ path }: { path: string }) => {
        calls += 1;
        if (calls === 2) throw new Error("transient screencast failure");
        const { writeFileSync } = await import("node:fs");
        writeFileSync(path, VALID_PNG);
        return null;
      },
    };
    const sc = startCfScreencast({ page, framesDir });
    expect(await sc.captureFrame()).toBe(true); // frame-000000
    expect(await sc.captureFrame()).toBe(false); // dropped, index unchanged
    expect(await sc.captureFrame()).toBe(true); // frame-000001 (gapless)
    await sc.stop();

    expect(countFrames(framesDir)).toBe(2);
    expect(readdirSync(framesDir).sort()).toEqual([
      "frame-000000.png",
      "frame-000001.png",
    ]);
  });
});

describe("composeFramesToMp4 honest fallback", () => {
  test("returns null when there are no frames (no fake video)", async () => {
    const framesDir = join(dir, "empty-frames");
    const { mkdirSync } = await import("node:fs");
    mkdirSync(framesDir, { recursive: true });
    const result = await composeFramesToMp4(framesDir, join(dir, "session.mp4"));
    expect(result).toBeNull();
    expect(existsSync(join(dir, "session.mp4"))).toBe(false);
  });

  test("returns null when ffmpeg is absent (bogus binary) -> screenshots stand", async () => {
    const framesDir = join(dir, "frames");
    const sc = startCfScreencast({
      page: {
        screenshot: async ({ path }: { path: string }) => {
          const { writeFileSync } = await import("node:fs");
          writeFileSync(path, VALID_PNG);
          return null;
        },
      },
      framesDir,
    });
    await sc.captureFrame();
    await sc.stop();
    const result = await composeFramesToMp4(framesDir, join(dir, "session.mp4"), {
      ffmpegBin: "ffmpeg-definitely-not-installed-xyz",
    });
    expect(result).toBeNull();
    expect(existsSync(join(dir, "session.mp4"))).toBe(false);
  });

  test("composes a real playable mp4 from valid frames (ffprobe)", async () => {
    if (!ffmpegPresent()) {
      // No ffmpeg on this host: assert the deterministic frame pipeline instead.
      const framesDir = join(dir, "frames");
      const sc = startCfScreencast({
        page: {
          screenshot: async ({ path }: { path: string }) => {
            const { writeFileSync } = await import("node:fs");
            writeFileSync(path, VALID_PNG);
            return null;
          },
        },
        framesDir,
      });
      for (let i = 0; i < 5; i++) await sc.captureFrame();
      await sc.stop();
      expect(countFrames(framesDir)).toBe(5);
      const args = buildFramesToMp4Args(framesDir, join(dir, CF_SESSION_VIDEO_NAME));
      expect(args).toContain("libx264");
      return;
    }
    const framesDir = join(dir, "frames");
    const sc = startCfScreencast({
      page: {
        screenshot: async ({ path }: { path: string }) => {
          const { writeFileSync } = await import("node:fs");
          writeFileSync(path, VALID_PNG);
          return null;
        },
      },
      framesDir,
    });
    for (let i = 0; i < 6; i++) await sc.captureFrame();
    await sc.stop();

    const out = join(dir, CF_SESSION_VIDEO_NAME);
    const result = await composeFramesToMp4(framesDir, out);
    expect(result).not.toBeNull();
    expect(result!.videoFormat).toBe("mp4");
    expect(result!.frameCount).toBe(6);
    expect(existsSync(out)).toBe(true);
    const probe = ffprobe(out);
    expect(probe.codec).toBe("h264");
    expect(probe.frames).toBe(6);
  });
});

describe("cfBrowserBackend end-to-end video (fake env.BROWSER)", () => {
  const scenario: ReadonlyArray<BrainStep> = [
    { kind: "navigate", url: "/welcome", label: "go to welcome" },
    { kind: "wait-for", condition: { kind: "text-visible", value: "Welcome" }, label: "welcome shows" },
    { kind: "screenshot", label: "welcome" },
    { kind: "assert", check: { kind: "url-includes", value: "/welcome" }, label: "on welcome url" },
  ];

  /** Fake launch whose page writes VALID PNGs on screenshot (frames + steps). */
  function makeFakeCfLaunch(text: Record<string, string>): CfPlaywrightLaunch {
    return async () => {
      let currentUrl = "about:blank";
      const resolve = (url: string) =>
        /^https?:/.test(url) ? url : `https://example.test${url}`;
      const textForUrl = () => {
        for (const [k, v] of Object.entries(text)) {
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
          const { writeFileSync } = await import("node:fs");
          writeFileSync(path, VALID_PNG);
          return null;
        },
        waitForURL: async (pred) => {
          if (!pred(currentUrl)) throw new Error("url predicate not met");
        },
        waitForSelector: async (selector) => {
          const t = textForUrl();
          const needle = selector.startsWith("text=") ? selector.slice("text=".length) : selector;
          if (!t.includes(needle)) throw new Error(`selector not visible: ${selector}`);
          return null;
        },
      };
      const browser: CfPlaywrightBrowser = {
        newPage: async () => page,
        close: async () => undefined,
      };
      return browser;
    };
  }

  test("armed run captures a screencast and composes session.mp4 (video artifact)", async () => {
    const backend = cfBrowserBackend({
      armed: true,
      browser: {},
      // video armed via empty env default-ON
      env: {},
      captureVideo: true,
      launch: makeFakeCfLaunch({ "/welcome": "Welcome to OpenAgents" }),
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

    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("cf-browser");

    // Screencast frames were captured into the frames subdir (NOT the top-level
    // screenshots list).
    const framesDir = join(dir, CF_FRAMES_DIRNAME);
    expect(countFrames(framesDir)).toBeGreaterThanOrEqual(1);

    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    // Per-step screenshots remain the artifact basis (frames not counted here).
    expect(onDisk.artifacts.screenshots.length).toBeGreaterThanOrEqual(1);
    expect(
      onDisk.artifacts.screenshots.every((s) => !s.startsWith(CF_FRAMES_DIRNAME)),
    ).toBe(true);

    if (ffmpegPresent()) {
      // A CF-executed run yields the SAME video artifact a local run does.
      expect(onDisk.artifacts.video).toBe(CF_SESSION_VIDEO_NAME);
      expect(onDisk.artifacts.videoFormat).toBe("mp4");
      const probe = ffprobe(join(dir, CF_SESSION_VIDEO_NAME));
      expect(probe.codec).toBe("h264");
      expect(probe.frames).toBeGreaterThanOrEqual(1);
    } else {
      // Honest fallback when ffmpeg is absent: no video, screenshots stand.
      expect(onDisk.artifacts.video).toBeUndefined();
    }
  });

  test("video disabled -> screenshots-only fallback (no video, no frames dir)", async () => {
    const backend = cfBrowserBackend({
      armed: true,
      browser: {},
      captureVideo: false,
      launch: makeFakeCfLaunch({ "/welcome": "Welcome to OpenAgents" }),
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

    expect(outcome.result.status).toBe("pass");
    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    // No video when capture is off (honest, screenshots remain the basis).
    expect(onDisk.artifacts.video).toBeUndefined();
    expect(onDisk.artifacts.screenshots.length).toBeGreaterThanOrEqual(1);
    expect(existsSync(join(dir, CF_FRAMES_DIRNAME))).toBe(false);
  });

  test("frames can't compose (no ffmpeg) -> honest screenshots-only", async () => {
    const backend = cfBrowserBackend({
      armed: true,
      browser: {},
      captureVideo: true,
      ffmpegBin: "ffmpeg-definitely-not-installed-xyz",
      launch: makeFakeCfLaunch({ "/welcome": "Welcome to OpenAgents" }),
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

    expect(outcome.result.status).toBe("pass");
    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    // Frames were captured, but compose failed -> NO fake video; screenshots win.
    expect(countFrames(join(dir, CF_FRAMES_DIRNAME))).toBeGreaterThanOrEqual(1);
    expect(onDisk.artifacts.video).toBeUndefined();
    expect(onDisk.artifacts.screenshots.length).toBeGreaterThanOrEqual(1);
  });
});
