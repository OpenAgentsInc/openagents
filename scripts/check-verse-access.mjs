// Native pending-edit browser conformance with an explicitly synthetic account service.
// Real test-account provider completion is a separate deployment qualification.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import net from "node:net";
import { randomBytes } from "node:crypto";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const out = process.env.AETHYRFORGE_DEPLOY_SCRATCH;
assert.ok(out && process.env.OPENAGENTS_TEST_WEB_BINARY);
fs.mkdirSync(out, { recursive: true });
const config = JSON.parse(
  fs.readFileSync(process.env.AETHYRFORGE_RELEASE_CONFIG),
);
const fixtureKey = "oak_fixture.synthetic-browser-key";
const fixtureToken = "sess_" + "a".repeat(64);
let active = true;
const expiry = Math.floor(Date.now() / 1000) + 3600;
const fixtureSession = {
  id: "fixture-browser-session",
  kind: "user",
  account: "fixture-account",
  created_at: Math.floor(Date.now() / 1000) - 1,
  expires_at: expiry,
  state: "active",
};
const accountServer = http.createServer(async (req, res) => {
  const auth = req.headers.authorization;
  if (req.method === "POST")
    console.log("Synthetic account route", req.method, req.url);
  const send = (status, value) => {
    res.writeHead(status, { "content-type": "application/json" });
    res.end(JSON.stringify(value));
  };
  if (
    req.url === "/v1/sessions" &&
    req.method === "POST" &&
    auth === "Bearer " + fixtureKey
  ) {
    active = true;
    return send(200, { session: fixtureSession, token: fixtureToken });
  }
  if (auth !== "Bearer " + fixtureToken || !active)
    return send(401, {
      error: {
        code: "unauthenticated",
        message: "Synthetic fixture signed out.",
      },
    });
  if (req.url === "/v1/session" && req.method === "DELETE") {
    active = false;
    return send(200, { ended: true });
  }
  if (req.url === "/v1/session") return send(200, { session: fixtureSession });
  if (req.url === "/v1/account")
    return send(200, {
      account: {
        id: "fixture-account",
        label: "Browser fixture",
        principals: [],
      },
      workspaces: [],
    });
  return send(404, {
    error: { code: "not_found", message: "Synthetic fixture route not found." },
  });
});
await new Promise((resolve) => accountServer.listen(0, "127.0.0.1", resolve));
const privateDir = path.join(out, "fixture-private");
fs.mkdirSync(privateDir, { recursive: true, mode: 0o700 });
fs.chmodSync(privateDir, 0o700);
const reservation = net.createServer();
await new Promise((resolve) => reservation.listen(0, "127.0.0.1", resolve));
const nativePort = reservation.address().port;
await new Promise((resolve) => reservation.close(resolve));
const nativeOrigin = `http://127.0.0.1:${nativePort}`;
const secret = path.join(privateDir, "fixture-csrf.key");
fs.writeFileSync(secret, randomBytes(32), { mode: 0o600 });
const cloud = path.join(privateDir, "fixture-cloud.json");
fs.writeFileSync(
  cloud,
  JSON.stringify({
    schema: "openagents.cloud.web-config.v1",
    public_origin: nativeOrigin,
    account_service: `http://127.0.0.1:${accountServer.address().port}`,
    csrf_secret: secret,
  }),
  { mode: 0o600 },
);
const release = {
  engine_url: config.editor + "/",
  sdk_url: config.editor + "/sdk/" + config.sdk_sha256 + "/aethyrforge_host.js",
  release: config.release,
  engine_abi: "aethyrforge.browser.v1",
  project_format: 1,
  project: { id: "haunted-house", branch: "main", revision: 0 },
};
const isolatedHome = path.join(out, "native-home");
fs.mkdirSync(isolatedHome, { recursive: true });
const server = spawn(
  process.env.OPENAGENTS_TEST_WEB_BINARY,
  [
    "--listen",
    `127.0.0.1:${nativePort}`,
    "--store",
    path.join(out, "tasks"),
    "--chat-store",
    path.join(out, "chats"),
    "--cloud-config",
    cloud,
    "--pylons-relay",
    "off",
    "--profile-relay",
    "off",
  ],
  {
    env: {
      ...process.env,
      HOME: isolatedHome,
      OPENAGENTS_HAUNT_RELEASE: JSON.stringify(release),
    },
  },
);
process.on("exit", () => server.kill("SIGTERM"));
let log = "";
server.stdout.on("data", (data) => (log += data));
server.stderr.on("data", (data) => (log += data));
const deadline = Date.now() + 60000;
let base;
while (Date.now() < deadline) {
  base = /listening on (http:\/\/[^ ]+)/.exec(log)?.[1];
  if (base) break;
  if (server.exitCode !== null) throw new Error(log);
  await new Promise((r) => setTimeout(r, 50));
}
assert.ok(base, log);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = browser.contexts()[0];
let page;
const errors = [];
let grants = 0;
let lastGrant;
try {
  await context.route("https://openagents.com/**", async (route) => {
    const request = route.request(),
      url = new URL(request.url());
    if (url.pathname === "/sign-in")
      console.log("Routed synthetic sign-in", request.method());
    const headers = {
      ...request.headers(),
      host: new URL(base).host,
      cookie: request.headers().cookie || "",
    };
    delete headers["content-length"];
    for (const key of ["origin", "referer"]) {
      if (headers[key]?.startsWith("https://openagents.com"))
        headers[key] = headers[key].replace(
          "https://openagents.com",
          nativeOrigin,
        );
    }
    const response = await context.request.fetch(
      base + url.pathname + url.search,
      {
        method: request.method(),
        headers,
        data: request.postDataBuffer() || undefined,
        maxRedirects: 0,
      },
    );
    if (
      url.pathname.startsWith("/auth/") ||
      url.pathname === "/login" ||
      url.pathname === "/sign-in"
    )
      console.log(
        "Synthetic sign-in route",
        url.pathname,
        response.status(),
        response.headers().location,
      );
    const returned = { ...response.headers() };
    for (const key of [
      "content-encoding",
      "content-length",
      "transfer-encoding",
    ])
      delete returned[key];
    // CDP-intercepted first-navigation 303s stall even in the independent tiny probe.
    // Assert native redirects, then adapt their transport explicitly for this synthetic fixture.
    // Real provider redirect completion remains a separate qualification.
    if (response.status() >= 300 && response.status() < 400) {
      const destination = new URL(returned.location, request.url());
      assert.equal(destination.origin, "https://openagents.com");
      delete returned.location;
      delete returned["content-security-policy"];
      returned["content-type"] = "text/html";
      await route.fulfill({
        status: 200,
        headers: returned,
        body: `<script>location.replace(${JSON.stringify(destination.href)})</script>`,
      });
      return;
    }
    let body =
      response.status() >= 300 && response.status() < 400
        ? Buffer.alloc(0)
        : await response.body();
    if (url.pathname === "/v1/verse/studio/grants" && response.ok()) {
      grants++;
      lastGrant = JSON.parse(body).grant;
    }
    await route.fulfill({ status: response.status(), headers: returned, body });
    if (url.pathname.startsWith("/auth/"))
      console.log("Auth response fulfilled", url.pathname);
  });
  // Resource errors this check expects: its own wrong-tab authorization below
  // is refused on purpose, and the local website consumer runs without the
  // Everglade and Pylons wasm builds that the home page loads in production.
  const expectedResourceError = (message) => {
    if (!message.text().startsWith("Failed to load resource")) return false;
    const url = message.location().url;
    if (!url) return false;
    const { origin, pathname } = new URL(url);
    if (origin !== "https://openagents.com") return false;
    if (message.text().includes("status of 403"))
      return pathname === "/v1/verse/studio/authorize";
    return (
      message.text().includes("status of 404") &&
      /^\/(everglade\/everglade_web|pylons\/layer\/pylons_web)(_bg\.wasm|\.js)$/.test(pathname)
    );
  };
  context.on("page", (popup) =>
    popup.on("console", (message) => {
      if (message.type() === "error" && !expectedResourceError(message))
        errors.push("Popup: " + message.text());
    }),
  );
  page = await context.newPage();
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => {
    window.openProbe = [];
    const open = window.open;
    window.open = function (...args) {
      const result = open.apply(window, args);
      window.openProbe.push({
        args,
        opened: Boolean(result),
        closed: result?.closed,
      });
      return result;
    };
  });
  page.on("console", (m) => {
    if (m.type() === "error" && !expectedResourceError(m)) errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("request", (request) => {
    if (
      request.url().startsWith(config.editor) ||
      request.url().startsWith(config.preview)
    ) {
      assert.equal(request.headers().authorization, undefined);
      assert.equal(request.headers().cookie, undefined);
    }
  });
  const snapshot = () =>
    page.evaluate(() => JSON.parse(window.hauntDemo.snapshot()));
  const ready = () =>
    page.waitForFunction(
      () =>
        window.hauntDemo &&
        JSON.parse(window.hauntDemo.snapshot()).phase === "open",
      null,
      { timeout: 125000 },
    );
  await page.goto("https://openagents.com/new-lander/house");
  await ready();
  const editor = page.frames().find((f) => f.url().startsWith(config.editor));
  assert.ok(editor);
  const preview = page.frames().find((f) => f.url().startsWith(config.preview));
  assert.ok(preview);
  // The edit panel is modal and makes the world inert, so select the
  // werewolf first (as check-verse-material does), then open Edit. Picking
  // by pointer is covered by check-verse-workspace.
  await editor.evaluate(() =>
    window.aethyrforgeEditor.service.command({
      kind: "entity.select",
      id: "werewolf-1",
    }),
  );
  await page.waitForFunction(
    () =>
      JSON.parse(window.hauntDemo.snapshot()).status.selection?.id ===
      "werewolf-1",
  );
  await page.click("[data-aethyr-id=world-edit]");
  await page.locator("[data-aethyr-id=edit-panel]").waitFor({ state: "visible" });
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "hidden" });
  await page.waitForFunction(
    () =>
      document.querySelector("#edit-target").textContent === "werewolf-1" &&
      !document.querySelector("#edit-submit").disabled,
  );
  await page.fill("[data-aethyr-id=edit-x] :is(input,textarea)", "-2.1");
  await page.fill("[data-aethyr-id=edit-y] :is(input,textarea)", "0");
  await page.fill("[data-aethyr-id=edit-note] :is(input,textarea)", "Keep this requested move while signing in.");
  const before = await snapshot();
  await page.click("[data-aethyr-id=edit-submit]");
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "visible" });
  const pending = await page.evaluate(() =>
    JSON.parse(window.hauntDemo.pending_change()),
  );
  assert.equal(grants, 0);
  await page.locator("[data-aethyr-id=edit-confirm]").waitFor({ state: "hidden" });
  await page.click("[data-aethyr-id=edit-cancel]");
  // The controller closes the dialog on its next render.
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "hidden" });
  assert.equal(
    await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),
    pending.pending.draft,
  );
  await page.click("[data-aethyr-id=edit-submit]");
  const popup = await Promise.all([
    context.waitForEvent("page"),
    page.click("[data-aethyr-id=edit-signin]"),
  ]).then((v) => v[0]);
  await popup.waitForLoadState("domcontentloaded");
  await popup.close();
  await page.waitForTimeout(2200);
  assert.equal(grants, 0);
  assert.equal(
    (await snapshot()).status.project.revision,
    before.status.project.revision,
  );
  // A full navigation exercises the allowlisted checkpoint return independently of the popup.
  await page.reload();
  await ready();
  await page.waitForFunction(
    () =>
      JSON.parse(window.hauntDemo.snapshot()).status.selection?.id ===
      "werewolf-1",
  );
  await page.locator("[data-aethyr-id=edit-panel]").waitFor({ state: "visible" });
  assert.equal(
    await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),
    pending.pending.draft,
  );
  assert.equal(await page.locator("[data-aethyr-id=edit-x] :is(input,textarea)").inputValue(), "-2.1");
  const resumed = await snapshot();
  for (let i = 0; i < 3; i++)
    assert.ok(
      Math.abs(
        resumed.status.checkpoint.eye[i] - before.status.checkpoint.eye[i],
      ) < 0.05,
    );
  await page.click("[data-aethyr-id=edit-submit]");
  const login = await Promise.all([
    context.waitForEvent("page"),
    page.click("[data-aethyr-id=edit-signin]"),
  ]).then((v) => v[0]);
  await login.waitForURL("**/sign-in");
  await login.fill('input[name="credential"]', fixtureKey);
  const [loginResponse] = await Promise.all([
    login.waitForResponse(
      (r) =>
        new URL(r.url()).pathname === "/sign-in" &&
        r.request().method() === "POST",
    ),
    login.locator('form[action="/sign-in"] button[type="submit"]').click(),
  ]);
  assert.ok(loginResponse.ok());
  await page
    .locator("[data-aethyr-id=edit-confirm]")
    .waitFor({ state: "visible", timeout: 20000 });
  const signed = await snapshot();
  assert.equal(signed.session, resumed.session);
  assert.equal(signed.status.generation, resumed.status.generation);
  assert.deepEqual(signed.status.checkpoint, resumed.status.checkpoint);
  assert.equal(signed.status.selection.id, "werewolf-1");
  assert.equal(signed.status.project.revision, 0);
  assert.equal(grants, 0);
  assert.equal(
    await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),
    pending.pending.draft,
  );
  await login.close();
  active = false;
  await page
    .locator("[data-aethyr-id=edit-signin]")
    .waitFor({ state: "visible", timeout: 15000 });
  assert.equal(
    await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),
    pending.pending.draft,
  );
  assert.equal((await snapshot()).session, resumed.session);
  const again = await Promise.all([
    context.waitForEvent("page"),
    page.click("[data-aethyr-id=edit-signin]"),
  ]).then((v) => v[0]);
  await again.waitForURL("**/sign-in");
  await again.fill('input[name="credential"]', fixtureKey);
  const [renewedResponse] = await Promise.all([
    again.waitForResponse(
      (r) =>
        new URL(r.url()).pathname === "/sign-in" &&
        r.request().method() === "POST",
    ),
    again.locator('form[action="/sign-in"] button[type="submit"]').click(),
  ]);
  assert.ok(renewedResponse.ok());
  await page
    .locator("[data-aethyr-id=edit-confirm]")
    .waitFor({ state: "visible", timeout: 20000 });
  await again.close();
  assert.equal(grants, 0);
  await page.screenshot({ path: path.join(out, "explicit-confirmation.png") });
  await page.click("[data-aethyr-id=edit-confirm]");
  await page.waitForFunction(
    () =>
      document.querySelector("#edit-status").textContent ===
      "We applied your change.",
    null,
    { timeout: 30000 },
  );
  await page.waitForFunction(
    () =>
      Math.abs(
        JSON.parse(window.hauntDemo.snapshot()).status.selection.position[0] +
          2.1,
      ) < 0.02,
  );
  const applied = await snapshot();
  assert.equal(grants, 1);
  assert.equal(applied.status.project.revision, 1);
  assert.equal(applied.status.save, "unsaved");
  assert.notEqual(lastGrant.draft.id, applied.status.project.id);
  const csrf = await page.evaluate(async () => {
    const r = await fetch("/v1/verse/studio/session");
    return (await r.json()).csrf;
  });
  const rejection = await page.evaluate(
    async ({ grant, csrf, action }) => {
      const r = await fetch("/v1/verse/studio/authorize", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ csrf, tab: "wrong-tab", grant, action }),
      });
      return r.status;
    },
    { grant: lastGrant, csrf, action: pending.pending.action },
  );
  assert.equal(rejection, 403);
  await page.screenshot({ path: path.join(out, "confirmed-move.png") });
  const result = {
    qualification:
      "synthetic account service with real native sign-in form and hosted engine; not real test-account provider completion",
    browser: browser.version(),
    protocol_version: 2,
    artifact_override: false,
    redirect_transport_adapter:
      "native 303 asserted; browser navigation adapted around independently reproduced CDP interception stall",
    expired_login_preserves_request_and_requires_confirmation: true,
    release: config.release,
    guest_edit_opens: true,
    cancel_keeps_draft_and_view: true,
    canceled_popup_keeps_world: true,
    redirect_checkpoint_restores_tools_draft_selection: true,
    login_does_not_execute: true,
    confirmation_required: true,
    confirmed_move_changes_real_engine: true,
    wrong_tab_refused_by_server: true,
    private_draft_separate_from_tutorial: true,
    save_state: applied.status.save,
    errors,
  };
  assert.deepEqual(errors, []);
  fs.writeFileSync(
    path.join(out, "pending-edit-fixture.json"),
    JSON.stringify(result, null, 2) + "\n",
  );
  console.log(JSON.stringify(result));
  await page.close();
} catch (error) {
  console.log(
    JSON.stringify({
      errors,
      pages: context.pages().map((p) => p.url()),
      targets: await (
        await browser.newBrowserCDPSession()
      ).send("Target.getTargets"),
      openProbe: page ? await page.evaluate(() => window.openProbe) : null,
      form: page
        ? await page.evaluate(() =>
            Array.from(document.querySelectorAll("#edit-form input")).map(
              (e) => ({
                id: e.id,
                value: e.value,
                step: e.step,
                valid: e.validity.valid,
                disabled: e.disabled,
              }),
            ),
          )
        : null,
      pending: page
        ? await page.evaluate(() => window.hauntDemo?.pending_change())
        : null,
      body: page ? await page.locator("body").innerText() : null,
      snapshot: page
        ? await page.evaluate(() => window.hauntDemo?.snapshot())
        : null,
    }),
  );
  if (page)
    await page.screenshot({ path: path.join(out, "browser-error.png") });
  throw error;
} finally {
  await context.close();
  await browser.close();
  server.kill("SIGTERM");
  accountServer.close();
  fs.writeFileSync(path.join(out, "native-fixture.log"), log);
}
