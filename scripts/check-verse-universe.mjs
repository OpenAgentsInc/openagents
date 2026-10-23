// Native Rust pages and the pinned private universe, in an isolated browser profile.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const out = process.env.AETHYRFORGE_DEPLOY_SCRATCH;
assert.ok(out && process.env.OPENAGENTS_TEST_WEB_BINARY);
const config = JSON.parse(
  fs.readFileSync(process.env.AETHYRFORGE_RELEASE_CONFIG),
);
fs.mkdirSync(out, { recursive: true });
const isolatedHome = path.join(out, "browser-native-home");
fs.mkdirSync(isolatedHome, { recursive: true });
const server = spawn(
  process.env.OPENAGENTS_TEST_WEB_BINARY,
  [
    "--listen",
    "127.0.0.1:0",
    "--store",
    path.join(out, "tasks"),
    "--chat-store",
    path.join(out, "chats"),
    "--pylons-relay",
    "off",
    "--profile-relay",
    "off",
  ],
  { env: { ...process.env, HOME: isolatedHome } },
);
process.on("exit", () => server.kill("SIGTERM"));
let log = "";
server.stdout.on("data", (d) => (log += d));
server.stderr.on("data", (d) => (log += d));
const until = Date.now() + 60000;
let base;
while (Date.now() < until) {
  base = /listening on (http:\/\/[^ ]+)/.exec(log)?.[1];
  if (base) break;
  if (server.exitCode !== null) throw Error(log);
  await new Promise((r) => setTimeout(r, 50));
}
assert.ok(base, log);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = browser.contexts()[0];
const errors = [],
  privateRequests = [];
