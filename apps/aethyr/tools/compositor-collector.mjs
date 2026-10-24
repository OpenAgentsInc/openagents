/** Test-only CDP collector. Callers own the single live scene and browser context. */
import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { open, readFile, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  identifyCanvasSurface,
  analyzeCompositorTrace,
} from "./compositor-trace.mjs";

const exec = promisify(execFile);
const MAX_TRACE_BYTES = 128 * 1024 * 1024;
const hash = (value) => createHash("sha256").update(value).digest("hex");
const save = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
const BASE_CATEGORIES = [
  "viz",
  "graphics.pipeline",
  "blink.user_timing",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-display.framedisplayed",
];
export async function browserPresentationInfo(browser) {
  const session = await browser.newBrowserCDPSession();
  try {
    const version = await session.send("Browser.getVersion");
    const command = await session
      .send("Browser.getBrowserCommandLine")
      .catch(() => null);
    const flags = command?.arguments ?? [];
    const headless =
      flags.some((value) => /^--headless(?:=|$)/.test(value)) ||
      /HeadlessChrome/.test(version.userAgent)
        ? true
        : command
          ? false
          : null;
    return {
      ...version,
      headless,
      commandLineAvailable: Boolean(command),
      observedFlags: flags.filter((value) =>
        /^--(?:headless|use-angle|use-gl|disable-gpu|enable-features|disable-features|window-size|force-device-scale-factor)(?:=|$)/.test(
          value,
        ),
      ),
      limitation:
        "Headless and unknown modes cannot establish headed Mac presentation pacing.",
    };
  } finally {
    await session.detach();
  }
}

/** Never enables screenshots or tracing-wide JavaScript profiling. */
export async function startCompositorTrace(
  browser,
  path,
  { layers = false, timeoutMs = 20000 } = {},
) {
  assert(Number.isInteger(timeoutMs) && timeoutMs >= 100 && timeoutMs <= 20000);
  const session = await browser.newBrowserCDPSession();
  let active = false,
    stream = null,
    detached = false;
  const bounded = async (operation, milliseconds, description) => {
    let timer;
    return Promise.race([
      operation,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(description)), milliseconds);
      }),
    ]).finally(() => clearTimeout(timer));
  };
  const complete = new Promise((resolve, reject) => {
    session.once("Tracing.tracingComplete", (metadata) => {
      stream = metadata.stream ?? null;
      resolve(metadata);
    });
    session.once("close", () =>
      reject(Error("The tracing session closed before flushing.")),
    );
  });
  complete.catch(() => {});
  const cleanup = async () => {
    if (stream) {
      const handle = stream;
      stream = null;
      await bounded(
        session.send("IO.close", { handle }),
        2000,
        "Closing the trace stream timed out.",
      ).catch(() => {});
    }
    if (!detached) {
      detached = true;
      await bounded(
        session.detach(),
        2000,
        "Detaching the trace session timed out.",
      ).catch(() => {});
    }
  };
  try {
    const categories = [
      ...BASE_CATEGORIES,
      ...(layers ? ["disabled-by-default-devtools.timeline.layers"] : []),
    ];
    await bounded(
      session.send("Tracing.start", {
        transferMode: "ReturnAsStream",
        streamFormat: "proto",
        traceConfig: {
          recordMode: "recordUntilFull",
          includedCategories: categories,
          enableSampling: false,
          traceBufferSizeInKb: 65536,
        },
      }),
      timeoutMs,
      "Starting the trace timed out.",
    );
    active = true;
    return {
      path,
      categories,
      async stop() {
        assert(active, "Stop a trace only once.");
        active = false;
        const deadline = Date.now() + timeoutMs;
        const remaining = () => Math.max(1, deadline - Date.now());
        let file;
        try {
          await bounded(
            session.send("Tracing.end"),
            remaining(),
            "Ending the trace timed out.",
          );
          const metadata = await bounded(
            complete,
            remaining(),
            "Chrome did not flush its bounded trace.",
          );
          assert(stream, "Chrome returned no proto trace stream.");
          file = await open(path, "wx");
          const digest = createHash("sha256");
          let bytes = 0;
          for (;;) {
            const chunk = await bounded(
              session.send("IO.read", { handle: stream, size: 1024 * 1024 }),
              remaining(),
              "Reading the bounded trace timed out.",
            );
            const data = Buffer.from(
              chunk.data,
              chunk.base64Encoded ? "base64" : "utf8",
            );
            bytes += data.length;
            assert(
              bytes <= MAX_TRACE_BYTES,
              "The trace exceeded its 128 MiB limit.",
            );
            digest.update(data);
            await file.write(data);
            if (chunk.eof) break;
            assert(
              Date.now() < deadline,
              "Reading the bounded trace timed out.",
            );
          }
          return {
            schema: "aethyrforge.compositor-trace.v1",
            path,
            bytes,
            sha256: digest.digest("hex"),
            categories,
            dataLossOccurred: metadata.dataLossOccurred ?? null,
            traceFormat: metadata.traceFormat ?? null,
          };
        } finally {
          await file?.close().catch(() => {});
          await cleanup();
        }
      },
      async abort() {
        try {
          if (active) {
            active = false;
            await bounded(
              session.send("Tracing.end"),
              2000,
              "Aborting the trace timed out.",
            ).catch(() => {});
            await bounded(
              complete,
              2000,
              "Flushing the aborted trace timed out.",
            ).catch(() => {});
          }
        } finally {
          await cleanup();
        }
      },
    };
  } catch (error) {
    if (active)
      await bounded(
        session.send("Tracing.end"),
        2000,
        "Ending the failed trace timed out.",
      ).catch(() => {});
    await cleanup();
    throw error;
  }
}

