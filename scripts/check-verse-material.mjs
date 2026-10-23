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
assert.ok(config.protocol_version >= 6, "Use an editable wire-v6 release");
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
  const editor=page.frames().find(f=>f.url().startsWith(config.editor));
  assert.ok(editor);
  await editor.evaluate(()=>window.aethyrforgeEditor.service.command({kind:"entity.select",id:"werewolf-1"}));
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).status.selection?.id==="werewolf-1");
  await editor.evaluate(()=>window.aethyrforge.cameraPose([-2.3,1.8,-12.3],[-2.4,1.8,-14.8],undefined,"fly"));
  await page.click("#world-edit");
  await page.click("#tab-materials");
  await page.click("#face-open");
  await page.locator("#face-controls:not([disabled])").waitFor();
  const face=async()=>{
    const [download]=await Promise.all([page.waitForEvent("download"),page.click("#face-export")]);
    const file=await download.path();return JSON.parse(fs.readFileSync(file));
  };
  const stroke=async(x,y)=>{
    await page.locator("#face-controls:not([disabled])").waitFor();
    await page.locator("#face-canvas").evaluate(e=>e.scrollIntoView({block:"center",inline:"nearest"}));
    const b=await page.locator("#face-canvas").boundingBox();assert.ok(b);
    assert.equal(await page.evaluate(({b,x,y})=>document.elementFromPoint(b.x+b.width*x,b.y+b.height*y)?.id,{b,x,y}),"face-canvas","The stroke starts on the visible enabled canvas");
    await page.mouse.move(b.x+b.width*x,b.y+b.height*y);await page.mouse.down();
    await page.mouse.move(b.x+b.width*(x+.02),b.y+b.height*(y+.03),{steps:3});await page.mouse.up();
  };
  const confirm=async()=>{
    await page.locator("#edit-confirm").waitFor({state:"visible"});await page.click("#edit-confirm");
    await page.locator("#edit-auth").waitFor({state:"hidden"});
  };
  await page.evaluate(()=>{const read=File.prototype.text;File.prototype.text=function(){const result=read.call(this);if(this.name==="delayed-face.json")return new Promise(resolve=>{window.releaseFaceRead=()=>result.then(resolve);window.faceReadStarted=true;});return result;};});
  await page.evaluate(()=>{window.generationSamples=[];new MutationObserver(()=>{const text=document.querySelector("#face-status").textContent;const match=/Generated locally in ([0-9.]+) ms/.exec(text);if(text.includes("Generated locally in under 1 ms"))window.generationSamples.push({less_than_ms:1});else if(match)window.generationSamples.push({rounded_ms:Number(match[1]),rounding_ms:1});}).observe(document.querySelector("#face-status"),{childList:true,subtree:true,characterData:true});});
  assert.equal(await page.locator("#face-canvas").evaluate(e=>getComputedStyle(e).touchAction),"none","Painting styles are served under CSP");
  const baseline=(await snapshot()).status.project.revision;
  await stroke(.19,.43);
  const handmade=await face();
  await page.click("#tab-request");
  await page.fill("#edit-kiln-request","Add two thin healed blue scars on the left cheek; keep the fur, colors, and expression.");
  await page.click("#edit-request-review");
  await page.click("#tab-materials");
  await page.waitForFunction(()=>document.querySelector("#face-status").textContent.includes("Preview")||document.querySelector("#face-status").textContent.includes("preview"));
  // Hold a real completed preview at the private bridge, then issue a newer refusal.
  await editor.evaluate(()=>{
    const service=window.aethyrforgeEditor.service, original=service.materialView.bind(service);
    window.heldPreviewStarted=false;window.heldPreviewCompleted=false;
    service.materialView=async(request,base)=>{
      const result=await original(request,base);
      if(request.mode==='preview'&&!window.heldPreviewStarted){
        window.heldPreviewStarted=true;
        await new Promise(resolve=>window.releaseHeldPreview=resolve);
        window.heldPreviewCompleted=true;
      }
      return result;
    };
    window.restoreMaterialView=()=>service.materialView=original;
  });
  const beforeHeld=(await snapshot()).receipts.map(r=>r.request_id);
  await page.click("#face-preview");
  await editor.waitForFunction(()=>window.heldPreviewStarted);
  await page.fill("#face-request","remove all scars");await page.click("#face-generate");
  const refusal=await page.locator("#face-status").textContent();
  assert.match(refusal,/Erase handmade paint only removes brush marks/);
  await editor.evaluate(()=>window.releaseHeldPreview());
  await page.waitForFunction(ids=>JSON.parse(window.hauntDemo.snapshot()).receipts.some(r=>!ids.includes(r.request_id)&&r.outcome?.detail?.type==='material_preview'),beforeHeld);
  await page.waitForTimeout(150);
  assert.equal(await page.locator("#face-status").textContent(),refusal,"An earlier preview cannot replace the newer refusal");
  await editor.evaluate(()=>window.restoreMaterialView());
  const generated=await face();
  assert.equal(generated.handmade_png,handmade.handmade_png,"Generation preserves handmade pixels");
  assert.equal(generated.document.recipe.count,2);
  assert.deepEqual(generated.document.recipe.color,[72,104,160,225]);
  assert.equal((await snapshot()).status.project.revision,baseline,"Draft preview is read only");
  await page.fill("#face-request","remove all scars");await page.click("#face-generate");
  await page.waitForFunction(()=>document.querySelector("#face-status").textContent.includes("Erase handmade paint only removes brush marks"));
  const refused=await face();assert.deepEqual(refused,generated);
  await page.click("#face-open");assert.deepEqual(await face(),generated,"Open reuses the edited draft");
  await page.selectOption("#face-brush-mode","erase");await stroke(.19,.43);
  await page.locator("#face-handmade-strength").evaluate(e=>{e.value="0.6";e.dispatchEvent(new Event("input",{bubbles:true}));});
  const refined=await face();assert.notEqual(refined.handmade_png,generated.handmade_png);
  const expectedMaterial=await page.evaluate(async source=>{
    const material={document:structuredClone(source.document),generated:null,handmade:null};
    for(const kind of ['generated','handmade']) {
      const png_base64=source[kind+'_png'];const bytes=Uint8Array.from(atob(png_base64),c=>c.charCodeAt(0));
      const bitmap=await createImageBitmap(new Blob([bytes],{type:'image/png'}));const canvas=document.createElement('canvas');canvas.width=canvas.height=256;const ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();
      const rgba=ctx.getImageData(0,0,256,256).data;if(!rgba.some((value,index)=>index%4===3&&value))continue;
      const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
      material[kind]={resource:{sha256,bytes:bytes.length,media_type:'image/png'},png_base64};material.document[kind].image=material[kind].resource;
    }
    return material;
  },refined);
  const expectedDigest=await page.evaluate(async({url,material})=>(await import(url)).action_sha256(JSON.stringify({type:'preview_material',material})),{url:config.editor+'/sdk/'+config.sdk_sha256+'/aethyrforge_host.js',material:expectedMaterial});
  // Drain the current candidate before timing a new request for that exact draft.
  await page.waitForFunction(hash=>JSON.parse(window.hauntDemo.snapshot()).receipts.some(r=>r.outcome?.detail?.type==='material_preview'&&r.outcome.detail.action_sha256===hash),expectedDigest);
  const previousReceipts=(await snapshot()).receipts.map(r=>r.request_id);
  await page.evaluate(()=>{
    const encode=HTMLCanvasElement.prototype.toDataURL;
    HTMLCanvasElement.prototype.toDataURL=function(...args){const at=performance.now();try{return encode.apply(this,args);}finally{if(window.previewSample)window.previewSample.png_encoding_ms+=performance.now()-at;}};
    document.querySelector('#face-preview').addEventListener('click',()=>window.previewSample={started:performance.now(),png_encoding_ms:0},{capture:true,once:true});
  });
  await page.click("#face-preview");
  await page.waitForFunction(({ids,hash})=>JSON.parse(window.hauntDemo.snapshot()).receipts.some(r=>!ids.includes(r.request_id)&&r.outcome?.state==='applied'&&r.outcome?.detail?.type==='material_preview'&&r.outcome.detail.action_sha256===hash),{ids:previousReceipts,hash:expectedDigest});
  const previewTiming=await page.evaluate(()=>{const sample=window.previewSample;const result={click_event_to_adopted_frame_receipt_ms:performance.now()-sample.started,png_encoding_ms:sample.png_encoding_ms};window.previewSample=null;return result;});
  await page.screenshot({path:path.join(out,"face-preview-desktop.png")});
  await page.click("#face-cancel");
  assert.equal((await snapshot()).status.project.revision,baseline);
  await page.click("#face-apply");await confirm();
  await page.waitForFunction(revision=>JSON.parse(window.hauntDemo.snapshot()).status.project.revision>revision,baseline);
  const accepted=await face();
  await page.click("#tab-history");await page.fill("#project-name","Synthetic face project");await page.click("#project-create");
  await page.waitForFunction(()=>document.querySelector("#project-picker").value.startsWith("prj_"));
  const project=await page.locator("#project-picker").inputValue();
  await page.setInputFiles("#face-import",{name:"delayed-face.json",mimeType:"application/json",buffer:Buffer.from(JSON.stringify(generated))});
  await page.waitForFunction(()=>window.faceReadStarted);
  await page.click("#project-save-copy");await page.locator("#edit-confirm").waitFor({state:"visible"});
  await page.evaluate(()=>window.releaseFaceRead());
  await page.locator("#edit-auth").waitFor({state:"hidden"});
  await page.click("#tab-materials");const newerFace=await face();
  await page.click("#tab-history");await page.click("#project-save-copy");
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("face draft"));
  assert.equal(savedPosts.length,0);
  await page.click("#tab-materials");await page.click("#face-apply");await confirm();
  await page.click("#tab-history");await page.click("#project-save-copy");await confirm();
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("version 1"));
  assert.equal(savedPosts[0].document.layers.length,2);
  const [download]=await Promise.all([page.waitForEvent("download"),page.click("#project-export")]);
  const archive=JSON.parse(fs.readFileSync(await download.path()));
  assert.equal(archive.schema,"openagents.verse.project-export.v2");assert.equal(archive.materials.length,1);
  assert.ok(archive.materials[0].generated.png_base64);assert.ok(archive.materials[0].handmade.png_base64);
  // File.text is held to model a real delayed local import, without replacing material handlers.
  await page.click("#tab-materials");
  await page.evaluate(()=>{window.faceReadStarted=false;});
  await page.setInputFiles("#face-import",{name:"delayed-face.json",mimeType:"application/json",buffer:Buffer.from(JSON.stringify(accepted))});
  await page.waitForFunction(()=>window.faceReadStarted);
  await page.selectOption("#face-brush-mode","paint");
  const beforeStroke=await face();
  await stroke(.48,.58);const latestStroke=await face();
  assert.notEqual(latestStroke.handmade_png,beforeStroke.handmade_png,"The newer stroke actually paints the canvas");
  await page.evaluate(()=>window.releaseFaceRead());await page.waitForTimeout(300);
  assert.deepEqual(await face(),latestStroke,"Delayed import cannot replace newer strokes");
  // A read begun before Save may finish during its POST. The saved base still advances;
  // imported pixels remain editable and the completion must describe them as unsaved.
  await page.click("#face-discard");await page.click("#face-open");await page.locator("#face-controls:not([disabled])").waitFor();
  await page.evaluate(()=>{window.faceReadStarted=false;});
  await page.setInputFiles("#face-import",{name:"delayed-face.json",mimeType:"application/json",buffer:Buffer.from(JSON.stringify(newerFace))});
  await page.waitForFunction(()=>window.faceReadStarted);
  await page.click("#tab-history");await page.click("#project-save-copy");
  const arrival=new Promise(resolve=>saveArrived=resolve);delaySave=true;
  await page.click("#edit-confirm");await arrival;
  await page.evaluate(()=>window.releaseFaceRead());
  await page.waitForFunction(()=>document.querySelector("#face-status").textContent.includes("Face sources opened"));
  releaseSave();await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("Newer edits"));
  assert.equal(savedPosts.at(-1).expected_version,1);
  await page.click("#tab-materials");assert.deepEqual(await face(),newerFace);
  await page.click("#face-apply");await confirm();
  await page.click("#tab-history");await page.click("#project-save-copy");await confirm();
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("version 3"));
  assert.equal(savedPosts.at(-1).expected_version,2,"Late ACK advanced the accepted base");
  await page.click("#project-open");await confirm();
  await page.click("#tab-materials");await page.click("#face-open");await page.locator("#face-controls:not([disabled])").waitFor();
  assert.deepEqual(await face(),newerFace,"Native Save/Open retains both editable layers");
  const currentA=await face();
  const phrase=async(text)=>{
    await page.waitForFunction(()=>window.speechFixture.last&&!window.speechFixture.last.aborted);
    await page.evaluate(text=>{const recognition=window.speechFixture.last;recognition.onspeechstart?.({});recognition.onresult({results:[[{transcript:text}]]});},text);
    await page.waitForTimeout(250);
  };
  await page.click("#edit-voice-start");
  await phrase("Add three thin healed red scars on the forehead; keep the fur.");
  await page.waitForFunction(()=>document.querySelector("#face-status").textContent.includes("Face preview ready"));
  const currentB=await face();assert.equal(currentB.document.recipe.count,3);
  assert.deepEqual(currentB.document.recipe.color,[140,62,54,230]);
  assert.equal(currentB.handmade_png,currentA.handmade_png);
  await phrase("review change");await page.locator("#edit-confirm").waitFor({state:"visible"});
  await phrase("confirm change");await page.locator("#edit-auth").waitFor({state:"hidden"});
  await page.click("#edit-voice-stop");
  await page.click("#tab-history");await page.click("#project-save-copy");await confirm();
  await page.waitForFunction(()=>document.querySelector("#project-status").textContent.includes("version 4"));
  const savedB=savedPosts.at(-1).document;
  const savedA=savedPosts.at(-2).document;
  const generatedHash=document=>document.layers.find(layer=>layer.kind==="generated").texture.sha256;
  assert.notEqual(generatedHash(savedA),generatedHash(savedB));
  await editor.evaluate(()=>{
    const service=window.aethyrforgeEditor.service,read=service.materialView.bind(service);
    window.recoveryMaterialInspects=0;service.materialView=(...args)=>{if(args[0].mode==="inspect")window.recoveryMaterialInspects++;return read(...args);};
  });
  const localKey=`openagents.verse.recovery.v1:fixture-account:${project}:haunted-house:main`;
  await page.evaluate(key=>localStorage.removeItem(key),localKey);
  await page.click("#edit-undo");await confirm();
  await page.waitForFunction(()=>document.querySelector("#face-controls").disabled);
  assert.equal(await page.locator("#face-export").isEnabled(),true,"Stale sources remain exportable");
  await page.waitForFunction(({key,hash})=>{
    const copy=JSON.parse(localStorage.getItem(key)??"null");return copy?.materials?.some(bundle=>bundle.generated?.resource.sha256===hash&&bundle.generated.png_base64);
  },{key:localKey,hash:generatedHash(savedA)},{timeout:22000});
  assert.ok(await editor.evaluate(()=>window.recoveryMaterialInspects)>0,"Recovery collected the image missing after Save B pruned A");
  await page.click("#tab-materials");await page.click("#face-open");
  await page.locator("#face-controls:not([disabled])").waitFor();
  assert.deepEqual(await face(),currentA,"Undo opens the applied A layers rather than stale B canvas");
  await page.click("#tab-history");await page.click("#edit-redo");await confirm();
  await page.waitForFunction(()=>document.querySelector("#face-controls").disabled);
  await page.click("#tab-materials");await page.click("#face-open");
  await page.locator("#face-controls:not([disabled])").waitFor();
  assert.deepEqual(await face(),currentB,"Redo refreshes the current applied face");
  await page.click("#tab-compile");
  await page.fill("#edit-source",(await page.locator("#edit-source").inputValue())+"\n// face lifecycle draft");
  const beforeDraftRevision=(await snapshot()).status.project.revision;
  await page.click("#edit-source-draft");await confirm();
  assert.equal((await snapshot()).status.project.revision,beforeDraftRevision,"Keeping source draft does not advance world history");
  assert.equal(await page.locator("#face-controls").evaluate(e=>e.disabled),false,"Same-context Draft keeps the canvas usable");
  await page.click("#tab-materials");
  const shared=page.locator("#panel-materials details").filter({has:page.locator("#edit-fog")});
  await shared.locator("summary").click();
  const fog=Number(await page.locator("#edit-fog").inputValue());
  await page.fill("#edit-fog",String(fog<0.05?fog+0.005:fog-0.005));
  await page.click("#edit-fog-apply");await confirm();
  await page.waitForFunction(()=>document.querySelector("#face-controls").disabled);
  assert.equal(await page.locator("#face-export").isEnabled(),true);
  await page.click("#face-open");await page.locator("#face-controls:not([disabled])").waitFor();
  await page.selectOption("#face-brush-mode","paint");await stroke(.25,.65);
  const finalPaint=await face();assert.notEqual(finalPaint.handmade_png,currentB.handmade_png,"Painting works after another accepted edit");
  await page.setViewportSize({width:390,height:844});await page.click("#tab-materials");
  await page.screenshot({path:path.join(out,"face-workspace-mobile.png")});
  assert.deepEqual(errors,[]);
  const result={release:config.release,protocol_version:6,real_gpu_engine:true,real_native_project_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,microphone_used:false,synthetic_recognition:true,spoken_request_fixture_reaches_generator:true,fixture_direct_session:process.env.AETHYRFORGE_FIXTURE_DIRECT_SESSION==="1",text_request_reaches_local_generator:true,handmade_preserved_on_generation:true,read_only_preview_cancel:true,manual_erase_strength:true,open_reuses_draft:true,refusal_preserves_source:true,held_preview_keeps_newer_refusal_feedback:true,delayed_import_keeps_newer_strokes:true,save_blocks_unapplied_face:true,late_import_save_ack_keeps_draft_and_advances_base:true,self_contained_export:true,native_reopen_retains_editable_layers:true,clean_face_reopens_after_undo_redo_and_environment:true,same_context_draft_keeps_canvas_usable:true,missing_image_collected_for_undo_recovery:true,local_generation_cpu_samples:await page.evaluate(()=>window.generationSamples),browser:browser.version(),native_consumer:path.basename(process.env.OPENAGENTS_TEST_WEB_BINARY),sdk_sha256:config.sdk_sha256,preview_timing:previewTiming,timing_method:"Click event through a fresh receipt for the exact draft hash; the private bridge confirms an adopted renderer frame and GPU queue completion. Parent PNG encoding is reported separately within that interval. Screenshot encoding and local generation CPU time are separate; this is not first-screen-pixel or ASR latency.",errors};
  fs.writeFileSync(path.join(out,"browser-material.json"),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
} catch(error) {
  if(page){await page.screenshot({path:path.join(out,"material-error.png")});fs.writeFileSync(path.join(out,"material-error-state.json"),JSON.stringify({error:String(error),errors,body:await page.locator("body").innerText()},null,2));}
  throw error;
} finally {
  // Finish native proxy callbacks before disposing their API request context.
  await context.unrouteAll({behavior:"wait"});
  await context.close();await browser.close();server.kill("SIGTERM");accountServer.close();
  fs.writeFileSync(path.join(out,"native-fixture.log"),log);
}
