/** Run only through the isolated browser launcher; all trace data stays private. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import {
  summarizeFrames,
  distribution,
} from "../experiments/c-town/measurement.mjs";
import { evaluateFramePacing } from "./performance-contract.mjs";
import {
  compositorObserverPlan,
  compositorSessionVerdict,
} from "./compositor-trace.mjs";
import {
  browserPresentationInfo,
  collectCanvasIdentity,
  recordCompositorWindow,
  analyzeCompositorSession,
} from "./compositor-collector.mjs";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(
  process.env.AF_CAPTURE_DIR && process.env.AF_TRACE_PROCESSOR,
  "Set private AF_CAPTURE_DIR and the official AF_TRACE_PROCESSOR binary path.",
);
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const directory = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  directory !== root && !directory.startsWith(root + sep),
  "Keep trace evidence outside Git.",
);
const path = resolve(directory, "compositor-probe.json");
let browser, context, page;
let phase = process.env.AF_COMPOSITOR_REANALYZE ? "analysis" : "collection";
const report = process.env.AF_COMPOSITOR_REANALYZE
  ? JSON.parse(await readFile(process.env.AF_COMPOSITOR_REANALYZE, "utf8"))
  : {
      schema: "aethyrforge.compositor-probe.v1",
      startedAt: new Date().toISOString(),
      ok: false,
      windows: [],
      errors: [],
      collection: { complete: false, error: null },
      cleanup: { complete: false, error: null },
      limitations: [
        "An exact-source Chrome155 compositor prototype, not an all-product performance verdict or physical display measurement.",
        "A source-attributed fresh canvas image is counted once, even when the compositor displays that same surface again.",
        "Existing fence diagnostics and gates remain unchanged. The probe adds no screenshot/video observer during timing.",
        "Short or headless runs remain unqualified. Appearance and full content qualification stay with the existing runtime/player matrix.",
      ],
    };
const save = () => writeFile(path, JSON.stringify(report, null, 2) + "\n");
try {
  if (!process.env.AF_COMPOSITOR_REANALYZE) {
    const target = new URL(process.env.AF_COMPOSITOR_URL || "");
    assert(
      target.protocol === "http:" &&
        ["127.0.0.1", "localhost"].includes(target.hostname) &&
        !target.username &&
        !target.password,
      "Use an explicitly selected local editor server with VITE_C_TOWN_BENCHMARK=1.",
    );
    const endpoint =
      process.env.OPENAGENTS_CHROME_WS ||
      (process.env.OPENAGENTS_CHROME_PORT &&
        `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
    assert(endpoint, "Run through openagents browser run.");
    const mode = process.env.AF_PERFORMANCE_MODE || "diagnostic";
    assert(["diagnostic", "qualification"].includes(mode));
    const durationMs = Number(
      process.env.AF_COMPOSITOR_WINDOW_MS ||
        (mode === "qualification" ? 30000 : 5000),
    );
    const warmupMs = Number(process.env.AF_RUNTIME_WARMUP_MS || 5000);
    const repeats = Number(process.env.AF_COMPOSITOR_REPEATS || 1);
    assert(
      Number.isInteger(durationMs) && durationMs >= 500 && durationMs <= 60000,
    );
    assert(Number.isInteger(warmupMs) && warmupMs >= 500 && warmupMs <= 30000);
    const instrumented = process.env.AF_COMPOSITOR_INSTRUMENTED === "1";
    const targetSetting = process.env.AF_RUNTIME_TARGET_HZ ?? "60";
    assert(["display", "60"].includes(targetSetting));
    const targetHz = targetSetting === "60" ? 60 : "display";
    const viewport = (process.env.AF_COMPOSITOR_VIEWPORT || "1280x720")
      .split("x")
      .map(Number);
    assert(
      viewport.length === 2 &&
        viewport.every(Number.isInteger) &&
        viewport[0] >= 320 &&
        viewport[0] <= 3840 &&
        viewport[1] >= 200 &&
        viewport[1] <= 2160,
    );
    report.collectorSources = await Promise.all(
      [
        "tools/compositor-probe.mjs",
        "tools/compositor-collector.mjs",
        "tools/compositor-trace.mjs",
        "tools/compositor-decode.py",
        "tools/performance-contract.mjs",
        "experiments/c-town/measurement.mjs",
      ].map(async (source) => {
        const bytes = await readFile(resolve(root, source));
        return {
          path: source,
          bytes: bytes.length,
          sha256: createHash("sha256").update(bytes).digest("hex"),
        };
      }),
    );
    report.configuration = {
      url: target.href,
      mode,
      durationMs,
      warmupMs,
      instrumented,
      targetHz,
      viewport,
      order: compositorObserverPlan(repeats),
      observer:
        "Tracing on/off, with identical common application/completion observation and optional GPU profiler setting",
    };
    browser = await chromium.connectOverCDP(endpoint);
    report.browser = await browserPresentationInfo(browser);
    assert(
      report.browser.product === "Chrome/155.0.8059.85",
      "This prototype requires the verified Chrome 155.0.8059.85 trace contract.",
    );
    context = await browser.newContext({
      viewport: { width: viewport[0], height: viewport[1] },
      serviceWorkers: "block",
    });
    await context.route("**/*", (route) => {
      const url = new URL(route.request().url());
      return url.protocol === "http:" &&
        ["127.0.0.1", "localhost"].includes(url.hostname) &&
        ["GET", "HEAD", "OPTIONS"].includes(route.request().method())
        ? route.continue()
        : route.abort("blockedbyclient");
    });
    page = await context.newPage();
    page.on("pageerror", (error) => report.errors.push(String(error)));
    await page.goto(target.href, { waitUntil: "domcontentloaded" });
    const deadline = Date.now() + 180000;
    for (;;) {
      const status = await page.evaluate(
        () => window.aethyrforge?.snapshot().status,
      );
      assert.notEqual(status?.phase, "error", status?.message);
      if (status?.phase === "ready" && status.frame > 5) break;
      assert(Date.now() < deadline, "The local editor did not become ready.");
      await page.waitForTimeout(100);
    }
    const discovery = await page.evaluate(() =>
      window.aethyrforge.agent.discover(),
    );
    const town = discovery.worldId === "medieval-town-lab";
    const shot = process.env.AF_COMPOSITOR_SHOT ?? (town ? "MT-11" : null);
    if (shot !== null) {
      assert(town, "Reference shots require the Medieval Town profile.");
      assert(
        [
          "MT-02",
          "MT-03",
          ...Array.from(
            { length: 14 },
            (_, i) => `MT-${String(i + 5).padStart(2, "0")}`,
          ),
        ].includes(shot),
        "Choose one of the sixteen supported Medieval Town reference shots.",
      );
      const requestId = await page.evaluate(
        (id) => window.aethyrforge.camera(id),
        shot,
      );
      assert(
        typeof requestId === "string",
        "The selected shot did not produce a command request.",
      );
      const deadline = Date.now() + 180000;
      for (;;) {
        const result = await page.evaluate(
          (id) =>
            window.aethyrforge
              .snapshot()
              .receipts.find((receipt) => receipt.requestId === id),
          requestId,
        );
        if (result) {
          assert.equal(result.ok, true, result.message);
          report.configuration.shot = { id: shot, receipt: result };
          break;
        }
        assert(Date.now() < deadline, "The reference shot did not finish.");
        await page.waitForTimeout(50);
      }
    } else report.configuration.shot = null;
    await page.waitForTimeout(500);
    report.before = await page.evaluate(async () => ({
      inspection: await window.aethyrforge.agent.inspect({ limit: 1 }),
      environment: window.aethyrforge.snapshot().environment,
    }));
    report.canvas = {
      before: await collectCanvasIdentity({
        browser,
        page,
        directory,
        label: "before",
      }),
    };
    for (const [round, order] of report.configuration.order.entries())
      for (const [index, label] of order.entries()) {
        const row = await recordCompositorWindow({
          browser,
          page,
          directory,
          label,
          durationMs,
          warmupMs,
          instrumented,
          targetHz,
          round,
          order: index,
        });
        report.windows.push(row);
        await save();
        console.log(
          `COMPOSITOR ${label}/${round}/${index}: submissions=${row.submittedFrames} durationMs=${row.durationMs}`,
        );
      }
    report.canvas.after = await collectCanvasIdentity({
      browser,
      page,
      directory,
      label: "after",
    });
    report.after = await page.evaluate(async () => ({
      inspection: await window.aethyrforge.agent.inspect({ limit: 1 }),
      environment: window.aethyrforge.snapshot().environment,
    }));
    for (const key of ["sourceIdentity", "shaderIdentity", "revision"])
      assert.equal(
        report.after.inspection.context[key],
        report.before.inspection.context[key],
        `Tracing changed ${key}.`,
      );
    assert.deepEqual(report.after.environment, report.before.environment);
    assert.deepEqual(report.errors, []);
    report.collection.complete = true;
    await page.evaluate(() => window.aethyrforgeEditor.service.close());
    await context.close();
    context = undefined;
    page = undefined;
    await browser.close();
    browser = undefined;
    report.cleanup.complete = true;
    phase = "analysis";
    // All GPU resources close before parsing or statistics work.
    await save();
  }
  assert.equal(report.schema, "aethyrforge.compositor-probe.v1");
  await analyzeCompositorSession(report, {
    processor: process.env.AF_TRACE_PROCESSOR,
    directory,
    mode: report.configuration.mode,
  });
  for (const row of report.windows) {
    const bytes = await readFile(row.application.path);
    assert.equal(bytes.length, row.application.bytes);
    assert.equal(
      createHash("sha256").update(bytes).digest("hex"),
      row.application.sha256,
    );
    const { metrics } = JSON.parse(bytes.toString());
    row.summary = summarizeFrames(metrics.samples);
    row.existingFenceDiagnostics = evaluateFramePacing(metrics, {
      durationMs: row.durationMs,
      warmupMs: row.warmupMs,
      mode: "diagnostic",
      requireFullGpu: row.instrumented,
    });
  }
  report.observerCost = Object.fromEntries(
    ["off", "on"].map((label) => [
      label,
      {
        applicationCompletedHz: distribution(
          report.windows
            .filter((row) => row.label === label)
            .map(
              (row) => row.existingFenceDiagnostics.summary.completionRateHz,
            ),
        ),
        applicationSubmittedHz: distribution(
          report.windows
            .filter((row) => row.label === label)
            .map((row) => row.summary.acceptedSubmissionsPerSecond),
        ),
        scope:
          "Exact recorded ABBA order and unchanged source quality; this comparison measures trace observer cost, not a renderer optimization.",
      },
    ]),
  );
  report.analysis = { complete: true, error: null };
  report.ok = compositorSessionVerdict(report);
} catch (error) {
  report.ok = false;
  const detail = String(error?.stack ?? error);
  if (phase === "collection") report.collection.error = detail;
  else report.analysis = { complete: false, error: detail };
  report.error = detail;
  process.exitCode = 1;
} finally {
  if (page)
    await page
      .evaluate(() => window.aethyrforgeEditor?.service.close())
      .catch(() => {});
  await context?.close().catch((error) => {
    report.cleanup.error = String(error);
    report.cleanup.complete = false;
    report.ok = false;
  });
  await browser?.close().catch((error) => {
    report.cleanup.error = String(error);
    report.cleanup.complete = false;
    report.ok = false;
  });
  report.finishedAt = new Date().toISOString();
  await save();
  if (!report.ok) process.exitCode = 1;
  console.log(
    JSON.stringify({
      path,
      ok: report.ok,
      attributed: report.attributed ?? false,
      compositorPacingQualified: report.compositorPacingQualified ?? false,
      headless: report.browser?.headless ?? null,
      error: report.error ?? null,
    }),
  );
}