export async function collectCanvasIdentity({
  browser,
  page,
  directory,
  label = "before",
}) {
  const frameHandle = await page.locator("#preview-frame").elementHandle();
  const frame = frameHandle
    ? await frameHandle.contentFrame()
    : page.mainFrame();
  assert(frame, "The editor has no live preview frame.");
  let session,
    sharedPageSession = false;
  try {
    session = await page.context().newCDPSession(frame);
  } catch (error) {
    if (
      !frameHandle ||
      !String(error?.message ?? error).includes(
        "This frame does not have a separate CDP session, it is a part of the parent frame's session",
      )
    )
      throw error;
    session = await page.context().newCDPSession(page);
    sharedPageSession = true;
  }
  let trace;
  try {
    const document = await session.send("DOM.getDocument", {
      depth: -1,
      pierce: true,
    });
    const frameTree = await session.send("Page.getFrameTree");
    const frames = new Map();
    const visitFrames = (tree) => {
      frames.set(tree.frame.id, tree.frame);
      for (const child of tree.childFrames ?? []) visitFrames(child);
    };
    visitFrames(frameTree.frameTree);
    const attrs = (node) =>
      Object.fromEntries(
        Array.from({ length: (node.attributes?.length ?? 0) / 2 }, (_, i) =>
          node.attributes.slice(i * 2, i * 2 + 2),
        ),
      );
    const children = (node) => [
      ...(node.children ?? []),
      ...(node.shadowRoots ?? []),
      ...(node.contentDocument ? [node.contentDocument] : []),
    ];
    const fullFrameURL = (value) => {
      const url = new URL(value.url);
      if (value.urlFragment !== undefined) url.hash = value.urlFragment;
      return url.href;
    };
    const observedFrames = [...frames.values()].map((value) => ({
      id: value.id,
      url: value.url,
      urlFragment: value.urlFragment ?? null,
      fullURL: fullFrameURL(value),
    }));
    const selectedURL = frame.url();
    let targetFrameId = frameTree.frameTree.frame.id;
    let targetDocument = document.root;
    let frameOwnerBackendNodeId = null;
    let ownerFrameField = null;
    if (sharedPageSession) {
      const owners = [];
      let inspected = 0;
      const findOwner = (node) => {
        assert(
          ++inspected <= 100000,
          "The preview DOM exceeds the bounded attribution limit.",
        );
        if (node.nodeName === "IFRAME" && attrs(node).id === "preview-frame")
          owners.push(node);
        for (const child of children(node)) findOwner(child);
      };
      findOwner(document.root);
      assert.equal(
        owners.length,
        1,
        "The shared session has no unique preview iframe owner.",
      );
      const owner = owners[0];
      assert(
        owner.contentDocument,
        "The shared session cannot inspect the preview frame's content document.",
      );
      frameOwnerBackendNodeId = owner.backendNodeId;
      ownerFrameField = owner.frameId ?? null;
      const matching = [...frames.values()].filter(
        (value) => fullFrameURL(value) === selectedURL,
      );
      assert(
        matching.length > 0 && matching.length <= 64,
        `No bounded FrameTree URL match for the selected preview: ${JSON.stringify({ selectedURL, observedFrames, ownerBackendNodeId: owner.backendNodeId, ownerFrameField })}`,
      );
      const owned = [];
      for (const candidate of matching) {
        const ownerResult = await session.send("DOM.getFrameOwner", {
          frameId: candidate.id,
        });
        if (ownerResult.backendNodeId === owner.backendNodeId)
          owned.push(candidate);
      }
      assert.equal(
        owned.length,
        1,
        `The selected preview URL has no unique matching DOM frame owner: ${JSON.stringify({ selectedURL, observedFrames, ownerBackendNodeId: owner.backendNodeId, ownerFrameField })}`,
      );
      targetFrameId = owned[0].id;
      targetDocument = owner.contentDocument;
    }
    const targetFrame = frames.get(targetFrameId);
    assert(
      targetFrame && fullFrameURL(targetFrame) === selectedURL,
      `The CDP frame identity does not match the selected preview URL: ${JSON.stringify({ selectedURL, targetFrameId, observedFrames, frameOwnerBackendNodeId, ownerFrameField })}`,
    );
    const canvases = [];
    let inspected = 0;
    const visit = (node) => {
      assert(
        ++inspected <= 100000,
        "The preview DOM exceeds the bounded attribution limit.",
      );
      if (node.nodeName === "CANVAS" && attrs(node).id === "world")
        canvases.push({
          backendNodeId: node.backendNodeId,
          nodeId: node.nodeId,
          frameId: targetFrameId,
          frameURL: fullFrameURL(targetFrame),
          frameOwnerBackendNodeId,
          frameDocumentBackendNodeId: targetDocument.backendNodeId,
          ownerFrameField,
          cdpSessionScope: sharedPageSession ? "shared-page" : "frame",
        });
      // The selected document is explicit; nested frame documents cannot substitute a canvas.
      for (const child of [
        ...(node.children ?? []),
        ...(node.shadowRoots ?? []),
      ])
        visit(child);
    };
    visit(targetDocument);
    assert.equal(
      canvases.length,
      1,
      "A unique DOM canvas in the selected preview frame is required before tracing.",
    );
    const canvas = canvases[0];
    let layers = [];
    session.on("LayerTree.layerTreeDidChange", (event) => {
      layers = event.layers ?? [];
    });
    await session.send("LayerTree.enable");
    trace = await startCompositorTrace(
      browser,
      resolve(directory, `compositor-layers-${label}-${randomUUID()}.pftrace`),
      { layers: true },
    );
    await page.waitForTimeout(500);
    const raw = await trace.stop();
    trace = undefined;
    const layerIds = layers
      .filter((layer) => layer.backendNodeId === canvas.backendNodeId)
      .map((layer) => layer.layerId);
    return {
      ...canvas,
      layerIds,
      trace: raw,
      layerTreeObserved: layers.length > 0,
      target: await session
        .send("Target.getTargetInfo")
        .then((value) => ({
          targetId: value.targetInfo.targetId,
          type: value.targetInfo.type,
          url: value.targetInfo.url,
        }))
        .catch(() => null),
    };
  } finally {
    await trace?.abort();
    await session.send("LayerTree.disable").catch(() => {});
    await session.detach();
  }
}

