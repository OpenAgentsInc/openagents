#!/usr/bin/env node
// Sarah episode RC assemble helper (#9234).
//
// One command: sarah-master + screenshare + optional cutaway + T1/T2 →
// *-rc-no-music.mp4. Continuous Sarah audio. Picture cuts only (A/B/C).
// Derives T1 from a post-spoken silence window (default after "zed").
// Verifies frames at T2−1s / T2−0.5s / T2 and fails on Finder/Desktop-like
// pixels. Optional Desktop ↔ docs/transcripts spoken-body lock.
//
// Usage:
//   node scripts/sarah-avatar/assemble-rc.mjs \
//     --sarah-master ~/Desktop/Sarah/262/262-sarah-master.mp4 \
//     --screenshare ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4 \
//     --cutaway /path/to/omega2.jpg \
//     --cutaway-seconds 6.3192 \
//     --desktop-transcript ~/Desktop/Sarah/262/262transcript.md \
//     --repo-transcript docs/transcripts/262.md \
//     --out ~/Desktop/Sarah/262/262-rc-no-music.mp4

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildAssembleFilterComplex,
  checkTranscriptLock,
  DEFAULT_T1_SEARCH,
  DEFAULT_T2_FRACTION,
  deriveT1FromSilenceIntervals,
  evaluateCutPointQc,
  parseCutawayManifest,
  parseNumberFlag,
  parseQcOffsetsFlag,
  parseSilenceDetectLog,
  planCutPointQcTimes,
  planMidBeats,
  resolveCutTimes,
  selectCutaways,
} from "./assemble-rc-lib.mjs";
import { looksLikeDesktopOrFinderTail } from "../omega-screen-control/recording-lifecycle.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");

const args = process.argv.slice(2);
const flag = (name, fallback = undefined) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback;
};
const flags = (name) =>
  args.flatMap((arg, index) =>
    arg === `--${name}` && index + 1 < args.length ? [args[index + 1]] : [],
  );
const hasFlag = (name) => args.includes(`--${name}`);

function printHelp() {
  console.log(`Sarah episode RC assemble (#9234)

Usage:
  node scripts/sarah-avatar/assemble-rc.mjs \\
    --sarah-master <mp4> --screenshare <mp4> --out <mp4> \\
    [--cutaway <image|mp4>] [--cutaway-seconds <n>] \\
    [--cutaway-manifest <json> --cutaway-id <id> [--cutaway-id <id>...]] \\
    [--list-cutaways] [--skip-cutaways] [--episode <number>] \\
    [--t1 <sec>] [--t1-auto] [--t1-search-from 14] [--t1-search-to 20] \\
    [--t1-silence-db -35] [--t1-pad-after-silence 0.2] \\
    [--t2 <sec>] [--t2-fraction 0.78] \\
    [--desktop-transcript <path>] [--repo-transcript <path>] \\
    [--require-transcript-lock] [--skip-transcript-lock] \\
    [--work-dir <dir>] [--skip-qc] [--qc-offsets -1,-0.5,0]

Behavior:
  - A/B/C picture cut; Sarah audio continuous 0→D
  - T1 defaults to silence-derived pause in the search window (after "zed")
  - T2 defaults to 0.78*D (override when audio disagrees)
  - Optional mid beats: one direct cutaway, or multiple manifest selections
  - Discovers cutaways/manifest.json beside the Sarah master, then
    .artifacts/episode-N/cutaways/manifest.json when --episode is present
  - QC fails if Finder/Desktop-like pixels appear near T2
  - Transcript lock compares Desktop paste to docs/transcripts spoken body
`);
}

function resolveCutawayManifestPath({ explicitPath, sarahMaster, episode }) {
  if (explicitPath) return resolve(explicitPath);
  const candidates = [];
  if (sarahMaster) {
    candidates.push(join(dirname(resolve(sarahMaster)), "cutaways", "manifest.json"));
  }
  if (episode) {
    candidates.push(
      join(repoRoot, ".artifacts", `episode-${episode}`, "cutaways", "manifest.json"),
    );
  }
  return candidates.find((candidate) => existsSync(candidate)) || null;
}

function readCutawayInventory(manifestPath) {
  if (!manifestPath || !existsSync(manifestPath)) {
    throw new Error(`cutaway manifest not found: ${manifestPath || "(none discovered)"}`);
  }
  return {
    manifest: parseCutawayManifest(JSON.parse(readFileSync(manifestPath, "utf8"))),
    manifestPath,
  };
}

