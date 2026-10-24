import test from "node:test";
import assert from "node:assert/strict";
import {
  indexPerformanceSuite,
  verifyApplicationWindow,
  verifyPerformanceInput,
} from "../tools/performance-suite.mjs";
import { createHash } from "node:crypto";
import { mkdtemp, writeFile, rm, mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
const { evaluateApplicationPacing } = await import(
  new URL("../tools/application-pacing.mjs", import.meta.url).href
);
const { evaluateFramePacing } = await import(
  new URL("../tools/performance-contract.mjs", import.meta.url).href
);
const browserEvidence = {
  schema: "aethyrforge.browser-mode.v1",
  product: "Chrome/155.0",
  userAgent: "Chrome/155.0",
  commandLineAvailable: true,
  headlessArguments: [],
  mode: "headed",
  scope: "Actual Browser.getVersion and Browser.getBrowserCommandLine",
};
import {
  ENEMY_WORKLOADS,
  AIM_WORKLOADS,
} from "../tools/gameplay-workloads.mjs";
import { gameplayActivity } from "./fixtures/gameplay-activity.ts";
const views = ["800x500", "1280x720"].map((value) => {
  const [width, height] = value.split("x").map(Number);
  return { width, height };
});
const shots = [
  "MT-02",
  "MT-03",
  ...Array.from(
    { length: 14 },
    (_, i) => `MT-${String(i + 5).padStart(2, "0")}`,
  ),
];
const pacing = {
  qualified: true,
  queueQualified: true,
  gpuBudgetQualified: true,
  summary: {
    mode: "qualification",
    durationMs: 30000,
    warmupMs: 5000,
    minHz: 60,
  },
};
const repeated = (row: any) =>
  views.flatMap((viewport) =>
    [0, 1].map((round) => ({
      ...row,
      viewport,
      round,
      pacing: structuredClone(pacing),
    })),
  );
function fixture() {
  const editors = [false, true].map((town) => ({
    role: town ? "editor-town" : "editor-starter",
    evidenceVerified: true,
    report: {
      schema: "aethyrforge.runtime-browser.v1",
      world: town ? "medieval-town" : "starter",
      ok: true,
      performance: { qualified: true },
      capabilities: { enemies: town, aiming: town },
      assetInventory: { unchanged: true },
      windows: [
        ...(town ? shots : [null]).flatMap((shot) =>
          ["static", "sway"].flatMap((route) =>
            repeated({ feature: "render", shot, route }),
          ),
        ),
        ...(town
          ? ["day", "night"].flatMap((scenario) =>
              ["static", "sway"].flatMap((route) =>
                repeated({ feature: "render", shot: null, scenario, route }),
              ),
            )
          : []),
        ...(town
          ? ["day", "night"].flatMap((scenario) =>
              [...ENEMY_WORKLOADS, ...AIM_WORKLOADS].flatMap((feature) =>
                repeated({
                  feature,
                  scenario,
                  route: "static",
                  activity: gameplayActivity(feature, true),
                }),
              ),
            )
          : []),
        ...(town ? ["day", "night"] : ["day"]).flatMap((scenario) =>
          ["ui", "edit"].flatMap((feature) =>
            repeated({ scenario, feature, route: "static" }),
          ),
        ),
      ],
    },
  }));
  const profiles = ["empty", "room", "character", "town"].map((profile) => {
    const town = profile === "town";
    const ids = [
      ...(town ? shots : ["spawn"]).flatMap((shot) =>
        ["static", "sway"].map((route) => ({ id: `view:${shot}`, route })),
      ),
      ...(town
        ? ["day", "night"].flatMap((scenario) =>
            ["static", "sway"].map((route) => ({
              id: "view:spawn",
              scenario,
              route,
            })),
          )
        : []),
      ...(profile === "room" ? ["first"] : ["first", "third"]).map((view) => ({
        id: `controller:${view}`,
      })),
      { id: "quest:staged-key-exit-reset" },
      ...(town
        ? [
            ...["lightning_bolt", "meteor_shower", "thunderbolt"].map(
              (value) => ({ id: `spell:${value}` }),
            ),
            { id: "destruction:persistent-settled-rubble" },
            ...["day", "night"].flatMap((scenario) =>
              ENEMY_WORKLOADS.map((feature) => ({
                id: `${feature}:${scenario}`,
                scenario,
                activity: gameplayActivity(feature),
              })),
            ),
            ...["day", "night"].flatMap((mode) =>
              [
                "prepare-active",
                "cancel-and-restore",
                "complete-settled-rubble",
              ].map((phase) => ({ id: `castle:${mode}:${phase}` })),
            ),
          ]
        : []),
    ];
    return {
      profile,
      ok: true,
      performance: {
        ok: true,
        qualified: true,
        capabilities: {
          character: true,
          quest: true,
          destruction: town,
          castle: town,
          enemies: town,
        },
        windows: profile === "empty" ? [] : ids.flatMap((row) => repeated(row)),
        idleResponsiveness: views.flatMap((viewport) =>
          [0, 1].map((round) => ({
            viewport,
            round,
            callbacks: 1800,
            durationMs: 30000,
            samples: Array.from({ length: 1800 }, (_, i) => (i * 1000) / 60),
          })),
        ),
      },
    };
  });
  return [
    ...editors,
    {
      role: "players",
      evidenceVerified: true,
      report: {
        schema: "aethyrforge.standalone-player-browser.v1",
        ok: true,
        profiles,
      },
    },
  ];
}
test("suite requires every host and capability permutation at both resolutions with two real repeats", () => {
  const report = indexPerformanceSuite(fixture());
  assert.equal(report.allProductQualified, true);
  assert.equal(report.presentationMs, null);
  assert.equal(report.results.length, 6);
});
test("missing, skipped, diagnostic, and duplicate evidence cannot establish an all-product result", () => {
  const absent = indexPerformanceSuite([]);
  assert.equal(absent.allProductQualified, false);
  assert(absent.results.every((value) => !value.ok));
  const missing: any = fixture();
  missing[1].report.windows.pop();
  assert.equal(indexPerformanceSuite(missing).allProductQualified, false);
  const diagnostic: any = fixture();
  diagnostic[0].report.windows[0].pacing.summary.mode = "diagnostic";
  assert.equal(indexPerformanceSuite(diagnostic).allProductQualified, false);
  const duplicate: any = fixture();
  duplicate.push(duplicate[0]);
  assert.equal(indexPerformanceSuite(duplicate).allProductQualified, false);
});
test("an absent Castle or changed raw evidence fails despite optimistic aggregate flags", () => {
  const noCastle: any = fixture();
  noCastle[2].report.profiles.find(
    (value: any) => value.profile === "town",
  ).performance.capabilities.castle = false;
  assert.equal(indexPerformanceSuite(noCastle).allProductQualified, false);
  const changed: any = fixture();
  changed[2].evidenceVerified = false;
  changed[2].evidenceErrors = [{ error: "Raw frame evidence changed" }];
  assert.equal(indexPerformanceSuite(changed).allProductQualified, false);
  const noRepeat: any = fixture();
  for (const row of noRepeat[1].report.windows) row.round = 0;
  assert.equal(indexPerformanceSuite(noRepeat).allProductQualified, false);
});

test("idle player UI callbacks need real pacing and two repeats without inventing engine rendering", () => {
  const stalled: any = fixture();
  const empty = stalled[2].report.profiles.find(
    (value: any) => value.profile === "empty",
  );
  empty.performance.idleResponsiveness[0].samples = [0, 29990];
  assert.equal(indexPerformanceSuite(stalled).allProductQualified, false);
  const once: any = fixture();
  once[2].report.profiles.find(
    (value: any) => value.profile === "empty",
  ).performance.idleResponsiveness = views.map((viewport) => ({
    viewport,
    round: 0,
    callbacks: 1800,
    durationMs: 30000,
    samples: Array.from({ length: 1800 }, (_, i) => (i * 1000) / 60),
  }));
  assert.equal(indexPerformanceSuite(once).allProductQualified, false);
});

test("Town spawn needs day and night, both routes and resolutions, and two repeats independently of all gallery views", () => {
  for (const role of ["editor", "player"])
    for (const missing of ["all", "night", "sway", "1280x720", "repeat"]) {
      const values: any = fixture();
      const target =
        role === "editor"
          ? values[1].report
          : values[2].report.profiles.find(
              (value: any) => value.profile === "town",
            ).performance;
      target.windows = target.windows.filter((row: any) => {
        const spawn =
          role === "editor"
            ? row.feature === "render" && row.shot === null
            : row.id === "view:spawn";
        if (!spawn) return true;
        return !(
          missing === "all" ||
          (missing === "night" && row.scenario === "night") ||
          (missing === "sway" && row.route === "sway") ||
          (missing === "1280x720" && row.viewport.width === 1280) ||
          (missing === "repeat" && row.round === 1)
        );
      });
      const indexed = indexPerformanceSuite(values);
      assert.equal(indexed.allProductQualified, false, `${role}/${missing}`);
      assert(
        indexed.results
          .find(
            (value) =>
              value.role ===
              (role === "editor" ? "editor-town" : "player-town"),
          )!
          .missing.some((value) => value.permutation.startsWith("view:spawn:")),
      );
    }
});

test("latest Town needs real patrol, combat/corpse/restore and editor targeting evidence", () => {
  for (const feature of [...ENEMY_WORKLOADS, ...AIM_WORKLOADS]) {
    const values: any = fixture();
    values[1].report.windows = values[1].report.windows.filter(
      (row: any) => row.feature !== feature,
    );
    const report = indexPerformanceSuite(values);
    assert.equal(report.allProductQualified, false, feature);
    assert(
      report.results
        .find((row) => row.role === "editor-town")!
        .missing.some((row) => row.permutation === `${feature}:day`),
    );
  }
  const forged: any = fixture();
  forged[2].report.profiles
    .find((p: any) => p.profile === "town")
    .performance.windows.find((row: any) => row.id === "enemy-lightning:night")
    .activity.phases.pop();
  assert.equal(indexPerformanceSuite(forged).allProductQualified, false);
  const absent: any = fixture();
  absent[2].report.profiles.find(
    (p: any) => p.profile === "town",
  ).performance.capabilities.enemies = false;
  assert.equal(indexPerformanceSuite(absent).allProductQualified, false);
});

function applicationFixture() {
  const inputs: any = fixture();
  for (const input of inputs) {
    input.applicationEvidenceVerified = true;
    input.applicationEvidenceErrors = [];
    input.evidenceVerified = false;
    input.report.ok = false;
  }
  const appWindow = {
    schema: "aethyrforge.application-pacing.v1",
    windowQualified: true,
    gpuBudgetWindowQualified: true,
    summary: {
      mode: "qualification",
      durationMs: 30000,
      warmupMs: 5000,
      minHz: 60,
      authoritativeCountersAvailable: true,
      requireFullGpu: true,
    },
  };
  for (const input of inputs.slice(0, 2)) {
    input.report.browserModeEvidence = structuredClone(browserEvidence);
    input.report.applicationPerformance = {
      schema: "aethyrforge.application-matrix.v1",
      qualified: true,
      qualityVerified: true,
      cleanupVerified: true,
    };
    for (const row of input.report.windows)
      row.applicationPacing = structuredClone(appWindow);
  }
  for (const profile of inputs[2].report.profiles) {
    profile.performance.browserModeEvidence = structuredClone(browserEvidence);
    profile.audit = { ok: true };
    profile.errors = [];
    profile.checks = [
      {
        name: "The UI-free player and editor produce exactly equal fixed RGBA pixels",
      },
    ];
    profile.performance.applicationPerformance = {
      schema: "aethyrforge.application-matrix.v1",
      qualified: true,
    };
    for (const row of profile.performance.windows)
      row.applicationPacing = structuredClone(appWindow);
    for (const row of profile.performance.idleResponsiveness)
      row.warmupMs = 5000;
  }
  return inputs;
}

test("new application matrix stays independent of strict fence failures and never claims display pacing", () => {
  const result = indexPerformanceSuite(applicationFixture());
  assert.equal(result.ok, false);
  assert.equal(result.allProductQualified, false);
  assert.equal(result.application.qualified, true);
  assert.equal(result.application.compositorPresentation, null);
  assert.equal(result.application.physicalPresentation, null);
  assert.equal(result.application.results.length, 6);
  const legacy = indexPerformanceSuite(fixture());
  assert.equal(legacy.allProductQualified, true);
  assert.equal(legacy.application.qualified, false);
});

test("application aggregate rejects missing scope, duplicate rounds, unverified counters, and failed quality", () => {
  for (const kind of [
    "missing",
    "rounds",
    "raw",
    "quality",
    "scope",
    "gpu",
    "browser",
  ]) {
    const inputs = applicationFixture();
    if (kind === "missing") inputs[1].report.windows.pop();
    if (kind === "rounds")
      for (const row of inputs[1].report.windows) row.round = 0;
    if (kind === "raw") inputs[1].applicationEvidenceVerified = false;
    if (kind === "gpu")
      inputs[1].report.windows[0].applicationPacing.summary.requireFullGpu = false;
    if (kind === "browser") delete inputs[1].report.browserModeEvidence;
    if (kind === "quality")
      inputs[1].report.applicationPerformance.qualityVerified = false;
    if (kind === "scope")
      inputs[1].report.windows[0].applicationPacing.summary.authoritativeCountersAvailable = false;
    assert.equal(
      indexPerformanceSuite(inputs).application.qualified,
      false,
      kind,
    );
  }
  const player = applicationFixture();
  player[2].report.profiles[2].audit.ok = false;
  assert.equal(indexPerformanceSuite(player).application.qualified, false);
});

function rawApplicationFixture() {
  const counters = {
    frameAttempts: 0,
    submittedFrames: 0,
    skippedFrames: 0,
    rawCallbacks: 0,
    intentionalPacedCallbacks: 0,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
  };
  const after = {
    ...counters,
    frameAttempts: 1800,
    submittedFrames: 1800,
    rawCallbacks: 1800,
  };
  const samples = Array.from({ length: 1800 }, (_, index) => ({
    frame: index + 1,
    at: 10000 + (index * 1000) / 60,
    submitted: true,
    fullGpuMs: 5,
    gpuMs: 3,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
  }));
  const metrics = {
    recording: false,
    truncated: false,
    sampleScope: "render-attempts",
    targetHz: "display",
    observation: { startedAtMs: 10000, stoppedAtMs: 40000 },
    callbackCounts: {
      rawCallbacks: 1800,
      intentionalPacedCallbacks: 0,
      suspendedCallbacks: 0,
      renderAttempts: 1800,
    },
    resources: {
      frameCounters: after,
      renderer: { fullGpuScope: "render-compute-pass-envelope" },
    },
    samples,
    completionTruncated: false,
    completionRange: { firstFrame: 1, lastFrame: 1800 },
    completedFrames: samples.map((sample) => ({
      frame: sample.frame,
      submittedAt: sample.at + 0.1,
      submitReturnedAt: sample.at + 0.2,
      completedAt: sample.at + 2.1,
      latencyMs: 2,
      ok: true,
    })),
  };
  const productCounters = { before: counters, after };
  const applicationOptions = {
    mode: "qualification",
    warmupMs: 5000,
    minHz: 60,
    requireFullGpu: true,
  };
  const raw = {
    schema: "aethyrforge.pacing-window.v1",
    world: "starter",
    shot: null,
    feature: "render",
    scenario: "day",
    route: "static",
    round: 0,
    viewport: { width: 800, height: 500 },
    metrics,
    productCounters,
    browserModeEvidence: structuredClone(browserEvidence),
    applicationOptions,
  };
  const row: any = {
    shot: null,
    feature: "render",
    scenario: "day",
    route: "static",
    round: 0,
    viewport: raw.viewport,
    productCounters: structuredClone(productCounters),
    applicationPacing: evaluateApplicationPacing(metrics, {
      ...applicationOptions,
      productCounters,
      browserMode: "headed",
    }),
    pacing: evaluateFramePacing(metrics, {
      durationMs: 30000,
      warmupMs: 5000,
      mode: "qualification",
      requireFullGpu: true,
    }),
  };
  assert.equal(row.applicationPacing.windowQualified, true);
  assert.equal(row.pacing.qualified, true);
  return { raw, row };
}

test("raw application verification recomputes counters, post-submit deadlines, scope, and browser mode", () => {
  const { raw, row } = rawApplicationFixture();
  assert.equal(
    verifyApplicationWindow(row, raw, browserEvidence).windowQualified,
    true,
  );
  for (const kind of ["counter", "scope", "post-submit", "mode"]) {
    const changed = structuredClone(raw);
    if (kind === "counter")
      changed.productCounters.before.kernelDroppedSeconds = 1;
    if (kind === "scope")
      changed.metrics.resources.renderer.fullGpuScope = "main-HDR-only";
    if (kind === "post-submit")
      delete (changed.metrics.completedFrames[0] as any).submitReturnedAt;
    if (kind === "mode")
      changed.browserModeEvidence.headlessArguments = [
        "--headless=new",
      ] as never[];
    assert.throws(
      () => verifyApplicationWindow(row, changed, browserEvidence),
      Error,
      kind,
    );
  }
});

test("suite revalidates boundary loss independently while preserving strict historical evaluation", async () => {
  const scratch = join(
    process.env.OPENAGENTS_SCRATCH ?? join(homedir(), ".openagents", "scratch"),
  );
  await mkdir(scratch, { recursive: true });
  const directory = await mkdtemp(join(scratch, "application-evidence-test-"));
  try {
    const { raw, row } = rawApplicationFixture();
    const image = Buffer.from([19, 38, 57, 255]);
    const imagePath = join(directory, "owned.rgba");
    await writeFile(imagePath, image);
    const sha = (bytes: Buffer) =>
      createHash("sha256").update(bytes).digest("hex");
    const reference = {
      path: imagePath,
      sha256: sha(image),
      bytes: image.length,
    };
    row.quality = { exact: true, before: reference, after: reference };
    const rawPath = join(directory, "window.json"),
      reportPath = join(directory, "report.json");
    const report = {
      schema: "aethyrforge.runtime-browser.v1",
      world: "starter",
      browserModeEvidence: browserEvidence,
      windows: [row],
    };
    const save = async () => {
      const bytes = Buffer.from(JSON.stringify(raw));
      await writeFile(rawPath, bytes);
      row.rawEvidence = {
        path: rawPath,
        sha256: sha(bytes),
        bytes: bytes.length,
      };
      await writeFile(reportPath, JSON.stringify(report));
    };
    await save();
    const clean = await verifyPerformanceInput("editor-starter", reportPath);
    assert.equal(clean.evidenceVerified, true);
    assert.equal(clean.applicationEvidenceVerified, true);
    // Identical sampled loss values hide first-frame loss from the historical rule.
    for (const sample of raw.metrics.samples) sample.hostClampedSeconds = 0.01;
    raw.productCounters.after.hostClampedSeconds = 0.01;
    row.productCounters = structuredClone(raw.productCounters);
    row.applicationPacing = evaluateApplicationPacing(raw.metrics, {
      ...raw.applicationOptions,
      productCounters: raw.productCounters,
      browserMode: "headed",
    });
    await save();
    const changed = await verifyPerformanceInput("editor-starter", reportPath);
    assert.equal(changed.evidenceVerified, true);
    assert.equal(changed.applicationEvidenceVerified, false);
    assert.match(
      String(
        changed.applicationEvidenceErrors?.[0] &&
          (changed.applicationEvidenceErrors[0] as any).error,
      ),
      /fail sustained application/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("suite independently accepts explicit-entry v2 evidence without relabeling legacy application rows", () => {
  const { raw, row } = rawApplicationFixture();
  const original = structuredClone(row.applicationPacing);
  Object.assign(raw.metrics, { callbackClock: "entry" });
  for (const sample of raw.metrics.samples)
    Object.assign(sample, { callbackEnteredAt: sample.at + 0.05 });
  assert.throws(
    () => verifyApplicationWindow(row, raw, browserEvidence),
    /saved application verdict differs/,
  );
  row.applicationPacing = evaluateApplicationPacing(raw.metrics, {
    ...raw.applicationOptions,
    productCounters: raw.productCounters,
    browserMode: "headed",
  });
  assert.equal(
    verifyApplicationWindow(row, raw, browserEvidence).schema,
    "aethyrforge.application-pacing.v2",
  );
  assert.equal(original.schema, "aethyrforge.application-pacing.v1");
  const inputs = applicationFixture();
  for (const input of inputs.slice(0, 2))
    for (const window of input.report.windows)
      window.applicationPacing.schema = "aethyrforge.application-pacing.v2";
  assert.equal(indexPerformanceSuite(inputs).application.qualified, true);
});
