// Voice-led controls with synthetic speech/account events and the real pinned GPU world.
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
  await context.addInitScript(() => {
    // Synthetic speech events test the host state machine, not a microphone or provider.
    window.speechFixture={starts:0,instances:[],last:null};
    window.SpeechRecognition=class {
      start(){window.speechFixture.starts++;window.speechFixture.instances.push(this);window.speechFixture.last=this;this.queued=this.onresult;}
      abort(){this.aborted=true;}
    };
  });
  page=await context.newPage();
  page.on("pageerror",e=>errors.push(e.message));
  page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
  await page.setViewportSize({width:1440,height:1000});
  const ready=()=>page.waitForFunction(()=>window.hauntDemo && JSON.parse(window.hauntDemo.snapshot()).phase==="open" && JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:125000});
  const snapshot=()=>page.evaluate(()=>JSON.parse(window.hauntDemo.snapshot()));
  const phrase=async(text)=>{
    await page.waitForFunction(()=>window.speechFixture.last && !window.speechFixture.last.aborted);
    await page.evaluate(text=>{const r=window.speechFixture.last;r.onspeechstart?.({});r.onresult({results:[[{transcript:text}]]});},text);
    await page.waitForTimeout(250);
  };
  await page.goto("https://openagents.com/new-lander");
  assert.equal(await page.evaluate(()=>window.speechFixture.starts),0);
  await page.goto("https://openagents.com/new-lander/house");await ready();
  assert.equal(await page.evaluate(()=>window.speechFixture.starts),0);
  const original=await snapshot();
  await page.click("[data-aethyr-id=edit-voice-start]");
  await phrase("show options");
  await page.locator("[data-aethyr-id=request-choices]").waitFor({ state: "visible" });
  await page.locator("[data-aethyr-id=edit-panel]").waitFor({ state: "hidden" });
  await phrase("show mesh");
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).status.view==="mesh");
  await phrase("hide mesh");
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).status.view==="shaded");
  await phrase("compile");
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({state:"visible"});
  const spoken=JSON.parse(await page.evaluate(()=>window.hauntDemo.pending_change()));
  assert.equal(spoken.pending.action.type,"compile");assert.match(spoken.pending.action.source_sha256,/^[a-f0-9]{64}$/);
  await phrase("confirm change");assert.equal(grants,0);
  await phrase("cancel change");await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "hidden" });
  await page.click("[data-aethyr-id=world-edit]");
  await page.click("[data-aethyr-id=edit-source-compile]");
  const clicked=JSON.parse(await page.evaluate(()=>window.hauntDemo.pending_change()));
  assert.deepEqual(clicked.pending.action,spoken.pending.action);
  await phrase("cancel change");await phrase("inspect");
  await page.locator("[data-aethyr-id=edit-panel]").waitFor({ state: "hidden" });
  assert.match(await page.locator("[data-aethyr-id=edit-voice-status]").innerText(),/Listening/);
  await phrase("share");assert.match(await page.locator("[data-aethyr-id=voice-response]").innerText(),/isn't available/);
  assert.equal((await snapshot()).status.project.revision,0);
  await phrase("show options");
  await page.locator("[data-aethyr-id=request-choices]").waitFor({ state: "visible" });
  assert.equal(await page.locator("[data-aethyr-id=voice-heard]").innerText(),"show options");
  assert.equal(await page.locator("[data-aethyr-id=voice-response]").innerText(),"Choose an option, or say its name.");
  // The editor is closed here; its status line is the release's bridge copy.
  assert.equal(await page.locator("#edit-status").textContent(),"Choose an option, or say its name.");
  await page.screenshot({path:path.join(out,"voice-world-desktop.png")});
  // Hold a queued recognition callback, stop, and deliver it late.
  await page.waitForFunction(()=>window.speechFixture.last && !window.speechFixture.last.aborted);
  await page.evaluate(()=>window.lateSpeech=window.speechFixture.last.queued);
  await page.click("[data-aethyr-id=edit-voice-stop]");const starts=await page.evaluate(()=>window.speechFixture.starts);
  await page.evaluate(()=>window.lateSpeech({results:[[{transcript:"compile"}]]}));await page.waitForTimeout(800);
  assert.equal(await page.evaluate(()=>window.speechFixture.starts),starts);
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({ state: "hidden" });
  await page.click("[data-aethyr-id=edit-voice-start]");
  await page.evaluate(()=>window.speechFixture.last.onerror({error:"not-allowed"}));await page.waitForTimeout(700);
  assert.match(await page.locator("[data-aethyr-id=edit-voice-status]").innerText(),/denied/);
  const deniedStarts=await page.evaluate(()=>window.speechFixture.starts);await page.waitForTimeout(700);
  assert.equal(await page.evaluate(()=>window.speechFixture.starts),deniedStarts);
  await page.screenshot({path:path.join(out,"voice-permission-denied.png")});
  await page.setViewportSize({width:390,height:844});
  await page.click("[data-aethyr-id=edit-voice-start]");await phrase("share");
  assert.match(await page.locator("[data-aethyr-id=voice-response]").innerText(),/isn't available/);
  await phrase("show options");
  assert.equal(await page.locator("[data-aethyr-id=voice-response]").innerText(),"Choose an option, or say its name.");
  await page.screenshot({path:path.join(out,"voice-world-mobile.png")});
  await phrase("stop");assert.match(await page.locator("[data-aethyr-id=edit-voice-status]").innerText(),/off/);
  assert.equal((await snapshot()).session,original.session);
  await page.reload();await ready();assert.equal(await page.evaluate(()=>window.speechFixture.starts),0);
  assert.deepEqual(errors,[]);
  fs.writeFileSync(path.join(out,"browser-voice.json"),JSON.stringify({release:config.release,synthetic_speech:true,real_speech_provider:false,real_signin_provider:false,errors,in_world_options:true,current_feedback_after_new_request:true,spoken_controls:true,exact_compile_equivalence:true,guest_confirm_does_not_grant:true,stop_late_event_ignored:true,denial_no_resume:true,reload_no_resume:true,same_world_session:true},null,2));
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