if (hasFlag("help") || hasFlag("h")) {
  printHelp();
  process.exit(0);
}

function resolveFfmpegBin() {
  if (process.env.FFMPEG_BIN && existsSync(process.env.FFMPEG_BIN)) {
    return process.env.FFMPEG_BIN;
  }
  for (const candidate of ["ffmpeg", "/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]) {
    try {
      execFileSync(candidate, ["-version"], { stdio: "ignore" });
      return candidate;
    } catch {
      // try next
    }
  }
  throw new Error("ffmpeg not found on PATH (set FFMPEG_BIN)");
}

function resolveFfprobeBin(ffmpegBin) {
  if (process.env.FFPROBE_BIN && existsSync(process.env.FFPROBE_BIN)) {
    return process.env.FFPROBE_BIN;
  }
  const sibling = String(ffmpegBin).replace(/ffmpeg$/, "ffprobe");
  for (const candidate of [
    sibling,
    "ffprobe",
    "/opt/homebrew/bin/ffprobe",
    "/usr/local/bin/ffprobe",
  ]) {
    try {
      execFileSync(candidate, ["-version"], { stdio: "ignore" });
      return candidate;
    } catch {
      // try next
    }
  }
  return null;
}

function probeMediaInfo(ffmpegBin, mediaPath) {
  const ffprobeBin = resolveFfprobeBin(ffmpegBin);
  if (ffprobeBin) {
    try {
      const raw = execFileSync(
        ffprobeBin,
        [
          "-v",
          "error",
          "-select_streams",
          "v:0",
          "-show_entries",
          "format=duration:stream=width,height",
          "-of",
          "json",
          mediaPath,
        ],
        { encoding: "utf8" },
      );
      const json = JSON.parse(raw);
      const duration = Number(json?.format?.duration);
      const stream = Array.isArray(json?.streams) ? json.streams[0] : null;
      return {
        durationSec: Number.isFinite(duration) ? duration : null,
        width: Number(stream?.width) || null,
        height: Number(stream?.height) || null,
      };
    } catch {
      // fall through to ffmpeg -i
    }
  }
  // `ffmpeg -i` exits non-zero with metadata on stderr (no output muxer).
  const result = spawnSync(ffmpegBin, ["-hide_banner", "-i", mediaPath], {
    encoding: "utf8",
  });
  const text = `${result.stderr || ""}${result.stdout || ""}`;
  const durMatch = text.match(/Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)/);
  let durationSec = null;
  if (durMatch) {
    const h = Number(durMatch[1]);
    const m = Number(durMatch[2]);
    const s = Number(durMatch[3]);
    if ([h, m, s].every((n) => Number.isFinite(n))) {
      durationSec = h * 3600 + m * 60 + s;
    }
  }
  const sizeMatch = text.match(/Video:.*?\s(\d{2,5})x(\d{2,5})/);
  return {
    durationSec,
    width: sizeMatch ? Number(sizeMatch[1]) : null,
    height: sizeMatch ? Number(sizeMatch[2]) : null,
  };
}

function detectSilenceIntervals(ffmpegBin, mediaPath, { noiseDb, fromSec, toSec }) {
  // Limit analysis to the search window for speed.
  const result = spawnSync(
    ffmpegBin,
    [
      "-hide_banner",
      "-ss",
      String(Math.max(0, fromSec - 0.5)),
      "-to",
      String(toSec + 0.5),
      "-i",
      mediaPath,
      "-af",
      `silencedetect=noise=${noiseDb}dB:d=0.2`,
      "-f",
      "null",
      "-",
    ],
    { encoding: "utf8" },
  );
  const text = `${result.stderr || ""}${result.stdout || ""}`;
  const intervals = parseSilenceDetectLog(text).map((iv) => ({
    // Re-base absolute times (we used -ss).
    start: iv.start + Math.max(0, fromSec - 0.5),
    end: iv.end + Math.max(0, fromSec - 0.5),
    duration: iv.duration,
  }));
  return intervals;
}

