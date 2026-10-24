import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { distribution } from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  evaluateApplicationPacing,
  applicationBrowserMode,
} from "./application-pacing.mjs";
import {
  ENEMY_WORKLOADS,
  AIM_WORKLOADS,
  gameplayActivityComplete,
} from "./gameplay-workloads.mjs";

const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const viewports = ["800x500", "1280x720"];
const galleries = [
  "MT-02",
  "MT-03",
  ...Array.from(
    { length: 14 },
    (_, i) => `MT-${String(i + 5).padStart(2, "0")}`,
  ),
];
const resolution = (row) => `${row.viewport?.width}x${row.viewport?.height}`;
const requiredWindow = (row) =>
  row.pacing?.qualified === true &&
  row.pacing?.queueQualified === true &&
  row.pacing?.gpuBudgetQualified === true &&
  row.pacing.summary?.mode === "qualification" &&
  row.pacing.summary.durationMs >= 30000 &&
  row.pacing.summary.warmupMs >= 5000 &&
  row.pacing.summary.minHz >= 60;
function coverage(rows, requirements, predicate = requiredWindow) {
  return requirements.flatMap((need) =>
    viewports.flatMap((viewport) => {
      const matched = rows.filter(
        (row) =>
          resolution(row) === viewport && need.match(row) && predicate(row),
      );
      const rounds = new Set(matched.map((row) => row.round));
      return rounds.size >= 2
        ? []
        : [
            {
              permutation: need.id,
              viewport,
              repeats: rounds.size,
              requiredRepeats: 2,
            },
          ];
    }),
  );
}
function editorRequirements(town) {
  const views = town ? galleries : [null];
  return [
    ...views.flatMap((shot) =>
      ["static", "sway"].map((route) => ({
        id: `view:${shot ?? "spawn"}:${route}`,
        match: (row) =>
          row.feature === "render" && row.shot === shot && row.route === route,
      })),
    ),
    ...(town
      ? ["day", "night"].flatMap((scenario) =>
          ["static", "sway"].map((route) => ({
            id: `view:spawn:${scenario}:${route}`,
            match: (row) =>
              row.feature === "render" &&
              row.shot === null &&
              row.scenario === scenario &&
              row.route === route,
          })),
        )
      : []),
    ...(town
      ? ["day", "night"].flatMap((scenario) =>
          [...ENEMY_WORKLOADS, ...AIM_WORKLOADS].map((feature) => ({
            id: `${feature}:${scenario}`,
            match: (row) =>
              row.feature === feature &&
              row.scenario === scenario &&
              row.route === "static" &&
              gameplayActivityComplete(row.activity, feature, { editor: true }),
          })),
        )
      : []),
    ...(town ? ["day", "night"] : ["day"]).flatMap((scenario) =>
      ["ui", "edit"].map((feature) => ({
        id: `${feature}:${scenario}`,
        match: (row) =>
          row.feature === feature &&
          row.scenario === scenario &&
          row.route === "static",
      })),
    ),
  ];
}
function playerRequirements(profile, capabilities) {
  const views = profile === "town" ? galleries : ["spawn"];
  return [
    ...views.flatMap((shot) =>
      ["static", "sway"].map((route) => ({
        id: `view:${shot}:${route}`,
        match: (row) => row.id === `view:${shot}` && row.route === route,
      })),
    ),
    ...(profile === "town"
      ? ["day", "night"].flatMap((scenario) =>
          ["static", "sway"].map((route) => ({
            id: `view:spawn:${scenario}:${route}`,
            match: (row) =>
              row.id === "view:spawn" &&
              row.scenario === scenario &&
              row.route === route,
          })),
        )
      : []),
    ...(profile === "room" ? ["first"] : ["first", "third"]).map(
      (perspective) => ({
        id: `controller:${perspective}`,
        match: (row) => row.id === `controller:${perspective}`,
      }),
    ),
    {
      id: "quest:staged-key-exit-reset",
      match: (row) => row.id === "quest:staged-key-exit-reset",
    },
    ...(capabilities?.enemies
      ? ["day", "night"].flatMap((scenario) =>
          ENEMY_WORKLOADS.map((feature) => ({
            id: `${feature}:${scenario}`,
            match: (row) =>
              row.id === `${feature}:${scenario}` &&
              row.scenario === scenario &&
              gameplayActivityComplete(row.activity, feature),
          })),
        )
      : []),
    ...(capabilities?.destruction
      ? ["lightning_bolt", "meteor_shower", "thunderbolt"].map((ability) => ({
          id: `spell:${ability}`,
          match: (row) => row.id === `spell:${ability}`,
        }))
      : []),
    ...(capabilities?.destruction
      ? [
          {
            id: "destruction:persistent-settled-rubble",
            match: (row) => row.id === "destruction:persistent-settled-rubble",
          },
        ]
      : []),
    ...(capabilities?.castle
      ? ["day", "night"].flatMap((scenario) =>
          [
            "prepare-active",
            "cancel-and-restore",
            "complete-settled-rubble",
          ].map((phase) => ({
            id: `castle:${scenario}:${phase}`,
            match: (row) => row.id === `castle:${scenario}:${phase}`,
          })),
        )
      : []),
  ];
}