function recordingClock(page) {
  const workers = page
    .workers()
    .filter((worker) => /preview.*worker|worker.*preview/i.test(worker.url()));
  assert.equal(
    workers.length,
    1,
    "A unique preview worker clock is required; direct HTML-canvas attribution is not implemented by this prototype.",
  );
  return workers[0];
}
const benchmark = (page, options) =>
  page.evaluate(
    (options) => window.aethyrforge.cTownBenchmark(options),
    options,
  );
export async function recordCompositorWindow({
  browser,
  page,
  directory,
  label,
  durationMs,
  warmupMs = 5000,
  instrumented = false,
  targetHz = "display",
  round = 0,
  order = 0,
}) {
  assert(["off", "on"].includes(label));
  assert(
    Number.isInteger(durationMs) && durationMs >= 500 && durationMs <= 60000,
  );
  assert([60, "display"].includes(targetHz));
  const clock = recordingClock(page),
    id = randomUUID();
  await benchmark(page, {
    op: "start",
    route: "static",
    instrumented,
    targetHz,
    capacity: 20000,
  });
  await page.waitForTimeout(warmupMs);
  await benchmark(page, { op: "stop", drain: true });
  let trace;
  const markers = {
    start: `af.compositor.${id}.start`,
    end: `af.compositor.${id}.end`,
  };
  try {
    if (label === "on") {
      trace = await startCompositorTrace(
        browser,
        resolve(directory, `compositor-${id}.pftrace`),
      );
      await page.waitForTimeout(250);
    }
    const started = await benchmark(page, {
      op: "start",
      route: "static",
      instrumented,
      targetHz,
      capacity: 20000,
    });
    if (trace)
      await clock.evaluate(
        ({ name, time }) => performance.mark(name, { startTime: time }),
        { name: markers.start, time: started.observation.startedAtMs },
      );
    await page.waitForTimeout(durationMs);
    const stopped = await benchmark(page, { op: "stop", drain: true });
    if (trace)
      await clock.evaluate(
        ({ name, time }) => performance.mark(name, { startTime: time }),
        { name: markers.end, time: stopped.observation.stoppedAtMs },
      );
    await page.waitForTimeout(250);
    const metrics = await benchmark(page, { op: "read", drain: true });
    const raw = trace ? await trace.stop() : null;
    trace = undefined;
    const path = resolve(directory, `compositor-${id}-frames.json`);
    await save(path, {
      schema: "aethyrforge.compositor-application-window.v1",
      metrics,
    });
    const bytes = await readFile(path);
    return {
      id,
      label,
      round,
      order,
      markers,
      durationMs:
        metrics.observation.stoppedAtMs - metrics.observation.startedAtMs,
      warmupMs,
      instrumented,
      targetHz,
      clock: {
        url: clock.url(),
        basis:
          "Trace user-timing markers explicitly placed at the actual worker recorder boundaries",
      },
      submittedFrames: metrics.samples.filter((sample) => sample.submitted)
        .length,
      application: { path, sha256: hash(bytes), bytes: bytes.length },
      trace: raw,
    };
  } finally {
    await trace?.abort();
    await clock
      .evaluate(
        (names) => names.forEach((name) => performance.clearMarks(name)),
        Object.values(markers),
      )
      .catch(() => {});
    await benchmark(page, { op: "stop", drain: true }).catch(() => {});
  }
}

