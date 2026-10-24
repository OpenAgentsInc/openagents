// Isolated browser verification. Product state and transport remain Rust/Wasm.
import assert from "node:assert/strict";
import fs from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const out = process.env.AETHYRFORGE_DEPLOY_SCRATCH;
const config = JSON.parse(
  fs.readFileSync(process.env.AETHYRFORGE_RELEASE_CONFIG),
);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = browser.contexts()[0];
const browserTargets = await browser.newBrowserCDPSession();
async function liveWorkers() {
  return (await browserTargets.send("Target.getTargets")).targetInfos.filter(
    (t) => t.type === "worker" && t.url.startsWith(config.preview + "/"),
  );
}
async function expectWorkers(count) {
  const until = Date.now() + 10000;
  let targets;
  do {
    targets = await liveWorkers();
    if (targets.length === count) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  } while (Date.now() < until);
  assert.equal(targets.length, count, JSON.stringify(targets));
}
const origin = "https://openagents.com",
  path = "/new-lander/house";
function hostConfig(c) {
  return {
    release: c.release,
    engine_abi: "aethyrforge.browser.v1",
    project_format: 1,
    project: { id: "haunted-house", branch: "main", revision: 0 },
  };
}
async function open(
  c,
  mode = process.env.AETHYRFORGE_CHECK_MODE || "production",
) {
  const page = await context.newPage();
  const errors = [],
    failed = [];
  let requests = 0;
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("response", (r) => {
    if (r.status() >= 400) failed.push([r.status(), r.url()]);
  });
  page.on("requestfailed", (r) =>
    failed.push([r.failure()?.errorText, r.url()]),
  );
  page.on("request", (r) => {
    if (r.url().startsWith(c.editor) || r.url().startsWith(c.preview)) {
      requests++;
      assert.equal(r.headers().authorization, undefined);
      assert.equal(r.headers().cookie, undefined);
    }
  });
  if (mode === "harness") {
    await page.route(origin + path, (route) =>
      route.fulfill({
        contentType: "text/html",
        body: `<!doctype html><title>Host SDK conformance</title><a href=/>OpenAgents</a><a id=world-retry href=${path}>Reload world</a><button id=world-fullscreen>Full screen</button><iframe id=engine title=World data-engine-url="${c.editor}/" style="width:100%;height:80vh;border:0"></iframe><p id=world-notice role=status>Opening</p><script id=engine-config type=application/json>${JSON.stringify(hostConfig(c))}</script><script type=module>import init,{mount_demo} from '${c.editor}/sdk/${c.sdk_sha256}/aethyrforge_host.js';await init();window.hauntDemo=mount_demo();</script>`,
      }),
    );
  } else if (mode === "candidate") {
    const candidate = process.env.AETHYRFORGE_HOST_RESPONSE_URL;
    await page.route(origin + "/new-lander/**", async (route) => {
      const url = new URL(route.request().url());
      const response = await context.request.get(
        candidate + url.pathname + url.search,
      );
      const headers = { ...response.headers() };
      delete headers["content-encoding"];
      delete headers["content-length"];
      delete headers["transfer-encoding"];
      await route.fulfill({
        status: response.status(),
        headers,
        body: await response.body(),
      });
    });
  }
  const started = Date.now();
  await page.goto(origin + path, { waitUntil: "domcontentloaded" });
  try {
    await page.waitForFunction(
      () =>
        window.hauntDemo &&
        ["open", "closed"].includes(
          JSON.parse(window.hauntDemo.snapshot()).phase,
        ),
      null,
      { timeout: 125000 },
    );
  } catch (e) {
    console.log(
      JSON.stringify({
        errors,
        failed,
        body: await page.locator("body").innerText(),
      }),
    );
    throw e;
  }
  const snapshot = () =>
    page.evaluate(() => JSON.parse(window.hauntDemo.snapshot()));
  let s = await snapshot();
  if (s.phase !== "open")
    console.log(JSON.stringify({ snapshot: s, errors, failed }));
  assert.equal(s.phase, "open");
  assert.equal(s.error, null);
  assert.deepEqual(errors, []);
  assert.deepEqual(failed, []);
  await page.waitForFunction(
    () => JSON.parse(window.hauntDemo.snapshot()).status.frame > 2,
  );
  s = await snapshot();
  assert.ok(s.status.checkpoint);
  assert.equal(
    await page.locator("[data-aethyr-id=world-notice]").isVisible(),
    false,
  );
  return {
    page,
    snapshot,
    before: s,
    ready_ms: Date.now() - started,
    errors,
    failed,
    mode,
    requests: () => requests,
  };
}
try {
  const run = await open(config);
  const { page, snapshot } = run;
  // Real input reaches the private preview's Rust camera kernel.
  const frame = page.frames().find((f) => f.url().startsWith(config.preview));
  assert.ok(frame);
  await frame.locator("#world").click();
  const position = (await snapshot()).status.checkpoint.eye;
  await page.keyboard.down("w");
  try {
    await page.waitForFunction(
      (p) =>
        JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye.some(
          (v, i) => Math.abs(v - p[i]) > 0.1,
        ),
      position,
      { timeout: 10000 },
    );
  } finally {
    await page.keyboard.up("w");
  }
  await page.keyboard.press("Escape");
  const moved = await snapshot();
  console.log("Host connected and movement passed:", config.release);
  await page.click("[data-aethyr-id=world-fullscreen]");
  await page.waitForFunction(() => document.fullscreenElement !== null);
  await page.evaluate(() => document.exitFullscreen());
  // Allow the key-up and its next observation to reach the camera before sampling.
  await page.waitForTimeout(1000);
  const beforeReload = await snapshot();
  await page.click("[data-aethyr-id=world-retry]");
  await page.waitForFunction(
    (session) => {
      const s = JSON.parse(window.hauntDemo.snapshot());
      return s.phase === "open" && s.session !== session;
    },
    beforeReload.session,
    { timeout: 125000 },
  );
  const restored = await snapshot();
  for (let i = 0; i < 3; i++)
    assert.ok(
      Math.abs(
        restored.status.checkpoint.eye[i] -
          beforeReload.status.checkpoint.eye[i],
      ) < 0.05,
      JSON.stringify({
        before: beforeReload.status.checkpoint,
        after: restored.status.checkpoint,
      }),
    );
  assert.notEqual(restored.status.generation, beforeReload.status.generation);
  // CDP lists live targets; Playwright can retain stale handles after cross-origin iframe reload.
  await expectWorkers(1);
  await page.screenshot({ path: out + "/house-" + run.mode + ".png" });
  assert.equal(restored.status.save, "unsaved");
  await page.click("[data-aethyr-id=world-map]");
  await page.waitForURL("**/new-lander/verse?world=haunted-house");
  await expectWorkers(0);
  await page.goBack({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(
    () =>
      window.hauntDemo &&
      JSON.parse(window.hauntDemo.snapshot()).phase === "open",
    null,
    { timeout: 125000 },
  );
  assert.notEqual((await snapshot()).session, restored.session);
  console.log("Back navigation reopened the world.");
  const result = {
    mode: run.mode,
    browser: browser.version(),
    release: config.release,
    ready_ms: run.ready_ms,
    requests_without_account_credentials: run.requests(),
    input_moved_player: true,
    reload_restored_checkpoint: true,
    close_removed_worker: true,
    map_return_and_back_reopened: true,
    fullscreen_worked: true,
    errors: run.errors,
    failed: run.failed,
    status: restored.status,
  };
  console.log(JSON.stringify(result, null, 2));
  await page.close();
  for (const failure of ["missing-sdk", "incompatible-release"]) {
    const check = await context.newPage();
    if (failure === "missing-sdk")
      await check.route(config.editor + "/sdk/**/aethyrforge_host.js", (r) =>
        r.fulfill({ status: 404, body: "Unavailable" }),
      );
    const responseOrigin =
      run.mode === "candidate"
        ? process.env.AETHYRFORGE_HOST_RESPONSE_URL
        : origin;
    // Every request to the site, not only the page: its scripts and styles
    // must come from the same website as the page under test.
    await check.route(origin + "/**", async (route) => {
      const url = new URL(route.request().url()),
        response = await context.request.get(
          responseOrigin + url.pathname + url.search,
        );
      const headers = { ...response.headers() };
      delete headers["content-encoding"];
      delete headers["content-length"];
      delete headers["transfer-encoding"];
      let body = await response.body();
      if (failure === "incompatible-release" && url.pathname === path) {
        const html = body.toString();
        const tag =
          /<script id="engine-config" type="application\/json">([^<]+)<\/script>/;
        assert.match(html, tag);
        body = Buffer.from(
          html.replace(
            tag,
            (_, text) =>
              '<script id="engine-config" type="application/json">' +
              JSON.stringify({
                ...JSON.parse(text),
                release: "missing-release",
              }) +
              "</script>",
          ),
        );
      }
      await route.fulfill({ status: response.status(), headers, body });
    });
    await check.goto(origin + path, { waitUntil: "domcontentloaded" });
    // The notice offers both, at every width; the frame stays out of sight.
    await check
      .locator("[data-aethyr-id=world-notice-retry]")
      .waitFor({ timeout: 125000 });
    assert.ok(
      await check.locator("[data-aethyr-id=world-notice-map]").isVisible(),
    );
    assert.equal(
      await check.locator("#engine").evaluate((e) => getComputedStyle(e).visibility),
      "hidden",
    );
    console.log("Unavailable engine offers retry/back:", failure);
    await check.close();
  }
  result.missing_sdk_retry = true;
  result.incompatible_release_retry = true;
  fs.writeFileSync(
    out + "/browser-" + run.mode + ".json",
    JSON.stringify(result, null, 2),
  );
} finally {
  await browser.close();
}