function idleRowQualified(row) {
  const values = row.samples;
  if (
    !Array.isArray(values) ||
    values.length < 2 ||
    !values.every(
      (value, index) =>
        Number.isFinite(value) && (!index || value > values[index - 1]),
    ) ||
    row.durationMs < 30000
  )
    return false;
  const intervals = values
    .slice(1)
    .map((value, index) => value - values[index]);
  const stats = distribution(intervals);
  return (
    values.length >= Math.floor((row.durationMs * 60) / 1000) - 1 &&
    values.at(-1) - values[0] >= row.durationMs - 33.34 &&
    stats.p95 <= 17.5 &&
    stats.p99 <= 20 &&
    stats.max <= 33.34 &&
    intervals.filter((value) => value > 25).length / intervals.length <= 0.001
  );
}
const modeEvidenceRecorded = (value) =>
  value?.schema === "aethyrforge.browser-mode.v1" &&
  typeof value.product === "string" &&
  typeof value.userAgent === "string" &&
  typeof value.commandLineAvailable === "boolean" &&
  Array.isArray(value.headlessArguments) &&
  value.headlessArguments.every(
    (flag) => typeof flag === "string" && /^--headless(?:=|$)/.test(flag),
  );
const requiredApplicationWindow = (row) =>
  [
    "aethyrforge.application-pacing.v1",
    "aethyrforge.application-pacing.v2",
  ].includes(row.applicationPacing?.schema) &&
  row.applicationPacing.windowQualified === true &&
  row.applicationPacing.gpuBudgetWindowQualified === true &&
  row.applicationPacing.summary?.requireFullGpu === true &&
  row.applicationPacing.summary?.mode === "qualification" &&
  row.applicationPacing.summary.durationMs >= 30000 &&
  row.applicationPacing.summary.warmupMs >= 5000 &&
  row.applicationPacing.summary.minHz >= 60 &&
  row.applicationPacing.summary.authoritativeCountersAvailable === true;

