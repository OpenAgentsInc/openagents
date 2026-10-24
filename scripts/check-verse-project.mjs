// Durable native project storage and real pinned engine, with a synthetic account service.
// No microphone/ASR call, real provider login, or payment is exercised.
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
assert.ok(config.protocol_version >= 5, "Use an editable wire-v5 release");
const projects = [];
let delayCreate=false, releaseCreate, createArrived;
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
  if (req.url === "/v1/account/github") return send(200,{github:{state:"none"},projects});
  if ((req.url === "/v1/projects" || req.url === "/v1/account/projects") && req.method === "POST") {
    let raw="";for await(const part of req)raw+=part;
    const body=JSON.parse(raw);
    const project={id:"prj_"+(projects.length+1).toString(16).padStart(16,"0"),name:body.name,instructions:null,created_unix:Math.floor(Date.now()/1000),repositories:[]};
    projects.push(project);if(delayCreate){delayCreate=false;createArrived?.();await new Promise(resolve=>releaseCreate=resolve);}return send(200,{project});
  }
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
if(process.env.AETHYRFORGE_LOCAL_CHECK==="1")await context.grantPermissions(["local-network-access"],{origin:"https://openagents.com"});
if(process.env.AETHYRFORGE_LOCAL_PROXY_CONFIG) {
  const proxy=JSON.parse(fs.readFileSync(process.env.AETHYRFORGE_LOCAL_PROXY_CONFIG));
  for(const [origin,upstream] of Object.entries(proxy)) {
    assert.ok(origin===config.editor||origin===config.preview);
    assert.equal(new URL(upstream).hostname,"127.0.0.1");
    await context.route(origin+"/**",async route=>{
      const incoming=new URL(route.request().url());
      // A separate Node fetch has no account-service cookies or credentials.
      const response=await fetch(upstream+incoming.pathname+incoming.search,{redirect:"manual"});
      const headers=Object.fromEntries(response.headers);
      for(const key of ["content-encoding","content-length","transfer-encoding"])delete headers[key];
      const body=response.status>=300&&response.status<400?Buffer.alloc(0):Buffer.from(await response.arrayBuffer());
      await route.fulfill({status:response.status,headers,body});
    });
  }
}
let page;
const errors = [];
let grants = 0;
let lastGrant;
let delaySave=false, releaseSave, saveArrived;
const savedPosts=[];
let delayCheckpoint=false, releaseCheckpoint, checkpointArrived;
let delayInspection=false, releaseInspection, inspectionArrived;
const checkHistory=process.env.AETHYRFORGE_CHECK_HISTORY === "1";

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
        timeout: 120000,
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
    if(request.method()==="POST" && /\/projects\/prj_[a-f0-9]+\/haunted-house\/main$/.test(url.pathname)) {
      savedPosts.push(JSON.parse(request.postData()));
      if(delaySave){delaySave=false;saveArrived?.();await new Promise(resolve=>releaseSave=resolve);}
    }
    if(request.method()==="POST" && url.pathname.endsWith("/checkpoint") && delayCheckpoint) {
      delayCheckpoint=false;checkpointArrived?.();await new Promise(resolve=>releaseCheckpoint=resolve);
      await route.fulfill({status:503,headers:{"content-type":"application/json"},body:JSON.stringify({error:{message:"Synthetic late checkpoint transport failure"}})});return;
    }
    if(delayInspection && url.pathname.endsWith("/history/1")) {
      delayInspection=false;inspectionArrived?.();await new Promise(resolve=>releaseInspection=resolve);
    }
    await route.fulfill({ status: response.status(), headers: returned, body });
    if (url.pathname.startsWith("/auth/"))
      console.log("Auth response fulfilled", url.pathname);
  });

  page=await context.newPage();
  page.on("pageerror",e=>errors.push(e.message));
  page.on("console",m=>{if(m.type()==="error")console.log("Browser console:",m.text());});
  await page.setViewportSize({width:1440,height:1000});
  if(process.env.AETHYRFORGE_FIXTURE_DIRECT_SESSION==="1") {
    // This fixture bootstrap does not exercise browser/provider sign-in.
    const formPage=await context.request.get(base+"/sign-in");
    const markup=await formPage.text();
    const form=await page.evaluate(markup=>{
      const document=new DOMParser().parseFromString(markup,"text/html");
      const element=document.querySelector('form[action="/sign-in"]');
      if(!element)throw Error("Native fixture sign-in form missing");
      return Object.fromEntries(new FormData(element));
    },markup);
    form.credential=fixtureKey;
    console.log("Fixture bootstrap: parsed native form");
    const response=await context.request.post(base+"/sign-in",{form,headers:{origin:base,referer:base+"/sign-in"},maxRedirects:0});
    console.log("Fixture bootstrap: response",response.status());
    fs.writeFileSync(path.join(out,"fixture-form.html"),markup);
    if(response.status()<300||response.status()>=400)fs.writeFileSync(path.join(out,"fixture-bootstrap-response.html"),await response.text());
    assert.equal(response.status(),303,"Native fixture form fields: "+Object.keys(form).join(", "));
    const cookies=await context.cookies(base);
    console.log("Fixture bootstrap: cookie count",cookies.length);
    assert.ok(cookies.length,"Native fixture session cookie");
    await context.addCookies(cookies.map(cookie=>({...cookie,domain:"openagents.com",secure:true})));
  } else {
  await page.goto("https://openagents.com/sign-in");
  await page.fill('input[name="credential"]',fixtureKey);
  await Promise.all([
    page.waitForURL("https://openagents.com/", {waitUntil:"domcontentloaded"}),
    page.locator('form[action="/sign-in"] button[type="submit"]').click(),
  ]);
  }
  console.log("Fixture host: opening house");
  await page.goto("https://openagents.com/new-lander/house");
  console.log("Fixture host: house document opened");
  await page.waitForFunction(()=>window.hauntDemo && JSON.parse(window.hauntDemo.snapshot()).phase==="open" && JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:process.env.AETHYRFORGE_LOCAL_CHECK==="1"?30000:125000});
  await page.click("[data-aethyr-id=world-edit]");
  await page.click("[data-aethyr-id=tab-history]");
  await page.fill("[data-aethyr-id=project-name] :is(input,textarea)","Synthetic saved house");
  await page.click("[data-aethyr-id=project-create]");
  await page.waitForFunction(()=>document.querySelector("#project-picker").value.startsWith("prj_"));
  const first=await page.locator("[data-aethyr-id=project-picker] select").inputValue();
  const original=await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue();
  const saving=original+"\n// saved first draft";
  await page.fill("[data-aethyr-id=edit-source] :is(input,textarea)",saving);
  await page.click("[data-aethyr-id=project-save-copy]");
  await page.locator("[data-aethyr-id=edit-confirm]").waitFor({state:"visible"});
  const arrival=new Promise(resolve=>saveArrived=resolve);delaySave=true;
  await page.click("[data-aethyr-id=edit-confirm]");await arrival;
  await page.click("[data-aethyr-id=edit-cancel]");
  const creationsBefore=projects.length;
  await page.click("[data-aethyr-id=project-create]");
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("finish before creating"));
  assert.equal(projects.length,creationsBefore,"Create during a Save must not start a request or change context");
  assert.equal(await page.locator("[data-aethyr-id=project-picker] select").inputValue(),first);
  const newer=saving+"\n// newer typing during the network wait";
  await page.fill("[data-aethyr-id=edit-source] :is(input,textarea)",newer);
  releaseSave();
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("Newer edits"));
  assert.equal(await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue(),newer);
  assert.equal(savedPosts[0].document.source_draft,saving);
  assert.equal(savedPosts[0].expected_version,0);
  await page.click("[data-aethyr-id=project-save-copy]");await page.click("[data-aethyr-id=edit-confirm]");
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("version 2"));
  assert.equal(savedPosts[1].expected_version,1);
  assert.equal(savedPosts[1].document.source_draft,newer);
  if(checkHistory) {
    await page.click("[data-aethyr-id=project-history-refresh]");
    await page.locator('[data-inspect-version="2"]').waitFor();
    const historyRows=await page.locator("[data-aethyr-id=project-history-rows]").innerText();
    assert.ok(historyRows.includes("Earnings: Not recorded"));
    assert.ok(historyRows.includes("Draft source")&&!historyRows.includes("Saved editable world"),"The list identifies accepted changes before inspection");
    const beforeInspection=JSON.parse(await page.evaluate(()=>window.hauntDemo.snapshot()));
    const arrived=new Promise(resolve=>inspectionArrived=resolve);delayInspection=true;
    await page.click('[data-inspect-version="1"]');await arrived;
    await page.click('[data-inspect-version="2"]');
    await page.waitForFunction(()=>document.querySelector("#project-history-changes").textContent.includes("Draft source"));
    releaseInspection();await page.waitForTimeout(250);
    assert.equal(await page.locator("[data-aethyr-id=project-history-version]").innerText(),"Version 2");
    const review=await page.locator("[data-aethyr-id=project-history-changes]").innerText();
    assert.ok(review.includes("Draft source")&&!review.includes("oa_material")&&!review.includes('"path"'),"Main inspection is a readable field list, not raw source/JSON");
    const afterInspection=JSON.parse(await page.evaluate(()=>window.hauntDemo.snapshot()));
    assert.equal(afterInspection.status.project.revision,beforeInspection.status.project.revision,"Inspection does not mutate the world");
    const [download]=await Promise.all([page.waitForEvent("download"),page.click("[data-aethyr-id=project-history-download]")]);
    await download.saveAs(path.join(out,"inspected-editable-version.json"));
    assert.equal(JSON.parse(fs.readFileSync(path.join(out,"inspected-editable-version.json"))).source_draft,newer);
    await page.screenshot({path:path.join(out,"contribution-review-desktop.png")});
    await page.setViewportSize({width:390,height:844});
    await page.locator("[data-aethyr-id=project-history-detail]").scrollIntoViewIfNeeded();
    await page.screenshot({path:path.join(out,"contribution-review-mobile.png")});
    await page.setViewportSize({width:1440,height:1000});
  }
  const remote=await page.evaluate(async(id)=>{
    const session=await(await fetch("/v1/verse/studio/session")).json();
    const url=`/v1/verse/studio/projects/${id}/haunted-house/main`;
    const opened=await(await fetch(url)).json();
    const document=structuredClone(opened.head.document);document.source_draft="Repairable remote draft";
    const response=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify({csrf:session.csrf,command_id:"fixture-external-save",expected_version:2,document,summary:"Synthetic external save"})});
    if(!response.ok)throw Error("External native save failed");return await response.json();
  },first);
  assert.equal(remote.saved.version,3);
  await page.click("[data-aethyr-id=project-open]");await page.locator("[data-aethyr-id=edit-confirm]").waitFor({state:"visible"});await page.click("[data-aethyr-id=edit-cancel]");
  await page.click("[data-aethyr-id=project-save-copy]");await page.click("[data-aethyr-id=edit-confirm]");
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("newer version"));
  assert.equal(savedPosts.at(-1).expected_version,2,"Fetching and canceling must not adopt remote version 3");
  await page.click("[data-aethyr-id=edit-cancel]");
  await page.click("[data-aethyr-id=project-open]");await page.click("[data-aethyr-id=edit-confirm]");
  await page.locator("[data-aethyr-id=edit-auth]").waitFor({state:"hidden"});
  await page.waitForFunction(()=>document.querySelector("#edit-source").value==="Repairable remote draft",null,{timeout:20000});
  assert.equal(await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue(),"Repairable remote draft");
  const arrivedCheckpoint=new Promise(resolve=>checkpointArrived=resolve);delayCheckpoint=true;
  await page.click("[data-aethyr-id=project-save-copy]");await page.click("[data-aethyr-id=edit-confirm]");await arrivedCheckpoint;
  assert.equal(savedPosts.at(-1).expected_version,3);
  await page.fill("[data-aethyr-id=project-name] :is(input,textarea)","Second synthetic project");
  const creationArrival=new Promise(resolve=>createArrived=resolve);delayCreate=true;
  await page.click("[data-aethyr-id=project-create]");console.log("Create overlap: click returned");await creationArrival;console.log("Create overlap: response held");
  const saveCount=savedPosts.length;
  const revisionDuringCreate=JSON.parse(await page.evaluate(()=>window.hauntDemo.snapshot())).status.project.revision;
  await page.click("[data-aethyr-id=project-save-copy]");console.log("Create overlap: Save clicked",await page.locator("[data-aethyr-id=project-status]").innerText());
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("current change finish"));console.log("Create overlap: Save refused");
  await page.click("[data-aethyr-id=tab-compile]");console.log("Create overlap: Compile tab active");
  await page.click("[data-aethyr-id=edit-source-draft]");console.log("Create overlap: draft edit clicked");
  await page.click("[data-aethyr-id=tab-history]");
  await page.waitForTimeout(150);
  assert.equal(savedPosts.length,saveCount,"Save cannot start during project creation");
  assert.equal(JSON.parse(await page.evaluate(()=>window.hauntDemo.snapshot())).status.project.revision,revisionDuringCreate);
  assert.equal(await page.locator("[data-aethyr-id=edit-auth]").isVisible(),false,"Creation cannot retarget a confirmed engine action");
  releaseCreate();
  await page.waitForFunction((id)=>document.querySelector("#project-picker").value!==id,first);
  releaseCheckpoint();await page.waitForTimeout(500);
  assert.ok(!(await page.locator("[data-aethyr-id=project-status]").innerText()).includes("could not save"),"A late checkpoint failure must not describe another project");
  const holdRecovery=async()=>{
    const editor=page.frames().find(f=>f.url().startsWith(config.editor));assert.ok(editor);
    await editor.evaluate(()=>{
      const service=window.aethyrforgeEditor.service;
      if(!window.originalProjectExport){window.originalProjectExport=service.projectDocument.bind(service);service.projectDocument=async(...args)=>{const document=await window.originalProjectExport(...args);if(window.holdNextRecovery){window.holdNextRecovery=false;window.recoveryHeld=true;await new Promise(resolve=>window.releaseRecovery=resolve);}return document;};}
      window.recoveryHeld=false;window.holdNextRecovery=true;
    });
    await page.fill("[data-aethyr-id=edit-source] :is(input,textarea)",(await page.locator("[data-aethyr-id=edit-source] :is(input,textarea)").inputValue())+"\n// fresh recovery "+Date.now());
    await editor.waitForFunction(()=>window.recoveryHeld,null,{timeout:18000});
    return editor;
  };
  const recoveryKey=id=>`openagents.verse.recovery.v1:fixture-account:${id}:haunted-house:main`;
  const oldProject=await page.locator("[data-aethyr-id=project-picker] select").inputValue();
  const oldEditor=await holdRecovery();
  await page.fill("[data-aethyr-id=project-name] :is(input,textarea)","Recovery during creation");await page.click("[data-aethyr-id=project-create]");
  await page.waitForFunction(id=>document.querySelector("#project-picker").value!==id,oldProject);
  const recoveryProject=await page.locator("[data-aethyr-id=project-picker] select").inputValue();
  await page.waitForFunction(key=>localStorage.getItem(key)!==null,recoveryKey(recoveryProject),{timeout:18000});
  const newCopy=await page.evaluate(key=>localStorage.getItem(key),recoveryKey(recoveryProject));
  await oldEditor.evaluate(()=>window.releaseRecovery());await page.waitForTimeout(250);
  assert.equal(await page.evaluate(key=>localStorage.getItem(key),recoveryKey(recoveryProject)),newCopy,"Old recovery completion cannot overwrite the new project's copy");
  await holdRecovery();
  await page.evaluate(key=>localStorage.removeItem(key),recoveryKey(first));
  await page.selectOption("[data-aethyr-id=project-picker] select",first);
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).phase==="open"&&JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:120000});
  await page.waitForFunction(key=>localStorage.getItem(key)!==null,recoveryKey(first),{timeout:20000});
  assert.equal(await page.locator("[data-aethyr-id=project-picker] select").inputValue(),first);
  await page.screenshot({path:path.join(out,"project-workspace-desktop.png")});
  const result={release:config.release,protocol_version:config.protocol_version,real_gpu_engine:true,real_native_project_store:true,synthetic_account_service:true,fixture_direct_session:process.env.AETHYRFORGE_FIXTURE_DIRECT_SESSION==="1",controlled_local_https_proxy:!!process.env.AETHYRFORGE_LOCAL_PROXY_CONFIG,real_signin_provider:false,microphone_used:false,readable_contribution_review:checkHistory,late_inspection_keeps_latest:checkHistory,inspection_non_mutating:checkHistory,late_save_keeps_newer_source:true,acknowledged_base_advances:true,canceled_load_keeps_active_base:true,confirmed_load_restores_repairable_draft:true,late_checkpoint_failure_scoped:true,create_during_save_is_serialized:true,save_and_edit_during_creation_are_serialized:true,recovery_across_create_and_picker_resumes:true,grants,errors};
  assert.deepEqual(errors,[]);
  fs.writeFileSync(path.join(out,"browser-project.json"),JSON.stringify(result,null,2));
  console.log(JSON.stringify(result));
} catch(error) {
  if(page){await page.screenshot({path:path.join(out,"project-error.png")});fs.writeFileSync(path.join(out,"project-error-state.json"),JSON.stringify({error:String(error),errors,body:await page.locator("body").innerText(),snapshot:await page.evaluate(()=>window.hauntDemo?.snapshot()),frames:await Promise.all(page.frames().map(async frame=>({url:frame.url(),state:await frame.evaluate(()=>({engine:window.aethyrforge?.snapshot(),notice:document.querySelector("#world-notice")?.textContent})).catch(()=>null)})))},null,2));}
  throw error;
} finally {
  await context.close();await browser.close();server.kill("SIGTERM");accountServer.close();
  fs.writeFileSync(path.join(out,"native-fixture.log"),log);
}
