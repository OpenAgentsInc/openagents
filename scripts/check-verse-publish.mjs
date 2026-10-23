// Immutable publication/player checks with real native storage and private GPU runtime.
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
assert.equal(config.protocol_version,7,"Use the playable wire-v7 runtime");
assert.ok(config.publish_runtime);
const projects = [];
let delayCreate=false, releaseCreate, createArrived;
const fixtureKey = "oak_fixture.synthetic-browser-key";
const fixtureToken = "sess_" + "a".repeat(64);
let active = true;
let failMembership=false, failEnsure=false, oldEnsure=false; let ensuredProject;
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
  if (req.url === "/v1/account/github") return failMembership?send(503,{error:{code:"unavailable",message:"Synthetic membership failure."}}):send(200,{github:{state:"none"},projects});
  if(req.url==="/v1/projects/ensure" && req.method==="POST"){
    let raw="";for await(const part of req)raw+=part;const body=JSON.parse(raw);
    if(oldEnsure){oldEnsure=false;return send(404,{error:{code:"not_found",message:"Synthetic old account service has no such route."}});}
    if(failEnsure){failEnsure=false;return send(503,{error:{code:"unavailable",message:"Synthetic allocation failure."}});}
    assert.equal(body.key,"verse-personal-v1");
    if(!ensuredProject){ensuredProject={id:"prj_ffffffffffffffff",name:body.name,instructions:null,created_unix:Math.floor(Date.now()/1000),repositories:[]};projects.push(ensuredProject);}
    return send(200,{project:ensuredProject});
  }
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
      OPENAGENTS_UNIVERSE_RELEASE: JSON.stringify({...release,engine_url:config.editor+"/universe.html",project:{id:"universe",branch:"main",revision:0}}),
      OPENAGENTS_VERSE_PUBLISH_RUNTIME: JSON.stringify(config.publish_runtime),
      ...(config.personal_runtime?{OPENAGENTS_VERSE_PERSONAL_RUNTIME:JSON.stringify(config.personal_runtime)}:{}),
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
const guestContexts=[];
let holdPublish=false, publishArrived, releasePublish, losePublishAck=false;
let holdRefreshSession=false, refreshSessionArrived, releaseRefreshSession;
let holdAllocationAck=false, allocationArrived, releaseAllocationAck;
let failNextBrowserSession=false;let tutorialGets=0;let planetFormArrived;
const publicationPosts=[];
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
    if(url.pathname==='/v1/verse/studio/session'&&failNextBrowserSession&&request.frame().page()===page){
      failNextBrowserSession=false;await route.fulfill({status:503,contentType:'application/json',body:JSON.stringify({error:{code:'unavailable',message:'Your progress could not open. Try again.'}})});return;
    }
    if(url.pathname==="/v1/verse/studio/tutorial"&&request.method()==="GET")tutorialGets++;
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
    if(request.method()==='POST'&&url.pathname.endsWith('/publications')) {
      publicationPosts.push(JSON.parse(request.postData()));
      if(holdPublish){holdPublish=false;publishArrived?.();await new Promise(resolve=>releasePublish=resolve);}
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
    if(losePublishAck&&request.method()==='POST'&&url.pathname.endsWith('/publications')){
      losePublishAck=false;await route.abort('failed');return;
    }
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
    if(url.pathname==='/v1/verse/studio/session'&&holdRefreshSession&&request.frame().page()===page){
      holdRefreshSession=false;refreshSessionArrived?.();await new Promise(resolve=>releaseRefreshSession=resolve);
    }
    if(url.pathname==='/v1/verse/studio/tutorial'&&request.method()==='POST'&&JSON.parse(request.postData()).operation==='allocate'&&holdAllocationAck){
      holdAllocationAck=false;allocationArrived?.();await new Promise(resolve=>releaseAllocationAck=resolve);
    }
    if(url.pathname.startsWith('/new-lander/planet/')&&request.method()==='POST'){
      console.log('Planet form response',JSON.stringify({status:response.status(),origin:request.headers().origin,site:request.headers()['sec-fetch-site']}));
      planetFormArrived?.({status:response.status(),origin:request.headers().origin});
    }
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

  await context.addInitScript(()=>{
    // Recognizer events exercise routing only; no microphone or ASR provider is used.
    window.speechFixture={last:null,starts:0};
    window.SpeechRecognition=class{start(){window.speechFixture.last=this;window.speechFixture.starts++;}abort(){this.aborted=true;}};
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
  if(process.env.AETHYRFORGE_PERSONAL_CHECK==='1') {
    const {checkPersonalWorld}=await import('./check-verse-planet.mjs');
    await checkPersonalWorld({page,context,base,config,out,errors});
  } else {
  console.log("Fixture host: opening house");
  await page.goto("https://openagents.com/new-lander/house");
  console.log("Fixture host: house document opened");
  await page.waitForFunction(()=>window.hauntDemo && JSON.parse(window.hauntDemo.snapshot()).phase==="open" && JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:process.env.AETHYRFORGE_LOCAL_CHECK==="1"?30000:125000});
  const snapshot=()=>page.evaluate(()=>JSON.parse(window.hauntDemo.snapshot()));
  const edit=async(tab)=>{if(await page.locator('#edit-panel').isHidden())await page.click('#world-edit');await page.click('#tab-'+tab);};
  const confirm=async()=>{await page.locator('#edit-confirm').waitFor({state:'visible'});await page.click('#edit-confirm');await page.locator('#edit-auth').waitFor({state:'hidden'});};
  const options=async()=>{await edit('request');await page.click('#request-options');};
  const csrf=async()=>page.evaluate(async()=> (await(await fetch('/v1/verse/studio/session')).json()).csrf);
  assert.equal(await page.locator('#publish-enabled').getAttribute('content'),'true');
  await page.waitForFunction(()=>!document.querySelector('#project-revert').disabled);
  const before=(await snapshot()).status;
  await edit('history');await page.fill('#project-name','Synthetic playable house');await page.click('#project-create');
  await page.waitForFunction(()=>document.querySelector('#project-picker').value.startsWith('prj_'));
  const project=await page.locator('#project-picker').inputValue();
  const source=await page.locator('#edit-source').inputValue();
  const changed=source.replace('return base;','return base * vec3<f32>(0.8, 0.6, 0.45);');assert.notEqual(changed,source);
  const draftEditor=page.frames().find(f=>f.url().startsWith(config.editor));assert.ok(draftEditor);
  await draftEditor.evaluate(()=>{
    const registry=window.aethyrforgeEditor.service.registry, get=registry.get.bind(registry);
    const previous=window.aethyrforge.snapshot().status.authoring.draft, heldStates=new WeakMap();
    window.draftStatusFixture={held:true,observed:false,release:()=>{registry.get=get;}};
    registry.get=(...args)=>{
      const state=get(...args);
      if(window.draftStatusFixture.held&&state?.authoring&&state.authoring.draft!==previous){
        window.draftStatusFixture.observed=true;
        if(!heldStates.has(state))heldStates.set(state,{...state,authoring:{...state.authoring,draft:previous}});
        return heldStates.get(state);
      }
      return state;
    };
  });
  await edit('compile');await page.fill('#edit-source',changed);await page.click('#edit-source-draft');
  await page.locator('#edit-confirm').waitFor({state:'visible'});await page.click('#edit-confirm');
  await draftEditor.waitForFunction(()=>window.draftStatusFixture.observed);
  await page.waitForTimeout(200);
  assert.equal(await page.locator('#edit-auth').isVisible(),true,'Draft is not acknowledged while its matching status is held');
  assert.equal((await snapshot()).status.authoring.draft,source);
  await draftEditor.evaluate(()=>{window.draftStatusFixture.held=false;window.draftStatusFixture.release();});
  await page.locator('#edit-auth').waitFor({state:'hidden'});
  assert.equal((await snapshot()).status.authoring.draft,changed,'Successful Draft carries the matching source');
  assert.equal((await snapshot()).status.project.revision,before.project.revision,'Keeping a draft does not advance world history');
  await page.click('#edit-source-compile');await confirm();
  assert.equal((await snapshot()).status.authoring.active_source,changed);
  const pose=(await snapshot()).status.checkpoint;assert.equal(pose.mode,before.checkpoint.mode);for(const key of ['eye','target'])assert.ok(pose[key].every((n,i)=>Math.abs(n-before.checkpoint[key][i])<0.03));
  if(process.env.AETHYRFORGE_DRAFT_ACK_ONLY==='1'){
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(out,'browser-draft-ack.json'),JSON.stringify({real_gpu_engine:true,synthetic_account_service:true,real_signin_provider:false,delayed_draft_status_not_acknowledged:true,immediate_compile_after_draft:true,draft_does_not_advance_history:true,errors},null,2));
    console.log('Delayed Draft status and immediate Compile passed');
  } else {
  if(await page.locator('#edit-close').isVisible())await page.click('#edit-close');assert.equal((await snapshot()).status.authoring.active_source,changed);
  await options();await page.click('#project-revert');await confirm();
  assert.equal((await snapshot()).status.authoring.active_source,source,'Revert is a new change to Starting house');
  await edit('compile');await page.fill('#edit-source',changed);await page.click('#edit-source-draft');await confirm();
  await page.click('#edit-source-compile');await confirm();
  assert.equal((await snapshot()).status.authoring.active_source,changed,'A fresh compile after Revert uses the shared command path');
  await edit('history');
  await page.click('#project-save-copy');await confirm();
  await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('version 1'));
  const book=()=>page.evaluate(async project=>(await(await fetch(`/v1/verse/studio/projects/${project}/haunted-house/main/publications`)).json()),project);
  assert.equal((await book()).book.sequence,0,'Compile, inspect, Revert and Save never publish automatically');
  await edit('compile');await page.selectOption('#publish-visibility','public');await options();await page.click('#project-publish');
  await page.locator('#publish-review[open]').waitFor();assert.match(await page.locator('#publish-review-target').innerText(),/Version 1 · Anyone with the link/);
  await page.click('#publish-confirm');await page.waitForFunction(()=>document.querySelector('#publish-status').textContent.includes('Confirm the playable'));
  assert.equal(publicationPosts.length,0,'Unchecked confirmation sends no publication');
  await page.check('#publish-confirm-rights');
  const arrived=new Promise(resolve=>publishArrived=resolve);holdPublish=true;losePublishAck=true;
  await page.click('#publish-confirm');await arrived;
  await page.click('#publish-confirm');
  const pendingBefore=await page.evaluate(()=>sessionStorage.getItem('openagents.verse.pending.v1'));
  await page.evaluate(()=>document.querySelector('#edit-source-draft').click());
  assert.equal(await page.evaluate(()=>sessionStorage.getItem('openagents.verse.pending.v1')),pendingBefore,'Second Confirm cannot release the first operation and admit a competing edit');
  assert.equal(publicationPosts.length,1);
  releasePublish();
  await page.waitForFunction(()=>document.querySelector('#publish-status').textContent.includes('could not'));
  assert.equal((await book()).book.sequence,1,'The native commit exists despite a lost response');
  await page.click('#publish-confirm');await page.locator('#publish-review').waitFor({state:'hidden'});
  assert.equal(publicationPosts.length,2);assert.equal(publicationPosts[0].command_id,publicationPosts[1].command_id);
  console.log('Publication retry completed');
  const first=await book();assert.equal(first.book.sequence,1,'Retry returns the existing publication');
  const firstId=first.book.channel,world=first.world;
  const firstPath=`/new-lander/play/${world}/${firstId}`;
  if(process.env.AETHYRFORGE_PLANET_ONLY==='1') {
    await options();await page.click('#project-share');
    await page.locator('#publish-share-card').waitFor({state:'visible'});
    const chosen=await page.evaluate(async()=>await(await fetch('/v1/verse/studio/tutorial')).json());
    assert.equal(chosen.progress.choice,'share');assert.equal(chosen.progress.planet,null);
    await page.goto('https://openagents.com/new-lander/verse');
    await page.waitForFunction(()=>window.universeDemo&&JSON.parse(window.universeDemo.snapshot()).phase==='open',null,{timeout:30000});
    await page.locator('#planet-ready').waitFor({state:'visible'});
    assert.equal(await page.locator('#world-title').innerText(),'My planet');
    const unsigned=await browser.newContext();guestContexts.push(unsigned);
    assert.equal((await unsigned.request.post(base+'/v1/verse/studio/tutorial',{headers:{origin:base},data:{csrf:'forged',operation:'allocate'}})).status(),401);
    const count=projects.length;failEnsure=true;
    await page.click('#planet-create');
    await page.waitForFunction(()=>!document.querySelector('#planet-create').disabled&&document.querySelector('#planet-status').textContent.includes('Try again'));
    assert.equal(projects.length,count);assert.equal(await page.locator('#planet-owned').isHidden(),true);
    oldEnsure=true;
    await page.click('#planet-create');
    await page.waitForFunction(()=>!document.querySelector('#planet-create').disabled&&document.querySelector('#planet-status').textContent.includes('Try again'));
    assert.equal(projects.length,count);assert.equal(await page.locator('#planet-owned').isHidden(),true);
    assert.doesNotMatch(await page.locator('#planet-status').innerText(),/removed/);
    const refreshHeld=new Promise(resolve=>refreshSessionArrived=resolve);holdRefreshSession=true;
    await refreshHeld;
    const allocationHeld=new Promise(resolve=>allocationArrived=resolve);holdAllocationAck=true;
    await page.click('#planet-create');await allocationHeld;
    if(process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE==='1'){
      failNextBrowserSession=true;releaseAllocationAck();
      await page.locator('#planet-owned').waitFor({state:'visible'});
      assert.match(await page.locator('#planet-status').innerText(),/Try again/);
      assert.equal(await page.locator('#planet-setup').getAttribute('href'),null,'A temporary identity failure preserves the award but disables access');
      assert.doesNotMatch(await page.locator('#planet-status').innerText(),/Log in/);
      const getsBefore=tutorialGets;releaseRefreshSession();
      await page.waitForTimeout(300);assert.equal(tutorialGets,getsBefore,"Refresh from before allocation is discarded before it can fetch progress");
      await page.waitForFunction(()=>document.querySelector('#planet-setup').hasAttribute('href'));
    } else {
      // A refresh that began before allocation must not acquire its new generation.
      active=false;const getsBefore=tutorialGets;releaseRefreshSession();
      await page.waitForTimeout(300);assert.equal(tutorialGets,getsBefore,"Refresh from before allocation cannot acquire the allocation generation");
      await page.waitForFunction(()=>document.querySelector('#planet-status').textContent.includes('Log in again'));
      assert.equal(await page.locator('#planet-owned').isHidden(),true);
      active=true;
      // The new context must poll again while the old allocation response is held.
      await page.locator('#planet-owned').waitFor({state:'visible'});
      releaseAllocationAck();
      await page.waitForTimeout(2200);
      assert.equal(await page.locator('#planet-owned').isVisible(),true,'A late old allocation cannot overwrite the rechecked award');
    }
    assert.match(await page.locator('#option-openagents small').textContent(),/Locked/);
    assert.equal(await page.locator('.verse-page .account-nav').isHidden(),true,'Confirmed owners are not offered Log in or Sign up');
    if(process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE==='1')assert.equal(await page.locator('#planet-owned h3').innerText(),'Congratulations');
    const planetPath=await page.locator('#planet-setup').getAttribute('href');assert.ok(planetPath.startsWith('/new-lander/planet/planet_'));
    assert.equal(projects.length,count+1);
    await page.screenshot({path:path.join(out,'planet-awarded-desktop.png')});
    if(process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE==='1'){
      await page.setViewportSize({width:390,height:844});
      await page.screenshot({path:path.join(out,'planet-congratulations-mobile.png')});
      await page.setViewportSize({width:1440,height:1000});
    }
    await page.reload();await page.locator('#planet-owned').waitFor({state:'visible'});
    assert.equal(await page.locator('#planet-setup').getAttribute('href'),planetPath);
    assert.match(await page.locator('#planet-status').innerText(),/Welcome back/);
    const secondTab=await context.newPage();
    await secondTab.goto('https://openagents.com/new-lander/verse');await secondTab.locator('#planet-owned').waitFor({state:'visible'});
    assert.equal(await secondTab.locator('#planet-setup').getAttribute('href'),planetPath);
    const replay=await secondTab.evaluate(async()=>{
      const session=await(await fetch('/v1/verse/studio/session')).json();
      return await(await fetch('/v1/verse/studio/tutorial',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({csrf:session.csrf,operation:'allocate'})})).json();
    });
    assert.equal('/new-lander/planet/'+replay.progress.planet.id,planetPath);assert.equal(projects.length,count+1);
    failMembership=true;
    await page.waitForFunction(()=>!document.querySelector('#planet-setup').hasAttribute('href'));
    assert.match(await page.locator('#planet-status').innerText(),/Try again|minute/);
    failMembership=false;
    await page.waitForFunction(()=>document.querySelector('#planet-setup').hasAttribute('href'));
    assert.equal(await page.locator('#planet-setup').getAttribute('href'),planetPath);
    await page.bringToFront();await page.setViewportSize({width:390,height:844});
    await page.screenshot({path:path.join(out,'planet-awarded-mobile.png')});
    assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
    await page.click('#planet-setup');await page.waitForURL('https://openagents.com'+planetPath);
    await page.fill('#planet-name','Moon garden');const formArrived=new Promise(resolve=>planetFormArrived=resolve);await page.click('button[type="submit"]');
    const formResult=await formArrived;assert.equal(formResult.status,303);assert.equal(formResult.origin,'https://openagents.com');
    await page.waitForFunction(()=>document.querySelector('h1')?.textContent==='Moon garden',null,{timeout:10000});
    await page.reload();assert.equal(await page.locator('h1').innerText(),'Moon garden');
    await page.screenshot({path:path.join(out,'planet-setup-mobile.png')});
    await secondTab.reload();await secondTab.waitForFunction(()=>document.querySelector('#world-title').textContent==='Moon garden');
    assert.deepEqual(errors,[]);
    const result={real_gpu_engine:true,real_native_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,explicit_published_share_return:true,allocation_failure_recovers:true,old_ensure_endpoint_404_preserves_ready_and_upgrade_retries:true,refresh_two_tabs_replay_same_planet:true,membership_failure_preserves_award:true,old_refresh_discarded_before_get:true,account_change_releases_old_busy:process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE!=="1",late_old_allocation_ignored:process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE!=="1",post_allocation_identity_unavailable_preserves_award:process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE==="1",rename_and_reopen:true,confirmed_identity_hides_map_signin:true,first_confirmed_award_congratulations:process.env.AETHYRFORGE_PLANET_IDENTITY_FAILURE==="1",later_visits_resume:true,company_planet_stays_locked:true,private_captures:true,errors};
    fs.writeFileSync(path.join(out,'browser-planet.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  } else {
  const guest=await browser.newContext({viewport:{width:1280,height:800},hasTouch:true});guestContexts.push(guest);
  if(process.env.AETHYRFORGE_LOCAL_CHECK==='1')await guest.grantPermissions(['local-network-access'],{origin:'https://openagents.com'});
  await guest.route('https://openagents.com/**',async route=>{
    const u=new URL(route.request().url());const response=await guest.request.fetch(base+u.pathname+u.search,{maxRedirects:0});
    const headers={...response.headers()};delete headers['content-length'];delete headers['content-encoding'];
    await route.fulfill({status:response.status(),headers,body:response.status()>=300&&response.status()<400?Buffer.alloc(0):await response.body()});
  });
  await guest.addInitScript(()=>{
    if(!location.pathname.endsWith('/player.html')||sessionStorage.getItem('fixture-player-reload-mode')!=='hold-fail')return;
    let editor;
    Object.defineProperty(window,'aethyrforgeEditor',{configurable:true,get:()=>editor,set(value){
      editor=value;if(!value?.service)return;
      const service=value.service,send=service.send.bind(service),get=service.receipts.get.bind(service.receipts);
      window.playerLoadFixture={id:null,fail:false,actual:()=>get(window.playerLoadFixture.id)};
      service.send=(request,...args)=>{const id=send(request,...args);if(request.type==='loadPlayable')window.playerLoadFixture.id=id;return id;};
      service.receipts.get=id=>{
        const receipt=get(id);
        if(id!==window.playerLoadFixture.id)return receipt;
        return window.playerLoadFixture.fail&&receipt?{...receipt,ok:false,message:'Injected playable load acknowledgment failure'}:undefined;
      };
    }});
  });
  const guestPage=await guest.newPage();guestPage.on('pageerror',e=>errors.push(e.message));await guestPage.goto('https://openagents.com'+firstPath);
  await guestPage.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:process.env.AETHYRFORGE_LOCAL_CHECK==='1'?30000:125000});
  console.log('Pinned guest player opened');
  const guestSnapshot=()=>guestPage.evaluate(()=>JSON.parse(window.versePlayer.snapshot()));
  assert.equal((await guestSnapshot()).release,firstId);assert.deepEqual((await guestSnapshot()).engine.capabilities,['inspect','observe']);
  assert.equal((await guestSnapshot()).engine.status.authoring,undefined);
  const guestEditor=guestPage.frames().find(f=>f.url().startsWith(config.editor));assert.ok(guestEditor);
  const denied=await guestEditor.evaluate(()=>{
    const s=window.aethyrforgeEditor.service,status=window.aethyrforge.snapshot().status;
    return s.author({type:'propose',operations:[{type:'remove',target:'werewolf-1'}]},status.revision,crypto.randomUUID());
  });
  await guestEditor.waitForFunction(id=>!!window.aethyrforgeEditor.service.receipts.get(id),denied);
  assert.equal(await guestEditor.evaluate(id=>window.aethyrforgeEditor.service.receipts.get(id).ok,denied),false,'Player worker refuses direct editor actions');
  await guestPage.bringToFront();
  const guestPreview=guestPage.frames().find(f=>f.url().startsWith(config.preview));assert.ok(guestPreview);
  await guestPreview.evaluate(()=>{
    window.playerInputEvents=[];window.playerPointerEvents=[];
    addEventListener('keydown',e=>window.playerInputEvents.push(e.code));
    for(const type of ['pointerdown','click','pointerlockchange','pointerlockerror'])addEventListener(type,e=>window.playerPointerEvents.push({type,time:performance.now(),trusted:e.isTrusted,pointer:e.pointerType,locked:document.pointerLockElement?.id,focus:document.hasFocus()}));
  });
  await guestPreview.locator('#world').click();
  try{await guestPreview.waitForFunction(()=>document.pointerLockElement?.id==='world',null,{timeout:5000});}
  catch(error){fs.writeFileSync(path.join(out,'player-pointer-events.json'),JSON.stringify(await guestPreview.evaluate(()=>window.playerPointerEvents),null,2));throw error;}
  assert.equal(await guestPreview.evaluate(()=>document.hasFocus()),true,'The nested player canvas has foreground focus');
  const mouseClick=await guestPreview.evaluate(()=>window.playerPointerEvents.find(e=>e.type==='click'&&e.pointer==='mouse'&&e.trusted));
  assert.ok(mouseClick,'The player receives a trusted mouse click');
  if(process.env.AETHYRFORGE_REQUIRE_EARLY_MOUSE==='1')assert.ok(mouseClick.time<700,'The dedicated early-click check must exercise the first 700ms');
  await guestPreview.locator('#world').focus();
  const guestPose=(await guestSnapshot()).engine.status.checkpoint;
  await guestPage.keyboard.down('w');await guestPage.waitForTimeout(700);await guestPage.keyboard.up('w');
  assert.ok(await guestPreview.evaluate(()=>window.playerInputEvents.includes('KeyW')),'Actual keyboard input reaches the player canvas');
  await guestPage.waitForFunction(eye=>JSON.parse(window.versePlayer.snapshot()).engine.status.checkpoint.eye.some((n,i)=>Math.abs(n-eye[i])>0.05),guestPose.eye,{timeout:5000});
  const walked=(await guestSnapshot()).engine.status.checkpoint;
  assert.ok(walked.eye.some((n,i)=>Math.abs(n-guestPose.eye[i])>0.05),'The guest actually walks in the published world');
  for(const frame of guestPage.frames())if(frame.url().startsWith(config.preview))await frame.evaluate(()=>document.exitPointerLock?.());
  await guestPage.screenshot({path:path.join(out,'playable-guest-desktop.png')});
  await guestPreview.waitForFunction(()=>!document.pointerLockElement);
  const guestCanvas=await guestPreview.locator('#world').boundingBox();
  await guestPage.touchscreen.tap(guestCanvas.x+guestCanvas.width*.7,guestCanvas.y+guestCanvas.height*.5);
  await guestPreview.locator('#world').click();await guestPage.waitForTimeout(150);
  assert.equal(await guestPreview.evaluate(()=>document.pointerLockElement===null),true,'A real preceding touch still suppresses its immediate compatibility click');
  console.log('Guest permissions and walking passed');
  fs.writeFileSync(path.join(out,'player-pointer-events.json'),JSON.stringify(await guestPreview.evaluate(()=>window.playerPointerEvents),null,2));
  const priorPlayerSession=(await guestSnapshot()).engine.session;
  await guestEditor.evaluate(()=>{sessionStorage.setItem('fixture-player-reload-mode','hold-fail');location.reload();});
  await guestEditor.waitForFunction(()=>window.playerLoadFixture?.actual()?.ok===true,null,{timeout:30000,polling:50});
  assert.notEqual((await guestSnapshot()).engine.session,priorPlayerSession);
  await guestPage.waitForTimeout(300);
  assert.equal(await guestPage.locator('#engine').isHidden(),true,'A replacement frame stays unavailable while LoadPlayable acknowledgment is held');
  assert.equal(await guestPage.locator('#world-notice').isVisible(),true);
  await guestPage.screenshot({path:path.join(out,'playable-reload-held.png')});
  await guestEditor.evaluate(()=>window.playerLoadFixture.fail=true);
  await guestPage.waitForFunction(()=>document.querySelector('#world-notice').textContent.includes('Choose Retry'));
  assert.equal(await guestPage.locator('#engine').isHidden(),true,'A failed load cannot expose the fresh base scene');
  await guestPage.screenshot({path:path.join(out,'playable-reload-failed.png')});
  const failedSession=(await guestSnapshot()).engine.session;
  await guestEditor.evaluate(()=>sessionStorage.removeItem('fixture-player-reload-mode'));
  await guestPage.click('#world-retry');
  await guestPage.waitForFunction(previous=>{
    const s=window.versePlayer&&JSON.parse(window.versePlayer.snapshot());
    return s?.engine.session!==previous&&s?.engine.receipts.some(r=>r.outcome.message==='Playable version opened.')&&!document.querySelector('#engine').hidden;
  },failedSession,{timeout:125000});
  assert.equal((await guestSnapshot()).release,firstId,'Retry reloads the same pinned release');
  assert.equal(await guestEditor.evaluate(()=>window.aethyrforge.snapshot().status.authoring.active_source),changed);
  await guestPage.screenshot({path:path.join(out,'playable-reload-recovered.png')});
  const recoveredSession=(await guestSnapshot()).engine.session;
  await guestEditor.evaluate(()=>{
    const registry=window.aethyrforgeEditor.service.registry,get=registry.get.bind(registry),states=new WeakMap();
    registry.get=(...args)=>{
      const value=get(...args);
      if(value?.phase==='ready'&&value?.view){if(!states.has(value))states.set(value,{...value,phase:'error',message:'Injected fatal renderer status'});return states.get(value);}
      return value;
    };
  });
  await guestPage.waitForFunction(()=>['closed','revoked'].includes(JSON.parse(window.versePlayer.snapshot()).engine.phase)&&document.querySelector('#engine').hidden);
  assert.equal(await guestPage.locator('#world-notice').isVisible(),true,'A post-success fatal status cannot leave the old frame interactive');
  await guestPage.click('#world-retry');
  await guestPage.waitForFunction(previous=>{
    const s=JSON.parse(window.versePlayer.snapshot());return s.engine.session!==previous&&s.engine.receipts.some(r=>r.outcome.message==='Playable version opened.')&&!document.querySelector('#engine').hidden;
  },recoveredSession,{timeout:30000});
  assert.equal((await guestSnapshot()).release,firstId);
  await guestPage.setViewportSize({width:390,height:844});
  assert.equal(await guestPage.locator('#player-controls').isVisible(),true);
  assert.equal(await guestPage.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await guestPage.screenshot({path:path.join(out,'playable-guest-mobile.png')});
  await guestPage.setViewportSize({width:1280,height:800});
  console.log('Player reload failure and recovery passed');
  await page.bringToFront();
  // Publish another saved version, keeping the first guest pinned.
  await edit('materials');
  const shared=page.locator('#panel-materials details').filter({has:page.locator('#edit-fog')});await shared.locator('summary').click();
  await page.fill('#edit-fog','0.025');await page.click('#edit-fog-apply');await confirm();
  await edit('history');await page.click('#project-save-copy');await confirm();await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('version 2'));
  await page.click('#edit-voice-start');
  await options();await page.click('#project-publish');await page.locator('#publish-review[open]').waitFor();
  const phrase=async text=>{
    await page.waitForFunction(()=>window.speechFixture.last&&!window.speechFixture.last.aborted);
    await page.evaluate(text=>window.speechFixture.last.onresult({results:[[{transcript:text}]]}),text);await page.waitForTimeout(200);
  };
  await phrase('confirm publish');await page.locator('#publish-review').waitFor({state:'hidden'});
  console.log('Second publication confirmed by voice handler');
  const second=await book(),secondId=second.book.channel;assert.notEqual(secondId,firstId);assert.equal(second.book.sequence,2);
  assert.equal(await page.locator('#publish-version').inputValue(),secondId,'Initial selection follows the current channel');
  assert.equal((await guestSnapshot()).release,firstId);
  assert.equal(await guestEditor.evaluate(()=>window.aethyrforge.snapshot().status.authoring.environment.fogDensity),before.authoring.environment.fogDensity,'A later publication does not change the existing player');
  await phrase('share');await page.locator('#publish-share-card').waitFor({state:'visible'});
  assert.equal(await page.locator('#publish-share-link').getAttribute('href'),`/new-lander/play/${world}/${secondId}`);
  await page.screenshot({path:path.join(out,'share-version-desktop.png')});await page.click('#publish-share-close');
  await phrase('play');await page.waitForURL('https://openagents.com/new-lander/play/**');
  await page.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});
  assert.equal(await page.evaluate(()=>JSON.parse(window.versePlayer.snapshot()).release),secondId,'Spoken Play opens the exact release through the real handler');
  await page.goBack({waitUntil:'domcontentloaded'});
  await page.waitForFunction(()=>window.hauntDemo&&JSON.parse(window.hauntDemo.snapshot()).phase==='open',null,{timeout:125000});
  await edit('history');await page.click('#project-list');await page.locator(`#project-picker option[value="${project}"]`).waitFor({state:'attached'});
  await page.selectOption('#project-picker',project);await page.waitForTimeout(300);
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).phase==='open',null,{timeout:125000});
  await edit('compile');await page.click('#publish-refresh');
  await page.waitForFunction(id=>document.querySelector('#publish-version').value===id,secondId);
  const editor=page.frames().find(f=>f.url().startsWith(config.editor));assert.ok(editor);
  await editor.evaluate(()=>window.aethyrforgeEditor.service.command({kind:'entity.select',id:'werewolf-1'}));
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).status.selection?.id==='werewolf-1');
  await edit('materials');await page.click('#face-open');await page.locator('#face-controls:not([disabled])').waitFor();
  await page.locator('#face-canvas').evaluate(e=>e.scrollIntoView({block:'center'}));const canvas=await page.locator('#face-canvas').boundingBox();
  assert.equal(await page.evaluate(b=>document.elementFromPoint(b.x+b.width*.5,b.y+b.height*.5)?.id,canvas),'face-canvas');
  await page.mouse.click(canvas.x+canvas.width*.5,canvas.y+canvas.height*.5);
  await edit('history');await page.click('#project-save-copy');await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('face draft'));
  await edit('compile');await page.fill('#edit-source','unfinished invalid private WGSL');
  await page.evaluate(()=>window.hauntDemo.close());
  await page.selectOption('#publish-version',firstId);await page.click('#publish-rollback');await page.locator('#publish-review[open]').waitFor();
  assert.equal(await page.locator('#publish-version').inputValue(),firstId,'Preparing management preserves the selected non-default version');
  assert.equal(await page.locator('#publish-review-target').innerText(),'Version 1 · Anyone with the link');assert.equal(await page.locator('#publish-review-link').getAttribute('href'),firstPath);
  await page.check('#publish-confirm-rights');await page.click('#publish-confirm');await page.locator('#publish-review').waitFor({state:'hidden'});
  assert.equal((await book()).book.channel,firstId,'Rollback works with bad source, a face draft, and a closed engine');
  await page.selectOption('#publish-version',secondId);await page.click('#publish-revoke');await page.locator('#publish-review[open]').waitFor();
  assert.equal(await page.locator('#publish-review-target').innerText(),'Version 2 · Anyone with the link');
  await page.screenshot({path:path.join(out,'revoke-version-desktop.png')});
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:path.join(out,'revoke-version-mobile.png')});
  await page.check('#publish-confirm-rights');await page.click('#publish-confirm');await page.locator('#publish-review').waitFor({state:'hidden'});
  assert.equal(await page.locator('#edit-source').inputValue(),'unfinished invalid private WGSL');
  assert.equal((await book()).book.sequence,4);
  const revoked=await guest.request.get(base+`/v1/verse/play/${world}/${secondId}`);assert.equal(revoked.status(),404);
  assert.equal((await guestSnapshot()).release,firstId,'Already opened visits stay pinned while new access is revoked');
  await edit('history');
  await page.evaluate(()=>document.querySelector('#project-share').click());
  await page.locator('#publish-share-card').waitFor({state:'visible'});
  await page.selectOption('#project-picker','');
  assert.equal(await page.locator('#publish-share-card').isHidden(),true);
  assert.equal(await page.locator('#publish-share-link').getAttribute('href'),null);
  await page.selectOption('#project-picker',project);await edit('compile');await page.click('#publish-refresh');
  await page.waitForFunction(()=>document.querySelector('#published-link').hasAttribute('href'));
  await page.evaluate(()=>document.querySelector('#project-share').click());
  await page.locator('#publish-share-card').waitFor({state:'visible'});
  active=false;
  await page.waitForFunction(()=>document.querySelector('#publish-share-card').hidden&&!document.querySelector('#publish-share-link').hasAttribute('href'));
  assert.equal(await page.locator('#publish-share-link').innerText(),'');
  assert.deepEqual(errors,[]);
  const result={protocol_version:7,real_gpu_engine:true,real_native_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,synthetic_recognition:true,microphone_used:false,compile_inspect_revert_without_publication:true,exact_saved_version_publish:true,lost_response_retry_idempotent:true,double_confirm_keeps_busy_owner:true,guest_without_editing_authority:true,guest_walks_published_world:true,trusted_mouse_click_gets_pointer_lock:true,mouse_click_document_ms:mouseClick.time,early_mouse_click_exercised:mouseClick.time<700,touch_ghost_guard:true,player_reload_held_failed_and_same_version_recovered:true,player_post_success_injected_fatal_status_recovers:true,later_drafts_and_releases_do_not_change_opened_version:true,spoken_publish_share_play:true,management_with_bad_source_face_draft_closed_engine:true,selected_version_audience_and_link_reviewed:true,rollback_sequence_advances:true,revoked_link_refuses_new_loads:true,share_clears_on_project_switch_signout_and_publication_refresh:true,private_captures:true,errors};
  fs.writeFileSync(path.join(out,'browser-publish.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  }
  }
  }
} catch(error){
  for(const guest of guestContexts)for(const guestPage of guest.pages())if(!guestPage.isClosed()){
    await guestPage.screenshot({path:path.join(out,'guest-error.png')});
    fs.writeFileSync(path.join(out,'guest-error-state.json'),JSON.stringify(await guestPage.evaluate(()=>({body:document.body.innerText,player:window.versePlayer?.snapshot(),frameHidden:document.querySelector('#engine')?.hidden})),null,2));
    const frames=[];
    for(const f of guestPage.frames())if(f.url().startsWith(config.editor)||f.url().startsWith(config.preview))frames.push(await f.evaluate(()=>({url:location.href,body:document.body.innerText,state:window.aethyrforge?.snapshot(),width:innerWidth,height:innerHeight})).catch(error=>({error:String(error)})));
    fs.writeFileSync(path.join(out,'guest-private-frame-error.json'),JSON.stringify(frames,null,2));
  }
  if(page&&!page.isClosed()){await page.screenshot({path:path.join(out,'publish-error.png')});fs.writeFileSync(path.join(out,'publish-error-state.json'),JSON.stringify({error:String(error),errors,body:await page.locator('body').innerText()},null,2));}
  throw error;
} finally {
  releaseRefreshSession?.();releaseAllocationAck?.();releasePublish?.();releaseSave?.();releaseCheckpoint?.();
  for(const guest of guestContexts){await guest.unrouteAll({behavior:'wait'});await guest.close();}
  await context.unrouteAll({behavior:'wait'});await context.close();await browser.close();server.kill('SIGTERM');accountServer.close();
  fs.writeFileSync(path.join(out,'native-fixture.log'),log);
}