/** Independent application result; never consumes or changes strict-v1 success flags. */
function indexApplicationSuite(inputs) {
  const role = (id) => inputs.find((value) => value.role === id);
  const results = [];
  for (const id of ["editor-starter", "editor-town"]) {
    const input = role(id),
      report = input?.report;
    const missing = coverage(
      report?.windows ?? [],
      editorRequirements(id === "editor-town"),
      requiredApplicationWindow,
    );
    const quality =
      report?.applicationPerformance?.qualityVerified === true &&
      report.applicationPerformance.cleanupVerified === true &&
      !report.failure &&
      !report.errors?.length &&
      !report.cleanupErrors?.length &&
      report.assetInventory?.unchanged === true;
    const capabilities =
      id !== "editor-town" ||
      (report?.capabilities?.enemies === true &&
        report?.capabilities?.aiming === true);
    results.push({
      role: id,
      qualified: Boolean(
        input?.applicationEvidenceVerified &&
        report?.schema === "aethyrforge.runtime-browser.v1" &&
        report.world === (id === "editor-town" ? "medieval-town" : "starter") &&
        report.applicationPerformance?.schema ===
          "aethyrforge.application-matrix.v1" &&
        report.applicationPerformance.qualified === true &&
        modeEvidenceRecorded(report.browserModeEvidence) &&
        quality &&
        capabilities &&
        !missing.length,
      ),
      available: Boolean(report?.applicationPerformance),
      missing,
      evidenceErrors: input?.applicationEvidenceErrors ?? [],
    });
  }
  const players = role("players");
  for (const profile of ["empty", "room", "character", "town"]) {
    const report = players?.report?.profiles?.find(
        (value) => value.profile === profile,
      ),
      performance = report?.performance;
    const missing =
      profile === "empty"
        ? viewports
            .filter(
              (viewport) =>
                new Set(
                  (performance?.idleResponsiveness ?? [])
                    .filter(
                      (row) =>
                        resolution(row) === viewport &&
                        row.warmupMs >= 5000 &&
                        idleRowQualified(row),
                    )
                    .map((row) => row.round),
                ).size < 2,
            )
            .map((viewport) => ({
              permutation: "idle-ui-responsiveness",
              viewport,
              requiredRepeats: 2,
            }))
        : coverage(
            performance?.windows ?? [],
            playerRequirements(profile, performance?.capabilities),
            requiredApplicationWindow,
          );
    const capabilities =
      profile !== "town" ||
      Boolean(
        performance?.capabilities?.character &&
        performance.capabilities.quest &&
        performance.capabilities.destruction &&
        performance.capabilities.castle &&
        performance.capabilities.enemies,
      );
    const quality =
      report?.audit?.ok === true &&
      Array.isArray(report.errors) &&
      report.errors.length === 0 &&
      !report.failure &&
      !players?.report?.failure &&
      !players?.report?.cleanupErrors?.length &&
      report.checks?.some((check) =>
        check.name?.includes("exactly equal fixed RGBA pixels"),
      );
    results.push({
      role: `player-${profile}`,
      qualified: Boolean(
        players?.applicationEvidenceVerified &&
        players.report.schema === "aethyrforge.standalone-player-browser.v1" &&
        performance?.applicationPerformance?.schema ===
          "aethyrforge.application-matrix.v1" &&
        (profile === "empty" ||
          performance.applicationPerformance.qualified === true) &&
        modeEvidenceRecorded(performance.browserModeEvidence) &&
        quality &&
        capabilities &&
        !missing.length,
      ),
      available: Boolean(performance?.applicationPerformance),
      missing,
      evidenceErrors: players?.applicationEvidenceErrors ?? [],
      scope:
        profile === "empty"
          ? "Idle browser/UI responsiveness; continuous rendering is not applicable"
          : "Published-player application scheduling/readiness with independent quality and capability coverage",
    });
  }
  const duplicates =
    inputs.length !== new Set(inputs.map((value) => value.role)).size;
  return {
    schema: "aethyrforge.application-suite.v1",
    qualified: !duplicates && results.every((value) => value.qualified),
    results,
    compositorPresentation: null,
    physicalPresentation: null,
    limitations: [
      "Separate application criteria leave strict-v1 report statuses and fence failures unchanged.",
      "At least two complete thirty-second/five-second-warmup windows per required permutation, authoritative whole-window counters, raw evidence verification, source quality, and cleanup are required.",
      "Browser mode comes from retained CDP command-line/user-agent evidence. Headless application results never imply headed compositor or physical display cadence.",
    ],
  };
}

