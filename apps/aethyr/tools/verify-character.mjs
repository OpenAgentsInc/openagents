/** Run through openagents browser run; screenshots contain private licensed assets. */
import assert from "node:assert/strict";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const checkout = await realpath(fileURLToPath(new URL("..", import.meta.url)));
assert(
  process.env.AF_CAPTURE_DIR,
  "Set AF_CAPTURE_DIR to a private output directory.",
);
await mkdir(resolve(process.env.AF_CAPTURE_DIR), { recursive: true });
const output = await realpath(resolve(process.env.AF_CAPTURE_DIR));
assert(
  output !== checkout && !output.startsWith(checkout + sep),
  "Keep licensed captures outside the checkout.",
);
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run this check through openagents browser run.");
const url = new URL(process.env.AF_EDITOR_URL || "http://127.0.0.1:55304/");
assert(
  ["http:", "https:"].includes(url.protocol) &&
    ["127.0.0.1", "localhost"].includes(url.hostname),
  "Verify a local editor.",
);
const report = {
  schema: "aethyrforge.character-browser.v1",
  startedAt: new Date().toISOString(),
  url: url.href,
  viewport: [1280, 720],
  ok: false,
  checks: [],
  receipts: [],
  captures: [],
  errors: [],
  console: [],
};
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
page.on("pageerror", (error) => report.errors.push(String(error)));
page.on("console", (message) => {
  if (["error", "warning"].includes(message.type()))
    report.console.push({
      type: message.type(),
      text: message.text().slice(0, 2048),
    });
});
const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
const status = async () => (await snapshot()).status;
const feet = (value) => value.character.position;
const distance = (a, b) => Math.hypot(...a.map((n, i) => n - b[i]));
const horizontalDistance = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const record = (name, details = {}) => {
  report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
async function until(test, message, timeout = 20_000) {
  const deadline = Date.now() + timeout;
  do {
    const value = await test();
    if (value) return value;
    await page.waitForTimeout(80);
  } while (Date.now() < deadline);
  throw Error(`${message}; status=${JSON.stringify(await status())}`);
}
async function receiptAfter(action, expected = true) {
  const prior = new Set(
    (await snapshot()).receipts.map((receipt) => receipt.requestId),
  );
  const requestId = await action();
  const receipt = await until(
    async () =>
      (await snapshot()).receipts.find((item) =>
        typeof requestId === "string"
          ? item.requestId === requestId
          : !prior.has(item.requestId),
      ),
    "No navigation receipt arrived",
  );
  report.receipts.push(receipt);
  assert.equal(receipt.ok, expected, receipt.message);
  return receipt;
}
async function waitFrames(prior, count = 2) {
  return until(async () => {
    const next = await status();
    assert.notEqual(next.phase, "error", next.message);
    return next.frame >= prior + count && next;
  }, "The preview stopped rendering");
}
async function capture(name) {
  const path = resolve(output, `${name}.png`);
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
  await page.evaluate(() => {
    const frame = document.querySelector("#preview-frame");
    if (!frame?.contentWindow) return;
    const url = new URL(frame.src);
    const sessionId = new URLSearchParams(url.hash.slice(1)).get("session");
    if (!sessionId) return;
    frame.contentWindow.postMessage(
      {
        protocol: "aethyrforge.preview.v1",
        sessionId,
        requestId: crypto.randomUUID(),
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      },
      url.origin,
    );
  });
  await page.keyboard.press("Escape");
  const child = await (
    await page.locator("#preview-frame").elementHandle()
  )?.contentFrame();
  await child?.evaluate(() => document.exitPointerLock());
}
async function focusWorld() {
  const canvas = page.frameLocator("#preview-frame").locator("#world");
  await canvas.click({ position: { x: 640, y: 300 } });
}
async function openField() {
  await releaseInput();
  await receiptAfter(() =>
    page.evaluate(() =>
      window.aethyrforge.cameraPose(
        [100, 1.62, 100],
        [100, 1.62, 90],
        1,
        "fly",
      ),
    ),
  );
  await until(
    async () => (await status()).view.mode === "fly",
    "Fly did not activate",
  );
  await receiptAfter(() =>
    page.getByRole("button", { name: "Fly", exact: true }).click(),
  );
  const value = await until(async () => {
    const next = await status();
    return next.view.mode === "walk" && next.character.grounded && next;
  }, "Walk did not activate on the open ground");
  assert(
    distance(feet(value), [100, 0, 100]) < 0.1,
    `Open-field feet moved during navigation change: ${feet(value)}`,
  );
  return value;
}
async function move(sprint) {
  const before = await openField();
  await focusWorld();
  const startedAt = Date.now();
  if (sprint) await page.keyboard.down("ShiftLeft");
  await page.keyboard.down("KeyW");
  try {
    await page.waitForTimeout(1200);
  } finally {
    await page.keyboard.up("KeyW");
    await page.keyboard.up("ShiftLeft");
  }
  const after = await waitFrames((await status()).frame);
  await releaseInput();
  assert(
    after.character.grounded,
    "Walking on open ground must stay grounded.",
  );
  assert(
    Math.abs(feet(after)[0] - feet(before)[0]) < 0.1,
    "Forward movement must follow the requested heading.",
  );
  assert(
    feet(after)[2] < feet(before)[2] - 1,
    "W did not move the character forward.",
  );
  return {
    sprint,
    elapsedMs: Date.now() - startedAt,
    distance: horizontalDistance(feet(before), feet(after)),
    before: feet(before),
    after: feet(after),
  };
}

try {
  report.browser = await browser.version();
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => !!window.aethyrforge, null, {
    timeout: 30_000,
  });
  const initial = await until(
    async () => {
      const value = await status();
      assert.notEqual(value.phase, "error", value.message);
      return (
        value.phase === "ready" &&
        value.frame > 2 &&
        value.character?.modelLoaded === true &&
        value.character.grounded &&
        value
      );
    },
    "The town and private Druid model did not become ready",
    180_000,
  );
  assert.equal(initial.view.mode, "walk");
  assert.equal(initial.character.perspective, "third");
  assert(
    horizontalDistance(initial.camera, feet(initial)) > 0.2,
    "Third-person camera must follow outside the character.",
  );
  record(
    "Town opens in third person with the Druid model loaded",
    initial.character,
  );
  await capture("character-third-person");
  const frameRates = [];
  const performanceStart = await status();
  const measuredAt = performance.now();
  for (let i = 0; i < 16; i++) {
    await page.waitForTimeout(250);
    frameRates.push((await status()).fps);
  }
  const performanceEnd = await status();
  const seconds = (performance.now() - measuredAt) / 1000;
  record("Live town frame rate measured with the animated Druid", {
    viewport: report.viewport,
    seconds,
    submittedFrames: performanceEnd.frame - performanceStart.frame,
    submissionFps: (performanceEnd.frame - performanceStart.frame) / seconds,
    reportedFps: frameRates,
  });

  await focusWorld();
  await page.keyboard.press("KeyV");
  const first = await until(async () => {
    const value = await status();
    return (
      value.character.perspective === "first" &&
      horizontalDistance(value.camera, feet(value)) < 0.06 &&
      Math.abs(value.camera[1] - feet(value)[1] - 1.7) < 0.06 &&
      value
    );
  }, "V did not switch to first person");
  assert(
    distance(feet(first), feet(initial)) < 0.06,
    "Changing perspective moved the player.",
  );
  assert(
    horizontalDistance(first.camera, feet(first)) < 0.06,
    "First-person eye must track the player's head.",
  );
  assert(
    Math.abs(first.camera[1] - feet(first)[1] - 1.7) < 0.06,
    "First-person eye height is incorrect.",
  );
  await releaseInput();
  await capture("character-first-person");
  await receiptAfter(() =>
    page
      .getByRole("button", { name: "Switch to third person", exact: true })
      .click(),
  );
  const third = await until(async () => {
    const value = await status();
    return value.character.perspective === "third" && value;
  }, "The view button did not restore third person");
  assert(
    distance(feet(first), feet(third)) < 0.06,
    "The view button moved the player.",
  );
  record(
    "Real V key and view button switch perspectives without moving the feet",
    { first: first.camera, third: third.camera, feet: feet(third) },
  );

  await focusWorld();
  const previousDistance = third.character.orbitDistance;
  await page.mouse.wheel(0, -180);
  const zoomed = await until(async () => {
    const value = await status();
    return value.character.orbitDistance < previousDistance - 0.01 && value;
  }, "Mouse wheel did not zoom the follow camera");
  assert(
    zoomed.character.orbitDistance >= 2.5 &&
      zoomed.character.orbitDistance <= 40,
  );
  assert(
    distance(feet(zoomed), feet(third)) < 0.06,
    "Zooming moved the player.",
  );
  await releaseInput();
  record("Wheel zoom changes the bounded follow distance", {
    before: previousDistance,
    after: zoomed.character.orbitDistance,
  });

  const normal = await move(false);
  const sprint = await move(true);
  const ratio = sprint.distance / normal.distance;
  assert(
    ratio > 1.15 && ratio < 3,
    `Sprint must move faster over equal held-input periods: ${JSON.stringify({ normal, sprint, ratio })}`,
  );
  record("Real W and Shift input move and sprint on open ground", {
    normal,
    sprint,
    ratio,
  });

  const beforeJump = await openField();
  await focusWorld();
  const jumpSamples = [];
  await page.keyboard.down("Space");
  try {
    await until(
      async () => {
        const value = await status();
        jumpSamples.push({
          frame: value.frame,
          feet: feet(value),
          grounded: value.character.grounded,
          animation: value.character.animation,
        });
        return (
          !value.character.grounded &&
          feet(value)[1] > feet(beforeJump)[1] + 0.1
        );
      },
      "Space did not start a jump",
      6000,
    );
    await until(
      async () => {
        const value = await status();
        jumpSamples.push({
          frame: value.frame,
          feet: feet(value),
          grounded: value.character.grounded,
          animation: value.character.animation,
        });
        return (
          value.character.grounded &&
          Math.abs(feet(value)[1] - feet(beforeJump)[1]) < 0.06
        );
      },
      "The character did not land",
      8000,
    );
    const heldUntil = Date.now() + 1100;
    while (Date.now() < heldUntil) {
      const value = await status();
      jumpSamples.push({
        frame: value.frame,
        feet: feet(value),
        grounded: value.character.grounded,
        animation: value.character.animation,
      });
      assert(
        value.character.grounded,
        "Holding Space triggered another jump after landing.",
      );
      await page.waitForTimeout(100);
    }
  } finally {
    await page.keyboard.up("Space");
    await releaseInput();
  }
  record("Held Space produces one jump, then lands without repeating", {
    samples: jumpSamples,
  });
  await capture("character-open-field");

  // Pack-specific coordinates stay in private fixtures outside the repository.
  if (process.env.AF_CHARACTER_ROUTES) {
    const text = await readFile(process.env.AF_CHARACTER_ROUTES, "utf8");
    assert(text.length < 16384);
    const routes = JSON.parse(text);
    assert(Array.isArray(routes) && routes.length <= 8);
    for (const route of routes) {
      assert(typeof route.name === "string" && route.name.length < 128);
      assert(
        [route.eye, route.target].every(
          (p) =>
            Array.isArray(p) &&
            p.length === 3 &&
            p.every((n) => Number.isFinite(n) && Math.abs(n) < 20000),
        ),
      );
      assert(route.holdMs >= 500 && route.holdMs <= 3000);
      await releaseInput();
      const priorFrame = (await status()).frame;
      await receiptAfter(() =>
        page.evaluate(
          ({ eye, target }) =>
            window.aethyrforge.cameraPose(eye, target, 1, "walk"),
          route,
        ),
      );
      const start = await until(async () => {
        const s = await status();
        return (
          s.frame > priorFrame &&
          s.character?.grounded &&
          horizontalDistance(feet(s), route.eye) < 0.5 &&
          s
        );
      }, "The source route did not settle");
      await focusWorld();
      await page.keyboard.down("KeyW");
      try {
        await page.waitForTimeout(route.holdMs);
      } finally {
        await page.keyboard.up("KeyW");
      }
      const end = await waitFrames((await status()).frame);
      await releaseInput();
      const travelled = horizontalDistance(feet(start), feet(end));
      if (route.minTravel !== undefined)
        assert(
          travelled >= route.minTravel,
          `Source route movement: ${travelled}`,
        );
      if (route.maxTravel !== undefined)
        assert(
          travelled <= route.maxTravel,
          `Source wall did not stop movement: ${travelled}`,
        );
      if (route.minRise !== undefined)
        assert(
          feet(end)[1] - feet(start)[1] >= route.minRise,
          "The source stairs did not lift the character",
        );
      assert(end.character.grounded);
      record(route.name, {
        before: feet(start),
        after: feet(end),
        travelled,
        animation: end.character.animation,
      });
      await capture(`character-source-route-${report.checks.length}`);
    }
  }

  if (process.env.AF_VERIFY_CHARACTER_SHADER === "1") {
    const before = await status();
    await receiptAfter(
      () =>
        page.evaluate(() =>
          window.aethyrforge.shader("invalid WGSL character verification"),
        ),
      false,
    );
    const after = await waitFrames(before.frame, 3);
    assert(distance(feet(before), feet(after)) < 0.06);
    assert.equal(after.revision, before.revision);
    record("Rejected shader preserves the player while rendering continues", {
      beforeFrame: before.frame,
      afterFrame: after.frame,
    });
  }
  report.finalStatus = await status();
  assert.deepEqual(
    report.errors,
    [],
    "The browser reported an uncaught error.",
  );
  report.ok = true;
} catch (error) {
  report.failure = String(error?.stack || error);
  console.error(report.failure);
  process.exitCode = 1;
  try {
    report.finalStatus = await status();
  } catch {
    /* The page may have closed. */
  }
  try {
    await capture("character-failure");
  } catch {
    /* Best-effort evidence. */
  }
} finally {
  try {
    await releaseInput();
  } catch {
    /* The page may have closed. */
  }
  report.finishedAt = new Date().toISOString();
  try {
    await writeFile(
      resolve(output, "character-browser.json"),
      JSON.stringify(report, null, 2) + "\n",
    );
  } finally {
    await page.close().catch(() => {});
    await context.close().catch(() => {});
    await browser.close();
  }
}
