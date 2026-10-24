/** Guest-only character and lifecycle check, adapted from OpenAgents' public demo checker. */
import assert from "node:assert/strict";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

assert(
  process.env.AETHYRFORGE_RELEASE_CONFIG,
  "Set AETHYRFORGE_RELEASE_CONFIG.",
);
const config = JSON.parse(
  await readFile(process.env.AETHYRFORGE_RELEASE_CONFIG, "utf8"),
);
const mode = process.env.AETHYRFORGE_CHECK_MODE || "production";
assert(["production", "candidate"].includes(mode));
const host = new URL(
  process.env.AF_HOST_URL || "https://openagents.com/demo/medieval-town",
);
const editor = new URL(config.editor),
  preview = new URL(config.preview);
assert.equal(
  new Set([host.origin, editor.origin, preview.origin]).size,
  3,
  "The public host, editor, and preview need separate origins.",
);
for (const url of [host, editor, preview]) {
  assert(
    url.protocol === "https:" ||
      (url.protocol === "http:" &&
        ["localhost", "127.0.0.1"].includes(url.hostname)),
  );
  assert(
    !url.username && !url.password,
    "Verification URLs cannot contain credentials.",
  );
}
const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
const destination =
  process.env.AF_CAPTURE_DIR || process.env.AETHYRFORGE_DEPLOY_SCRATCH;
assert(
  destination,
  "Set a private AF_CAPTURE_DIR or AETHYRFORGE_DEPLOY_SCRATCH.",
);
await mkdir(resolve(destination), { recursive: true });
const output = await realpath(resolve(destination));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside Git.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run through openagents browser run.");
const report = {
  schema: "aethyrforge.character-production-browser.v1",
  mode,
  release: config.release,
  url: host.href,
  startedAt: new Date().toISOString(),
  viewport: [1280, 720],
  ok: false,
  checks: [],
  captures: [],
  errors: [],
  failed: [],
  credentialViolations: [],
  blockedWrites: [],
  privateRequests: 0,
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  serviceWorkers: "block",
});
const page = await context.newPage();
const targets = await browser.newBrowserCDPSession();
page.setDefaultTimeout(30_000);
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (message.type() === "error")
    report.errors.push(message.text().slice(0, 4096));
});
page.on("response", (response) => {
  if (response.status() >= 400)
    report.failed.push({ status: response.status(), url: response.url() });
});
page.on("requestfailed", (request) =>
  report.failed.push({
    error: request.failure()?.errorText,
    url: request.url(),
  }),
);
const audits = [];
page.on("request", (request) => {
  const url = new URL(request.url());
  if (![editor.origin, preview.origin].includes(url.origin)) return;
  report.privateRequests++;
  audits.push(
    request.allHeaders().then((headers) => {
      for (const name of ["authorization", "cookie"])
        if (headers[name])
          report.credentialViolations.push({ name, url: request.url() });
    }),
  );
});
// The check exercises local movement and view state; no account or cloud write is needed.
await page.route("**/*", async (route) => {
  if (!["GET", "HEAD", "OPTIONS"].includes(route.request().method())) {
    report.blockedWrites.push({
      method: route.request().method(),
      url: route.request().url(),
    });
    await route.abort("blockedbyclient");
  } else await route.continue();
});
if (mode === "candidate") {
  assert(
    process.env.AETHYRFORGE_HOST_RESPONSE_URL,
    "Set AETHYRFORGE_HOST_RESPONSE_URL for a candidate host.",
  );
  const candidate = new URL(process.env.AETHYRFORGE_HOST_RESPONSE_URL);
  assert(!candidate.username && !candidate.password);
  await page.route(host.origin + host.pathname + "**", async (route) => {
    const requested = new URL(route.request().url());
    const response = await context.request.get(
      new URL(requested.pathname + requested.search, candidate).href,
    );
    const headers = { ...response.headers() };
    for (const name of [
      "content-encoding",
      "content-length",
      "transfer-encoding",
    ])
      delete headers[name];
    await route.fulfill({
      status: response.status(),
      headers,
      body: await response.body(),
    });
  });
}
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
async function hostSnapshot() {
  return page.evaluate(() => JSON.parse(window.medievalDemo.snapshot()));
}
function editorFrame() {
  const frame = page.frames().find((frame) => {
    try {
      return new URL(frame.url()).origin === editor.origin;
    } catch {
      return false;
    }
  });
  assert(frame, "The public host did not mount its editor.");
  return frame;
}
function previewFrame() {
  const frame = page.frames().find((frame) => {
    try {
      return new URL(frame.url()).origin === preview.origin;
    } catch {
      return false;
    }
  });
  assert(frame, "The editor did not mount its isolated preview.");
  return frame;
}
async function status() {
  return editorFrame().evaluate(() => window.aethyrforge.snapshot().status);
}
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  throw Error(message);
}
async function ready() {
  await page.waitForFunction(
    () => {
      if (!window.medievalDemo) return false;
      const value = JSON.parse(window.medievalDemo.snapshot());
      return (
        value.phase === "open" ||
        (value.phase === "closed" && Boolean(value.error))
      );
    },
    null,
    { timeout: 180_000 },
  );
  const host = await hostSnapshot();
  assert.equal(host.phase, "open", JSON.stringify(host));
  assert.equal(host.error, null, JSON.stringify(host));
  return until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return (
        value.phase === "ready" &&
        value.frame > 2 &&
        value.character?.modelLoaded &&
        value.character.grounded &&
        value
      );
    },
    "The character model and walking preview did not become ready",
    180_000,
  );
}
async function workerCount(count) {
  return until(
    async () => {
      const workers = (
        await targets.send("Target.getTargets")
      ).targetInfos.filter(
        (target) =>
          target.type === "worker" &&
          target.url.startsWith(preview.origin + "/"),
      );
      return workers.length === count && true;
    },
    `Expected ${count} live preview workers`,
    15_000,
  );
}
async function capture(name) {
  const path = resolve(output, `character-${mode}-${name}.png`);
  await page.screenshot({ path });
  report.captures.push(path);
}
async function releaseInput() {
  for (const key of [
    "KeyW",
    "KeyA",
    "KeyS",
    "KeyD",
    "Space",
    "ShiftLeft",
    "ShiftRight",
  ])
    await page.keyboard.up(key);
  await page.keyboard.press("Escape");
  await previewFrame()
    .evaluate(() => document.exitPointerLock())
    .catch(() => {});
}
async function focusWorld() {
  await previewFrame()
    .locator("#world")
    .click({ position: { x: 500, y: 300 } });
}

