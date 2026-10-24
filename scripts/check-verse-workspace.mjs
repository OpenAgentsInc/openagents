// Guest Jarvis controls with a fresh native Rust website and pinned private engine.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const out = process.env.AETHYRFORGE_DEPLOY_SCRATCH;
assert.ok(out && process.env.OPENAGENTS_TEST_WEB_BINARY);
const config = JSON.parse(
  fs.readFileSync(process.env.AETHYRFORGE_RELEASE_CONFIG),
);
fs.mkdirSync(out, { recursive: true });
const isolatedHome = path.join(out, "browser-native-home");
fs.mkdirSync(isolatedHome, { recursive: true });
const server = spawn(
  process.env.OPENAGENTS_TEST_WEB_BINARY,
  [
    "--listen",
    "127.0.0.1:0",
    "--store",
    path.join(out, "tasks"),
    "--chat-store",
    path.join(out, "chats"),
    "--pylons-relay",
    "off",
    "--profile-relay",
    "off",
  ],
  { env: { ...process.env, HOME: isolatedHome, OPENAGENTS_HAUNT_RELEASE: JSON.stringify({ engine_url: config.editor+"/", sdk_url: config.editor+"/sdk/"+config.sdk_sha256+"/aethyrforge_host.js",release:config.release,engine_abi:"aethyrforge.browser.v1",project_format:1,project:{id:"haunted-house",branch:"main",revision:0}}) } },
);
process.on("exit", () => server.kill("SIGTERM"));
let log = "";
server.stdout.on("data", (d) => (log += d));
server.stderr.on("data", (d) => (log += d));
const until = Date.now() + 60000;
let base;
while (Date.now() < until) {
  base = /listening on (http:\/\/[^ ]+)/.exec(log)?.[1];
  if (base) break;
  if (server.exitCode !== null) throw Error(log);
  await new Promise((r) => setTimeout(r, 50));
}
assert.ok(base, log);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = browser.contexts()[0];
const errors = [],
  privateRequests = [];
