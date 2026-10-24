/** Compare retained Town renderer diagnostics without starting a browser or GPU job. */
import { createHash } from "node:crypto";
import { readFile, realpath, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { distribution } from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  applicationBrowserMode,
  evaluateApplicationPacing,
} from "./application-pacing.mjs";

const hash = (value) => createHash("sha256").update(value).digest("hex");
const finite = (value) => typeof value === "number" && Number.isFinite(value);
const key = (row) =>
  JSON.stringify([row.scenario, row.shot, row.route, row.count]);
const stages = [
  "stepMs",
  "wasmRequestMs",
  "renderCpuMs",
  "commandEncodeMs",
  "cullMs",
  "clusterMs",
  "gpuMs",
  "fullGpuMs",
  "submittedInstances",
  "submittedTriangles",
];
const commandFields = ["draws", "pipelines", "materials", "windBindings"];

function captureInputs(value) {
  if (!value) return null;
  const actors = value.actors;
  return {
    worldId: value.context?.worldId,
    sourceIdentity: value.context?.sourceIdentity,
    revision: value.context?.revision,
    width: value.width,
    height: value.height,
    time: value.time,
    exposure: value.exposure,
    camera: value.camera,
    actors: actors && {
      sourceIdentity: actors.sourceIdentity,
      scenario: actors.scenario,
      modelIdentity: actors.modelIdentity,
      actorIds: actors.actorIds,
      count: actors.count,
      poseSha256: actors.poseSha256,
      basis: actors.basis,
    },
  };
}

