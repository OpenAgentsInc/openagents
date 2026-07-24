import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rename, rm, stat } from "node:fs/promises";
import { basename, dirname, extname, join, resolve } from "node:path";
import { Effect, Schema as S } from "effect";
import { chromium, type Browser } from "playwright";

export const TimedTextCueStyle = S.Literals(["title", "center", "lower-third", "state-label"]);
export type TimedTextCueStyle = typeof TimedTextCueStyle.Type;

export const TimedTextCue = S.Struct({
  startSeconds: S.Number,
  endSeconds: S.Number,
  text: S.String,
  style: TimedTextCueStyle,
});
export type TimedTextCue = typeof TimedTextCue.Type;

export const TimedTextManifest = S.Struct({
  schemaVersion: S.Literal("openagents.media.timed_text.v1"),
  cues: S.Array(TimedTextCue),
});
export type TimedTextManifest = typeof TimedTextManifest.Type;

const decodeManifest = S.decodeUnknownSync(TimedTextManifest);

export interface VideoProbe {
  width: number;
  height: number;
  durationSeconds: number;
  hasAudio: boolean;
}

export interface TimedTextRenderOptions {
  inputPath: string;
  cuePath: string;
  outputPath: string;
  audioMode: "copy" | "aac";
  force: boolean;
  ffmpegBin?: string;
  ffprobeBin?: string;
}

export interface TimedTextArgs {
  input?: string;
  cues?: string;
  out?: string;
  audioMode: "copy" | "aac";
  ffmpeg?: string;
  ffprobe?: string;
  force: boolean;
}

export interface TimedTextRenderResult extends VideoProbe {
  outputPath: string;
  cueCount: number;
  audioMode: "copy" | "aac";
}

interface ProcessResult {
  stdout: string;
  stderr: string;
}

const MAX_CUES = 250;
const MAX_CUE_TEXT_LENGTH = 280;
const MAX_CUE_DURATION_SECONDS = 120;

export function parseTimedTextArgs(argv: ReadonlyArray<string>): TimedTextArgs {
  const args: TimedTextArgs = { audioMode: "copy", force: false };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    switch (flag) {
      case "--input":
        args.input = argv[++index];
        break;
      case "--cues":
        args.cues = argv[++index];
        break;
      case "--out":
        args.out = argv[++index];
        break;
      case "--audio": {
        const mode = argv[++index];
        if (mode !== "copy" && mode !== "aac") {
          throw new Error('timed-text: --audio must be "copy" or "aac"');
        }
        args.audioMode = mode;
        break;
      }
      case "--ffmpeg":
        args.ffmpeg = argv[++index];
        break;
      case "--ffprobe":
        args.ffprobe = argv[++index];
        break;
      case "--force":
        args.force = true;
        break;
      default:
        throw new Error(`timed-text: unknown flag "${flag}"`);
    }
  }
  return args;
}

function finiteNumber(value: number, field: string): void {
  if (!Number.isFinite(value)) {
    throw new Error(`${field} must be a finite number`);
  }
}

export function validateTimedTextManifest(
  input: unknown,
  videoDurationSeconds?: number,
): TimedTextManifest {
  const manifest = decodeManifest(input);

  if (manifest.cues.length === 0) {
    throw new Error("timed-text: the cue sheet must contain at least one cue");
  }
  if (manifest.cues.length > MAX_CUES) {
    throw new Error(`timed-text: the cue sheet exceeds the ${MAX_CUES}-cue limit`);
  }

  manifest.cues.forEach((cue, index) => {
    const prefix = `timed-text: cue ${index + 1}`;
    finiteNumber(cue.startSeconds, `${prefix} startSeconds`);
    finiteNumber(cue.endSeconds, `${prefix} endSeconds`);
    if (cue.startSeconds < 0) {
      throw new Error(`${prefix} startSeconds must be zero or greater`);
    }
    if (cue.endSeconds <= cue.startSeconds) {
      throw new Error(`${prefix} endSeconds must be greater than startSeconds`);
    }
    if (cue.endSeconds - cue.startSeconds > MAX_CUE_DURATION_SECONDS) {
      throw new Error(`${prefix} exceeds the ${MAX_CUE_DURATION_SECONDS}-second cue limit`);
    }
    if (cue.text.trim().length === 0) {
      throw new Error(`${prefix} text must not be empty`);
    }
    if (cue.text.length > MAX_CUE_TEXT_LENGTH) {
      throw new Error(`${prefix} text exceeds the ${MAX_CUE_TEXT_LENGTH}-character limit`);
    }
    if (videoDurationSeconds !== undefined && cue.endSeconds > videoDurationSeconds + 0.05) {
      throw new Error(
        `${prefix} ends after the source video (${videoDurationSeconds.toFixed(3)}s)`,
      );
    }
  });

  return manifest;
}

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function cuePresentation(style: TimedTextCueStyle): {
  frameClass: string;
  panelClass: string;
} {
  switch (style) {
    case "title":
      return { frameClass: "frame title-frame", panelClass: "panel title" };
    case "center":
      return { frameClass: "frame center-frame", panelClass: "panel center" };
    case "lower-third":
      return {
        frameClass: "frame lower-third-frame",
        panelClass: "panel lower-third",
      };
    case "state-label":
      return {
        frameClass: "frame state-label-frame",
        panelClass: "panel state-label",
      };
  }
}