let page;
const snapshots = [];
try {
  await context.route("https://openagents.com/**", async (route) => {
    const request = route.request(),
      url = new URL(request.url());
    const response = await context.request.fetch(
      base + url.pathname + url.search,
      {
        headers: { ...request.headers(), host: new URL(base).host, cookie: "" },
        maxRedirects: 0,
      },
    );
    const headers = { ...response.headers() };
    for (const key of [
      "content-length",
      "content-encoding",
      "transfer-encoding",
    ])
      delete headers[key];
    await route.fulfill({
      status: response.status(),
      headers,
      body: await response.body(),
    });
  });
  const open = async () => {
    const p = await context.newPage();
    p.on("pageerror", e => errors.push(e.message));
    p.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
    return p;
  };
  const snapshot = () => page.evaluate(() => JSON.parse(window.hauntDemo.snapshot()));
  const ready = () => page.waitForFunction(() => window.hauntDemo && JSON.parse(window.hauntDemo.snapshot()).phase === "open" && JSON.parse(window.hauntDemo.snapshot()).status.authoring, null, {timeout:125000});
  page = await open();
  await page.setViewportSize({width:1440,height:1000});
  await page.goto("https://openagents.com/new-lander/house");
  await ready();
  const first = await snapshot();
  const editor = page.frames().find(f => f.url().startsWith(config.editor));
  const preview = page.frames().find(f => f.url().startsWith(config.preview));
  assert.ok(editor && preview);
  await editor.evaluate(() => window.aethyrforge.cameraPose([0,1.62,-12],[-2.4,1.3,-14.8],undefined,"walk"));
  await preview.locator("#world").click();
  await page.mouse.click(720,500);
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => JSON.parse(window.hauntDemo.snapshot()).status.selection?.id === "werewolf-1");
  await page.click("[data-aethyr-id=world-edit]");
  await page.locator("[data-aethyr-id=panel-materials]").waitFor({state:"visible"});
  for (const name of ["objects","materials","request","compile","history"]) assert.ok(await page.locator(`[data-aethyr-id=panel-${name}]`).isVisible(),name);
  assert.equal(await page.locator("#engine").getAttribute("inert"), "");
  await page.fill("[data-aethyr-id=edit-note] :is(input,textarea)", "Keep the werewolf selection and this unsent note.");
  await page.fill("[data-aethyr-id=edit-kiln-request] :is(input,textarea)", "Keep this request; do not apply it yet.");
  const source = await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue();
  await page.fill("[data-aethyr-id=edit-source] :is(input,textarea)",source+"\n// unsent browser check");
  const beforeTyping = await snapshot();
  await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").focus();
  await page.keyboard.type(" WASD");
  await page.waitForTimeout(1100);
  const afterTyping=await snapshot();
  assert.deepEqual(afterTyping.status.checkpoint,beforeTyping.status.checkpoint);
  assert.equal(afterTyping.session,first.session);
  await page.screenshot({path:path.join(out,"jarvis-desktop.png")});
  const note=await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue();
  for(let i=0;i<3;i++){await page.click("[data-aethyr-id=edit-close]");await page.click("[data-aethyr-id=world-edit]");}
  assert.equal((await snapshot()).session, first.session);
  assert.equal(await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),note);
  if(await page.locator("[data-aethyr-id=edit-fog]").isHidden())await page.click("[data-aethyr-id=material-fog-toggle]");
  await page.click("[data-aethyr-id=edit-fog-apply]");
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({state:"visible"});
  await page.locator("[data-aethyr-id=edit-confirm]").waitFor({ state: "hidden" });
  await page.click("[data-aethyr-id=edit-cancel]");
  assert.equal((await snapshot()).status.project.revision,0);
  assert.equal(await page.locator("[data-aethyr-id=edit-note] :is(input,textarea)").inputValue(),note);
  await page.click("[data-aethyr-id=edit-close]");
  await page.click("[data-aethyr-id=world-mesh]");
  await page.waitForFunction(() => JSON.parse(window.hauntDemo.snapshot()).status.view === "mesh");
  assert.equal((await snapshot()).status.project.revision,0);
  await page.screenshot({path:path.join(out,"jarvis-mesh.png")});
  await page.click("[data-aethyr-id=world-mesh]");
  await page.waitForFunction(() => JSON.parse(window.hauntDemo.snapshot()).status.view === "shaded");
  await page.click("[data-aethyr-id=world-edit]");
  await page.setViewportSize({width:390,height:844});
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(()=>document.querySelector("#jarvis-toolbar").getAttribute("role")),"tablist");
  for(const name of ["objects","materials","request","compile","history"]){
    await page.click(`[data-aethyr-id=tab-${name}]`);
    await page.locator(`[data-aethyr-id=panel-${name}]`).waitFor({ state: "visible" });
    for(const other of ["objects","materials","request","compile","history"].filter(n=>n!==name)) await page.locator(`[data-aethyr-id=panel-${other}]`).waitFor({ state: "hidden" });
  }
  await page.click("[data-aethyr-id=tab-request]");
  await page.screenshot({path:path.join(out,"jarvis-mobile.png")});
  await page.reload(); await ready();
  await page.waitForFunction(() => JSON.parse(window.hauntDemo.snapshot()).status.selection?.id === "werewolf-1");
  // The editor stays closed after a reload; the drafts are restored into the release's fields.
  assert.equal(await page.locator("#edit-note").inputValue(),note);
  assert.equal(await page.locator("#edit-kiln-request").inputValue(),"Keep this request; do not apply it yet.");
  assert.equal(await page.locator("#edit-source").inputValue(),source+"\n// unsent browser check");
  assert.notEqual((await snapshot()).session,first.session);
  snapshots.push(await snapshot());
  assert.deepEqual(errors,[]);
  fs.writeFileSync(path.join(out,"browser-workspace.json"),JSON.stringify({release:config.release,errors,desktop_windows:true,mobile_tabs:true,typing_pose_preserved:true,panel_reopen_keeps_session:true,guest_pending_cancel:true,readonly_mesh:true,reload_draft_selection_restored:true,real_provider:false,snapshots},null,2));
} finally {
  if(page) await page.close().catch(()=>{});
  await browser.close();
  server.kill("SIGTERM");
  fs.writeFileSync(path.join(out,"native.log"),log);
}