/** Recompute an application verdict from its pinned raw evidence before indexing. */
export function verifyApplicationWindow(row, raw, browserEvidence) {
  if (
    !row.applicationPacing ||
    !raw.productCounters ||
    !raw.applicationOptions ||
    !modeEvidenceRecorded(raw.browserModeEvidence) ||
    !modeEvidenceRecorded(browserEvidence)
  )
    throw Error(
      "Missing independent application evidence or authoritative boundary counters.",
    );
  if (
    JSON.stringify(raw.productCounters) !== JSON.stringify(row.productCounters)
  )
    throw Error("Raw whole-window counters differ from the matrix row.");
  if (
    JSON.stringify(raw.browserModeEvidence) !== JSON.stringify(browserEvidence)
  )
    throw Error("Raw browser-mode evidence differs from its host report.");
  if (
    raw.applicationOptions.mode !== "qualification" ||
    raw.applicationOptions.requireFullGpu !== true
  )
    throw Error(
      "Application matrix qualification requires sustained mode and full-GPU evidence.",
    );
  const evaluated = evaluateApplicationPacing(raw.metrics, {
    ...raw.applicationOptions,
    productCounters: raw.productCounters,
    browserMode: applicationBrowserMode(raw.browserModeEvidence),
  });
  if (JSON.stringify(evaluated) !== JSON.stringify(row.applicationPacing))
    throw Error(
      "The saved application verdict differs from its raw observations.",
    );
  if (!evaluated.windowQualified)
    throw Error(
      "Raw observations fail sustained application scheduling/readiness qualification.",
    );
  return evaluated;
}