export async function decodeCompositorTrace(trace, { processor, output }) {
  const input = await readFile(trace.path);
  assert.equal(input.length, trace.bytes);
  assert.equal(hash(input), trace.sha256, "The private trace changed.");
  const decoder = fileURLToPath(
    new URL("./compositor-decode.py", import.meta.url),
  );
  const { stdout } = await exec(
    "python3",
    [decoder, "--processor", processor, "--trace", trace.path, "--out", output],
    { timeout: 100000, maxBuffer: 8192 },
  );
  return {
    reference: JSON.parse(stdout),
    decoded: JSON.parse(await readFile(output, "utf8")),
  };
}

/** Decode after closing the GPU world so trace processing cannot perturb later windows. */
export async function analyzeCompositorSession(
  report,
  { processor, directory, mode = "diagnostic" },
) {
  const before = await decodeCompositorTrace(report.canvas.before.trace, {
    processor,
    output: resolve(directory, "compositor-before-decoded.json"),
  });
  const after = await decodeCompositorTrace(report.canvas.after.trace, {
    processor,
    output: resolve(directory, "compositor-after-decoded.json"),
  });
  const first = identifyCanvasSurface(before.decoded, {
    ...report.canvas.before,
    dataLossOccurred: report.canvas.before.trace.dataLossOccurred,
  });
  const last = identifyCanvasSurface(after.decoded, {
    ...report.canvas.after,
    dataLossOccurred: report.canvas.after.trace.dataLossOccurred,
  });
  assert.deepEqual(
    first.frameSink,
    last.frameSink,
    "The canvas surface changed during these windows.",
  );
  assert.equal(first.rendererPid, last.rendererPid);
  assert.equal(first.backendNodeId, last.backendNodeId);
  report.canvas.attribution = first;
  report.canvas.decodedEvidence = [before.reference, after.reference];
  for (const row of report.windows.filter((row) => row.trace)) {
    const decoded = await decodeCompositorTrace(row.trace, {
      processor,
      output: resolve(directory, `compositor-${row.id}-decoded.json`),
    });
    row.decoded = decoded.reference;
    row.compositor = analyzeCompositorTrace(decoded.decoded, {
      canvas: first,
      startMark: row.markers.start,
      endMark: row.markers.end,
      dataLossOccurred: row.trace.dataLossOccurred,
      mode,
      expectedSubmittedFrames: row.submittedFrames,
      headless: report.browser.headless,
      warmupMs: row.warmupMs,
    });
  }
  report.attributed = report.windows
    .filter((row) => row.trace)
    .every((row) => row.compositor?.attributed);
  report.compositorPacingQualified =
    report.browser.headless === false &&
    report.windows.filter((row) => row.trace).length >= 2 &&
    report.windows
      .filter((row) => row.trace)
      .every((row) => row.compositor?.pacingQualified);
  report.existingFenceGatesChanged = false;
  return report;
}
