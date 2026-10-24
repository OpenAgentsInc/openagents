/** Run through openagents browser run; keep character captures outside Git. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(process.env.AF_CAPTURE_DIR, "Set a private AF_CAPTURE_DIR.");
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(output !== checkout && !output.startsWith(checkout + sep), "Keep character captures outside Git.");
const endpoint = process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT && `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55316/");
assert(["http:", "https:"].includes(url.protocol) && ["localhost", "127.0.0.1"].includes(url.hostname) &&
  !url.username && !url.password, "Verify a local Open Ground server.");
const minimumFps = Number(process.env.AF_MIN_FPS || 54);
assert(Number.isFinite(minimumFps) && minimumFps > 0 && minimumFps <= 240);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const sourceGlbSha = "8d28a9ef055ac10ed8afbe832888cb042195ed2aaa02a20458297cfbb5476d40";
const sourceSwordSha = "eb8e0611db727471e853cce1ee07d05a6057a665b1f2a1efc04026bc6120ca5f";
const sourceBaseSha = "152f3ef170b06264bfdeca5e3e41e7b1fe0eac766408fe0e335af6e868f6ee34";
const sourceNormalSha = "5318eacc5cd087a64df57c65ce9d0894eef1e3dc59bc299183eef49eef083437";
const report = {
  schema: "aethyrforge.quick-mmorpg-browser.v1", startedAt: new Date().toISOString(),
  url: url.href, viewport: [1280, 720], minimumSubmissionFps: minimumFps, ok: false,
  checks: [], captures: [], receipts: [], scenes: [], characters: [], assets: [],
  errors: [], console: [], failed: [], limitations: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, serviceWorkers: "block" });
await context.addInitScript(() => {
  window.__mmorpgReady = [];
  window.__mmorpgGraphics = [];
  window.__mmorpgInput = [];
  for (const type of ["keydown", "keyup", "pointerlockchange"])
    (type === "pointerlockchange" ? document : window).addEventListener(type, (event) => {
      if (window.__mmorpgInput.length < 256)
        window.__mmorpgInput.push({ type, code: event.code, at: performance.now(), locked: !!document.pointerLockElement });
    });
  window.addEventListener("message", (event) => {
    const frame = document.getElementById("preview-frame");
    if (!frame?.contentWindow || event.source !== frame.contentWindow || event.origin !== new URL(frame.src).origin ||
      event.data?.protocol !== "aethyrforge.preview.v1") return;
    if (event.data.type === "ready") window.__mmorpgReady.push({ hasSourceSky: event.data.hasSourceSky });
    const message = event.data?.status?.message || event.data?.message;
    if (typeof message === "string" && /^Graphics(?::| device stopped:)/.test(message))
      window.__mmorpgGraphics.push(message.slice(0, 4096));
  });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
let closing = false;
const bodies = [];
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({ type: message.type(), text: message.text().slice(0, 2048) });
});
page.on("requestfailed", (request) => {
  if (!closing) report.failed.push({ url: request.url(), error: request.failure()?.errorText });
});
page.on("response", (response) => {
  const path = new URL(response.url()).pathname;
  if (response.status() >= 400) report.failed.push({ url: response.url(), status: response.status() });
  if (/\/private-assets\/scene-ground(?:-night)?\.json$/.test(path))
    bodies.push(response.json().then((scene) => report.scenes.push({ url: response.url(), scene })));
  if (/\/character-quick-mmorpg-paladin-[a-f0-9]{64}\.json$/.test(path))
    bodies.push(response.body().then((bytes) => report.characters.push({ url: response.url(), sha256: sha256(bytes),
      byteLength: bytes.length, manifest: JSON.parse(bytes) })));
  if (/\/character-[a-f0-9]{64}\.(?:png|vertices\.bin|indices\.bin)$/.test(path))
    bodies.push(response.body().then((bytes) => report.assets.push({ url: response.url(), sha256: sha256(bytes),
      byteLength: bytes.length, pngExtent: path.endsWith(".png") ? [bytes.readUInt32BE(16), bytes.readUInt32BE(20)] : undefined })));
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
let readyEstablished = false;
const status = async () => {
  const value = (await snapshot()).status;
  if (readyEstablished && (value.phase !== "ready" || !value.character)) {
    report.statusDuringFailure = value;
    throw Error(`The ready world restarted or failed during input: ${JSON.stringify(value)}`);
  }
  return value;
};
const feet = (s) => s.character.position;
const horizontal = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const distance = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
const speed = (s) => Math.hypot(s.character.velocity[0], s.character.velocity[2]);
const record = (name, details = {}) => { report.checks.push({ name, ...details }); console.log(`PASS ${name}`); };
const percentile = (values, f) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor(values.length * f))] ?? null;
const submittedFrames = (s) => s.runtimeMetrics?.submittedFrames ?? s.frame;
const heading = (s) => Math.atan2(s.view.target[0] - s.view.eye[0], s.view.target[2] - s.view.eye[2]);
const angularDelta = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
function healthy(s) {
  assert.equal(s.phase, "ready", s.message);
  assert.equal(s.character?.controller, "quick-mmorpg-v1");
  assert.equal(s.character.modelLoaded, true);
  assert.equal(s.character.perspective, "third");
  assert(s.character.grounded, "The Paladin left the continuous ground");
  assert(Math.abs(feet(s)[1]) < .02, "The Paladin's feet left the zero-height plane");
  assert.notEqual(s.gameplay?.enabled, true, "Open Ground enabled spell gameplay");
  assert.equal(s.game, undefined, "Open Ground installed a quest");
  assert.equal(s.instances, 1);
  assert.equal(s.triangles, 2);
}
function grass(s) {
  const g = s.groundGrass;
  assert(g && g.nearBlades > 0 && g.farBlades > 0, "Both submitted grass detail levels are required");
  for (const key of ["nearPatches", "farPatches", "nearBlades", "farBlades", "blades", "triangles", "drawCalls", "accountedBytes"])
    assert(Number.isSafeInteger(g[key]) && g[key] >= 0, `Invalid grass ${key}`);
  assert.equal(g.drawCalls, 2);
  assert.equal(g.blades, g.nearBlades + g.farBlades);
  assert.equal(g.triangles, g.nearBlades * 12 + g.farBlades * 6);
  assert(g.accountedBytes > 0 && Number.isFinite(g.windTime));
  return g;
}
async function until(test, message, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  do { const value = await test(); if (value) return value; await page.waitForTimeout(40); } while (Date.now() < deadline);
  throw Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function waitFrames(count = 3) {
  const before = (await status()).frame;
  return until(async () => { const s = await status(); assert.notEqual(s.phase, "error", s.message); return s.frame >= before + count && s; }, "Rendering stopped");
}
async function capture(name) {
  const path = resolve(output, `${name}.png`), pixels = await page.screenshot({ path });
  report.captures.push(path); return pixels;
}
async function focusWorld() {
  const frame = await (await page.locator("#preview-frame").elementHandle()).contentFrame();
  await frame.locator("#world").click({ position: { x: 640, y: 300 } });
  await frame.waitForFunction(() => document.hasFocus() && document.pointerLockElement === document.getElementById("world"));
  return frame;
}
async function releaseInput(unlock = false) {
  for (const key of ["KeyW", "KeyA", "KeyS", "KeyD", "ShiftLeft", "Space", "Backspace"])
    await page.keyboard.up(key);
  if (unlock) {
    await page.keyboard.press("Escape");
    const frame = await (await page.locator("#preview-frame").elementHandle())?.contentFrame();
    await frame?.evaluate(() => document.exitPointerLock());
  }
}
async function settled() {
  await releaseInput();
  return until(async () => { const s = await status(); healthy(s); return s.character.animation === "idle" && speed(s) < .005 && s; }, "The source deceleration did not settle");
}
async function receiptAfter(action, expected = true) {
  const prior = new Set((await snapshot()).receipts.map((r) => r.requestId));
  const requestId = await action();
  const result = await until(async () => (await snapshot()).receipts.find((r) => typeof requestId === "string" ? r.requestId === requestId : !prior.has(r.requestId)), "No command receipt arrived");
  report.receipts.push(result); assert.equal(result.ok, expected, result.message); return result;
}
async function walkAndRun() {
  await settled(); await focusWorld();
  const before = await status();
  await page.keyboard.down("KeyW");
  try {
    await page.waitForTimeout(1800);
    const walk = await until(async () => { const s = await status(); return s.character.animation === "walk" && s; }, "W did not select Walk");
    healthy(walk); assert(speed(walk) > 2.9 && speed(walk) < 3.7, `Wrong source walk speed: ${speed(walk)}`);
    assert(horizontal(feet(walk), feet(before)) > 4);
    await capture("paladin-walk");
    await page.keyboard.down("ShiftLeft"); await page.waitForTimeout(1600);
    const run = await until(async () => { const s = await status(); return s.character.animation === "run" && s; }, "Shift+W did not select Run");
    healthy(run); assert(speed(run) > 5.9 && speed(run) < 7.3, `Wrong source run speed: ${speed(run)}`);
    const ratio = speed(run) / speed(walk); assert(ratio > 1.8 && ratio < 2.2);
    await capture("paladin-run");
    return { before: before.character, walk: walk.character, run: run.character, ratio };
  } finally { await page.keyboard.up("KeyW"); await page.keyboard.up("ShiftLeft"); }
}
async function turnAndReverse() {
  await settled(); await page.waitForTimeout(1200); await focusWorld();
  const before = await status();
  await page.keyboard.down("KeyD");
  try { await page.waitForTimeout(1000); } finally { await page.keyboard.up("KeyD"); }
  await page.waitForTimeout(1300); const right = await waitFrames();
  const rightAngle = angularDelta(heading(right), heading(before));
  assert(Math.abs(rightAngle) > 1.25 && Math.abs(rightAngle) < 1.9, `D did not turn about90degrees: ${rightAngle}`);
  assert(horizontal(feet(right), feet(before)) < .03, "A/D strafed instead of turning");
  await page.keyboard.down("KeyA");
  try { await page.waitForTimeout(1000); } finally { await page.keyboard.up("KeyA"); }
  await page.waitForTimeout(1300); const left = await waitFrames();
  const leftAngle = angularDelta(heading(left), heading(right));
  assert(Math.abs(leftAngle) > 1.25 && Math.abs(leftAngle) < 1.9 && leftAngle * rightAngle < 0);
  assert(Math.abs(angularDelta(heading(left), heading(before))) < .25, "Opposite turns did not restore heading");
  await page.keyboard.down("KeyS");
  let backward;
  try { await page.waitForTimeout(1400); backward = await status(); } finally { await page.keyboard.up("KeyS"); }
  healthy(backward); assert.equal(backward.character.animation, "walk");
  const direction = [left.view.target[0] - left.view.eye[0], left.view.target[2] - left.view.eye[2]];
  const move = [feet(backward)[0] - feet(left)[0], feet(backward)[2] - feet(left)[2]];
  assert(move[0] * direction[0] + move[1] * direction[1] < -1, "S did not move backward");
  return { rightAngle, leftAngle, before: before.character, right: right.character, left: left.character, backward: backward.character };
}
async function action(key, animation, duration) {
  await settled(); await focusWorld();
  // Enter from Idle with a measurable remaining velocity: the source stops
  // translation during actions and preserves that velocity until Idle returns.
  await page.keyboard.down("KeyW"); await page.waitForTimeout(700); await page.keyboard.up("KeyW");
  await until(async () => (await status()).character.animation === "idle", "The source FSM did not enter Idle");
  await page.keyboard.down(key);
  let start;
  try { start = await until(async () => { const s = await status(); return s.character.animation === animation && s; }, `${key} did not start ${animation}`); }
  finally { await page.keyboard.up(key); }
  healthy(start); assert(Number.isFinite(start.character.animationTime));
  await page.keyboard.down("KeyW"); await page.keyboard.down("KeyD");
  const samples = [start];
  try {
    await page.waitForTimeout(500);
    const middle = await status(); samples.push(middle); healthy(middle);
    assert.equal(middle.character.animation, animation);
    assert(middle.character.animationTime > start.character.animationTime + .2, "The authored clip clock stopped");
    assert(horizontal(feet(middle), feet(start)) < .003, `${animation} moved despite its action lock`);
    assert(distance(middle.character.velocity, start.character.velocity) < .003, `${animation} changed its held velocity`);
    await capture(`paladin-${animation}`);
  } finally { await page.keyboard.up("KeyW"); await page.keyboard.up("KeyD"); }
  const returned = await until(async () => { const s = await status(); samples.push(s); healthy(s); return s.character.animation === "idle" && s; }, `${animation} did not finish and return to Idle`, (duration + 3) * 1000);
  assert(samples.every((s) => s.character.grounded && Math.abs(feet(s)[1]) < .02), "Space triggered a jump");
  return { initial: start.character, returned: returned.character, samples: samples.map((s) => ({ frame: s.frame, character: s.character })) };
}
async function mouseAndViewIgnored() {
  await settled(); await page.waitForTimeout(1800); await focusWorld();
  const before = await status();
  await page.keyboard.press("KeyV");
  for (let i = 0; i < 6; i++) { await page.mouse.move(320 + 100 * i, 150 + 50 * i); await page.waitForTimeout(120); }
  await page.mouse.wheel(0, 240); await page.waitForTimeout(500);
  const after = await waitFrames(); healthy(after);
  assert(Math.abs(angularDelta(heading(after), heading(before))) < .01, "Mouse look changed the source follow camera");
  assert(distance(after.camera, before.camera) < .03, "Mouse or V changed the fixed follow camera");
  assert(horizontal(feet(after), feet(before)) < .01);
  return { before: { character: before.character, view: before.view }, after: { character: after.character, view: after.view } };
}
async function frameSample() {
  await settled(); await focusWorld();
  await page.evaluate(() => {
    const sample = { active: true, last: null, intervals: [] }; window.__mmorpgCadence = sample;
    const next = (now) => { if (!sample.active) return; if (sample.last !== null && sample.intervals.length < 4096) sample.intervals.push(now - sample.last); sample.last = now; requestAnimationFrame(next); };
    requestAnimationFrame(next);
  });
  const before = await status(), samples = [], start = performance.now();
  await page.keyboard.down("KeyW");
  let after, stop, intervals;
  let turningRight = false, turningLeft = false, sprinting = false;
  try {
    for (let i = 0; i < 60; i++) {
      for (const [key, enabled, previous] of [["KeyD", i >= 12 && i < 16, turningRight], ["KeyA", i >= 32 && i < 36, turningLeft], ["ShiftLeft", i >= 40 && i < 56, sprinting]])
        if (enabled !== previous) await page.keyboard[enabled ? "down" : "up"](key);
      turningRight = i >= 12 && i < 16; turningLeft = i >= 32 && i < 36; sprinting = i >= 40 && i < 56;
      await page.waitForTimeout(250); const s = await status(); healthy(s);
      samples.push({ atMs: performance.now() - start, frame: s.frame, submittedFrames: submittedFrames(s), fps: s.fps,
        character: s.character, heading: heading(s), grass: grass(s) });
    }
    after = await status(); stop = performance.now();
  } finally {
    await releaseInput();
    intervals = await page.evaluate(() => { window.__mmorpgCadence.active = false; return window.__mmorpgCadence.intervals; });
  }
  const seconds = (stop - start) / 1000, frames = submittedFrames(after) - submittedFrames(before);
  const travelled = samples.reduce((sum, value, i) => sum + horizontal(value.character.position, i ? samples[i - 1].character.position : feet(before)), 0);
  const result = { seconds, submittedFrames: frames, submissionFps: frames / seconds,
    hudFpsMedian: percentile(samples.map((s) => s.fps), .5), hudFpsMinimum: Math.min(...samples.map((s) => s.fps)),
    uiRafP95Ms: percentile(intervals, .95), uiRafOver50Ms: intervals.filter((v) => v > 50).length,
    travelledMetres: travelled, maxHeadingChangeRadians: Math.max(...samples.map((s) => Math.abs(angularDelta(s.heading, heading(before))))), samples };
  report.frameSample = result;
  assert(result.submissionFps >= minimumFps, `Submitted frame cadence ${result.submissionFps.toFixed(2)} fell below ${minimumFps}`);
  assert(travelled > 35 && result.maxHeadingChangeRadians > 1, "Sustained walking and turning did not execute");
  return result;
}
async function windMotion() {
  await settled(); await page.waitForTimeout(1600);
  const before = await status(), first = await capture("paladin-grass-wind-a");
  await page.waitForTimeout(850); const second = await capture("paladin-grass-wind-b"), after = await status();
  assert(distance(feet(before), feet(after)) < .003 && distance(before.camera, after.camera) < .03);
  assert(grass(after).windTime > grass(before).windTime + .3);
  const regions = await page.evaluate(async ({ first, second }) => {
    const decode = (bytes) => createImageBitmap(new Blob([Uint8Array.from(atob(bytes), (c) => c.charCodeAt(0))], { type: "image/png" }));
    const a = await decode(first), b = await decode(second);
    const regions = [{ x: 80, y: 430, width: 360, height: 180 }, { x: 840, y: 430, width: 360, height: 180 }];
    const results = regions.map((r) => {
      const c = new OffscreenCanvas(r.width, r.height), ctx = c.getContext("2d");
      ctx.drawImage(a, r.x, r.y, r.width, r.height, 0, 0, r.width, r.height);
      const x = ctx.getImageData(0, 0, r.width, r.height).data; ctx.clearRect(0, 0, r.width, r.height);
      ctx.drawImage(b, r.x, r.y, r.width, r.height, 0, 0, r.width, r.height);
      const y = ctx.getImageData(0, 0, r.width, r.height).data;
      let changed = 0, absolute = 0;
      for (let i = 0; i < x.length; i += 4) { const delta = [0, 1, 2].map((k) => Math.abs(x[i + k] - y[i + k])); if (Math.max(...delta) >= 12) changed++; absolute += delta.reduce((n, v) => n + v, 0); }
      return { ...r, changedRatio: changed / (r.width * r.height), meanAbsoluteChannelChange: absolute / (r.width * r.height * 3) };
    }); a.close(); b.close(); return results;
  }, { first: first.toString("base64"), second: second.toString("base64") });
  assert(regions.every((r) => r.changedRatio > .01 && r.meanAbsoluteChannelChange > .3), `Grass did not visibly sway: ${JSON.stringify(regions)}`);
  return { beforeGrass: grass(before), afterGrass: grass(after), regions };
}
function checkSourceCharacter() {
  const entry = report.characters[0]; assert(entry, "The source Paladin manifest was not loaded");
  assert(entry.url.includes(entry.sha256), "The character manifest hash does not match its filename");
  const m = entry.manifest;
  assert.equal(m.name, "player/quick-mmorpg-paladin");
  assert.equal(m.metadata.sourceSha256, sourceGlbSha); assert.equal(m.metadata.swordSha256, sourceSwordSha);
  assert.equal(m.metadata.sourceCommit, "547884332ca650abe96264f7230702d36481b9bc");
  assert.equal(m.metadata.originalAppScale, 6); assert.equal(m.metadata.worldScale, 1 / 6); assert.equal(m.metadata.effectiveScale, 1);
  assert.deepEqual(m.controller, { profile: "quick-mmorpg-v1", worldScale: 1 / 6 });
  assert.deepEqual(m.metadata.includedMeshes, ["Paladin_J_Nordstrom", "Paladin_J_Nordstrom_Helmet"]);
  assert.equal(m.joints.length, 66); assert.equal(m.geometry.vertexCount, 12141); assert.equal(m.geometry.indexCount, 46596);
  assert.equal(m.metadata.swordTriangles, 872); assert.equal(m.metadata.swordAnchor, "RightHandIndex1");
  assert.equal(m.metadata.totalAnimationKeys, 76050);
  assert.deepEqual(m.clips.map((c) => c.name), ["idle", "walk", "run", "attack", "dance", "death"]);
  for (const clip of m.clips) assert.equal(clip.loop, ["idle", "walk", "run"].includes(clip.name));
  const body = m.materials.find((material) => material.name === "Paladin_MAT"); assert(body);
  assert.equal(body.metallic, .5); assert.equal(body.roughness, .5527864098548889); assert.equal(body.doubleSided, true);
  for (const [field, hash] of [["texture", sourceBaseSha], ["normalTexture", sourceNormalSha]]) {
    const descriptor = body[field]; assert(descriptor); assert.equal(descriptor.sha256, hash);
    assert.equal(descriptor.width, 2048); assert.equal(descriptor.height, 2048);
    const actual = report.assets.find((asset) => asset.url === new URL(descriptor.url, entry.url).href); assert(actual, `The GPU material did not request ${field}`);
    assert.equal(actual.sha256, hash); assert.equal(actual.byteLength, descriptor.byteLength); assert.deepEqual(actual.pngExtent, [2048, 2048]);
  }
  for (const descriptor of [m.geometry.vertices, m.geometry.indices]) {
    const actual = report.assets.find((asset) => asset.url === new URL(descriptor.url, entry.url).href); assert(actual);
    assert.equal(actual.sha256, descriptor.sha256); assert.equal(actual.byteLength, descriptor.byteLength);
  }
  return { url: entry.url, sha256: entry.sha256, geometry: m.geometry, joints: m.joints.length,
    clips: m.clips.map(({ name, duration, loop }) => ({ name, duration, loop })), material: body,
    textureEvidence: "Original PNG bytes loaded and hash-checked; screenshots and WebGPU errors test the rendered path. Mip submission is not separately exposed." };
}

try {
  report.browser = await browser.version();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge);
  const initial = await until(async () => {
    const s = await status(); assert.notEqual(s.phase, "error", s.message);
    return s.phase === "ready" && s.frame > 3 && s.character?.modelLoaded && s.character.grounded && s;
  }, "The Paladin scene did not become ready", 180_000);
  healthy(initial); readyEstablished = true; grass(initial); await Promise.all(bodies);
  const day = report.scenes.find((s) => new URL(s.url).pathname.endsWith("/scene-ground.json"));
  const night = report.scenes.find((s) => new URL(s.url).pathname.endsWith("/scene-ground-night.json"));
  assert(day && night); assert.equal(day.scene.characterURL, night.scene.characterURL);
  for (const { scene } of [day, night]) {
    assert.equal(scene.instances.length, 1); assert.equal(scene.meshes.length, 1); assert.equal(scene.meshes[0].name, "OpenGroundPlane");
    assert.equal(scene.source.groundZone.profile, "ground-sky-player-v1"); assert.equal(scene.source.groundZone.sizeMetres, 2048);
    assert(scene.environmentURL && scene.collisionURL && scene.grassURL);
    for (const key of ["gameplayURL", "enemyCharacterURL", "enemyPatrolsURL", "creaturesURL", "particlesURL", "waterURL"])
      assert(!scene[key], `The movement-only scene contains ${key}`);
  }
  record("Original Paladin body, helmet, sword, six clips and original PBR maps load", checkSourceCharacter());
  const ui = await page.evaluate(() => {
    const r = document.getElementById("preview-frame").getBoundingClientRect();
    return { title: document.title, text: document.body.innerText,
      frame: { x: r.x, y: r.y, width: r.width, height: r.height }, viewport: [innerWidth, innerHeight],
      buttons: [...document.querySelectorAll("button")].filter((e) => e.getClientRects().length).map((e) => e.getAttribute("aria-label") || e.textContent.trim()),
      editors: [...document.querySelectorAll("textarea,input[type=text],[contenteditable=true]")].filter((e) => e.getClientRects().length).length,
      fps: document.querySelector('[aria-label="Frames per second"]')?.textContent };
  });
  assert.match(ui.title, /Open Ground/); assert.equal(ui.editors, 0); assert.match(ui.fps, /\d+ FPS/);
  assert(!ui.buttons.some((name) => /spell|meteor|lightning|quest|edit|jump|first person|third person/i.test(name)));
  assert.match(ui.text, /W\/S move.*A\/D turn/s); assert.match(ui.text, /Space attack.*Backspace dance/s);
  assert(Math.abs(ui.frame.x) < 1 && Math.abs(ui.frame.y) < 1 && Math.abs(ui.frame.width - 1280) < 1 && Math.abs(ui.frame.height - 720) < 1);
  const sky = await page.evaluate(() => window.__mmorpgReady); assert(sky.some((s) => s.hasSourceSky === true));
  record("Fullscreen Open Ground exposes source controls and fixed third-person view", { ui, sky, character: initial.character });
  await settled(); await page.waitForTimeout(1400); await capture("paladin-idle-day");
  record("Grass visibly sways while the Paladin and follow camera stand still", await windMotion());
  record("W and Shift reach source walk and run speeds", await walkAndRun());
  record("A/D turn in place and S moves backward using the Walk clip", await turnAndReverse());
  record("Space plays Attack from Idle, holds momentum and does not jump", await action("Space", "attack", 1.54));
  record("Backspace plays Dance once and returns to Idle", await action("Backspace", "dance", 4.81));
  record("Pointer lock focuses the world while mouse, scroll and V leave source framing unchanged", await mouseAndViewIgnored());
  record("Fifteen seconds of walking, turning and sprinting maintain submitted grass cadence", await frameSample());
  await settled(); await page.waitForTimeout(1200); await capture("paladin-after-movement");
  report.limitations.push("The walking-only surface denies lighting edits, and its read-only capture API has no night override. Both prepared scene manifests select the same Paladin, but night rendering was not exercised.");
  const denied = await receiptAfter(() => page.evaluate(() => window.aethyrforge.selectAbility("lightning_bolt")), false);
  const final = await waitFrames(); healthy(final); assert.equal(final.revision, initial.revision);
  record("Spell selection stays denied and source controls do not edit the world", { receipt: denied, revision: final.revision });
  report.finalStatus = final; report.graphicsMessages = await page.evaluate(() => window.__mmorpgGraphics);
  await Promise.all(bodies);
  assert.deepEqual(report.graphicsMessages, []); assert.deepEqual(report.errors, []);
  assert.deepEqual(report.failed.filter((f) => new URL(f.url).pathname !== "/favicon.ico"), []);
  assert(!report.console.some((m) => m.type === "error" && /GPUValidation|device lost|WGSL.*error/i.test(m.text)));
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error); console.error(report.failure); process.exitCode = 1;
  try { report.finalStatus = (await snapshot()).status; } catch {}
  try { report.graphicsMessages = await page.evaluate(() => window.__mmorpgGraphics); } catch {}
  try { await capture("paladin-failure"); } catch {}
} finally {
  closing = true;
  try { await releaseInput(true); } catch {}
  try { const frame = await (await page.locator("#preview-frame").elementHandle())?.contentFrame(); report.inputEvents = await frame?.evaluate(() => window.__mmorpgInput); } catch {}
  report.finishedAt = new Date().toISOString();
  try { await writeFile(resolve(output, "quick-mmorpg-browser.json"), JSON.stringify(report, null, 2) + "\n"); }
  finally { await page.close().catch(() => {}); await context.close().catch(() => {}); await browser.close(); }
}
