// Check real touch targets and retained world state as the HUD changes size.
import { chromium } from "playwright";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
const origin = process.env.VERSE_UI_REVIEW_URL || "https://openagents.com";
const dir = process.env.VERSE_UI_CAPTURE_DIR;
assert(
  dir && process.env.OPENAGENTS_CHROME_WS,
  "Set VERSE_UI_CAPTURE_DIR and run under openagents browser run.",
);
fs.mkdirSync(dir, { recursive: true });
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = await browser.newContext({
  viewport: { width: 390, height: 844 },
  isMobile: true,
  hasTouch: true,
});
const page = await context.newPage();
page.setDefaultTimeout(30000);
async function targets(selector) {
  for (const control of await page.locator(selector).all()) {
    assert(
      await control.evaluate((el) => {
        const box = el.getBoundingClientRect();
        return (
          box.width >= 24 &&
          box.height >= 24 &&
          box.x >= 0 &&
          box.y >= 0 &&
          box.right <= innerWidth + 1 &&
          box.bottom <= innerHeight + 1 &&
          el.contains(
            document.elementFromPoint(
              box.x + box.width / 2,
              box.y + box.height / 2,
            ),
          )
        );
      }),
      "Covered or unreachable control: " + (await control.textContent()),
    );
  }
}
async function capture(name) {
  assert(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
    "Horizontal overflow: " + name,
  );
  await page.screenshot({ path: path.join(dir, name + ".png") });
  console.log("PASS", name);
}
try {
  await page.goto(origin + "/new-lander/house");
  await page.waitForFunction(
    () =>
      window.hauntDemo &&
      JSON.parse(window.hauntDemo.snapshot()).phase === "open" &&
      JSON.parse(window.hauntDemo.snapshot()).status.frame > 5,
    null,
    { timeout: 125000 },
  );
  const before = await page.evaluate(() => {
    window.__hudEngine = document.getElementById("engine");
    window.__hudCanvas = document.getElementById("face-canvas");
    return JSON.parse(window.hauntDemo.snapshot()).status;
  });
  for (const [width, height] of [
    [320, 568],
    [390, 844],
    [844, 390],
    [1440, 1000],
  ]) {
    await page.setViewportSize({ width, height });
    await targets(
      "#world-controls a:visible,#world-controls button:visible,#world-controls summary:visible",
    );
    await page.locator(".hud-utilities summary").tap();
    await targets(
      "#world-controls a:visible,#world-controls button:visible,#world-controls summary:visible",
    );
    await capture("display-" + width);
    await page.locator(".hud-utilities summary").tap();
    await page.locator("#world-edit").tap();
    for (const tab of [
      "objects",
      "materials",
      "request",
      "compile",
      "history",
    ]) {
      await targets("#jarvis-toolbar button:visible,#edit-close");
      await page.locator("#tab-" + tab).tap();
      assert(await page.locator("#panel-" + tab).isVisible());
      if (tab === "history" || tab === "materials")
        await capture(tab + "-" + width);
    }
    await page.locator("#edit-close").tap();
    await targets(
      "#world-controls a:visible,#world-controls button:visible,#world-controls summary:visible",
    );
  }
  assert(
    await page.evaluate(
      () =>
        window.__hudEngine === document.getElementById("engine") &&
        window.__hudCanvas === document.getElementById("face-canvas"),
    ),
    "Resizing must retain the engine and paint canvas.",
  );
  const after = await page.evaluate(
    () => JSON.parse(window.hauntDemo.snapshot()).status,
  );
  assert.equal(after.generation, before.generation);
  assert.deepEqual(after.checkpoint, before.checkpoint);
  assert(after.frame > before.frame);
  await page.locator("#world-map").tap();
  await page.waitForURL((url) => url.pathname === "/new-lander/verse");
  await page.waitForFunction(
    () =>
      window.universeDemo &&
      JSON.parse(window.universeDemo.snapshot()).phase === "open",
    null,
    { timeout: 125000 },
  );
  if (await page.locator("[data-aethyr-id=continue-guest]").isVisible())
    await page.locator("[data-aethyr-id=continue-guest]").tap();
  if (await page.locator("[data-aethyr-id=verse-explore]").isVisible())
    await page.locator("[data-aethyr-id=verse-explore]").tap();
  for (const [width, height] of [
    [390, 844],
    [1440, 1000],
  ]) {
    await page.setViewportSize({ width, height });
    await page.locator("[data-aethyr-id=option-haunted-house]").tap();
    await targets(".world-option:visible");
    await capture("universe-" + width);
  }
  console.log(
    "PASS retained engine, paint canvas, position, generation, and map navigation",
  );
} finally {
  await context.close();
  await browser.close();
}