export function buildCueHtml(cue: TimedTextCue, width: number, height: number): string {
  const presentation = cuePresentation(cue.style);
  const text = escapeHtml(cue.text).replace(/\r?\n/g, "<br>");
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  * { box-sizing: border-box; }
  html, body {
    width: ${width}px;
    height: ${height}px;
    margin: 0;
    overflow: hidden;
    background: transparent;
    font-family: "Berkeley Mono", "Commit Mono", ui-monospace, SFMono-Regular,
      Menlo, monospace;
  }
  .frame {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    padding: 4.5%;
  }
  .panel {
    color: #ffffff;
    font-weight: 720;
    letter-spacing: 0.035em;
    line-height: 1.14;
    text-wrap: balance;
    text-shadow: 0 2px 14px rgba(0, 0, 0, 0.72);
  }
  .title-frame {
    align-items: center;
    justify-content: center;
    background: rgba(5, 7, 13, 0.76);
  }
  .title {
    max-width: 88%;
    padding: 0.55em 0.8em;
    font-size: clamp(38px, 5.6vw, 92px);
    text-align: center;
    text-transform: uppercase;
  }
  .center-frame {
    align-items: center;
    justify-content: center;
  }
  .center {
    max-width: 86%;
    padding: 0.48em 0.7em;
    border: 1px solid rgba(58, 123, 255, 0.62);
    border-radius: 10px;
    background: rgba(12, 15, 19, 0.92);
    font-size: clamp(34px, 4.6vw, 76px);
    text-align: center;
  }
  .lower-third-frame {
    align-items: flex-end;
    justify-content: flex-start;
  }
  .lower-third {
    max-width: 82%;
    padding: 0.52em 0.72em;
    border: 1px solid rgba(58, 123, 255, 0.72);
    border-radius: 7px;
    background: rgba(12, 15, 19, 0.94);
    box-shadow: 0 0 8px rgba(58, 123, 255, 0.24);
    font-size: clamp(28px, 3.4vw, 56px);
  }
  .state-label-frame {
    align-items: flex-start;
    justify-content: flex-start;
    padding: 2.5%;
  }
  .state-label {
    max-width: 78%;
    padding: 0.42em 0.62em;
    border: 1px solid rgba(79, 208, 255, 0.76);
    border-radius: 5px;
    background: rgba(12, 15, 19, 0.94);
    color: #8fb6ff;
    font-size: clamp(20px, 2.25vw, 38px);
    text-transform: uppercase;
  }
</style>
</head>
<body>
  <div class="${presentation.frameClass}">
    <div class="${presentation.panelClass}">${text}</div>
  </div>
</body>
</html>`;
}

function runProcess(command: string, args: ReadonlyArray<string>): Promise<ProcessResult> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, [...args], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
      if (stderr.length > 16_000) stderr = stderr.slice(-16_000);
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolvePromise({ stdout, stderr });
      } else {
        reject(
          new Error(
            `${basename(command)} exited ${code ?? "without a status"}\n${stderr.slice(-4000)}`,
          ),
        );
      }
    });
  });
}

export function parseVideoProbe(raw: string): VideoProbe {
  const parsed = JSON.parse(raw) as {
    streams?: Array<{
      codec_type?: string;
      width?: number;
      height?: number;
    }>;
    format?: { duration?: string };
  };
  const video = parsed.streams?.find((stream) => stream.codec_type === "video");
  const durationSeconds = Number(parsed.format?.duration);
  if (
    video?.width === undefined ||
    video.height === undefined ||
    !Number.isInteger(video.width) ||
    !Number.isInteger(video.height) ||
    video.width <= 0 ||
    video.height <= 0
  ) {
    throw new Error("timed-text: ffprobe did not return a valid video size");
  }
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    throw new Error("timed-text: ffprobe did not return a valid video duration");
  }
  return {
    width: video.width,
    height: video.height,
    durationSeconds,
    hasAudio: parsed.streams?.some((stream) => stream.codec_type === "audio") ?? false,
  };
}

async function probeVideo(inputPath: string, ffprobeBin: string): Promise<VideoProbe> {
  const result = await runProcess(ffprobeBin, [
    "-v",
    "error",
    "-show_entries",
    "stream=codec_type,width,height:format=duration",
    "-of",
    "json",
    inputPath,
  ]);
  return parseVideoProbe(result.stdout);
}

function formatTime(value: number): string {
  return value.toFixed(3).replace(/\.?0+$/, "");
}

export function buildTimedTextFfmpegArgs(
  inputPath: string,
  cuePngPaths: ReadonlyArray<string>,
  manifest: TimedTextManifest,
  outputPath: string,
  audioMode: "copy" | "aac",
): string[] {
  if (cuePngPaths.length !== manifest.cues.length) {
    throw new Error("timed-text: the rendered cue count does not match the manifest");
  }

  const args = ["-y", "-i", inputPath];
  cuePngPaths.forEach((cuePath) => {
    args.push("-loop", "1", "-framerate", "30", "-i", cuePath);
  });

  const filters: string[] = [];
  let current = "[0:v]";
  manifest.cues.forEach((cue, index) => {
    const next = `[overlay${index}]`;
    const start = formatTime(cue.startSeconds);
    const end = formatTime(cue.endSeconds);
    filters.push(
      `${current}[${index + 1}:v]overlay=0:0:enable='between(t\\,${start}\\,${end})':shortest=1${next}`,
    );
    current = next;
  });

  args.push(
    "-filter_complex",
    filters.join(";"),
    "-map",
    current,
    "-map",
    "0:a?",
    "-map_metadata",
    "0",
    "-c:v",
    "libx264",
    "-preset",
    "medium",
    "-crf",
    "18",
    "-pix_fmt",
    "yuv420p",
    "-c:a",
    audioMode,
  );
  if (audioMode === "aac") {
    args.push("-b:a", "192k");
  }
  args.push("-movflags", "+faststart", outputPath);
  return args;
}

async function renderCuePngs(
  browser: Browser,
  manifest: TimedTextManifest,
  probe: VideoProbe,
  directory: string,
): Promise<Array<string>> {
  const page = await browser.newPage({
    viewport: { width: probe.width, height: probe.height },
    deviceScaleFactor: 1,
  });
  try {
    const paths: string[] = [];
    for (let index = 0; index < manifest.cues.length; index += 1) {
      const cue = manifest.cues[index];
      const cuePath = join(directory, `cue-${String(index).padStart(3, "0")}.png`);
      await page.setContent(buildCueHtml(cue, probe.width, probe.height), {
        waitUntil: "load",
      });
      await page.screenshot({
        path: cuePath,
        type: "png",
        omitBackground: true,
      });
      paths.push(cuePath);
    }
    return paths;
  } finally {
    await page.close();
  }
}

async function assertRegularFile(path: string, label: string): Promise<void> {
  await access(path);
  const details = await stat(path);
  if (!details.isFile()) {
    throw new Error(`timed-text: ${label} must be a regular file`);
  }
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

async function renderTimedTextPromise(
  options: TimedTextRenderOptions,
): Promise<TimedTextRenderResult> {
  const inputPath = resolve(options.inputPath);
  const cuePath = resolve(options.cuePath);
  const outputPath = resolve(options.outputPath);
  const ffmpegBin = options.ffmpegBin ?? "ffmpeg";
  const ffprobeBin = options.ffprobeBin ?? "ffprobe";

  if (inputPath === outputPath) {
    throw new Error("timed-text: input and output paths must be different");
  }
  if (extname(outputPath).toLowerCase() !== ".mp4") {
    throw new Error("timed-text: the output path must use the .mp4 extension");
  }
  await assertRegularFile(inputPath, "input");
  await assertRegularFile(cuePath, "cue sheet");
  const outputExists = await pathExists(outputPath);
  if (outputExists && !options.force) {
    throw new Error("timed-text: output already exists; use --force to replace it");
  }

  const parsed = JSON.parse(await readFile(cuePath, "utf8")) as unknown;
  const probe = await probeVideo(inputPath, ffprobeBin);
  const manifest = validateTimedTextManifest(parsed, probe.durationSeconds);
  await mkdir(dirname(outputPath), { recursive: true });
  const temporaryDirectory = await mkdtemp(join(dirname(outputPath), ".openagents-timed-text-"));
  const renderedOutputPath = join(temporaryDirectory, "render.mp4");
  let browser: Browser | undefined;

  try {
    browser = await chromium.launch({ headless: true });
    const cuePngs = await renderCuePngs(browser, manifest, probe, temporaryDirectory);
    const ffmpegArgs = buildTimedTextFfmpegArgs(
      inputPath,
      cuePngs,
      manifest,
      renderedOutputPath,
      options.audioMode,
    );
    await runProcess(ffmpegBin, ffmpegArgs);
    if (outputExists) {
      await rm(outputPath);
    }
    await rename(renderedOutputPath, outputPath);
  } finally {
    await browser?.close().catch(() => undefined);
    await rm(temporaryDirectory, { recursive: true, force: true });
  }

  return {
    ...probe,
    outputPath,
    cueCount: manifest.cues.length,
    audioMode: options.audioMode,
  };
}

export function renderTimedText(
  options: TimedTextRenderOptions,
): Effect.Effect<TimedTextRenderResult, Error> {
  return Effect.tryPromise({
    try: () => renderTimedTextPromise(options),
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });
}
