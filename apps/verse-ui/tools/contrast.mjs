import { chromium } from "playwright";
import AxeBuilder from "@axe-core/playwright";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const base = process.env.VERSE_UI_REVIEW_URL || "https://openagents.com";
const directory = process.env.VERSE_UI_CAPTURE_DIR;
assert(
  directory && process.env.OPENAGENTS_CHROME_WS,
  "Set VERSE_UI_CAPTURE_DIR and run under openagents browser run.",
);
fs.mkdirSync(directory, { recursive: true });
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = await browser.newContext({ colorScheme: "light" });
if (process.env.VERSE_UI_REVIEW_CSS) {
  await context.route(base + "/new-lander/ui.css", (route) =>
    route.fulfill({
      contentType: "text/css",
      body: fs.readFileSync(process.env.VERSE_UI_REVIEW_CSS),
    }),
  );
}
const page = await context.newPage();
const rows = [];
page.setDefaultTimeout(25000);

// Resolve alpha through the containing surfaces, including inherited opacity.
// Axe separately checks painted backgrounds; this also covers its translucent
// cards and image-background cases using the computed foreground/background pair.
function computedContrast() {
  const rgba = (value) => {
    const parts = value.match(/[\d.]+/g)?.map(Number);
    return parts?.length >= 3
      ? [...parts.slice(0, 3), parts[3] ?? 1]
      : [0, 0, 0, 0];
  };
  const over = (front, back) => [
    ...front.slice(0, 3).map((c, i) => c * front[3] + back[i] * (1 - front[3])),
    1,
  ];
  const luminance = (rgb) =>
    rgb
      .slice(0, 3)
      .map((n) => {
        const v = n / 255;
        return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
      })
      .reduce((a, v, i) => a + v * [0.2126, 0.7152, 0.0722][i], 0);
  const ratio = (a, b) => {
    const x = luminance(a),
      y = luminance(b);
    return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
  };
  const failures = [],
    checked = [];
  for (const element of document.querySelectorAll("body *")) {
    if (
      !(element instanceof HTMLElement) ||
      !element.checkVisibility({
        checkOpacity: true,
        checkVisibilityCSS: true,
      }) ||
      element.closest("[inert],[aria-hidden=true],script,style,canvas,svg")
    )
      continue;
    const text = [...element.childNodes]
      .filter((n) => n.nodeType === Node.TEXT_NODE)
      .map((n) => n.textContent)
      .join("")
      .trim();
    if (!text || !element.getClientRects().length) continue;
    const chain = [];
    for (let node = element; node; node = node.parentElement)
      chain.unshift(node);
    let background = [0, 0, 0, 1];
    for (const node of chain)
      background = over(
        rgba(getComputedStyle(node).backgroundColor),
        background,
      );
    const style = getComputedStyle(element);
    let foreground = over(rgba(style.color), background);
    for (const node of chain)
      foreground = over(
        [...foreground.slice(0, 3), Number(getComputedStyle(node).opacity)],
        background,
      );
    const contrast = ratio(foreground, background);
    const disabled = element.closest(":disabled,[aria-disabled=true]");
    const large =
      parseFloat(style.fontSize) >= 24 ||
      (parseFloat(style.fontSize) >= 18.66 && Number(style.fontWeight) >= 700);
    const minimum = disabled ? 3 : large ? 3 : 4.5;
    const row = {
      target: element.id
        ? "#" + element.id
        : element.tagName.toLowerCase() +
          "." +
          [...element.classList].slice(0, 2).join("."),
      text: text.slice(0, 90),
      ratio: Number(contrast.toFixed(2)),
      minimum,
      foreground: style.color,
      background,
      disabled: Boolean(disabled),
    };
    checked.push(row);
    if (contrast + 0.01 < minimum) failures.push(row);
  }
  return { checked: checked.length, failures };
}

async function audit(name) {
  await page.waitForTimeout(180);
  const axe = await new AxeBuilder({ page })
    .withRules(["color-contrast"])
    .analyze();
  const computed = [];
  for (const frame of page.frames()) {
    try {
      computed.push({
        url: frame.url(),
        ...(await frame.evaluate(computedContrast)),
      });
    } catch {
      /* A navigating frame is covered by the next state. */
    }
  }
  const row = {
    name,
    viewport: page.viewportSize(),
    violations: axe.violations,
    incomplete: axe.incomplete,
    computed,
    overflow: await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
  };
  rows.push(row);
  fs.writeFileSync(
    path.join(directory, "contrast.json"),
    JSON.stringify(rows, null, 2),
  );
  console.log(
    JSON.stringify({
      name,
      violations: axe.violations.flatMap((v) =>
        v.nodes.map((n) => ({ target: n.target, summary: n.failureSummary })),
      ),
      computed: computed.flatMap((r) => r.failures),
      overflow: row.overflow,
    }),
  );
  await page.screenshot({ path: path.join(directory, name + ".png") });
}

