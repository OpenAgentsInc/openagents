import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "@playwright/test";

const base = process.env.NEW_LANDER_URL;
const output = process.env.HAUNT_EVIDENCE;
assert.ok(base && output && process.env.OPENAGENTS_CHROME_PORT);
await mkdir(output, { recursive: true });
const browser = await chromium.connectOverCDP(
  `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1600, height: 1100 },
});
const page = await context.newPage();
const errors = [],
  failed = [];
page.on("pageerror", (error) => errors.push(error.message));
page.on("response", (response) => {
  if (response.status() >= 400)
    failed.push([response.status(), response.url()]);
});
const state = () => page.evaluate(() => window.hauntPreview.snapshot());
const ready = () =>
  page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#open-edit").disabled,
    null,
    { timeout: 45000 },
  );
const samePose = (a, b) => {
  for (const key of ["x", "z", "yaw", "pitch"])
    assert.ok(Math.abs(a[key] - b[key]) < 0.001, `${key} moved`);
};

try {
  await page.goto(`${base}/new-lander`);
  await page.locator(".hero .enter-button").click();
  await page.getByRole("button", { name: "Continue as guest" }).click();
  await page.locator('.world-option[data-world="haunted-house"]').click();
  await page.getByRole("link", { name: "Enter Haunted House ↗" }).click();
  await ready();
  const initial = await state();
  await page.locator("#house").focus();
  await page.keyboard.down("KeyW");
  await page.waitForFunction(
    (z) => window.hauntPreview.snapshot().pose.z < z - 0.3,
    initial.pose.z,
  );
  await page.keyboard.up("KeyW");
  await page.getByRole("button", { name: "Edit", exact: false }).click();
  await page.getByRole("dialog", { name: "Make it your world." }).waitFor();
  await page.waitForFunction(
    (frame) => window.hauntPreview.snapshot().frame > frame + 2,
    (await state()).frame,
  );
  const before = await state();
  assert.equal(await page.locator(".workspace-panel").count(), 5);
  assert.ok((await page.locator("#change-rows tr").count()) >= 8);
  assert.ok(
    (await page.locator("#change-rows").innerText()).includes("Not recorded"),
  );
  await page.screenshot({
    path: path.join(output, "jarvis-panels-desktop.png"),
  });
  await page.locator("#tint").selectOption("blue");
  await page
    .getByRole("dialog", { name: "Log in to make a change." })
    .waitFor();
  assert.equal((await state()).project.revision, before.project.revision);
  assert.equal((await state()).project.tint, before.project.tint);
  samePose(before.pose, (await state()).pose);
  assert.equal(await page.locator("#tint").inputValue(), before.project.tint);
  await page.getByRole("button", { name: "Keep exploring" }).click();
  await page.locator("#close-edit").click();
  samePose(before.pose, (await state()).pose);
  await page.locator(".map-button").click();
  await page.waitForURL("**/new-lander/verse?world=haunted-house");
  assert.equal(await page.getByRole("dialog").count(), 0);
  await page.getByRole("link", { name: "Enter Haunted House ↗" }).click();
  await ready();
  await page.waitForFunction(
    (z) => Math.abs(window.hauntPreview.snapshot().pose.z - z) < 0.001,
    before.pose.z,
  );
  await page.screenshot({ path: path.join(output, "guest-house.png") });

  // Test only the member UI with isolated response fixtures. This never signs in
  // to an account or asserts live authentication/payment acceptance.
  let fixtureAuthor = "Test creator Alice";
  await page.route(`${base}/new-lander/house`, async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    assert.ok(source.includes('name="verse-member" content="false"'));
    await route.fulfill({
      response,
      body: source
        .replace(
          'name="verse-member" content="false"',
          'name="verse-member" content="true"',
        )
        .replace(
          "</head>",
          `<meta name="verse-author" content="${fixtureAuthor}"></head>`,
        ),
    });
  });
  await page.reload();
  await ready();
  await page.locator("#open-edit").click();
  await page.waitForFunction(
    (frame) => window.hauntPreview.snapshot().frame > frame + 2,
    (await state()).frame,
  );
  const working = await state();
  await page.locator("#tint").selectOption("blue");
  await page.getByText(fixtureAuthor, { exact: true }).waitFor();
  assert.equal((await state()).project.tint, "blue");
  samePose(working.pose, (await state()).pose);
  const savedChanges = JSON.parse(
    await page.evaluate(() => window.kilnReadChanges()),
  );
  assert.equal(savedChanges.changes.at(-1).author, fixtureAuthor);
  assert.equal(savedChanges.changes.at(-1).earnings, null);
  const beforeFailure = await state();
  const rowsBeforeFailure = await page.locator("#change-rows tr").count();
  await page.locator("#shader").fill("not a valid shader");
  await page.getByRole("button", { name: "Compile changes" }).click();
  await page.locator("#diagnostics").waitFor();
  const failedShader = await state();
  assert.equal(failedShader.hash, beforeFailure.hash);
  assert.equal(failedShader.deviceId, beforeFailure.deviceId);
  samePose(failedShader.pose, beforeFailure.pose);
  assert.equal(
    await page.locator("#change-rows tr").count(),
    rowsBeforeFailure,
  );
  await page.locator("#reset").click();
  await page.waitForFunction(
    () => document.querySelector("#diagnostics").hidden,
  );
  await page.waitForFunction(() =>
    JSON.parse(window.kilnReadChanges() || "null")?.changes?.some(
      (change) => change.summary === "Changed the material shader",
    ),
  );
  await page.locator("#close-edit").click();
  fixtureAuthor = "Test creator Bea";
  await page.reload();
  await ready();
  assert.equal((await state()).project.tint, "blue");
  await page.locator("#open-edit").click();
  await page.getByText("Test creator Alice", { exact: true }).first().waitFor();
  await page.locator("#tint").selectOption("green");
  await page.getByText(fixtureAuthor, { exact: true }).waitFor();
  const saved = JSON.parse(await page.evaluate(() => window.kilnReadChanges()));
  assert.ok(
    saved.changes.some((change) => change.author === "Test creator Alice"),
  );
  assert.ok(
    saved.changes.some((change) => change.author === "Test creator Bea"),
  );
  assert.ok(saved.changes.every((change) => change.earnings === null));
  await page.screenshot({
    path: path.join(output, "fixture-member-changes.png"),
  });
  await page.locator("#undo").click();
  await page.waitForFunction(
    () => window.hauntPreview.snapshot().project.tint === "blue",
  );
  await page.locator("#close-edit").click();

  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 },
  });
  const small = await mobile.newPage();
  await small.goto(`${base}/new-lander/house`);
  await small.waitForFunction(
    () => !document.querySelector("#open-edit").disabled,
  );
  await small.locator("#open-edit").click();
  assert.equal(
    await small.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
    false,
  );
  await small.locator("#changes-title").scrollIntoViewIfNeeded();
  await small.screenshot({
    path: path.join(output, "jarvis-panels-mobile.png"),
  });
  await small.locator("#close-edit").click();
  await mobile.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(failed, []);
  await writeFile(
    path.join(output, "receipt.json"),
    JSON.stringify(
      {
        browser: browser.version(),
        checks: [
          "homepage → universe → guest → house",
          "walking",
          "five editing panels",
          "real source-change history",
          "guest edit blocked without mutation",
          "panel close and map return preserve position",
          "fixture member edits save to Kiln",
          "invalid shader preserves position/device/last shader and adds no history",
          "history survives reopen and retains two fixture authors",
          "undo",
          "mobile layout",
        ],
        scope:
          "Guest flow uses the real local Rust routes. Member presentation uses two clearly identified test-only response fixtures; no live account, shared collaboration, inference, or payment was exercised. Source history is real; local records never claim earnings.",
        consoleErrors: errors,
        failedResponses: failed,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`Verse house checks passed. Evidence: ${output}`);
} finally {
  await context.close();
  await browser.close();
}