function sampleFrameStats(ffmpegBin, mediaPath, timeSec) {
  const tmpDir = mkdtempSync(join(tmpdir(), "sarah-rc-frame-"));
  const png = join(tmpDir, "frame.png");
  try {
    execFileSync(
      ffmpegBin,
      ["-y", "-ss", String(Math.max(0, timeSec)), "-i", mediaPath, "-frames:v", "1", png],
      { stdio: "ignore" },
    );
    const raw = execFileSync(
      "python3",
      [
        "-c",
        `
from PIL import Image
import statistics
im = Image.open(${JSON.stringify(png)}).convert("RGB").resize((160, 90))
px = list(im.getdata())
lumas = [0.2126*r + 0.7152*g + 0.0722*b for r,g,b in px]
mean = statistics.fmean(lumas)
var = statistics.pvariance(lumas) if len(lumas) > 1 else 0.0
dark = sum(1 for y in lumas if y < 40) / len(lumas)
print(f"{mean:.4f},{var:.4f},{dark:.4f}")
`,
      ],
      { encoding: "utf8" },
    ).trim();
    const [meanLuma, variance, darkUiRatio] = raw.split(",").map(Number);
    if (![meanLuma, variance, darkUiRatio].every((n) => Number.isFinite(n))) {
      return null;
    }
    return { t: timeSec, meanLuma, variance, darkUiRatio };
  } catch {
    return null;
  } finally {
    try {
      rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // ignore
    }
  }
}

function scaleFilterForTarget(width, height) {
  // Match Sarah master frame when possible; otherwise FIT into 1920x1088 dark pad.
  const w = Number.isFinite(width) && width > 0 ? width : 1920;
  const h = Number.isFinite(height) && height > 0 ? height : 1088;
  return `scale=${w}:${h}:force_original_aspect_ratio=decrease,pad=${w}:${h}:(ow-iw)/2:(oh-ih)/2:0x0E0E10,setsar=1,fps=24,format=yuv420p`;
}