for (const width of [390, 1440]) {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
  await page.goto(base + "/new-lander");
  await audit(`home-${width}`);
  for (let i = 0; i < (await page.locator(".enter-button").count()); i++) {
    await page.locator(".enter-button").nth(i).hover();
    await audit(`home-hover-${i}-${width}`);
    await page.locator(".enter-button").nth(i).focus();
    await audit(`home-focus-${i}-${width}`);
  }
  for (const route of ["login", "signup"]) {
    await page.goto(base + "/" + route + "?return_to=%2Fnew-lander%2Fhouse");
    await audit(`${route}-${width}`);
    await page.locator(".oa-button").first().hover();
    await audit(`${route}-hover-${width}`);
  }
  await page.evaluate(() => {
    localStorage.clear();
    sessionStorage.clear();
  });
  await page.goto(base + "/new-lander/verse");
  await page.waitForFunction(() => window.universeDemo, null, {
    timeout: 30000,
  });
  await audit(`universe-entry-${width}`);
  await page.locator("[data-aethyr-id=continue-guest]").hover();
  await audit(`guest-hover-${width}`);
  await page.locator("[data-aethyr-id=continue-guest]").click();
  await audit(`destinations-${width}`);
  await page.goto(base + "/new-lander/house");
  await page.waitForFunction(() => window.verseUi && window.hauntDemo, null, {
    timeout: 30000,
  });
  await page.waitForFunction(
    () => JSON.parse(window.hauntDemo.snapshot()).phase === "open",
    null,
    { timeout: 60000 },
  );
  await audit(`house-${width}`);
  await page.locator("#world-edit").hover();
  await audit(`edit-hover-${width}`);
  await page.locator("#world-edit").click();
  for (const tab of ["objects", "materials", "request", "compile", "history"]) {
    await page.locator("#tab-" + tab).click();
    await audit(`${tab}-${width}`);
  }
  await page.getByRole("button", { name: "Find a tool", exact: true }).click();
  await audit(`tool-finder-${width}`);
  await page.keyboard.press("Escape");
  await page.locator("#tab-materials").click();
  await page.getByText("Shared material and fog", { exact: true }).click();
  await page.locator("#edit-fog").fill("0.03");
  await page.locator("#edit-fog-apply").click();
  await page.locator("#edit-auth[open]").waitFor();
  await audit(`first-edit-signin-${width}`);
  await page.locator("#edit-signin").hover();
  await audit(`first-edit-signin-hover-${width}`);
  await page.locator("#edit-cancel").click();
  await page.locator("#edit-close").click();
  await page.goto(base + "/demo/medieval-town");
  await page.waitForFunction(
    () =>
      window.medievalDemo &&
      JSON.parse(window.medievalDemo.snapshot()).phase === "open",
    null,
    { timeout: 60000 },
  );
  await audit(`medieval-${width}`);
}

// Account-only layouts use the actual Rust-rendered test fixtures. No production
// account, publication, invitation, or chat is created by these visual checks.
if (process.env.VERSE_UI_REVIEW_FIXTURES) {
  for (const file of fs
    .readdirSync(process.env.VERSE_UI_REVIEW_FIXTURES)
    .filter((f) => f.endsWith(".html"))) {
    const html = fs
      .readFileSync(
        path.join(process.env.VERSE_UI_REVIEW_FIXTURES, file),
        "utf8",
      )
      .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "");
    const url = base + "/__verse-contrast/" + file;
    await context.route(url, (route) =>
      route.fulfill({ contentType: "text/html", body: html }),
    );
    for (const width of [390, 1440]) {
      await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
      await page.goto(url);
      // These fixtures test form/card presentation, without private engine loads.
      await page
        .locator("iframe")
        .evaluateAll((nodes) => nodes.forEach((n) => n.remove()));
      await audit(`fixture-${file}-${width}`);
      await page.locator("#publish-review").evaluateAll((nodes) =>
        nodes.forEach((n) => {
          n.hidden = false;
          n.showModal();
        }),
      );
      await page
        .locator("#publish-share-card")
        .evaluateAll((nodes) => nodes.forEach((n) => (n.hidden = false)));
      await audit(`fixture-cards-${file}-${width}`);
      await page
        .locator("dialog[open]")
        .evaluateAll((nodes) => nodes.forEach((n) => n.close()));
      await page
        .locator("#publish-share-link")
        .evaluateAll((nodes) =>
          nodes.forEach(
            (n) =>
              (n.textContent = "https://openagents.com/play/example-version"),
          ),
        );
      await audit(`fixture-share-${file}-${width}`);
    }
    await context.unroute(url);
  }
}
fs.writeFileSync(
  path.join(directory, "contrast.json"),
  JSON.stringify(rows, null, 2),
);
await context.close();
await browser.close();
assert(
  rows.every(
    (r) =>
      !r.violations.length &&
      !r.overflow &&
      r.computed.every((f) => !f.failures.length),
  ),
  "Contrast/visibility failures: see contrast.json",
);
console.log(`PASS ${rows.length} contrast/visibility states`);
