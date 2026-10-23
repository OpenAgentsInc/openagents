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
      OPENAGENTS_VERSE_PUBLISH_RUNTIME: JSON.stringify(config.publish_runtime),
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
  const guest=await browser.newContext({viewport:{width:1280,height:800}});guestContexts.push(guest);
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
  await guestPreview.evaluate(()=>{window.playerInputEvents=[];addEventListener('keydown',e=>window.playerInputEvents.push(e.code));});
  await guestPreview.locator('#world').click();
  await guestPreview.waitForFunction(()=>document.pointerLockElement?.id==='world',null,{timeout:5000});
  assert.equal(await guestPreview.evaluate(()=>document.hasFocus()),true,'The nested player canvas has foreground focus');
  await guestPreview.locator('#world').focus();
  const guestPose=(await guestSnapshot()).engine.status.checkpoint;
  await guestPage.keyboard.down('w');await guestPage.waitForTimeout(700);await guestPage.keyboard.up('w');
  assert.ok(await guestPreview.evaluate(()=>window.playerInputEvents.includes('KeyW')),'Actual keyboard input reaches the player canvas');
  await guestPage.waitForFunction(eye=>JSON.parse(window.versePlayer.snapshot()).engine.status.checkpoint.eye.some((n,i)=>Math.abs(n-eye[i])>0.05),guestPose.eye,{timeout:5000});
  const walked=(await guestSnapshot()).engine.status.checkpoint;
  assert.ok(walked.eye.some((n,i)=>Math.abs(n-guestPose.eye[i])>0.05),'The guest actually walks in the published world');
  for(const frame of guestPage.frames())if(frame.url().startsWith(config.preview))await frame.evaluate(()=>document.exitPointerLock?.());
  await guestPage.screenshot({path:path.join(out,'playable-guest-desktop.png')});
  console.log('Guest permissions and walking passed');
  const priorPlayerSession=(await guestSnapshot()).engine.session;
  await guestEditor.evaluate(()=>{sessionStorage.setItem('fixture-player-reload-mode','hold-fail');location.reload();});
  await guestEditor.waitForFunction(()=>window.playerLoadFixture?.actual()?.ok===true,null,{timeout:125000});
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
  await edit('history');await page.click('#project-list');await page.locator(`#project-picker option[value="${project}"]`).waitFor();
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
  await page.evaluate(()=>document.querySelector('#project-share').click());
  await page.locator('#publish-share-card').waitFor({state:'visible'});
  await page.selectOption('#project-picker','');
  assert.equal(await page.locator('#publish-share-card').isHidden(),true);
  assert.equal(await page.locator('#publish-share-link').getAttribute('href'),null);
  await page.selectOption('#project-picker',project);await page.click('#publish-refresh');
  await page.waitForFunction(()=>document.querySelector('#published-link').hasAttribute('href'));
  await page.evaluate(()=>document.querySelector('#project-share').click());
  await page.locator('#publish-share-card').waitFor({state:'visible'});
  active=false;
  await page.waitForFunction(()=>document.querySelector('#publish-share-card').hidden&&!document.querySelector('#publish-share-link').hasAttribute('href'));
  assert.equal(await page.locator('#publish-share-link').innerText(),'');
  assert.deepEqual(errors,[]);
  const result={protocol_version:7,real_gpu_engine:true,real_native_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,synthetic_recognition:true,microphone_used:false,compile_inspect_revert_without_publication:true,exact_saved_version_publish:true,lost_response_retry_idempotent:true,double_confirm_keeps_busy_owner:true,guest_without_editing_authority:true,guest_walks_published_world:true,player_reload_held_failed_and_same_version_recovered:true,later_drafts_and_releases_do_not_change_opened_version:true,spoken_publish_share_play:true,management_with_bad_source_face_draft_closed_engine:true,selected_version_audience_and_link_reviewed:true,rollback_sequence_advances:true,revoked_link_refuses_new_loads:true,share_clears_on_project_switch_signout_and_publication_refresh:true,private_captures:true,errors};
  fs.writeFileSync(path.join(out,'browser-publish.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
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
  releasePublish?.();releaseSave?.();releaseCheckpoint?.();
  for(const guest of guestContexts){await guest.unrouteAll({behavior:'wait'});await guest.close();}
  await context.unrouteAll({behavior:'wait'});await context.close();await browser.close();server.kill('SIGTERM');accountServer.close();
  fs.writeFileSync(path.join(out,'native-fixture.log'),log);
}