function main() {
  const sarahMaster = flag("sarah-master");
  const screenshare = flag("screenshare");
  const outPath = flag("out");
  const episode = flag("episode");
  const selectedCutawayIds = flags("cutaway-id").flatMap((value) =>
    value
      .split(",")
      .map((id) => id.trim())
      .filter(Boolean),
  );
  const cutawayManifestPath = resolveCutawayManifestPath({
    explicitPath: flag("cutaway-manifest"),
    sarahMaster,
    episode,
  });
  if (hasFlag("list-cutaways")) {
    const { manifest, manifestPath } = readCutawayInventory(cutawayManifestPath);
    console.log(`cutaway inventory: ${manifestPath}`);
    for (const cutaway of manifest.cutaways) {
      console.log(
        `${cutaway.id}\t${cutaway.durationSec}s\t${cutaway.file}${cutaway.note ? `\t${cutaway.note}` : ""}`,
      );
    }
    return;
  }
  if (!sarahMaster || !screenshare || !outPath) {
    printHelp();
    console.error("error: --sarah-master, --screenshare, and --out are required");
    process.exit(2);
  }
  for (const [label, path] of [
    ["sarah-master", sarahMaster],
    ["screenshare", screenshare],
  ]) {
    if (!existsSync(path)) {
      console.error(`error: --${label} not found: ${path}`);
      process.exit(2);
    }
  }

  const directCutaway = flag("cutaway");
  if (directCutaway && selectedCutawayIds.length > 0) {
    console.error("error: --cutaway cannot be combined with --cutaway-id");
    process.exit(2);
  }
  if (directCutaway && !existsSync(directCutaway)) {
    console.error(`error: --cutaway not found: ${directCutaway}`);
    process.exit(2);
  }
  const directCutawaySeconds = parseNumberFlag(flag("cutaway-seconds"), "--cutaway-seconds") ?? 0;
  if (directCutawaySeconds > 0 && !directCutaway) {
    console.error("error: --cutaway-seconds requires --cutaway");
    process.exit(2);
  }
  if (directCutaway && directCutawaySeconds <= 0) {
    console.error("error: --cutaway requires --cutaway-seconds > 0");
    process.exit(2);
  }
  let selectedCutaways = [];
  if (selectedCutawayIds.length > 0) {
    const { manifest, manifestPath } = readCutawayInventory(cutawayManifestPath);
    selectedCutaways = selectCutaways(manifest, selectedCutawayIds).map((cutaway) => ({
      ...cutaway,
      path: resolve(dirname(manifestPath), cutaway.file),
    }));
    for (const cutaway of selectedCutaways) {
      if (!existsSync(cutaway.path)) {
        console.error(`error: cutaway ${cutaway.id} not found: ${cutaway.path}`);
        process.exit(2);
      }
    }
    console.log(`cutaway inventory: ${manifestPath}`);
  } else if (directCutaway) {
    selectedCutaways = [
      {
        id: "direct",
        path: resolve(directCutaway),
        durationSec: directCutawaySeconds,
      },
    ];
  } else if (cutawayManifestPath && !hasFlag("skip-cutaways")) {
    console.error(`error: cutaway inventory found: ${cutawayManifestPath}`);
    console.error(
      "Select one or more --cutaway-id values, inspect with --list-cutaways, or confirm --skip-cutaways.",
    );
    process.exit(2);
  }

  const ffmpegBin = resolveFfmpegBin();
  const masterInfo = probeMediaInfo(ffmpegBin, sarahMaster);
  const D = masterInfo.durationSec;
  if (!Number.isFinite(D)) {
    console.error("error: could not probe Sarah master duration");
    process.exit(1);
  }
  const screenshareInfo = probeMediaInfo(ffmpegBin, screenshare);
  const screenshareDuration = screenshareInfo.durationSec;
  const masterSize = {
    width: masterInfo.width,
    height: masterInfo.height,
  };

  // --- Transcript lock ---------------------------------------------------
  const desktopTranscript = flag("desktop-transcript");
  const repoTranscript =
    flag("repo-transcript") ||
    (flag("episode") ? join(repoRoot, "docs/transcripts", `${flag("episode")}.md`) : undefined);
  const requireLock = hasFlag("require-transcript-lock");
  const skipLock = hasFlag("skip-transcript-lock");
  if (!skipLock && (desktopTranscript || repoTranscript || requireLock)) {
    if (!desktopTranscript || !repoTranscript) {
      const msg =
        "transcript lock needs both --desktop-transcript and --repo-transcript " +
        "(or --episode with --desktop-transcript)";
      if (requireLock) {
        console.error(`error: ${msg}`);
        process.exit(2);
      }
      console.warn(`warn: ${msg}; skipping lock`);
    } else if (!existsSync(desktopTranscript) || !existsSync(repoTranscript)) {
      const msg = `transcript path missing (desktop=${desktopTranscript} repo=${repoTranscript})`;
      if (requireLock) {
        console.error(`error: ${msg}`);
        process.exit(2);
      }
      console.warn(`warn: ${msg}; skipping lock`);
    } else {
      const lock = checkTranscriptLock({
        desktopText: readFileSync(desktopTranscript, "utf8"),
        repoText: readFileSync(repoTranscript, "utf8"),
      });
      if (lock.ok) {
        console.log(`transcript-lock: ok (${lock.reason})`);
      } else {
        const detail = `transcript-lock: FAIL — ${lock.reason}`;
        if (requireLock) {
          console.error(`error: ${detail}`);
          console.error(
            "Update Desktop paste and docs/transcripts/<ep>.md to the same spoken words before assemble.",
          );
          process.exit(1);
        }
        console.warn(`warn: ${detail}`);
        console.warn("Acceptance: keep Desktop spoken paste and docs/transcripts/<ep>.md in sync.");
      }
    }
  } else if (!skipLock) {
    console.log(
      "transcript-lock: reminder — keep Desktop *transcript.md paste and docs/transcripts/<ep>.md spoken body in sync before assemble.",
    );
  }

  // --- T1 / T2 -----------------------------------------------------------
  const searchFrom =
    parseNumberFlag(flag("t1-search-from"), "--t1-search-from") ?? DEFAULT_T1_SEARCH.fromSec;
  const searchTo =
    parseNumberFlag(flag("t1-search-to"), "--t1-search-to") ?? DEFAULT_T1_SEARCH.toSec;
  const noiseDb =
    parseNumberFlag(flag("t1-silence-db"), "--t1-silence-db") ?? DEFAULT_T1_SEARCH.noiseDb;
  const padAfter =
    parseNumberFlag(flag("t1-pad-after-silence"), "--t1-pad-after-silence") ??
    DEFAULT_T1_SEARCH.padAfterSilenceSec;
  const explicitT1 = parseNumberFlag(flag("t1"), "--t1");
  const wantAutoT1 = hasFlag("t1-auto") || explicitT1 == null;

  let T1 = explicitT1;
  let t1Source = explicitT1 != null ? "flag" : null;
  if (wantAutoT1 || explicitT1 == null) {
    console.log(`t1: detecting silence in [${searchFrom}, ${searchTo}]s (noise ${noiseDb} dB)…`);
    const intervals = detectSilenceIntervals(ffmpegBin, sarahMaster, {
      noiseDb,
      fromSec: searchFrom,
      toSec: searchTo,
    });
    const derived = deriveT1FromSilenceIntervals(intervals, {
      fromSec: searchFrom,
      toSec: searchTo,
      padAfterSilenceSec: padAfter,
    });
    if (derived.ok) {
      if (explicitT1 != null) {
        const delta = Math.abs(explicitT1 - derived.t1);
        console.log(
          `t1: silence-derived ${derived.t1}s (${derived.reason}); ` +
            `--t1 ${explicitT1} differs by ${delta.toFixed(3)}s — using --t1`,
        );
        T1 = explicitT1;
        t1Source = "flag (silence also found)";
      } else {
        T1 = derived.t1;
        t1Source = `silence (${derived.reason})`;
        console.log(`t1: ${T1}s from ${t1Source}`);
      }
    } else if (explicitT1 != null) {
      console.warn(`warn: t1 silence detect failed (${derived.reason}); using --t1 ${explicitT1}`);
      T1 = explicitT1;
      t1Source = "flag (no silence)";
    } else {
      console.error(
        `error: could not derive T1 from silence (${derived.reason}). ` +
          "Pass --t1 explicitly after verifying the post-spoken pause on the audio.",
      );
      process.exit(1);
    }
  }

  const t2Fraction = parseNumberFlag(flag("t2-fraction"), "--t2-fraction") ?? DEFAULT_T2_FRACTION;
  const explicitT2 = parseNumberFlag(flag("t2"), "--t2");
  const cuts = resolveCutTimes({
    durationSec: D,
    t1: T1,
    t2: explicitT2,
    t2Fraction,
  });
  console.log(
    `cuts: D=${cuts.D.toFixed(3)} T1=${cuts.T1} (${t1Source}) T2=${cuts.T2}` +
      (cuts.t2FromFraction ? ` (fraction ${cuts.t2Fraction})` : " (flag)") +
      ` durB=${cuts.durB} durC=${cuts.durC}`,
  );

  const midPlan = planMidBeats({
    durB: cuts.durB,
    cutaways: selectedCutaways,
    screenshareDurationSec: screenshareDuration,
  });
  console.log(
    `mid: screenshare ${midPlan.screenshareUseSec}s` +
      (midPlan.cutawayUseSec > 0
        ? ` + ${selectedCutaways.map((cutaway) => `${cutaway.id} ${cutaway.durationSec}s`).join(" + ")}`
        : " (no cutaway)"),
  );

  // --- Work dir / normalize product picture ------------------------------
  const workDir = flag("work-dir") || join(dirname(resolve(outPath)), "rc-work", "assemble-rc");
  mkdirSync(workDir, { recursive: true });
  const scaleVf = scaleFilterForTarget(masterSize?.width, masterSize?.height);
  const ssNorm = join(workDir, "screenshare-norm.mp4");
  console.log(`prepare: normalize screenshare → ${ssNorm}`);
  execFileSync(
    ffmpegBin,
    [
      "-y",
      "-i",
      screenshare,
      "-vf",
      scaleVf,
      "-an",
      "-c:v",
      "libx264",
      "-pix_fmt",
      "yuv420p",
      ssNorm,
    ],
    { stdio: ["ignore", "inherit", "inherit"] },
  );

  const cutawayNorms = [];
  for (const [index, cutaway] of selectedCutaways.entries()) {
    const cutawayNorm = join(workDir, `cutaway-${index + 1}-norm.mp4`);
    const isImage = /\.(png|jpe?g|webp|gif)$/i.test(cutaway.path);
    console.log(`prepare: normalize cutaway ${cutaway.id} → ${cutawayNorm}`);
    const cutawayArgs = isImage
      ? [
          "-y",
          "-loop",
          "1",
          "-t",
          String(cutaway.durationSec + 0.25),
          "-i",
          cutaway.path,
          "-vf",
          scaleVf,
          "-an",
          "-c:v",
          "libx264",
          "-pix_fmt",
          "yuv420p",
          cutawayNorm,
        ]
      : [
          "-y",
          "-i",
          cutaway.path,
          "-vf",
          scaleVf,
          "-an",
          "-c:v",
          "libx264",
          "-pix_fmt",
          "yuv420p",
          cutawayNorm,
        ];
    execFileSync(ffmpegBin, cutawayArgs, {
      stdio: ["ignore", "inherit", "inherit"],
    });
    cutawayNorms.push(cutawayNorm);
  }

  // --- Assemble ----------------------------------------------------------
  const filter = buildAssembleFilterComplex({
    T1: cuts.T1,
    T2: cuts.T2,
    D: cuts.D,
    midPlan,
    hasCutaway: cutawayNorms.length > 0,
    cutawayLabels: cutawayNorms.map((_, index) => String(index + 2)),
  });
  writeFileSync(join(workDir, "filter_complex.txt"), filter);
  writeFileSync(
    join(workDir, "cuts.json"),
    JSON.stringify(
      {
        D: cuts.D,
        T1: cuts.T1,
        T2: cuts.T2,
        t1Source,
        t2FromFraction: cuts.t2FromFraction,
        t2Fraction: cuts.t2Fraction,
        midPlan,
        cutaways: selectedCutaways.map(({ id, file, durationSec }) => ({
          id,
          file,
          durationSec,
        })),
        out: resolve(outPath),
      },
      null,
      2,
    ),
  );

  const ffArgs = ["-y", "-i", sarahMaster, "-i", ssNorm];
  for (const cutawayNorm of cutawayNorms) ffArgs.push("-i", cutawayNorm);
  ffArgs.push(
    "-filter_complex",
    filter,
    "-map",
    "[vout]",
    "-map",
    "[aout]",
    "-c:v",
    "libx264",
    "-pix_fmt",
    "yuv420p",
    "-c:a",
    "aac",
    "-movflags",
    "+faststart",
    resolve(outPath),
  );
  console.log(`assemble: writing ${resolve(outPath)}`);
  execFileSync(ffmpegBin, ffArgs, { stdio: ["ignore", "inherit", "inherit"] });

  // --- Cut-point QC ------------------------------------------------------
  if (hasFlag("skip-qc")) {
    console.log("qc: skipped (--skip-qc)");
  } else {
    const offsets = parseQcOffsetsFlag(flag("qc-offsets"));
    const qcTimes = planCutPointQcTimes(cuts.T2, offsets);
    // Reference: mid of Part B (product picture, not Sarah).
    const refAt = cuts.T1 + cuts.durB * 0.4;
    const referenceStats = sampleFrameStats(ffmpegBin, resolve(outPath), refAt);
    const samples = qcTimes.map((q) => ({
      ...q,
      stats: sampleFrameStats(ffmpegBin, resolve(outPath), q.atSec),
    }));
    const qc = evaluateCutPointQc({ referenceStats, samples });
    for (const sample of samples) {
      const tag = sample.stats
        ? `luma=${sample.stats.meanLuma.toFixed(1)} var=${sample.stats.variance.toFixed(1)} dark=${sample.stats.darkUiRatio.toFixed(2)}`
        : "unsampled";
      const bad =
        sample.stats &&
        referenceStats &&
        looksLikeDesktopOrFinderTail(sample.stats, referenceStats);
      console.log(
        `qc: ${sample.label} @ ${sample.atSec.toFixed(3)}s ${tag}${bad ? " FINDER-LIKE" : ""}`,
      );
    }
    if (!qc.ok) {
      console.error("error: cut-point frame QC failed:");
      for (const f of qc.failures) {
        console.error(`  - ${f.label || "?"} @ ${f.atSec}: ${f.reason}`);
      }
      console.error(
        "Reject this RC. Re-record screenshare with safe-tail / SIGTERM quit, " +
          "or raise --cutaway-seconds to cover the dirty mid tail.",
      );
      process.exit(1);
    }
    console.log("qc: cut points look clean (no Finder/Desktop-like frames near T2)");
  }

  console.log(`ok: ${resolve(outPath)}`);
}

try {
  main();
} catch (err) {
  console.error(`error: ${err?.message || err}`);
  process.exit(1);
}
