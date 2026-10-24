import { chromium } from "playwright";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
const base = process.env.VERSE_UI_REVIEW_URL;
const dir = process.env.VERSE_UI_CAPTURE_DIR;
assert(
  base && dir && process.env.OPENAGENTS_CHROME_WS,
  "Set VERSE_UI_REVIEW_URL and VERSE_UI_CAPTURE_DIR, and run under openagents browser run.",
);
fs.mkdirSync(dir, { recursive: true });
const capture = (name) => path.join(dir, name);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const ctx = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  }),
  page = await ctx.newPage();
const candidate = process.env.VERSE_UI_CANDIDATE_URL;
if (candidate) {
  await ctx.route(base + "/new-lander/**", async (route) => {
    const url = new URL(route.request().url());
    const response = await ctx.request.get(
      candidate + url.pathname + url.search,
    );
    const headers = { ...response.headers() };
    for (const key of [
      "content-encoding",
      "content-length",
      "transfer-encoding",
    ])
      delete headers[key];
    await route.fulfill({
      status: response.status(),
      headers,
      body: await response.body(),
    });
  });
}
for (const [asset, type] of [
  ["ui.css", "text/css"],
  ["ui.js", "text/javascript"],
]) {
  const response = await ctx.request.get(
    (candidate || base) + "/new-lander/" + asset,
    {
      headers: { Cookie: "oa_cloud_login=synthetic-verse-ui-review" },
    },
  );
  assert.equal(
    response.status(),
    200,
    `${asset} must load with a login cookie`,
  );
  assert(response.headers()["content-type"].startsWith(type));
}
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
page.on("console", (e) => {
  if (e.type() === "error") errors.push(e.text());
});
await page.goto(base + "/new-lander/verse");
await page.waitForFunction(
  () =>
    window.verseUi &&
    window.universeDemo &&
    JSON.parse(window.universeDemo.snapshot()).phase === "open" &&
    JSON.parse(window.universeDemo.snapshot()).status.frame > 0,
  undefined,
  { timeout: 60000 },
);
assert.equal(
  await page.evaluate(() => getComputedStyle(document.body).backgroundColor),
  "rgb(0, 0, 0)",
);
await page.locator("[data-aethyr-id=continue-guest]").click();
assert(await page.locator("[data-aethyr-id=verse-choices]").isVisible());
await page.screenshot({ path: capture("choices-desktop.png") });
await page.getByRole("link", { name: "Haunted House ↗", exact: true }).click();
await page.waitForFunction(() => window.verseUi && window.hauntDemo);
await page
  .waitForFunction(
    () => {
      const snapshot = JSON.parse(window.hauntDemo.snapshot());
      return snapshot.phase === "open" && snapshot.status?.frame > 5;
    },
    undefined,
    { timeout: 60000 },
  )
  .catch(async (error) => {
    const snapshot = JSON.parse(
      await page.evaluate(() => window.hauntDemo.snapshot()),
    );
    console.log(
      JSON.stringify({
        phase: snapshot.phase,
        error: snapshot.error,
        frame: snapshot.status?.frame,
        uiErrors: errors,
      }),
    );
    throw error;
  });
const before = JSON.parse(
  await page.evaluate(() => window.hauntDemo.snapshot()),
).status;
assert(
  before?.frame > 0,
  "The real engine must be advancing before the UI check",
);
await page.evaluate(() => {
  window.__uiEngine = document.querySelector("#engine");
  window.__uiCanvas = document.querySelector("#face-canvas");
});
await page.locator("#world-edit").click();
assert(await page.locator("#edit-panel").isVisible());
await page.locator("#edit-note").fill("Keep this draft while reviewing tools.");
await page.getByRole("button", { name: "Find a tool", exact: true }).click();
await page.getByPlaceholder("Find a tool…").waitFor();
await page.screenshot({ path: capture("tool-finder.png") });
await page.keyboard.press("Escape");
assert(
  await page.locator("#edit-panel").isVisible(),
  "Escape from tool finder must keep Jarvis open",
);
await page.waitForFunction(
  () => document.activeElement?.getAttribute("aria-label") === "Find a tool",
);
assert.equal(
  await page.locator("#edit-note").inputValue(),
  "Keep this draft while reviewing tools.",
);
assert(
  await page.evaluate(
    () =>
      window.__uiEngine === document.querySelector("#engine") &&
      window.__uiCanvas === document.querySelector("#face-canvas"),
  ),
);
await page.getByText("Shared material and fog", { exact: true }).click();
await page.locator("#edit-fog").fill("0.03");
await page.locator("#edit-fog-apply").click();
await page.locator("#edit-auth[open]").waitFor();
await page.screenshot({ path: capture("signin-desktop.png") });
await page.locator("#edit-cancel").click();
assert(await page.locator("#edit-panel").isVisible());
assert.equal(
  await page.locator("#edit-note").inputValue(),
  "Keep this draft while reviewing tools.",
);
const after = JSON.parse(
  await page.evaluate(() => window.hauntDemo.snapshot()),
).status;
assert.equal(
  after.generation,
  before.generation,
  "UI operations must keep the same engine generation",
);
assert(
  after.frame > before.frame,
  "Engine frames must advance while using tools",
);
assert.deepEqual(
  after.checkpoint,
  before.checkpoint,
  "Canceling sign-in must keep the same place",
);
await page.screenshot({ path: capture("editor-final-desktop.png") });
await page.setViewportSize({ width: 390, height: 844 });
await page.locator("#tab-materials").click();
assert(await page.locator("#panel-materials").isVisible());
assert(!(await page.locator("#panel-objects").isVisible()));
await page.screenshot({ path: capture("editor-mobile.png") });
const overflow = await page.evaluate(
  () => document.documentElement.scrollWidth > innerWidth,
);
assert(!overflow, "No page overflow at narrow width");
await page.locator("#edit-close").click();
await page.goto(base + "/new-lander/verse");
await page.waitForFunction(() => window.universeDemo);
await page.screenshot({ path: capture("universe-mobile.png") });
await ctx.close();
await browser.close();
const result = {
  guestChoice: true,
  realPrivateEngine: true,
  jarvis: true,
  toolFinderFocus: true,
  escapeKeepsWorkspace: true,
  firstEditSignin: true,
  cancelKeepsDraft: true,
  engineAndCanvasIdentity: true,
  narrowTabs: true,
  noNarrowOverflow: true,
  errors,
};
fs.writeFileSync(
  capture("browser-result.json"),
  JSON.stringify(result, null, 2),
);
console.log(JSON.stringify(result));
assert.deepEqual(errors, []);
