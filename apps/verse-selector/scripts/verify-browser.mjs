// Verify the real Rust-served routes in an isolated Chrome profile.
import { chromium } from "@playwright/test";
import { PNG } from "pngjs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";

const base = process.env.NEW_LANDER_URL;
const output = process.env.NEW_LANDER_EVIDENCE;
assert.ok(base && output, "Set NEW_LANDER_URL and NEW_LANDER_EVIDENCE.");
assert.ok(
  process.env.OPENAGENTS_CHROME_PORT,
  "Run through openagents browser run.",
);
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(
  `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
  deviceScaleFactor: 1,
});
const page = await context.newPage();
const errors = [],
  failed = [],
  requests = [];
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => {
  if (message.type() === "error") errors.push(message.text());
});
page.on("request", (request) => requests.push(request.url()));
page.on("response", (response) => {
  if (response.status() >= 400)
    failed.push([response.status(), response.url()]);
});
await page.goto(`${base}/new-lander`);
await page
  .getByRole("heading", { name: "Your ideas. A whole universe." })
  .waitFor();
assert.ok(
  !requests.some((url) =>
    /verse\.js|\.wasm|\/home\/me|\/live\/games/.test(url),
  ),
  "Marketing must not start the game or account fetch.",
);
assert.equal(
  await page.evaluate(() => document.documentElement.scrollWidth > innerWidth),
  false,
);
await page.screenshot({
  path: path.join(output, "marketing-desktop.png"),
  fullPage: true,
});
const root = await context.request.get(`${base}/`);
assert.equal(root.status(), 200);
const rootHtml = await root.text();
assert.ok(
  rootHtml.includes("everglade-canvas") &&
    !rootHtml.includes("/new-lander/assets/verse.js"),
);
await page.locator(".hero .enter-button").click();
await page.waitForURL("**/new-lander/verse");
await page.waitForFunction(
  () => document.body.dataset.rendering === "webgpu",
  null,
  { timeout: 30000 },
);
await page.waitForFunction(
  () => getComputedStyle(document.querySelector("#universe")).opacity === "1",
);
await page
  .getByRole("dialog", { name: "Your next world is waiting." })
  .waitFor();
assert.equal(await page.locator(".atlas").getAttribute("data-entry"), "locked");
assert.equal(
  await page.locator("#universe-exploration").getAttribute("inert"),
  "",
);
const card = await page.locator(".signin-card").boundingBox();
assert.ok(Math.abs(card.x + card.width / 2 - 720) < 2);
assert.ok(Math.abs(card.y + card.height / 2 - 500) < 2);
const arrivalPosition = await page
  .locator("#marker-blue-rush")
  .getAttribute("style");
await page.waitForFunction(
  (value) =>
    document.querySelector("#marker-blue-rush")?.getAttribute("style") !==
    value,
  arrivalPosition,
);
await page.screenshot({
  path: path.join(output, "universe-entry-desktop.png"),
});
await page.emulateMedia({ reducedMotion: "reduce" });
const lockedPosition = await page
  .locator("#marker-blue-rush")
  .getAttribute("style");
await page.mouse.move(150, 500);
await page.mouse.down();
await page.mouse.move(350, 600, { steps: 5 });
await page.mouse.up();
await page.mouse.wheel(0, 300);
await page.keyboard.press("ArrowRight");
await page.keyboard.press("Escape");
assert.equal(
  await page.locator("#marker-blue-rush").getAttribute("style"),
  lockedPosition,
);
assert.ok(
  await page.getByRole("dialog").isVisible(),
  "Escape must not bypass entry.",
);
assert.ok(
  await page
    .locator(".signin-card")
    .evaluate((el) => el.contains(document.activeElement)),
);
await page.getByRole("button", { name: "Continue as guest" }).click();
assert.equal(await page.locator(".atlas").getAttribute("data-entry"), "guest");
assert.equal(await page.getByRole("dialog").count(), 0);
await page.emulateMedia({ reducedMotion: "no-preference" });
assert.equal(
  await page
    .getByRole("heading", { name: "OpenAgents", exact: true })
    .innerText(),
  "OpenAgents",
);
await page.locator('.world-option[data-world="haunted-house"]').click();
await page.waitForFunction(
  () => document.querySelector("#world-title")?.textContent === "Haunted House",
);
assert.ok(page.url().includes("world=haunted-house"));
assert.equal(
  await page.locator('.world-option[aria-pressed="true"]').count(),
  1,
);
await page.locator('.world-option[data-world="creator-worlds"]').click();
await page
  .locator('[data-destination="creator-worlds"] .locked-label')
  .waitFor();
await page.locator('.world-option[data-world="openagents"]').click();
await page.getByText("Locked for now", { exact: false }).waitFor();
await page.locator("#reset-view").click();
const before = await page.locator("#marker-blue-rush").getAttribute("style");
await page.locator("#zoom-in").click();
await page.waitForFunction(
  (value) =>
    document.querySelector("#marker-blue-rush")?.getAttribute("style") !==
    value,
  before,
);
await page.locator("#universe").focus();
const beforeKey = await page.locator("#marker-blue-rush").getAttribute("style");
await page.keyboard.press("ArrowRight");
await page.waitForFunction(
  (value) =>
    document.querySelector("#marker-blue-rush")?.getAttribute("style") !==
    value,
  beforeKey,
);
await page.locator("#reset-view").click();
await page.screenshot({ path: path.join(output, "universe-desktop.png") });
const png = PNG.sync.read(await page.locator("#universe").screenshot());
let lit = 0;
for (let i = 0; i < png.data.length; i += 4)
  if (png.data[i] + png.data[i + 1] + png.data[i + 2] > 100) lit++;
assert.ok(lit > 500, "The GPU canvas must contain rendered planets and stars.");
await page.locator(".back-link").click();
await page.waitForURL("**/new-lander");

await page.setViewportSize({ width: 390, height: 844 });
await page.screenshot({
  path: path.join(output, "marketing-mobile.png"),
  fullPage: true,
});
assert.equal(
  await page.evaluate(() => document.documentElement.scrollWidth > innerWidth),
  false,
);
await page.locator(".hero .enter-button").click();
await page.waitForFunction(() => document.body.dataset.rendering === "webgpu");
await page.waitForFunction(
  () => getComputedStyle(document.querySelector("#universe")).opacity === "1",
);
assert.equal(
  await page.getByRole("dialog").count(),
  0,
  "Guest entry persists during this visit.",
);
await page.locator('.world-option[data-world="blue-rush"]').click();
await page.getByRole("heading", { name: "Blue Rush", exact: true }).waitFor();
assert.equal(
  await page.evaluate(() => document.documentElement.scrollWidth > innerWidth),
  false,
);
await page.screenshot({
  path: path.join(output, "universe-mobile.png"),
  fullPage: true,
});

const noGpu = await browser.newContext({
  viewport: { width: 1280, height: 900 },
});
await noGpu.addInitScript(() =>
  Object.defineProperty(navigator, "gpu", {
    value: undefined,
    configurable: true,
  }),
);
const still = await noGpu.newPage();
await still.goto(`${base}/new-lander/verse`);
await still.waitForFunction(() => document.body.dataset.rendering === "still");
await still.getByRole("button", { name: "Continue as guest" }).click();
await still.locator('.world-option[data-world="haunted-house"]').click();
await still
  .getByRole("heading", { name: "Haunted House", exact: true })
  .waitFor();
assert.ok(await still.locator("#zoom-in").isDisabled());
await still.screenshot({ path: path.join(output, "universe-still.png") });
const freshMobile = await browser.newContext({
  viewport: { width: 390, height: 844 },
});
const mobileEntry = await freshMobile.newPage();
await mobileEntry.goto(`${base}/new-lander/verse?world=haunted-house`);
await mobileEntry.getByRole("dialog").waitFor();
const mobileCard = await mobileEntry.locator(".signin-card").boundingBox();
assert.ok(Math.abs(mobileCard.x + mobileCard.width / 2 - 195) < 2);
assert.ok(Math.abs(mobileCard.y + mobileCard.height / 2 - 422) < 2);
assert.ok(mobileCard.y >= 0 && mobileCard.y + mobileCard.height <= 844);
await mobileEntry.screenshot({
  path: path.join(output, "universe-entry-mobile.png"),
});
await mobileEntry.getByRole("button", { name: "Continue as guest" }).click();
await mobileEntry
  .getByRole("heading", { name: "Haunted House", exact: true })
  .waitFor();
assert.ok(mobileEntry.url().includes("world=haunted-house"));
const noJs = await browser.newContext({
  javaScriptEnabled: false,
  viewport: { width: 1280, height: 900 },
});
const plain = await noJs.newPage();
await plain.goto(`${base}/new-lander`);
await plain.locator(".hero .enter-button").waitFor();
assert.equal(await plain.locator("#products .product-card").count(), 4);
await plain.screenshot({
  path: path.join(output, "marketing-no-js.png"),
  fullPage: true,
});
assert.deepEqual(errors, []);
assert.deepEqual(failed, []);
await writeFile(
  path.join(output, "receipt.json"),
  JSON.stringify(
    {
      checks: [
        "marketing without game startup",
        "root remains the Grid",
        "centered entry card on desktop and mobile",
        "gentle arrival motion and reduced-motion support",
        "map input and Escape blocked until entry",
        "guest entry preserves destination and persists for this visit",
        "WebGPU rendered pixels",
        "selection and URL state",
        "locked/future worlds",
        "camera zoom and keyboard",
        "return to marketing",
        "desktop and mobile",
        "no-GPU still selector",
        "no-JavaScript marketing",
      ],
      browser: await browser.version(),
      gpuLitPixels: lit,
      consoleErrors: errors,
      failedResponses: failed,
      scope:
        "Marketing and curated universe selector. No account sign-in, tutorial completion, planet allocation, or public publishing was performed.",
    },
    null,
    2,
  ) + "\n",
);
await context.close();
await noGpu.close();
await noJs.close();
await freshMobile.close();
await browser.close();
console.log(
  `Browser checks passed; ${lit} lit pixels in the WebGPU canvas. Evidence: ${output}`,
);
