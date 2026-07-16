/**
 * QA-3 (#8908): the Desktop visual-baseline diff gate (`pnpm run qa:visual`).
 *
 * Launches the REAL Electron app as a second OS process in the windowless
 * visual-baseline probe mode (OPENAGENTS_DESKTOP_VISUAL_BASELINE_PROBE=1,
 * mirroring full-auto-restart-smoke.ts), which renders the fixed set of
 * frozen fixture shell states offscreen and writes one PNG per state. This
 * runner then compares every capture against the committed baselines under
 * `visual-baselines/` pixel-by-pixel with a small bounded threshold
 * (src/visual-baseline-diff.ts — pure TS over decoded PNG bytes, no image
 * dependency) and exits nonzero on drift, writing side-by-side review
 * artifacts (baseline | current | drift mask) to a scratch directory it
 * names in its output.
 *
 * Baseline refresh is an explicit, reviewed action only:
 *   pnpm run qa:visual -- --update-baselines
 *
 * Requires a prior build (`node --import tsx scripts/build.ts`), exactly like
 * the other Electron-launching smokes; the `qa:visual` package script builds
 * first.
 */
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  DEFAULT_DIFF_THRESHOLDS,
  decodePng,
  decodeVisualBaselineManifest,
  diffImages,
  encodePng,
  sideBySideImage,
  type VisualBaselineManifest,
} from "../src/visual-baseline-diff.ts";
import {
  VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
  QA_DESKTOP_VISUAL_LANE,
  QA_DESKTOP_VISUAL_RECEIPT_SCHEMA,
  VISUAL_BASELINE_STATES,
  VISUAL_BASELINE_WINDOW,
} from "../src/visual-baseline-contract.ts";
import { createHash } from "node:crypto";

const appRoot = path.resolve(import.meta.dirname, "..");
const baselinesDir = path.join(appRoot, "visual-baselines");
const manifestPath = path.join(baselinesDir, "manifest.json");
const electronBinary = path.join(appRoot, "node_modules", ".bin", "electron");
// `qa:visual` builds `dist/` immediately before this script runs. Always boot
// that just-built app through Electron: an older Forge artifact may still be
// present under `out/`, and preferring it would let the gate compare stale
// renderer pixels while silently ignoring the current source/build.
const command = [electronBinary, "."];
const PROBE_TIMEOUT_MS = 180_000;
const updateBaselines = process.argv.includes("--update-baselines");

const runProbe = (userData: string, shotsDir: string): Promise<string> =>
  new Promise<string>((resolve, reject) => {
    const [executable, ...args] = command;
    const child = spawn(executable!, args, {
      cwd: appRoot,
      env: {
        ...process.env,
        OPENAGENTS_DESKTOP_SMOKE: "0",
        OPENAGENTS_DESKTOP_USER_DATA: userData,
        OPENAGENTS_DESKTOP_VISUAL_BASELINE_PROBE: "1",
        OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS: shotsDir,
        // Frozen-clock fixtures are wall-clock independent, but pin the zone
        // anyway so any incidental local-time formatting cannot drift.
        TZ: "UTC",
      },
      stdio: ["ignore", "pipe", "inherit"],
    });
    let stdout = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk;
      process.stdout.write(chunk);
    });
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error(`visual-baseline probe timed out after ${PROBE_TIMEOUT_MS / 1000}s`));
    }, PROBE_TIMEOUT_MS);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      code === 0 ? resolve(stdout) : reject(new Error(`visual-baseline probe exited ${code}`));
    });
  });

type ProbeReceipt = Readonly<{
  ok: boolean;
  states: ReadonlyArray<{
    state: string;
    file: string;
    sha256: string;
    width: number;
    height: number;
  }>;
}>;

/** Parse the probe's one public-safe receipt line and re-assert its fields. */
const parseReceipt = (stdout: string): ProbeReceipt => {
  const line = stdout
    .split("\n")
    .find((value) => value.includes("[openagents-desktop visual-baseline] captured"));
  if (line === undefined) throw new Error("no visual-baseline receipt line in probe output");
  return JSON.parse(line.slice(line.indexOf("{"))) as ProbeReceipt;
};

const writeManifest = (
  states: ReadonlyArray<{
    state: string;
    file: string;
    sha256: string;
    width: number;
    height: number;
  }>,
): VisualBaselineManifest => ({
  schema: "openagents-desktop.visual-baselines.v1",
  platform: `${process.platform}-${process.arch}`,
  timezone: "UTC",
  window: {
    width: VISUAL_BASELINE_WINDOW.width,
    height: VISUAL_BASELINE_WINDOW.height,
    deviceScaleFactor: VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
  },
  thresholds: { ...DEFAULT_DIFF_THRESHOLDS },
  states: states.map((entry) => ({
    name: entry.state,
    file: entry.file,
    sha256: entry.sha256,
    width: entry.width,
    height: entry.height,
  })),
});