let page;
const snapshots = [];
try {
  await context.route("https://openagents.com/**", async (route) => {
    const request = route.request(),
      url = new URL(request.url());
    const response = await context.request.fetch(
      base + url.pathname + url.search,
      {
        headers: { ...request.headers(), host: new URL(base).host, cookie: "" },
        maxRedirects: 0,
      },
    );
    const headers = { ...response.headers() };
    for (const key of [
      "content-length",
      "content-encoding",
      "transfer-encoding",
    ])
      delete headers[key];
    await route.fulfill({
      status: response.status(),
      headers,
      body: await response.body(),
    });
  });
  const open = async () => {
    const p = await context.newPage();
    p.on("pageerror", (e) => errors.push(e.message));
    p.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text());
    });
    p.on("request", (r) => {
      if (r.url().includes("---aethyrforge-")) {
        privateRequests.push(r.url());
        assert.equal(r.headers().authorization, undefined);
        assert.equal(r.headers().cookie, undefined);
      }
    });
    return p;
  };
  const snapshot = (p) =>
    p.evaluate(() => JSON.parse(window.universeDemo.snapshot()));
  const ready = (p) =>
    p.waitForFunction(
      () =>
        window.universeDemo &&
        JSON.parse(window.universeDemo.snapshot()).phase === "open" &&
        JSON.parse(window.universeDemo.snapshot()).status.frame > 0,
      null,
      { timeout: 60000 },
    );
  const cameraControls = async (p, frame) => {
    assert.equal(await p.locator(".map-home").count(),0);
    assert.ok(await p.locator(".site-header .back-link").isVisible());
    const plus=frame.locator("#zoom-in");
    const button=await plus.boundingBox();assert.ok(button);
    const hit=await p.evaluate(({x,y})=>document.elementFromPoint(x,y)?.id,{x:button.x+button.width/2,y:button.y+button.height/2});
    assert.equal(hit,"engine");
    const before=await pose(p);const distance=c=>Math.hypot(...c.eye.map((n,i)=>n-c.target[i]));
    await plus.click();await p.waitForTimeout(250);
    assert.equal(new URL(p.url()).pathname,"/new-lander/verse");
    assert.ok(distance(await pose(p))<distance(before));
    await frame.locator("#reset").click();
    assert.equal(await frame.evaluate(()=>document.activeElement.id),"reset");
    await frame.locator("#universe").focus();
  };
  const pose = (p) =>
    p.evaluate(
      () => JSON.parse(window.universeDemo.snapshot()).status.checkpoint,
    );
  const yaw = (c) => Math.atan2(c.eye[0] - c.target[0], c.eye[2] - c.target[2]);
  page = await open();
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => {
    window.verseStartProbe = { gpu: 0, microphone: 0 };
    if (navigator.gpu) {
      const request = navigator.gpu.requestAdapter.bind(navigator.gpu);
      navigator.gpu.requestAdapter = (...args) => {
        window.verseStartProbe.gpu++;
        return request(...args);
      };
    }
    if (navigator.mediaDevices) {
      const get = navigator.mediaDevices.getUserMedia.bind(
        navigator.mediaDevices,
      );
      navigator.mediaDevices.getUserMedia = (...args) => {
        window.verseStartProbe.microphone++;
        return get(...args);
      };
    }
  });
  await page.goto("https://openagents.com/new-lander");
  await page.waitForTimeout(500);
  assert.equal(privateRequests.length, 0);
  assert.deepEqual(await page.evaluate(() => window.verseStartProbe), {
    gpu: 0,
    microphone: 0,
  });
  assert.equal(
    await page.locator("iframe,canvas,script[type=module]").count(),
    0,
  );
  await page.getByRole("link", { name: "Enter the Verse" }).first().click();
  await ready(page);
  assert.equal((await snapshot(page)).status.project.id, "universe");
  const card = page.locator("#verse-entry");
  assert.ok(await card.isVisible());
  const box = await card.boundingBox();
  assert.ok(Math.abs(box.x + box.width / 2 - 720) < 3);
  assert.ok(Math.abs(box.y + box.height / 2 - 450) < 3);
  assert.equal(
    await page.locator("#universe-exploration").getAttribute("inert"),
    "",
  );
  const lockedBefore = await pose(page);
  await page.keyboard.press("ArrowLeft");
  await page.mouse.wheel(0, 300);
  await page.waitForTimeout(1100);
  const lockedAfter = await pose(page);
  assert.ok(Math.abs(yaw(lockedAfter) - yaw(lockedBefore)) < 0.03);
  assert.ok(Math.abs(yaw(lockedAfter) - yaw(lockedBefore)) > 0.0001);
  const distance = (c) => Math.hypot(...c.eye.map((n, i) => n - c.target[i]));
  assert.ok(Math.abs(distance(lockedAfter) - distance(lockedBefore)) < 0.001);
  await page.screenshot({ path: path.join(out, "universe-entry.png") });
  await page.click("#continue-guest");
  assert.ok(await page.locator("#verse-choices").isVisible());
  await page.click("#verse-explore");
  assert.equal(await card.isVisible(), false);
  assert.equal(
    await page.locator("#universe-exploration").getAttribute("inert"),
    null,
  );
  await page.locator("#option-haunted-house").focus();
  await page.keyboard.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector("#world-title").textContent === "Haunted House" &&
      JSON.parse(window.universeDemo.snapshot()).status.selection?.id ===
        "haunted-house",
  );
  assert.equal(
    await page.locator("#destination-openagents").isVisible(),
    false,
  );
  assert.equal(
    await page.locator("#destination-haunted-house a").getAttribute("href"),
    "/new-lander/house",
  );
  await page.click("#option-openagents");
  await page.waitForFunction(
    () => document.querySelector("#world-title").textContent === "OpenAgents",
  );
  assert.ok(
    await page
      .locator("#destination-openagents")
      .innerText()
      .then((t) => t.includes("Locked")),
  );
  assert.equal(
    await page
      .locator('#destination-openagents a[href="/new-lander/house"]')
      .count(),
    0,
  );
  const preview = page
    .frames()
    .find((f) => f.url().startsWith(config.preview + "/universe-preview.html"));
  assert.ok(preview);
  await preview.locator("#universe").click({ position: { x: 710, y: 500 } });
  await cameraControls(page,preview);
  const manualBefore = await pose(page);
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(500);
  const manualAfter = await pose(page);
  assert.ok(Math.abs(yaw(manualAfter) - yaw(manualBefore)) > 0.04);
  await page.click("#option-haunted-house");
  await page.waitForFunction(
    () =>
      JSON.parse(window.universeDemo.snapshot()).status.selection?.id ===
      "haunted-house",
  );
  await page.waitForTimeout(1200);
  const beforeHouse = await pose(page);
  const original = (await snapshot(page)).session;
  await page.screenshot({ path: path.join(out, "universe-explore.png") });
  await page.locator("#destination-haunted-house a").click();
  await page.waitForFunction(
    () =>
      window.hauntDemo &&
      JSON.parse(window.hauntDemo.snapshot()).phase === "open",
    null,
    { timeout: 125000 },
  );
  assert.equal(
    page
      .frames()
      .filter((f) => f.url().includes("/universe") && f !== page.mainFrame())
      .length,
    0,
  );
  await page.click("#world-map");
  await ready(page);
  await page.waitForFunction(
    () => JSON.parse(window.universeDemo.visit()).selected === "haunted-house",
  );
  assert.equal(await card.isVisible(), false);
  assert.notEqual((await snapshot(page)).session, original);
  const returned = await pose(page);
  for (let i = 0; i < 3; i++)
    assert.ok(Math.abs(returned.eye[i] - beforeHouse.eye[i]) < 0.25);
  snapshots.push(await snapshot(page));
  await page.close();
  // A fresh tab keeps its own entry choice; reduced motion applies to the real preview.
  const reduced = await open();
  page = reduced;
  await reduced.emulateMedia({ reducedMotion: "reduce" });
  await reduced.setViewportSize({ width: 390, height: 844 });
  await reduced.goto("https://openagents.com/new-lander/verse");
  await ready(reduced);
  const reducedPreview = reduced
    .frames()
    .find((f) => f.url().startsWith(config.preview + "/universe-preview.html"));
  assert.equal(
    await reducedPreview.evaluate(
      () => matchMedia("(prefers-reduced-motion: reduce)").matches,
    ),
    true,
  );
  const stillBefore = await pose(reduced);
  await reduced.waitForTimeout(1100);
  assert.deepEqual(await pose(reduced), stillBefore);
  assert.ok(
    await reduced.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  );
  await reduced.screenshot({
    path: path.join(out, "universe-mobile-entry.png"),
  });
  await reduced.click("#continue-guest");
  await reduced.click("#verse-explore");
  await reduced.locator("#option-haunted-house").focus();
  await reduced.keyboard.press("Enter");
  assert.ok(await reduced.locator("#destination-haunted-house a").isVisible());
  await cameraControls(reduced,reducedPreview);
  await reduced.screenshot({path:path.join(out,"universe-mobile-explore.png")});
  await reduced.close();
  const blocked = await open();
  page = blocked;
  await blocked.addInitScript(() =>
    Object.defineProperty(navigator, "gpu", { value: undefined }),
  );
  await blocked.goto("https://openagents.com/new-lander/verse");
  await blocked.waitForFunction(() => window.universeDemo);
  await blocked.click("#continue-guest");
  await blocked.click("#verse-explore");
  await blocked.locator(".fallback-destinations").waitFor({ state: "visible" });
  await blocked.waitForFunction(
    () =>
      document
        .querySelector("#world-notice")
        .textContent.includes("unavailable"),
    null,
    { timeout: 25000 },
  );
  await blocked.locator('.fallback-destinations a[href="/new-lander"]').click();
  await blocked.waitForURL("**/new-lander");
  await blocked.close();
  assert.deepEqual(errors, []);
  const result = {
    qualification:
      "real native Rust pages and pinned hosted universe/house; guest checks only",
    browser: browser.version(),
    release: config.release,
    no_engine_or_microphone_before_enter: true,
    centered_entry_card: true,
    pre_choice_manual_input_locked: true,
    gentle_drift: true,
    reduced_motion_stops_drift: true,
    guest_explore_or_house_choice: true,
    company_world_locked: true,
    keyboard_destination_selection: true,
    post_choice_camera_input: true,
    mobile_layout: true,
    camera_toolbar_hit_test_and_zoom_desktop_mobile:true,
    house_handoff_replaces_universe_frames: true,
    return_restores_map_selection_and_camera: true,
    unsupported_webgpu_keeps_links_and_back: true,
    private_requests_without_account_credentials: privateRequests.length,
    real_provider_sign_in_complete: false,
    errors,
  };
  fs.writeFileSync(
    path.join(out, "browser-universe.json"),
    JSON.stringify(result, null, 2) + "\n",
  );
  console.log(JSON.stringify(result));
} catch (error) {
  if (page) {
    await page.screenshot({ path: path.join(out, "universe-error.png") });
    console.log(
      JSON.stringify({
        errors,
        url: page.url(),
        frames: page.frames().map((f) => f.url()),
        body: await page.locator("body").innerText(),
        snapshot: await page.evaluate(() => window.universeDemo?.snapshot()),
      }),
    );
  }
  throw error;
} finally {
  await context.close();
  await browser.close();
  server.kill("SIGTERM");
  fs.writeFileSync(path.join(out, "native-browser.log"), log);
}
