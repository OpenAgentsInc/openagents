// ffmpeg executor — the thin, impure half of the compose layer.
//
// It consumes a ComposePlan (produced by the pure `buildComposePlan`) and
// renders a polished mp4 using ONLY ffmpeg primitives (color sources for title
// cards, scale/pad + hstack for side-by-side, drawtext/drawbox for overlays,
// concat for stitching). No Remotion, no paid license — fully OSS.
//
// The command builder (`buildFfmpegArgs`) is exported and largely pure (string
// in / string array out) so it can be exercised in tests; `renderComposePlan`
// is the spawning wrapper that actually shells out.

import { spawn, spawnSync } from "node:child_process";
import { resolve } from "node:path";
import type {
  ClipSegment,
  ComposePlan,
  RGBA,
  TextOverlay,
  TitleCardSegment,
} from "./plan.ts";

export interface RunDirs {
  /** Required for `single` plans; one of the side-by-side dirs otherwise. */
  run?: string;
  before?: string;
  after?: string;
}

/** ffmpeg `color=` / `fontcolor=` value with alpha, e.g. "0x0D1117@1". */
function ffColor(c: RGBA): string {
  const hex = (n: number) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
  return `0x${hex(c.r)}${hex(c.g)}${hex(c.b)}@${c.a}`;
}