const main = async (): Promise<void> => {
  const userData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-visual-baseline-userdata-"));
  const shotsDir = mkdtempSync(path.join(tmpdir(), "openagents-desktop-visual-baseline-shots-"));
  try {
    const receipt = parseReceipt(await runProbe(userData, shotsDir));
    if (receipt.ok !== true || receipt.states.length !== VISUAL_BASELINE_STATES.length) {
      throw new Error(`probe receipt failed assertions: ${JSON.stringify(receipt)}`);
    }
    if (updateBaselines) {
      mkdirSync(baselinesDir, { recursive: true });
      for (const entry of receipt.states) {
        writeFileSync(
          path.join(baselinesDir, entry.file),
          readFileSync(path.join(shotsDir, entry.file)),
        );
      }
      writeFileSync(manifestPath, `${JSON.stringify(writeManifest(receipt.states), null, 2)}\n`);
      console.log(
        `[openagents-desktop visual-baseline] baselines UPDATED ${JSON.stringify({
          states: receipt.states.map((entry) => entry.state),
          manifest: path.relative(appRoot, manifestPath),
        })}`,
      );
      return;
    }
    const manifest = decodeVisualBaselineManifest(JSON.parse(readFileSync(manifestPath, "utf8")));
    if (manifest === null)
      throw new Error(`invalid or missing baseline manifest at ${manifestPath}`);
    const currentPlatform = `${process.platform}-${process.arch}`;
    if (manifest.platform !== currentPlatform) {
      throw new Error(
        `baseline platform ${manifest.platform} does not match ${currentPlatform}; re-baseline with --update-baselines on the canonical platform`,
      );
    }
    const artifactsDir = mkdtempSync(
      path.join(tmpdir(), "openagents-desktop-visual-baseline-artifacts-"),
    );
    const results: Array<Record<string, unknown>> = [];
    let drifted = 0;
    for (const entry of receipt.states) {
      const manifestEntry = manifest.states.find((candidate) => candidate.name === entry.state);
      if (manifestEntry === undefined) {
        drifted += 1;
        results.push({ state: entry.state, ok: false, reason: "missing_baseline" });
        continue;
      }
      const baselineBytes = new Uint8Array(
        readFileSync(path.join(baselinesDir, manifestEntry.file)),
      );
      const baselineSha = createHash("sha256").update(baselineBytes).digest("hex");
      if (baselineSha !== manifestEntry.sha256) {
        drifted += 1;
        results.push({ state: entry.state, ok: false, reason: "baseline_manifest_sha_mismatch" });
        continue;
      }
      const currentBytes = new Uint8Array(readFileSync(path.join(shotsDir, entry.file)));
      const baseline = decodePng(baselineBytes);
      const current = decodePng(currentBytes);
      const diff = diffImages(baseline, current, manifest.thresholds);
      if (diff.ok) {
        results.push({
          state: entry.state,
          ok: true,
          differentPixels: diff.differentPixels,
          differentRatio: diff.differentRatio,
        });
        continue;
      }
      drifted += 1;
      const artifact = path.join(artifactsDir, `${entry.state}.side-by-side.png`);
      writeFileSync(artifact, encodePng(sideBySideImage(baseline, current, diff.diffMask)));
      writeFileSync(path.join(artifactsDir, `${entry.state}.current.png`), currentBytes);
      results.push({
        state: entry.state,
        ok: false,
        reason: diff.reason,
        differentPixels: diff.differentPixels,
        differentRatio: diff.differentRatio,
        artifact,
      });
    }
    const summary = {
      schema: QA_DESKTOP_VISUAL_RECEIPT_SCHEMA,
      lane: QA_DESKTOP_VISUAL_LANE,
      ok: drifted === 0,
      states: results,
      thresholds: manifest.thresholds,
      ...(drifted === 0 ? {} : { artifactsDir }),
    };
    if (drifted > 0) {
      console.error(
        `[openagents-desktop visual-baseline] gate FAILED — pixel drift ${JSON.stringify(summary)}`,
      );
      console.error(
        `[openagents-desktop visual-baseline] side-by-side artifacts written to ${artifactsDir}`,
      );
      process.exitCode = 1;
      return;
    }
    rmSync(artifactsDir, { recursive: true, force: true });
    console.log(`[openagents-desktop visual-baseline] gate OK ${JSON.stringify(summary)}`);
  } finally {
    rmSync(userData, { recursive: true, force: true });
    rmSync(shotsDir, { recursive: true, force: true });
  }
};

await main().catch((error) => {
  console.error(
    "[openagents-desktop visual-baseline] gate FAILED",
    error instanceof Error ? error.message : error,
  );
  process.exit(1);
});