try {
  report.browser = browser.version();
  assert.deepEqual(await context.cookies(), []);
  const begun = performance.now();
  await page.goto(host.href, { waitUntil: "domcontentloaded" });
  const initial = await ready();
  const pinned = JSON.parse(await page.locator("#engine-config").textContent());
  assert.equal(
    pinned.release,
    config.release,
    "The host pins a different release.",
  );
  report.readyMs = performance.now() - begun;
  assert.equal(initial.view.mode, "walk");
  assert.equal(initial.character.perspective, "third");
  assert(
    horizontal(initial.camera, initial.character.position) > 0.2,
    "Third person must start outside the player.",
  );
  assert(
    await editorFrame()
      .getByRole("navigation", { name: "Play controls", exact: true })
      .isVisible(),
    "The public demo must show its play controls.",
  );
  assert.equal(
    await editorFrame()
      .getByRole("navigation", { name: "World tools", exact: true })
      .count(),
    0,
    "The public demo must omit authoring tools.",
  );
  for (const name of ["Assets", "Build", "Shader"]) {
    assert.equal(
      await editorFrame().getByRole("button", { name, exact: true }).count(),
      0,
      `The public demo must omit the ${name} tool.`,
    );
  }
  assert(
    await editorFrame()
      .getByRole("button", { name: "Switch to first person", exact: true })
      .isVisible(),
    "The public demo must offer the perspective button.",
  );
  await workerCount(1);
  record(
    "Guest host opens the walking Druid in third person with a single preview worker",
    { readyMs: report.readyMs, character: initial.character },
  );
  await capture("third-person");

  const beforePerformance = await status();
  const measuredAt = performance.now(),
    frameRates = [];
  for (let i = 0; i < 12; i++) {
    await page.waitForTimeout(250);
    frameRates.push((await status()).fps);
  }
  const afterPerformance = await status(),
    seconds = (performance.now() - measuredAt) / 1000;
  record("Public animated-world frame rate measured", {
    seconds,
    submittedFrames: afterPerformance.frame - beforePerformance.frame,
    submissionFps: (afterPerformance.frame - beforePerformance.frame) / seconds,
    reportedFps: frameRates,
  });

  await focusWorld();
  await page.keyboard.press("KeyV");
  const first = await until(async () => {
    const value = await status();
    return (
      value.character?.perspective === "first" &&
      horizontal(value.camera, value.character.position) < 0.06 &&
      Math.abs(value.camera[1] - value.character.position[1] - 1.7) < 0.06 &&
      value
    );
  }, "The public preview did not switch to first person on V");
  assert(
    distance(first.character.position, initial.character.position) < 0.06,
    "A view change moved the feet.",
  );
  await releaseInput();
  await capture("first-person");
  await focusWorld();
  await page.keyboard.press("KeyV");
  const third = await until(async () => {
    const value = await status();
    return (
      value.character?.perspective === "third" &&
      horizontal(value.camera, value.character.position) > 0.2 &&
      value
    );
  }, "The public preview did not return to third person on V");
  assert(distance(first.character.position, third.character.position) < 0.06);
  record(
    "Real V input switches first and third person without moving the player",
  );
  await releaseInput();
  await editorFrame()
    .getByRole("button", { name: "Switch to first person", exact: true })
    .click();
  const buttonFirst = await until(async () => {
    const value = await status();
    return (
      value.character?.perspective === "first" &&
      horizontal(value.camera, value.character.position) < 0.06 &&
      Math.abs(value.camera[1] - value.character.position[1] - 1.7) < 0.06 &&
      value
    );
  }, "The play-controls button did not switch to first person");
  await editorFrame()
    .getByRole("button", { name: "Switch to third person", exact: true })
    .click();
  const buttonThird = await until(async () => {
    const value = await status();
    return (
      value.character?.perspective === "third" &&
      horizontal(value.camera, value.character.position) > 0.2 &&
      value
    );
  }, "The play-controls button did not return to third person");
  assert(
    distance(buttonFirst.character.position, buttonThird.character.position) <
      0.06,
  );
  assert(
    distance(third.character.position, buttonThird.character.position) < 0.06,
  );
  record(
    "Visible public play controls switch first and third person without authoring tools or player movement",
  );
  await focusWorld();
  const orbit = third.character.orbitDistance;
  await page.mouse.wheel(0, -180);
  const zoomed = await until(async () => {
    const value = await status();
    return value.character.orbitDistance < orbit - 0.01 && value;
  }, "The public preview did not zoom on the mouse wheel");
  assert(
    zoomed.character.orbitDistance >= 2.5 &&
      zoomed.character.orbitDistance <= 40,
  );
  assert(distance(zoomed.character.position, third.character.position) < 0.06);
  await releaseInput();
  record("Mouse wheel changes the bounded follow distance", {
    before: orbit,
    after: zoomed.character.orbitDistance,
  });

  const beforeMove = await status();
  await focusWorld();
  await page.keyboard.down("KeyW");
  try {
    await page.waitForTimeout(800);
  } finally {
    await page.keyboard.up("KeyW");
  }
  await releaseInput();
  const moved = await until(async () => {
    const value = await status();
    return (
      value.frame > beforeMove.frame + 2 &&
      horizontal(value.character.position, beforeMove.character.position) >
        0.2 &&
      value
    );
  }, "Real W input did not move the player");
  assert(moved.character.grounded, "Walking ended without ground support.");
  record("Real W input moves the player through the hosted town", {
    before: beforeMove.character.position,
    after: moved.character.position,
    distance: horizontal(
      beforeMove.character.position,
      moved.character.position,
    ),
  });

  await page.waitForTimeout(500);
  const beforeReload = await hostSnapshot(),
    feetBeforeReload = (await status()).character.position;
  assert.equal(beforeReload.status.checkpoint.mode, "walk");
  await page.locator("#world-retry").click();
  await page.waitForFunction(
    (session) =>
      window.medievalDemo &&
      JSON.parse(window.medievalDemo.snapshot()).phase === "open" &&
      JSON.parse(window.medievalDemo.snapshot()).session !== session,
    beforeReload.session,
    { timeout: 180_000 },
  );
  const restored = await ready(),
    hostRestored = await hostSnapshot();
  assert(
    distance(restored.character.position, feetBeforeReload) < 0.06,
    JSON.stringify({
      before: feetBeforeReload,
      after: restored.character.position,
    }),
  );
  for (const field of ["eye", "target"])
    assert(
      distance(
        hostRestored.status.checkpoint[field],
        beforeReload.status.checkpoint[field],
      ) < 0.06,
      `Reload changed ${field}.`,
    );
  assert.notEqual(
    hostRestored.status.generation,
    beforeReload.status.generation,
  );
  await workerCount(1);
  await capture("restored-third-person");
  record(
    "Reload replaces the worker and restores the walking player's position and checkpoint",
  );

  await page.goto("about:blank");
  await workerCount(0);
  record("Leaving the public demo disposes its preview worker");
  await page.goBack({ waitUntil: "domcontentloaded" });
  const reopened = await ready(),
    hostReopened = await hostSnapshot();
  assert.notEqual(hostReopened.session, hostRestored.session);
  await workerCount(1);
  record("Back opens a fresh walking session", {
    session: hostReopened.session,
    character: reopened.character,
  });
  report.finalStatus = reopened;
  await page.close();
  await workerCount(0);
  await Promise.all(audits);
  assert.deepEqual(
    report.credentialViolations,
    [],
    "Credentials reached a private origin.",
  );
  assert.deepEqual(
    report.blockedWrites,
    [],
    "The guest walkthrough attempted a cloud write.",
  );
  assert.deepEqual(report.errors, [], "The browser reported an error.");
  assert.deepEqual(report.failed, [], "A request failed.");
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error);
  process.exitCode = 1;
  console.error(report.failure);
  if (!page.isClosed()) {
    try {
      report.finalStatus = await status();
    } catch {}
    try {
      await capture("failure");
    } catch {}
  }
} finally {
  if (!page.isClosed()) {
    await releaseInput().catch(() => {});
    await page.close().catch(() => {});
  }
  report.finishedAt = new Date().toISOString();
  await writeFile(
    resolve(output, `character-production-${mode}.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  await targets.detach().catch(() => {});
  await context.close().catch(() => {});
  await browser.close();
}
