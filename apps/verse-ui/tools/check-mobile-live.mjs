// Real production APIs and GPU, with a dedicated account and phone-sized touch input.
// External identity-provider approval and live microphone recognition are not automated.
import { chromium } from "playwright";
import fs from "node:fs";
import assert from "node:assert/strict";
import path from "node:path";
import os from "node:os";
import { execFileSync } from "node:child_process";
assert(
  process.env.OPENAGENTS_CHROME_WS,
  "Run this check with openagents browser run.",
);
const scratch =
  process.env.OPENAGENTS_SCRATCH ||
  execFileSync("openagents", ["scratch"], { encoding: "utf8" }).trim();
const dir =
  path.resolve(
    process.env.OPENAGENTS_VERIFY_OUTPUT ||
      path.join(scratch, "verse-mobile-live-" + Date.now()),
  ) + path.sep;
fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
const credentialFile =
  process.env.OPENAGENTS_TEST_ACCOUNT_ENV ||
  path.join(os.homedir(), "work/.secrets/openagents-test-account.env");
assert.equal(
  fs.statSync(credentialFile).mode & 0o077,
  0,
  "Keep the test credential file private.",
);
const env = {};
for (const l of fs.readFileSync(credentialFile, "utf8").split("\n")) {
  const m = l.match(/^(?:export )?([A-Z_]+)=(.*)$/);
  if (m) env[m[1]] = m[2].replace(/^['"]|['"]$/g, "");
}
const scrub = (value) =>
  String(value).replaceAll(
    env.OPENAGENTS_TOKEN || "__missing__",
    "[test credential]",
  );
assert.equal(env.OPENAGENTS_TEST_ACCOUNT, "acct_83d16a35955ef908");
const origin = "https://openagents.com",
  browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS),
  mobile = {
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  },
  context = await browser.newContext(mobile);
let signed = false;
const events = [],
  steps = [];
const headers = {
  authorization: "Bearer " + env.OPENAGENTS_TOKEN,
  "x-openagents-test": "1",
};
await context.route(origin + "/**", async (route) => {
  const p = new URL(route.request().url()).pathname,
    h = { ...route.request().headers(), "x-openagents-test": "1" };
  if (
    signed &&
    (/^\/v1\/verse\//.test(p) ||
      /^\/new-lander\/(planet|invite|visit|play)\//.test(p))
  )
    h.authorization = headers.authorization;
  await route.continue({ headers: h });
});
const page = await context.newPage();
page.setDefaultTimeout(30000);
page.on("pageerror", (e) => events.push({ error: scrub(e.message) }));
page.on("response", (r) => {
  const u = new URL(r.url());
  if (u.origin === origin && u.pathname.startsWith("/v1/verse/"))
    events.push({
      path: u.pathname,
      method: r.request().method(),
      status: r.status(),
    });
});
const record = () =>
  fs.writeFileSync(
    dir + "journey-events.json",
    JSON.stringify({ steps, events }, null, 2),
  );
const shot = async (name) => {
  assert(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
    "Horizontal overflow at " + name,
  );
  await page.screenshot({ path: dir + "journey-" + name + ".png" });
  steps.push(name);
  record();
  console.log("PASS", name);
};
const snap = () => page.evaluate(() => JSON.parse(window.hauntDemo.snapshot()));
const ready = async () => {
  await page.bringToFront();
  return page.waitForFunction(
    () =>
      window.hauntDemo &&
      JSON.parse(window.hauntDemo.snapshot()).phase === "open" &&
      JSON.parse(window.hauntDemo.snapshot()).status.authoring &&
      JSON.parse(window.hauntDemo.snapshot()).status.frame > 5,
    null,
    { timeout: 125000 },
  );
};
const tap = async (id) => page.locator(id).tap();
const edit = async (tab) => {
  if (await page.locator("[data-aethyr-id=edit-panel]").isHidden()) await tap("[data-aethyr-id=world-edit]");
  await tap("[data-aethyr-id=tab-" + tab + "]");
};
const confirm = async () => {
  await page.locator("[data-aethyr-id=edit-confirm]").waitFor({ state: "visible" });
  const saving =
    (await page.locator("[data-aethyr-id=edit-auth-title]").textContent()) ===
    "Save this world?";
  const prior = await page.locator("[data-aethyr-id=edit-history-entries]").textContent();
  await tap("[data-aethyr-id=edit-confirm]");
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "hidden" });
  // The engine receipt can arrive before the editor finishes its confirmation.
  if (!saving)
    await page.waitForFunction(
      (text) =>
        document.querySelector("#edit-history-entries").textContent !== text,
      prior,
    );
};
const close = async () => {
  if (await page.locator("[data-aethyr-id=edit-close]").isVisible()) await tap("[data-aethyr-id=edit-close]");
};
const options = async () => {
  await edit("request");
  await tap("[data-aethyr-id=request-options]");
};
const save = async () => {
  await edit("history");
  await tap("[data-aethyr-id=project-save-copy]");
  await confirm();
  await page.waitForFunction(() =>
    document
      .querySelector("#project-status")
      .textContent.includes("Saved to project"),
  );
};
const publish = async () => {
  await edit("compile");
  await page.selectOption("[data-aethyr-id=publish-visibility] select", "public");
  await options();
  await tap("[data-aethyr-id=project-publish]");
  await page.locator("[data-aethyr-id=publish-review][open]").waitFor();
  await page.locator("[data-aethyr-id=publish-confirm-rights] input").check();
  await tap("[data-aethyr-id=publish-confirm]");
  await page.locator("[data-aethyr-id=publish-review]").waitFor({ state: "hidden" });
  const path = await page.locator("[data-aethyr-id=published-link]").getAttribute("href");
  assert(path);
  return path;
};
const playerReady = async (p) => {
  await p.bringToFront();
  return p.waitForFunction(
    () =>
      window.versePlayer &&
      JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(
        (r) => r.outcome.message === "Playable version opened.",
      ),
    null,
    { timeout: 125000 },
  );
};
const visitGuest = async (path) => {
  const guest = await browser.newContext(mobile);
  try {
    const p = await guest.newPage();
    await p.goto(origin + path);
    await playerReady(p);
    await p.screenshot({
      path: dir + "journey-guest-" + steps.length + ".png",
    });
    assert(!(await p.locator("[data-aethyr-id=world-edit]").count()));
  } finally {
    await guest.close();
  }
};
let project, houseLink, planetPath, planetLink;
try {
  await page.goto(origin + "/new-lander");
  await page.locator(".enter-button").first().tap();
  await tap("[data-aethyr-id=continue-guest]");
  await page.getByRole("link", { name: "Haunted House ↗", exact: true }).tap();
  await ready();
  assert.equal(
    await page.locator("#publish-enabled").getAttribute("content"),
    "true",
  );
  await shot("01-guest-house");
  await tap("[data-aethyr-id=world-mesh]");
  await page.waitForFunction(
    () =>
      document.querySelector("#world-mesh").getAttribute("aria-pressed") ===
      "true",
  );
  await tap("[data-aethyr-id=world-mesh]");
  await page.waitForFunction(
    () =>
      document.querySelector("#world-mesh").getAttribute("aria-pressed") ===
      "false",
  );
  await tap("[data-aethyr-id=world-map]");
  await page.waitForURL((u) => u.pathname === "/new-lander/verse");
  await page.getByRole("link", { name: "Haunted House ↗", exact: true }).tap();
  await ready();
  await edit("materials");
  await page.getByText("Shared material and fog", { exact: true }).tap();
  await page.locator("[data-aethyr-id=edit-fog] :is(input,textarea)").fill("0.035");
  await tap("[data-aethyr-id=edit-fog-apply]");
  await page.locator("[data-aethyr-id=edit-auth][open]").waitFor();
  await shot("02-sign-in-required");
  const before = await snap();
  const popupPromise = context.waitForEvent("page");
  await tap("[data-aethyr-id=edit-signin]");
  const popup = await popupPromise;
  await popup.waitForLoadState("domcontentloaded");
  assert.equal(new URL(popup.url()).pathname, "/login");
  await popup.screenshot({ path: dir + "journey-sign-in.png" });
  // Only the dedicated test key is injected; the external provider login is not automated.
  const identity = await (
    await context.request.get(origin + "/v1/verse/studio/session", { headers })
  ).json();
  assert.equal(identity.account, env.OPENAGENTS_TEST_ACCOUNT);
  assert(identity.signed_in);
  signed = true;
  await popup.close();
  await page.locator("[data-aethyr-id=edit-confirm]").waitFor({ state: "visible" });
  const kept = await snap();
  assert.equal(kept.status.generation, before.status.generation);
  assert.deepEqual(kept.status.checkpoint, before.status.checkpoint);
  await confirm();
  await shot("03-authorized-edit");
  await edit("compile");
  const source = await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue(),
    pose = (await snap()).status.checkpoint,
    generation = (await snap()).status.generation;
  await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").fill("invalid shader on purpose");
  await tap("[data-aethyr-id=edit-source-draft]");
  await confirm();
  await tap("[data-aethyr-id=edit-source-compile]");
  await confirm();
  await page.waitForFunction(
    () =>
      JSON.parse(window.hauntDemo.snapshot()).status.authoring.diagnostics
        .length > 0,
  );
  const failed = await snap();
  assert.equal(failed.status.authoring.active_source, source);
  assert.equal(failed.status.generation, generation);
  assert.deepEqual(failed.status.checkpoint, pose);
  await shot("04-invalid-shader-keeps-world");
  const changed = source.replace(
    "return base;",
    "return base * vec3<f32>(0.8, 0.6, 0.45);",
  );
  assert.notEqual(changed, source);
  await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").fill(changed);
  await tap("[data-aethyr-id=edit-source-draft]");
  await confirm();
  await page.waitForFunction(
    (draft) =>
      JSON.parse(window.hauntDemo.snapshot()).status.authoring.draft === draft,
    changed,
  );
  await tap("[data-aethyr-id=edit-source-compile]");
  await confirm();
  assert.equal((await snap()).status.authoring.active_source, changed);
  await close();
  await shot("05-compiled-world");
  // Position the camera as a selection test precondition, then use actual touch.
  const creator = page
    .frames()
    .find((f) => f.url().includes("---aethyrforge-editor"));
  assert(creator);
  await creator.evaluate(() =>
    window.aethyrforgeEditor.service.cameraPose(
      [-2.3, 1.8, -12.3],
      [-2.4, 1.8, -14.8],
      undefined,
      "fly",
    ),
  );
  await page.waitForFunction(
    () =>
      JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye[2] < -12,
  );
  const facePreview = page
    .frames()
    .find((f) => f.url().includes("---aethyrforge-preview"));
  const pickBox = await facePreview.locator("#world").boundingBox();
  await page.touchscreen.tap(
    pickBox.x + pickBox.width / 2,
    pickBox.y + pickBox.height / 2,
  );
  await page.waitForFunction(
    () =>
      JSON.parse(window.hauntDemo.snapshot()).status.selection?.id ===
      "werewolf-1",
  );
  await edit("materials");
  await tap("[data-aethyr-id=face-open]");
  await page.locator("[data-aethyr-id=face-generate]:enabled").waitFor();
  await page
    .locator("[data-aethyr-id=face-request] :is(input,textarea)")
    .fill("two thin healed blue scars on the left cheek");
  await tap("[data-aethyr-id=face-generate]");
  await page.waitForFunction(() =>
    document
      .querySelector("#face-status")
      .textContent.includes("Generated locally"),
  );
  await page.locator("#face-canvas").scrollIntoViewIfNeeded();
  const paintBox = await page.locator("#face-canvas").boundingBox();
  const paint = await context.newCDPSession(page);
  await paint.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x: paintBox.x + 30, y: paintBox.y + 40 }],
  });
  await paint.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x: paintBox.x + 60, y: paintBox.y + 70 }],
  });
  await paint.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  await paint.detach();
  const faceRevision = (await snap()).status.project.revision;
  await tap("[data-aethyr-id=face-apply]");
  await confirm();
  await page.waitForFunction(
    (revision) =>
      JSON.parse(window.hauntDemo.snapshot()).status.project.revision >
      revision,
    faceRevision,
  );
  const paintedMaterials = await creator.evaluate(async () =>
    (await window.aethyrforgeEditor.service.projectDocument()).originals.filter(
      (source) => source.name.startsWith("material-v1-"),
    ),
  );
  assert.equal(paintedMaterials.length, 1);
  assert(JSON.parse(paintedMaterials[0].text).handmade.image);
  await shot("05b-painted-face");
  await edit("history");
  await page.locator("[data-aethyr-id=project-name] :is(input,textarea)").fill("Kiki mobile journey 12583");
  await tap("[data-aethyr-id=project-create]");
  await page.waitForFunction(() =>
    document.querySelector("#project-picker").value.startsWith("prj_"),
  );
  project = await page.locator("[data-aethyr-id=project-picker] select").inputValue();
  fs.writeFileSync(dir + "journey-created.json", JSON.stringify({ project }));
  await save();
  await close();
  await tap("[data-aethyr-id=project-save]");
  await confirm();
  await page.waitForFunction(() =>
    document.querySelector("#project-status").textContent.includes("version 2"),
  );
  await shot("06-saved");
  await page.reload();
  await ready();
  await edit("history");
  await tap("[data-aethyr-id=project-list]");
  await page.selectOption("[data-aethyr-id=project-picker] select", project);
  await tap("[data-aethyr-id=project-open]");
  await confirm();
  assert.equal((await snap()).status.authoring.active_source, changed);
  const reopenedCreator = page
    .frames()
    .find((f) => f.url().includes("---aethyrforge-editor"));
  assert.deepEqual(
    await reopenedCreator.evaluate(async () =>
      (
        await window.aethyrforgeEditor.service.projectDocument()
      ).originals.filter((source) => source.name.startsWith("material-v1-")),
    ),
    paintedMaterials,
  );
  await shot("07-reopened");

  houseLink = await publish();
  await shot("08-published-house");
  await options();
  await tap("[data-aethyr-id=project-share]");
  await page.locator("[data-aethyr-id=publish-share-card]").waitFor({ state: "visible" });
  assert.equal(
    await page.locator("[data-aethyr-id=publish-share-link]").getAttribute("href"),
    houseLink,
  );
  await shot("09-share-house");
  await visitGuest(houseLink);
  await tap("[data-aethyr-id=publish-share-close]");
  await options();
  await tap("[data-aethyr-id=project-play]");
  await page.waitForURL(origin + houseLink);
  await playerReady(page);
  await shot("10-play-house");
  await page.getByRole("link", { name: "Back to planets", exact: true }).tap();
  await page.waitForURL((u) => u.pathname === "/new-lander/verse");
  await page
    .locator("[data-aethyr-id=planet-ready]:visible,[data-aethyr-id=planet-owned]:visible")
    .first()
    .waitFor();
  await shot("11-planet-award-ready");
  if (await page.locator("[data-aethyr-id=planet-ready]").isVisible())
    await tap("[data-aethyr-id=planet-create]");
  await page.locator("[data-aethyr-id=planet-owned]").waitFor({ state: "visible" });
  planetPath = await page.locator("[data-aethyr-id=planet-setup]").getAttribute("href");
  assert(planetPath);
  await shot("12-planet-awarded");
  await page.reload();
  await page.locator("[data-aethyr-id=planet-owned]").waitFor({ state: "visible" });
  assert.equal(
    await page.locator("[data-aethyr-id=planet-setup]").getAttribute("href"),
    planetPath,
  );
  await tap("[data-aethyr-id=planet-setup]");
  await page
    .locator("[data-aethyr-id=planet-name] input")
    .fill("Kiki mobile garden");
  const renameResponse = page.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      new URL(r.url()).pathname === planetPath,
  );
  await tap("[data-aethyr-id=planet-save]");
  assert.equal((await renameResponse).status(), 303);
  await page.waitForFunction(
    () =>
      document.querySelector("[data-aethyr-id=setup-title]")?.textContent ===
      "Kiki mobile garden",
  );
  await tap("[data-aethyr-id=planet-enter]");
  await page.waitForURL("**/world");
  await ready();
  await shot("13-personal-world");
  const personalProject = await page
    .locator("#personal-project")
    .getAttribute("content");
  const currentHead = await (
    await context.request.get(
      origin +
        "/v1/verse/studio/projects/" +
        personalProject +
        "/personal-planet/main",
      { headers },
    )
  ).json();
  if (currentHead.head) {
    await edit("history");
    await tap("[data-aethyr-id=project-open]");
    await confirm();
    await close();
  }
  await page.locator("[data-aethyr-id=planet-seed] :is(input,textarea)").fill("42");
  await page.locator("[data-aethyr-id=planet-day] :is(input,textarea)").fill("2");
  await tap("[data-aethyr-id=planet-generate]");
  await confirm();
  await page.waitForFunction(() =>
    document.querySelector("#planet-status").textContent.includes("ready"),
  );
  await shot("14-generated-courtyard");
  await page.selectOption("[data-aethyr-id=planet-style] select", "bold-fantasy");
  await page.locator("[data-aethyr-id=planet-hour] :is(input,textarea)").fill("18");
  await tap("[data-aethyr-id=planet-apply]");
  await confirm();
  await shot("15-style-and-time");
  await page.locator("[data-aethyr-id=planet-card-toggle]").tap();
  await save();
  await shot("16-saved-planet");
  planetLink = await publish();
  await shot("17-published-planet");
  await visitGuest(planetLink);
  await options();
  await tap("[data-aethyr-id=project-share]");
  await page.locator("[data-aethyr-id=publish-share-card]").waitFor({ state: "visible" });
  await shot("18-shared-planet");
  fs.writeFileSync(
    dir + "journey-links.json",
    JSON.stringify({ project, houseLink, planetPath, planetLink }, null, 2),
  );
  // The owner opens invitation controls from the playable page.
  await tap("[data-aethyr-id=publish-share-close]");
  await options();
  await tap("[data-aethyr-id=project-play]");
  await playerReady(page);
  await shot("19-personal-player");
  await page.goto(origin + planetLink.replace("/play/", "/invite/"));
  await tap("[data-aethyr-id=access-anonymous]");
  await tap("[data-aethyr-id=hours-1]");
  await tap("[data-aethyr-id=invite-create]");
  const invitation = page
    .locator('[data-aethyr-id=invitations] a[href^="/new-lander/visit/"]')
    .last();
  await invitation.waitFor();
  const invitePath = await invitation.getAttribute("href");
  assert(invitePath);
  await shot("20-invitation-created");
  await page.goto(origin + invitePath);
  await playerReady(page);
  // Visits are play only: no chat (owner, 2026-10-24).
  assert.equal(await page.locator("#visit, [id^=visit-]").count(), 0);
  assert(!/chat/i.test(await page.locator("body").innerText()));
  await shot("21-visit-player");
  await visitGuest(invitePath);
  await page.getByRole("link", { name: "Back to planets", exact: true }).tap();
  await page.locator("[data-aethyr-id=planet-owned]").waitFor({ state: "visible" });
  await tap("[data-aethyr-id=planet-setup]");
  await tap("#planet-enter");
  await ready();
  await edit("history");
  await tap("[data-aethyr-id=project-open]");
  await confirm();
  const ownerFrame = page
    .frames()
    .find((f) => f.url().includes("---aethyrforge-editor"));
  assert(ownerFrame);
  const doc = await ownerFrame.evaluate(() =>
    window.aethyrforgeEditor.service.projectDocument(),
  );
  assert.equal(Object.keys(doc.entities).length, 24);
  assert.equal(
    JSON.parse(doc.originals.find((o) => o.name === "planet-v1.json").text)
      .seed,
    42,
  );
  await close();
  await page.locator("[data-aethyr-id=planet-card-toggle]").tap();
  await shot("23-returned-to-saved-planet");
  const poseBefore = (await snap()).status.checkpoint;
  const preview = page
    .frames()
    .find((f) => f.url().includes("---aethyrforge-preview"));
  assert(preview);
  const canvas = await preview.locator("#world").boundingBox();
  assert(canvas);
  const dock = await page.locator("#world-dock").boundingBox();
  const visibleBottom = dock
    ? Math.min(canvas.y + canvas.height, dock.y)
    : canvas.y + canvas.height;
  const cdp = await context.newCDPSession(page),
    x = canvas.x + canvas.width * 0.2,
    y = canvas.y + (visibleBottom - canvas.y) * 0.62;
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ x, y }],
  });
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x, y: y - 60 }],
  });
  await page.waitForTimeout(900);
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  await page.waitForFunction(
    (eye) =>
      JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye.some(
        (v, i) => Math.abs(v - eye[i]) > 0.05,
      ),
    poseBefore.eye,
    { timeout: 15000 },
  );
  await cdp.detach();
  await shot("24-touch-walking");
  await tap("[data-aethyr-id=world-map]");
  await page.waitForURL((url) => url.pathname === "/new-lander/verse");
  await shot("25-world-navigation");

  const result = {
    steps,
    real_account: true,
    api_responses_are_real: true,
    oauth_provider_completed: false,
    physical_iphone: false,
    microphone_used: false,
  };
  assert.deepEqual(
    events.filter((e) => e.error),
    [],
  );
  fs.writeFileSync(dir + "result.json", JSON.stringify(result, null, 2));
  console.log("COMPLETE", JSON.stringify(result));
} catch (e) {
  await page.screenshot({ path: dir + "journey-failure.png" }).catch(() => {});
  fs.writeFileSync(
    dir + "journey-failure.json",
    JSON.stringify(
      {
        error: scrub(e.message),
        path: new URL(page.url()).pathname,
        body: await page.locator("body").innerText(),
        snapshot: await page.evaluate(() => window.hauntDemo?.snapshot()),
      },
      null,
      2,
    ),
  );
  e.message = scrub(e.message);
  if (e.stack) e.stack = scrub(e.stack);
  if (Array.isArray(e.log)) e.log = e.log.map(scrub);
  console.log("FAIL", e.message);
  throw e;
} finally {
  record();
  await context.close();
  await browser.close();
}