/** Index verified native reports; it never turns a skipped workload into a pass. */
export function indexPerformanceSuite(inputs) {
  const results = [];
  const role = (id) => inputs.find((value) => value.role === id);
  for (const id of ["editor-starter", "editor-town"]) {
    const input = role(id),
      report = input?.report;
    const expected = id === "editor-town" ? "medieval-town" : "starter";
    const missing = coverage(
      report?.windows ?? [],
      editorRequirements(id === "editor-town"),
    );
    const ok = Boolean(
      input?.evidenceVerified &&
      report?.schema === "aethyrforge.runtime-browser.v1" &&
      report.world === expected &&
      report.ok === true &&
      report.performance?.qualified === true &&
      report.assetInventory?.unchanged === true &&
      (id !== "editor-town" ||
        (report.capabilities?.enemies === true &&
          report.capabilities?.aiming === true)) &&
      !missing.length,
    );
    results.push({
      role: id,
      ok,
      available: Boolean(report),
      reference: input?.reference ?? null,
      missing,
      evidenceErrors: input?.evidenceErrors ?? [],
    });
  }
  const players = role("players");
  for (const profile of ["empty", "room", "character", "town"]) {
    const report = players?.report?.profiles?.find(
        (value) => value.profile === profile,
      ),
      performance = report?.performance;
    const missing =
      profile === "empty"
        ? viewports
            .filter(
              (viewport) =>
                new Set(
                  (performance?.idleResponsiveness ?? [])
                    .filter(
                      (row) =>
                        resolution(row) === viewport && idleRowQualified(row),
                    )
                    .map((row) => row.round),
                ).size < 2,
            )
            .map((viewport) => ({
              permutation: "idle-ui-responsiveness",
              viewport,
              requiredRepeats: 2,
            }))
        : coverage(
            performance?.windows ?? [],
            playerRequirements(profile, performance?.capabilities),
          );
    const requiredCapabilities =
      profile !== "town" ||
      Boolean(
        performance?.capabilities?.character &&
        performance.capabilities.quest &&
        performance.capabilities.destruction &&
        performance.capabilities.castle &&
        performance.capabilities.enemies,
      );
    const ok = Boolean(
      players?.evidenceVerified &&
      players.report.schema === "aethyrforge.standalone-player-browser.v1" &&
      players.report.ok === true &&
      report?.ok === true &&
      performance?.ok === true &&
      (profile === "empty" || performance.qualified === true) &&
      requiredCapabilities &&
      !missing.length,
    );
    results.push({
      role: `player-${profile}`,
      ok,
      available: Boolean(report),
      reference: players?.reference ?? null,
      requiredCapabilities,
      missing,
      evidenceErrors: players?.evidenceErrors ?? [],
      scope:
        profile === "empty"
          ? "Idle browser/UI responsiveness; continuous engine rendering not applicable."
          : "Source render, controller, quest and advertised gameplay permutations.",
    });
  }
  const duplicates =
    inputs.length !== new Set(inputs.map((value) => value.role)).size;
  return {
    schema: "aethyrforge.performance-suite.v1",
    ok: !duplicates && results.every((value) => value.ok),
    allProductQualified: !duplicates && results.every((value) => value.ok),
    presentationMs: null,
    results,
    limitations: [
      "One measured browser/GPU and exact pinned content/builds; this is not an all-device or all-future-world guarantee.",
      "GPU queue completion and timestamp budgets do not measure physical presentation.",
      "Missing, diagnostic, skipped, or insufficiently repeated permutations remain unqualified.",
      "Native reports retain their original source/build identity. The index does not imply that different builds are a paired comparison.",
    ],
    duplicates,
    application: indexApplicationSuite(inputs),
  };
}
export async function verifyPerformanceInput(role, path) {
  const bytes = await readFile(path),
    report = JSON.parse(bytes.toString()),
    errors = [],
    applicationErrors = [];
  const profiles = role === "players" ? (report.profiles ?? []) : [report];
  for (const profile of profiles) {
    const windows =
      role === "players"
        ? (profile.performance?.windows ?? [])
        : (profile.windows ?? []);
    for (const row of windows) {
      try {
        if (!row.rawEvidence) throw Error("Missing raw completion evidence.");
        const rawBytes = await readFile(row.rawEvidence.path);
        if (
          digest(rawBytes) !== row.rawEvidence.sha256 ||
          rawBytes.length !== row.rawEvidence.bytes
        )
          throw Error("Raw frame evidence changed.");
        const raw = JSON.parse(rawBytes.toString());
        if (
          raw.schema !== "aethyrforge.pacing-window.v1" ||
          raw.route !== row.route ||
          raw.round !== row.round ||
          JSON.stringify(raw.viewport) !== JSON.stringify(row.viewport)
        )
          throw Error("Raw frame context differs from its matrix row.");
        if (
          role === "players"
            ? raw.profile !== profile.profile ||
              raw.id !== row.id ||
              raw.scenario !== row.scenario
            : raw.world !== report.world ||
              raw.shot !== row.shot ||
              raw.feature !== row.feature ||
              raw.scenario !== row.scenario
        )
          throw Error("Raw frame identity differs from its matrix row.");
        const workload =
          role === "players"
            ? ENEMY_WORKLOADS.find(
                (feature) => row.id === `${feature}:${row.scenario}`,
              )
            : [...ENEMY_WORKLOADS, ...AIM_WORKLOADS].find(
                (feature) => feature === row.feature,
              );
        if (
          workload &&
          (JSON.stringify(raw.activity) !== JSON.stringify(row.activity) ||
            !gameplayActivityComplete(raw.activity, workload, {
              editor: role !== "players",
            }))
        )
          throw Error(
            "The saved native gameplay/real-input workload is incomplete or differs from its raw window.",
          );
        const observation = raw.metrics.observation;
        const evaluated = evaluateFramePacing(raw.metrics, {
          durationMs: observation.stoppedAtMs - observation.startedAtMs,
          warmupMs: row.pacing.summary.warmupMs,
          mode: "qualification",
          requireFullGpu: true,
        });
        if (!evaluated.qualified)
          throw Error("Raw observations fail sustained pacing qualification.");
        if (role !== "players") {
          if (
            !row.quality?.exact ||
            row.quality.before.sha256 !== row.quality.after.sha256
          )
            throw Error("Source image equality was not checked.");
          for (const image of [row.quality.before, row.quality.after]) {
            const pixels = await readFile(image.path);
            if (
              digest(pixels) !== image.sha256 ||
              pixels.length !== image.bytes
            )
              throw Error("Source reference pixels changed.");
          }
        }
      } catch (error) {
        errors.push({
          profile: profile.profile ?? null,
          id: row.id ?? row.shot ?? row.feature,
          route: row.route,
          round: row.round,
          error: String(error),
        });
      }
      try {
        const rawBytes = await readFile(row.rawEvidence?.path);
        if (
          digest(rawBytes) !== row.rawEvidence.sha256 ||
          rawBytes.length !== row.rawEvidence.bytes
        )
          throw Error("Raw application frame evidence changed.");
        const raw = JSON.parse(rawBytes.toString());
        if (
          raw.schema !== "aethyrforge.pacing-window.v1" ||
          raw.route !== row.route ||
          raw.round !== row.round ||
          JSON.stringify(raw.viewport) !== JSON.stringify(row.viewport) ||
          (role === "players"
            ? raw.profile !== profile.profile ||
              raw.id !== row.id ||
              raw.scenario !== row.scenario
            : raw.world !== report.world ||
              raw.shot !== row.shot ||
              raw.feature !== row.feature ||
              raw.scenario !== row.scenario)
        )
          throw Error("Raw application context differs from its matrix row.");
        const workload =
          role === "players"
            ? ENEMY_WORKLOADS.find(
                (feature) => row.id === `${feature}:${row.scenario}`,
              )
            : [...ENEMY_WORKLOADS, ...AIM_WORKLOADS].find(
                (feature) => feature === row.feature,
              );
        if (
          workload &&
          (JSON.stringify(raw.activity) !== JSON.stringify(row.activity) ||
            !gameplayActivityComplete(raw.activity, workload, {
              editor: role !== "players",
            }))
        )
          throw Error(
            "Raw application gameplay/real-input workload is incomplete or changed.",
          );
        verifyApplicationWindow(
          row,
          raw,
          role === "players"
            ? profile.performance?.browserModeEvidence
            : report.browserModeEvidence,
        );
        if (role !== "players") {
          if (
            !row.quality?.exact ||
            row.quality.before.sha256 !== row.quality.after.sha256
          )
            throw Error("Application source image equality was not checked.");
          for (const image of [row.quality.before, row.quality.after]) {
            const pixels = await readFile(image.path);
            if (
              digest(pixels) !== image.sha256 ||
              pixels.length !== image.bytes
            )
              throw Error("Application source reference pixels changed.");
          }
        }
      } catch (error) {
        applicationErrors.push({
          profile: profile.profile ?? null,
          id: row.id ?? row.shot ?? row.feature,
          route: row.route,
          round: row.round,
          error: String(error),
        });
      }
    }
  }
  return {
    role,
    report,
    applicationEvidenceVerified: applicationErrors.length === 0,
    applicationEvidenceErrors: applicationErrors,
    evidenceVerified: errors.length === 0,
    reference: {
      path: resolve(path),
      sha256: digest(bytes),
      bytes: bytes.length,
      schema: report.schema,
    },
    evidenceErrors: errors,
  };
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const options = new Map();
  for (let i = 2; i < process.argv.length; i += 2) {
    if (!process.argv[i + 1])
      throw Error("Provide a value for each report option.");
    options.set(process.argv[i], process.argv[i + 1]);
  }
  const inputs = [];
  for (const role of ["editor-starter", "editor-town", "players"]) {
    const path = options.get(`--${role}`);
    if (path) inputs.push(await verifyPerformanceInput(role, path));
  }
  const report = indexPerformanceSuite(inputs),
    path = options.get("--out");
  if (!path) throw Error("Choose an output JSON path outside the checkout.");
  const checkout = resolve(fileURLToPath(new URL("..", import.meta.url)));
  const target = resolve(path);
  if (target === checkout || target.startsWith(checkout + "/"))
    throw Error("Keep qualification evidence outside Git.");
  await writeFile(target, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify({
      ok: report.ok,
      allProductQualified: report.allProductQualified,
      path: target,
    }),
  );
  if (!report.ok) process.exitCode = 1;
}