/** Escape text for use inside a drawtext `text=` value. */
export function escapeDrawText(text: string): string {
  // Order matters: backslash first.
  return text
    .replace(/\\/g, "\\\\")
    .replace(/:/g, "\\:")
    .replace(/'/g, "’") // curly apostrophe avoids quote-escaping pitfalls
    .replace(/%/g, "\\%")
    .replace(/\n/g, " ");
}

/** Resolve a normalized x anchor + align into a drawtext `x=` expression. */
function drawTextX(o: TextOverlay): string {
  const px = `(w*${o.x})`;
  switch (o.align) {
    case "left":
      return px;
    case "right":
      return `(${px}-text_w)`;
    case "center":
    default:
      return `(${px}-text_w/2)`;
  }
}

/**
 * Build the filter chain for one overlay.
 *
 * When `drawText` is true (the ffmpeg build has the `drawtext` filter, i.e.
 * libfreetype), we render real text (with an optional `box`). When it is false
 * (a stripped ffmpeg build without drawtext), we DEGRADE GRACEFULLY: boxed
 * overlays (pills, the verdict badge) still render as a `drawbox` so the framing
 * and verdict color survive; plain unboxed text is dropped. This keeps the
 * fully-OSS pipeline producing a valid, framed mp4 on any ffmpeg build.
 */
function overlayFilters(o: TextOverlay, drawText: boolean): string[] {
  const x = drawTextX(o);
  const y = `(h*${o.y}-text_h/2)`;
  if (drawText) {
    const base =
      `drawtext=text='${escapeDrawText(o.text)}'` +
      `:x=${x}:y=${y}` +
      `:fontsize=${o.fontSize}` +
      `:fontcolor=${ffColor(o.color)}`;
    if (o.box !== undefined) {
      return [
        `${base}:box=1:boxcolor=${ffColor(o.box.color)}:boxborderw=${o.box.padding}`,
      ];
    }
    return [base];
  }
  // Fallback: approximate a pill/badge with a drawbox sized to the text length.
  if (o.box !== undefined) {
    const approxW = `${Math.round(o.text.length * o.fontSize * 0.55 + o.box.padding * 2)}`;
    const boxH = `${Math.round(o.fontSize + o.box.padding * 2)}`;
    const bx =
      o.align === "right"
        ? `(w*${o.x}-${approxW})`
        : o.align === "center"
          ? `(w*${o.x}-${approxW}/2)`
          : `(w*${o.x})`;
    const by = `(h*${o.y}-${boxH}/2)`;
    return [
      `drawbox=x=${bx}:y=${by}:w=${approxW}:h=${boxH}:color=${ffColor(o.box.color)}:t=fill`,
    ];
  }
  return [];
}

function badgeColor(verdict: "pass" | "fail"): RGBA {
  return verdict === "pass"
    ? { r: 34, g: 197, b: 94, a: 1 }
    : { r: 239, g: 68, b: 68, a: 1 };
}

/** Render a verdict badge as a colored, boxed drawtext (or drawbox fallback). */
function badgeFilters(
  badge: { text: string; verdict: "pass" | "fail"; x: number; y: number },
  drawText: boolean,
): string[] {
  const overlay: TextOverlay = {
    text: badge.text,
    x: badge.x,
    y: badge.y,
    fontSize: 40,
    color: { r: 13, g: 17, b: 23, a: 1 },
    box: { color: badgeColor(badge.verdict), padding: 18 },
    align: "center",
  };
  return overlayFilters(overlay, drawText);
}

function titleCardFilter(seg: TitleCardSegment, drawText: boolean): string[] {
  const chain: string[] = [];
  for (const o of seg.overlays) chain.push(...overlayFilters(o, drawText));
  if (seg.badge !== undefined) chain.push(...badgeFilters(seg.badge, drawText));
  return chain;
}

/**
 * Build the full ffmpeg argv for a plan. Returns `{ args }`. Pure given its
 * inputs (it only resolves paths, no spawning).
 *
 * Strategy: each segment becomes a labelled stream in the filtergraph; we then
 * `concat` them. Title cards come from a `color` source; clips come from inputs.
 * Side-by-side clips are scaled+padded to half-width and `hstack`ed.
 */
export interface BuildOptions {
  /**
   * Whether the target ffmpeg build has the `drawtext` filter (libfreetype).
   * Defaults to true. When false the executor degrades to drawbox-only overlays
   * so the render still succeeds on stripped ffmpeg builds.
   */
  drawText?: boolean;
}

export function buildFfmpegArgs(
  plan: ComposePlan,
  dirs: RunDirs,
  outPath: string,
  options: BuildOptions = {},
): { args: string[]; inputCount: number } {
  const drawText = options.drawText ?? true;
  const { width, height } = plan;
  const inputs: string[] = [];
  const filterParts: string[] = [];
  const segmentLabels: string[] = [];

  // Letterbox / pad fill color. Use the title card background when present so
  // the framing is visually consistent across segments.
  const firstSeg = plan.segments[0];
  const padColor: RGBA =
    firstSeg !== undefined && firstSeg.kind === "title-card"
      ? firstSeg.background
      : { r: 13, g: 17, b: 23, a: 1 };
  const pad = ffColor(padColor);

  const resolveSource = (dir: "run" | "before" | "after", path: string): string => {
    const base = dirs[dir];
    if (base === undefined) {
      throw new Error(`buildFfmpegArgs: missing run dir for "${dir}"`);
    }
    return resolve(base, path);
  };

  const titleSeconds = (seg: TitleCardSegment) => seg.durationSeconds;

  let clipInputIndex = 0;
  // First, register all clip inputs in segment order so input indexes are stable.
  const clipInputForSegment: number[][] = [];
  for (const seg of plan.segments) {
    if (seg.kind === "clip") {
      const indexes: number[] = [];
      for (const src of seg.sources) {
        inputs.push("-i", resolveSource(src.dir, src.path));
        indexes.push(clipInputIndex);
        clipInputIndex += 1;
      }
      clipInputForSegment.push(indexes);
    } else {
      clipInputForSegment.push([]);
    }
  }

  let clipSegIdx = 0;
  plan.segments.forEach((seg, segIdx) => {
    const label = `seg${segIdx}`;
    if (seg.kind === "title-card") {
      const tc = seg;
      const dur = titleSeconds(tc);
      const drawChain = titleCardFilter(tc, drawText);
      const drawSuffix = drawChain.length > 0 ? `,${drawChain.join(",")}` : "";
      filterParts.push(
        `color=c=${ffColor(tc.background)}:s=${width}x${height}:d=${dur}:r=30` +
          `,format=yuv420p${drawSuffix}[${label}]`,
      );
      segmentLabels.push(label);
    } else {
      const clip = seg as ClipSegment;
      const ins = clipInputForSegment[segIdx];
      const drawChain: string[] = [];
      for (const o of clip.overlays) drawChain.push(...overlayFilters(o, drawText));
      if (clip.badge !== undefined) drawChain.push(...badgeFilters(clip.badge, drawText));
      const drawSuffix = drawChain.length > 0 ? `,${drawChain.join(",")}` : "";

      if (clip.layout === "side-by-side" && ins.length >= 2) {
        const halfW = Math.floor(width / 2);
        const left = `sbsL${clipSegIdx}`;
        const right = `sbsR${clipSegIdx}`;
        filterParts.push(
          `[${ins[0]}:v]scale=${halfW}:${height}:force_original_aspect_ratio=decrease,` +
            `pad=${halfW}:${height}:(ow-iw)/2:(oh-ih)/2:color=${pad},setsar=1[${left}]`,
        );
        filterParts.push(
          `[${ins[1]}:v]scale=${halfW}:${height}:force_original_aspect_ratio=decrease,` +
            `pad=${halfW}:${height}:(ow-iw)/2:(oh-ih)/2:color=${pad},setsar=1[${right}]`,
        );
        filterParts.push(
          `[${left}][${right}]hstack=inputs=2,format=yuv420p${drawSuffix}[${label}]`,
        );
      } else {
        filterParts.push(
          `[${ins[0]}:v]scale=${width}:${height}:force_original_aspect_ratio=decrease,` +
            `pad=${width}:${height}:(ow-iw)/2:(oh-ih)/2:color=${pad},setsar=1,format=yuv420p${drawSuffix}[${label}]`,
        );
      }
      segmentLabels.push(label);
      clipSegIdx += 1;
    }
  });

  // Concat all segment streams (video-only) into the final stream.
  const concatInputs = segmentLabels.map((l) => `[${l}]`).join("");
  filterParts.push(
    `${concatInputs}concat=n=${segmentLabels.length}:v=1:a=0[outv]`,
  );

  const args: string[] = ["-y"];
  for (const part of inputs) args.push(part);
  args.push(
    "-filter_complex",
    filterParts.join(";"),
    "-map",
    "[outv]",
    "-c:v",
    "libx264",
    "-pix_fmt",
    "yuv420p",
    "-movflags",
    "+faststart",
    outPath,
  );

  return { args, inputCount: clipInputIndex };
}

export interface RenderResult {
  outPath: string;
  args: string[];
  /** Whether real text was rendered (drawtext) or the drawbox fallback was used. */
  drawText: boolean;
}

/**
 * Probe whether the given ffmpeg binary has the `drawtext` filter (requires a
 * libfreetype-enabled build). Returns false if ffmpeg cannot be run at all.
 */
export function drawtextAvailable(ffmpegBin = "ffmpeg"): boolean {
  try {
    const out = spawnSync(ffmpegBin, ["-hide_banner", "-filters"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return typeof out.stdout === "string" && /\bdrawtext\b/.test(out.stdout);
  } catch {
    return false;
  }
}

/** Spawn ffmpeg to render the plan. Rejects with stderr tail on non-zero exit. */
export function renderComposePlan(
  plan: ComposePlan,
  dirs: RunDirs,
  outPath: string,
  ffmpegBin = "ffmpeg",
): Promise<RenderResult> {
  const drawText = drawtextAvailable(ffmpegBin);
  const { args } = buildFfmpegArgs(plan, dirs, outPath, { drawText });
  return new Promise((resolvePromise, reject) => {
    const child = spawn(ffmpegBin, args, { stdio: ["ignore", "ignore", "pipe"] });
    let stderr = "";
    child.stderr.on("data", (d: Buffer) => {
      stderr += d.toString();
      if (stderr.length > 8000) stderr = stderr.slice(-8000);
    });
    child.on("error", (err) => reject(err));
    child.on("close", (code) => {
      if (code === 0) resolvePromise({ outPath, args, drawText });
      else reject(new Error(`ffmpeg exited ${code}\n${stderr.slice(-2000)}`));
    });
  });
}