/** Keep source, capture, and raw-window errors visible even when pacing fails. */
export async function compareTownRenderer({ baseline, candidate }) {
  const result = {
    schema: "aethyrforge.town-renderer-comparison.v1",
    generatedAt: new Date().toISOString(),
    scope:
      "Evidence integrity and exact fixed source images; failed pacing verdicts remain separate",
    ok: false,
    qualified: false,
    checks: [],
    runs: [],
    pairs: [],
    unmatched: [],
    limitations: [
      "This offline comparison never establishes complete performance qualification or physical display presentation.",
      "Matching repeats pair by workload and occurrence within each retained run; chronological drift and external host activity remain possible confounders.",
      "Source controls compare exact RGBA bytes and fixed capture inputs. Live swarm positions and captures are not cross-run deterministic controls.",
      "Capture source/shader generation IDs may differ across sessions. Source identity, authored revision, camera, exposure, dimensions, time, and native actor model/pose must match.",
      "CPU spans overlap GPU execution and nested CPU spans; do not add their distributions.",
      "Cached bundle command inventory is not executed commands per frame. Missing actual sourceRenderCommands observations remain unavailable.",
    ],
  };
  const check = (id, ok, detail) => {
    result.checks.push({
      id,
      ok: !!ok,
      ...(detail === undefined ? {} : { detail }),
    });
    return !!ok;
  };
  async function load(label, directory) {
    const root = await realpath(resolve(directory));
    const evidence = [];
    async function bytes(name) {
      const path = await realpath(resolve(root, name));
      if (!path.startsWith(root + sep))
        throw Error(`Evidence leaves ${label}: ${name}`);
      const value = await readFile(path);
      evidence.push({ path: name, bytes: value.length, sha256: hash(value) });
      return value;
    }
    const report = JSON.parse(await bytes("town-swarm.json"));
    check(
      `${label}.schema`,
      report.schema === "aethyrforge.town-swarm-browser.v1",
    );
    check(
      `${label}.errors`,
      !report.failure &&
        !report.errors?.length &&
        !report.cleanupErrors?.length,
      {
        failure: report.failure ?? null,
        errors: report.errors,
        cleanupErrors: report.cleanupErrors,
      },
    );
    check(
      `${label}.source-stable`,
      !!report.sourceBefore?.sha256 &&
        report.sourceBefore.sha256 === report.sourceAfter?.sha256,
    );
    for (const [name, pin] of [
      ["before", report.sourceBefore],
      ["after", report.sourceAfter],
    ])
      check(
        `${label}.source-${name}-digest`,
        !!pin?.files && hash(JSON.stringify(pin.files)) === pin.sha256,
      );
    check(`${label}.assets-stable`, report.assetClosure?.unchanged === true);
    const windows = [];
    for (const row of report.windows ?? []) {
      const tag = `${label}.window-${row.index}`;
      let raw;
      try {
        raw = JSON.parse(await bytes(row.rawEvidence));
      } catch (error) {
        check(`${tag}.raw-readable`, false, String(error));
        windows.push({ row, raw: null, captures: {} });
        continue;
      }
      check(
        `${tag}.raw-identity`,
        raw.schema === "aethyrforge.pacing-window.v1" &&
          raw.world === "medieval-town" &&
          raw.index === row.index &&
          key(raw) === key(row),
      );
      const metrics = raw.metrics;
      const captures = {};
      for (const phase of ["before", "restored"]) {
        const metadata = row.quality?.[phase];
        try {
          if (!metadata) throw Error(`Missing ${phase} source capture.`);
          const pixels = await bytes(metadata.path);
          check(
            `${tag}.${phase}-digest`,
            hash(pixels) === metadata.pixelSha256,
          );
          check(
            `${tag}.${phase}-length`,
            Number.isSafeInteger(metadata.width) &&
              Number.isSafeInteger(metadata.height) &&
              metadata.width > 0 &&
              metadata.height > 0 &&
              pixels.length === metadata.width * metadata.height * 4,
          );
          check(`${tag}.${phase}-oracle`, metadata.oracle?.ok === true);
          check(
            `${tag}.${phase}-actors`,
            metadata.actors?.count === 8 &&
              metadata.actors.basis === "native-start" &&
              metadata.actors.actorIds?.length === 8,
          );
          captures[phase] = { metadata, pixels };
        } catch (error) {
          check(`${tag}.${phase}-readable`, false, String(error));
        }
      }
      const before = captures.before,
        restored = captures.restored;
      check(
        `${tag}.source-restored`,
        !!before &&
          !!restored &&
          isDeepStrictEqual(
            captureInputs(before.metadata),
            captureInputs(restored.metadata),
          ) &&
          before.pixels.equals(restored.pixels),
      );
      check(
        `${tag}.raw-capture`,
        !!before &&
          isDeepStrictEqual(
            captureInputs(before.metadata),
            captureInputs(raw.quality?.before),
          ) &&
          before.metadata.pixelSha256 === raw.quality?.before?.pixelSha256,
      );
      let pacing, applicationPacing;
      try {
        const options = raw.applicationOptions;
        pacing = evaluateFramePacing(metrics, options);
        applicationPacing = evaluateApplicationPacing(metrics, {
          ...options,
          productCounters: raw.productCounters,
          browserMode: applicationBrowserMode(raw.browserModeEvidence),
        });
        check(
          `${tag}.pacing-recomputed`,
          isDeepStrictEqual(pacing, row.pacing) &&
            isDeepStrictEqual(applicationPacing, row.applicationPacing),
        );
      } catch (error) {
        check(`${tag}.pacing-readable`, false, String(error));
      }
      const accepted = Array.isArray(metrics?.samples)
        ? metrics.samples.filter((sample) => sample?.submitted === true)
        : [];
      check(`${tag}.accepted-present`, accepted.length > 0);
      const stageValues = Object.fromEntries(
        stages.map((field) => {
          const values = accepted.map((sample) => sample[field] ?? null);
          const valid = values.every(
            (value) => value === null || (finite(value) && value >= 0),
          );
          check(`${tag}.stage-${field}`, valid);
          return [field, valid ? distribution(values) : null];
        }),
      );
      const observedCommands = {};
      for (const field of commandFields) {
        const values = accepted.map(
          (sample) => sample.sourceRenderCommands?.[field] ?? null,
        );
        const valid = values.every(
          (value) =>
            value === null || (Number.isSafeInteger(value) && value >= 0),
        );
        check(`${tag}.commands-${field}`, valid);
        observedCommands[field] = valid ? distribution(values) : null;
      }
      const lastCommands = metrics?.resources?.renderer?.renderCommands;
      if (lastCommands !== undefined && lastCommands !== null)
        check(
          `${tag}.last-source-command-scope`,
          lastCommands.scope ===
            "last-accepted-frame-source-draw-loop-commands" &&
            commandFields.every(
              (field) =>
                Number.isSafeInteger(lastCommands[field]) &&
                lastCommands[field] >= 0,
            ),
        );
      windows.push({
        row,
        raw,
        captures,
        summary: {
          index: row.index,
          key: key(row),
          observationMs: row.observationMs,
          recordedPacingPassed: row.pacing?.ok === true,
          recordedApplicationPacingPassed: row.applicationPacing?.ok === true,
          pacing,
          applicationPacing,
          stages: stageValues,
          sourceRenderCommands: {
            scope:
              "Actual accepted-frame source draw-loop commands, including admitted zero-instance bundle commands; excludes authored geometry, characters, effects, and post-processing; missing samples stay unavailable",
            values: observedCommands,
          },
          lastAcceptedSourceCommands:
            metrics?.resources?.renderer?.renderCommands ?? null,
          cachedCommandInventory: {
            scope:
              "Snapshot of source submission cache inventory; not commands per frame",
            value: metrics?.resources?.renderer?.sourceSubmission ?? null,
          },
          gpuAccountedBytes:
            metrics?.resources?.renderer?.gpuAccountedBytes ?? null,
        },
      });
    }
    check(`${label}.windows-present`, windows.length > 0);
    const run = {
      label,
      root,
      configuration: report.configuration,
      browser: report.browser,
      host: report.host,
      source: report.sourceBefore,
      assetClosure: report.assetClosure,
      recordedOk: report.ok,
      recordedQualified: report.qualified,
      evidence,
      windows: windows.map(
        (value) =>
          value.summary ?? { index: value.row.index, unavailable: true },
      ),
    };
    result.runs.push(run);
    return { report, windows, run };
  }
  const left = await load("baseline", baseline);
  const right = await load("candidate", candidate);
  for (const field of [
    "viewport",
    "durationMs",
    "warmupMs",
    "targetHz",
    "minHz",
    "instrumented",
  ])
    check(
      `configuration.${field}`,
      isDeepStrictEqual(
        left.report.configuration?.[field],
        right.report.configuration?.[field],
      ),
    );
  check(
    "browser",
    !!left.report.browser && left.report.browser === right.report.browser,
  );
  check(
    "host",
    !!left.report.host &&
      isDeepStrictEqual(left.report.host, right.report.host),
  );
  check(
    "asset-closure",
    !!left.report.assetClosure?.sha256 &&
      left.report.assetClosure.sha256 === right.report.assetClosure?.sha256 &&
      left.report.assetClosure.rawBytes ===
        right.report.assetClosure?.rawBytes &&
      left.report.assetClosure.files === right.report.assetClosure?.files,
  );
  const baselineFiles = left.report.sourceBefore?.files ?? {};
  const candidateFiles = right.report.sourceBefore?.files ?? {};
  result.changedSourcePaths = [
    ...new Set([...Object.keys(baselineFiles), ...Object.keys(candidateFiles)]),
  ]
    .filter((path) => baselineFiles[path] !== candidateFiles[path])
    .sort();
  const remaining = [...right.windows];
  for (const first of left.windows) {
    const at = remaining.findIndex(
      (other) => key(other.row) === key(first.row),
    );
    if (at === -1) {
      result.unmatched.push({
        run: "baseline",
        index: first.row.index,
        key: key(first.row),
      });
      continue;
    }
    const second = remaining.splice(at, 1)[0];
    const pair = {
      key: key(first.row),
      baseline: first.row.index,
      candidate: second.row.index,
      quality: [],
      changes: {},
    };
    for (const phase of ["before", "restored"]) {
      const a = first.captures[phase],
        b = second.captures[phase];
      const inputsMatch =
        !!a &&
        !!b &&
        isDeepStrictEqual(captureInputs(a.metadata), captureInputs(b.metadata));
      let changedPixels = null,
        maxChannelError = null;
      if (a && b && a.pixels.length === b.pixels.length) {
        changedPixels = 0;
        maxChannelError = 0;
        for (let offset = 0; offset < a.pixels.length; offset += 4) {
          let changed = false;
          for (let channel = 0; channel < 4; channel++) {
            const delta = Math.abs(
              a.pixels[offset + channel] - b.pixels[offset + channel],
            );
            if (delta) changed = true;
            maxChannelError = Math.max(maxChannelError, delta);
          }
          if (changed) changedPixels++;
        }
      }
      const exact = !!a && !!b && a.pixels.equals(b.pixels);
      check(
        `pair.${first.row.index}.${second.row.index}.${phase}`,
        inputsMatch && exact,
      );
      pair.quality.push({
        phase,
        inputsMatch,
        exact,
        changedPixels,
        maxChannelError,
        baselineSha256: a?.metadata.pixelSha256 ?? null,
        candidateSha256: b?.metadata.pixelSha256 ?? null,
      });
    }
    function change(a, b) {
      return finite(a) && finite(b)
        ? {
            baseline: a,
            candidate: b,
            difference: b - a,
            percent: a === 0 ? null : (b / a - 1) * 100,
          }
        : null;
    }
    pair.changes.completionRateHz = change(
      first.summary?.pacing?.summary.completionRateHz,
      second.summary?.pacing?.summary.completionRateHz,
    );
    for (const field of stages)
      pair.changes[field] = Object.fromEntries(
        ["p50", "p95", "p99", "max"].map((percentile) => [
          percentile,
          change(
            first.summary?.stages[field]?.[percentile],
            second.summary?.stages[field]?.[percentile],
          ),
        ]),
      );
    result.pairs.push(pair);
  }
  for (const value of remaining)
    result.unmatched.push({
      run: "candidate",
      index: value.row.index,
      key: key(value.row),
    });
  check(
    "pairing.complete",
    result.unmatched.length === 0 && result.pairs.length > 0,
    result.unmatched,
  );
  result.ok = result.checks.every((entry) => entry.ok);
  return result;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const [baseline, candidate, output, ...extra] = process.argv.slice(2);
  if (!baseline || !candidate || !output || extra.length)
    throw Error(
      "Use node tools/compare-town-renderer.mjs BASELINE_DIR CANDIDATE_DIR OUTPUT_JSON.",
    );
  const repository = await realpath(
    dirname(fileURLToPath(new URL("../package.json", import.meta.url))),
  );
  const destination = resolve(output);
  const parent = await realpath(dirname(destination));
  if (parent === repository || parent.startsWith(repository + sep))
    throw Error("Keep private comparison reports outside Git.");
  const report = await compareTownRenderer({ baseline, candidate });
  await writeFile(destination, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify({
      output: destination,
      ok: report.ok,
      qualified: false,
      pairs: report.pairs.length,
      failedChecks: report.checks
        .filter((entry) => !entry.ok)
        .map((entry) => entry.id),
    }),
  );
  if (!report.ok) process.exitCode = 1;
}
