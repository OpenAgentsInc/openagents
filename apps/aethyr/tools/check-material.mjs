// Real GPU authoring mechanics; run through openagents browser run. Captures stay in scratch.
import { createServer } from "vite";
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import assert from "node:assert/strict";
const root = fileURLToPath(new URL("..", import.meta.url)),
  out = process.env.AF_CAPTURE_DIR;
assert.ok(
  out &&
    process.env.AETHYRFORGE_PRIVATE_ASSETS &&
    process.env.OPENAGENTS_CHROME_WS,
);
const world = '"haunted-house"';
assert.match(process.env.AF_ASSET_SET??'',/^[a-f0-9]{64}$/);
const identity={"import.meta.env.VITE_OPENAGENTS_ASSET_SET":JSON.stringify(process.env.AF_ASSET_SET),"import.meta.env.VITE_OPENAGENTS_ENGINE_RELEASE":JSON.stringify('local-material-check')};
const editor = await createServer({
  root,
  configFile: root + "/vite.config.ts",
  server: { port: 0, strictPort: false },
  define: { ...identity,"import.meta.env.VITE_WORLD_PROFILE": world },
});
await editor.listen();
const editorOrigin = new URL(editor.resolvedUrls.local[0]).origin;
const preview = await createServer({
  root,
  configFile: root + "/vite.config.ts",
  cacheDir: "node_modules/.vite-material-check",
  server: { port: 0, strictPort: false },
  define: {
    ...identity,
    "import.meta.env.VITE_WORLD_PROFILE": world,
    "import.meta.env.VITE_EDITOR_ORIGIN": JSON.stringify(editorOrigin),
  },
});
await preview.listen();
const previewOrigin = new URL(preview.resolvedUrls.local[0]).origin;
editor.config.define["import.meta.env.VITE_PREVIEW_URL"] = JSON.stringify(
  previewOrigin + "/preview.html",
);
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = await browser.newContext({
  viewport: { width: 1280, height: 800 },
  hasTouch: true,
});
const result = {
  scope:
    "Real private renderer/kernel/service material mechanics with canvas-drawn test images. This does not prove the public material UI, local scar generator, authorization, or speech/provider flow.",
  errors: [],
  timings: [],
  timing_method:
    "Wall time from command submission through receipt/status polling, not first-frame visibility. Screenshot encoding and model/provider latency are excluded.",
};
let page;
try {
  page = await context.newPage();
  result.console=[];result.http_errors=[];
  page.on("console",m=>{if(["error","warning"].includes(m.type()))result.console.push(m.text().slice(0,2000));});
  page.on("response",r=>{if(r.status()>=400)result.http_errors.push({status:r.status(),path:new URL(r.url()).pathname});});
  page.on("pageerror", (e) => result.errors.push(e.message));
  await page.goto(editorOrigin);
  await page.waitForFunction(
    () =>
      window.aethyrforge?.snapshot().status.phase === "error" || (window.aethyrforge?.snapshot().status.phase === "ready" && window.aethyrforge.snapshot().status.authoring),
    null,
    { timeout: 120000 },
  );
  const snapshot = () => page.evaluate(() => window.aethyrforge.snapshot());
  const start = await snapshot();
  assert.equal(start.status.phase,'ready',JSON.stringify(start.status));
  const service = (fn,arg) => page.evaluate(fn,arg);
  const perform=async(action,ok=true)=>{
    const before=(await snapshot()).status;
    const id=await service(({action,base})=>window.aethyrforgeEditor.service.author(action,base,crypto.randomUUID()),{action,base:before.revision});
    await page.waitForFunction(id=>window.aethyrforge.snapshot().receipts.some(r=>r.requestId===id),id,{timeout:16000});
    const receipt=(await snapshot()).receipts.find(r=>r.requestId===id);
    assert.equal(receipt.ok,ok,receipt.message);
    if(ok)await page.waitForFunction(revision=>window.aethyrforge.snapshot().status.revision>revision,before.revision);
    return receipt;
  };
  const exported=()=>service(()=>window.aethyrforgeEditor.service.projectDocument());
  const initial=await exported();
  const wolf=Object.entries(initial.entities).find(([,e])=>e.asset==='custom:haunt:werewolf');
  assert.ok(wolf);
  const target={entity:wolf[0],asset:wolf[1].asset,surface:'werewolf.face',material:'custom:haunt:werewolf:face',uv_set:0,size:[256,256],uv_bounds:[0,0,1,1]};
  await service(target=>window.aethyrforgeEditor.service.command({kind:'entity.select',id:target}),target.entity);
  await page.waitForFunction(id=>window.aethyrforge.snapshot().status.selection?.id===id,target.entity);
  const [x,y,z]=wolf[1].position;
  await service(({x,y,z})=>window.aethyrforge.cameraPose([x+0.1,y+1.8,z+2.5],[x,y+1.8,z],undefined,'fly'),{x,y,z});
  await page.waitForTimeout(500);
  const pose=(await snapshot()).status.view;
  const assertPose=(view)=>{assert.deepEqual(view.eye,pose.eye);assert.deepEqual(view.target,pose.target);assert.equal(view.mode,pose.mode);assert.ok(view.animationTime>=pose.animationTime);};
  const bundle=await service(async target=>{
    const image=async(draw)=>{
      const canvas=document.createElement('canvas');canvas.width=canvas.height=256;
      const ctx=canvas.getContext('2d');draw(ctx);
      const png_base64=canvas.toDataURL('image/png').split(',')[1];
      const bytes=Uint8Array.from(atob(png_base64),c=>c.charCodeAt(0));
      const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
      return{resource:{sha256,bytes:bytes.length,media_type:'image/png'},png_base64};
    };
    const generated=await image(ctx=>{ctx.strokeStyle='#cc7788';ctx.lineWidth=5;ctx.beginPath();ctx.moveTo(25,145);ctx.lineTo(48,180);ctx.stroke();});
    const handmade=await image(ctx=>{ctx.fillStyle='#7088cc';ctx.fillRect(46,100,8,8);});
    return{document:{schema:'openagents.verse.material.v1',target,generated:{image:generated.resource,visible:true,strength:1},handmade:{image:handmade.resource,visible:true,strength:1},recipe:null},generated,handmade};
  },target);
  const view=(request)=>service(({request,base})=>window.aethyrforgeEditor.service.materialView(request,base),{request,base:(snapshotCache?.revision??0)});
  let snapshotCache=(await snapshot()).status;
  const baseline=await exported();
  await page.screenshot({path:out+'/face-before.png'});
  const at=Date.now();
  const previewed=await view({mode:'preview',material:bundle});
  assert.ok(previewed.frame>0);
  result.preview_adopted_frame_ms=Date.now()-at;
  result.preview_timing_method='Request start through a newer renderer frame after activation and GPU queue completion. Browser capture is separate; this is not a microphone or generation timing.';
  assert.deepEqual(await exported(),baseline);
  assertPose((await snapshot()).status.view);
  await page.screenshot({path:out+'/face-preview.png'});
  result.visible_face_changed_pixels=await page.evaluate(async({before,after})=>{
    const read=async encoded=>{const bytes=Uint8Array.from(atob(encoded),c=>c.charCodeAt(0));const bitmap=await createImageBitmap(new Blob([bytes],{type:'image/png'}));const canvas=document.createElement('canvas');canvas.width=bitmap.width;canvas.height=bitmap.height;const ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();return ctx.getImageData(540,305,210,165).data;};
    const a=await read(before),b=await read(after);let colored=0;
    for(let i=0;i<a.length;i+=4)if(Math.abs(b[i]-a[i])+Math.abs(b[i+1]-a[i+1])+Math.abs(b[i+2]-a[i+2])>15)colored++;
    return colored;
  },{before:fs.readFileSync(out+'/face-before.png').toString('base64'),after:fs.readFileSync(out+'/face-preview.png').toString('base64')});
  assert.ok(result.visible_face_changed_pixels>=20 && result.visible_face_changed_pixels<1500,`Expected a localized face color change: ${result.visible_face_changed_pixels}`);

  await view({mode:'clear'});
  assert.deepEqual(await exported(),baseline);
  await perform({type:'apply_material',material:bundle});
  snapshotCache=(await snapshot()).status;
  const saved=await exported();
  assert.equal(saved.layers.length,2);
  assert.ok(saved.originals.some(o=>o.name.startsWith('material-v1-')));
  const inspected=await view({mode:'inspect',target});
  assert.deepEqual(inspected.material,bundle);
  assertPose((await snapshot()).status.view);
  await page.screenshot({path:out+'/face-applied.png'});
  await perform({type:'undo'});snapshotCache=(await snapshot()).status;
  assert.equal((await exported()).layers.length,0);
  await perform({type:'redo'});snapshotCache=(await snapshot()).status;
  assert.equal((await exported()).layers.length,2);
  const prettySaved=structuredClone(saved);
  const prettySource=prettySaved.originals.find(s=>s.name.startsWith('material-v1-'));
  const reorder=value=>Array.isArray(value)?value.map(reorder):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).reverse().map(k=>[k,reorder(value[k])])):value;
  prettySource.text=JSON.stringify(reorder(JSON.parse(prettySource.text)),null,2).replaceAll('\n','\n'+' '.repeat(160));
  assert.ok(Buffer.byteLength(prettySource.text)>4096 && Buffer.byteLength(prettySource.text)<=24*1024);
  prettySource.sha256=await service(async text=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text)))].map(n=>n.toString(16).padStart(2,'0')).join(''),prettySource.text);
  const reorderedBundle=reorder(bundle);
  await perform({type:'restore_material_project',document:prettySaved,materials:[reorderedBundle]});snapshotCache=(await snapshot()).status;
  assert.deepEqual((await exported()).originals,prettySaved.originals);
  const beforeDifferent=await exported();
  const different=structuredClone(bundle);different.document.generated.strength=0.5;
  await perform({type:'restore_material_project',document:prettySaved,materials:[different]},false);
  assert.deepEqual(await exported(),beforeDifferent);
  result.reordered_pretty_source_roundtrip=true;
  result.different_source_refused_without_commit=true;
  const beforeRefusal=await exported(),beforeState=(await snapshot()).status;
  const oversized=structuredClone(bundle);oversized.document.generated.image.bytes=16385;oversized.generated.resource.bytes=16385;
  await perform({type:'apply_material',material:oversized},false);
  assert.deepEqual(await exported(),beforeRefusal);
  assert.equal((await snapshot()).status.revision,beforeState.revision);
  assertPose((await snapshot()).status.view);
  const invalid=structuredClone(bundle);invalid.generated.png_base64='invalid';
  await assert.rejects(view({mode:'preview',material:invalid}));
  assert.deepEqual(await exported(),beforeRefusal);
  const cleared=await view({mode:'clear'});
  const other={...target,entity:'missing-werewolf'};
  await assert.rejects(view({mode:'inspect',target:other}));
  assert.deepEqual(await exported(),beforeRefusal);
  result.preview_readonly=true;result.cancel_preserved_authored_state=true;result.apply_undo_redo_restore_sources=true;result.failed_size_and_image_kept_state_pose=true;
  result.selection=(await snapshot()).status.selection;
  await page.screenshot({path:out+'/face-reopened.png'});
  // Add an independent instance through the same supported placement command.
  await perform({type:'propose',operations:[{type:'place',asset:target.asset,position:[x+1.3,y,z]}]});
  snapshotCache=(await snapshot()).status;
  const twins=await exported();
  const secondWolf=Object.entries(twins.entities).find(([id,entity])=>id!==target.entity&&entity.asset===target.asset);
  assert.ok(secondWolf);
  await service(({x,y,z})=>window.aethyrforge.cameraPose([x+.65,y+1.8,z+3.9],[x+.65,y+1.8,z],undefined,'fly'),{x,y,z});
  await page.waitForTimeout(300);
  await page.screenshot({path:out+'/two-wolves-before.png'});
  const changed=await service(async original=>{
    const material=structuredClone(original);
    const canvas=document.createElement('canvas');canvas.width=canvas.height=256;
    const ctx=canvas.getContext('2d');ctx.fillStyle='rgba(30,200,90,.9)';ctx.fillRect(20,138,40,50);
    const png_base64=canvas.toDataURL('image/png').split(',')[1];
    const bytes=Uint8Array.from(atob(png_base64),c=>c.charCodeAt(0));
    const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
    material.generated={resource:{sha256,bytes:bytes.length,media_type:'image/png'},png_base64};
    material.document.generated.image=material.generated.resource;return material;
  },bundle);
  await view({mode:'preview',material:changed});
  await page.screenshot({path:out+'/two-wolves-after.png'});
  result.per_instance=await page.evaluate(async({before,after,second})=>{
    const read=async encoded=>{const bytes=Uint8Array.from(atob(encoded),c=>c.charCodeAt(0));const bitmap=await createImageBitmap(new Blob([bytes],{type:'image/png'}));const canvas=document.createElement('canvas');canvas.width=bitmap.width;canvas.height=bitmap.height;const ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();return ctx.getImageData(0,0,canvas.width,canvas.height);};
    const a=await read(before),b=await read(after);
    const count=([x,y,width,height])=>{let changed=0;for(let row=y;row<y+height;row++)for(let col=x;col<x+width;col++){const i=4*(row*a.width+col);if(Math.abs(a.data[i]-b.data[i])+Math.abs(a.data[i+1]-b.data[i+1])+Math.abs(a.data[i+2]-b.data[i+2])>15)changed++;}return changed;};
    const regions={selected_face:[460,300,130,180],second_face:[690,300,130,180],unrelated_surface:[120,300,180,160]};
    return{second_entity:second,regions,changed_pixels:Object.fromEntries(Object.entries(regions).map(([name,bounds])=>[name,count(bounds)]))};
  },{before:fs.readFileSync(out+'/two-wolves-before.png').toString('base64'),after:fs.readFileSync(out+'/two-wolves-after.png').toString('base64'),second:secondWolf[0]});
  assert.ok(result.per_instance.changed_pixels.selected_face>=10,JSON.stringify(result.per_instance));
  assert.equal(result.per_instance.changed_pixels.second_face,0,JSON.stringify(result.per_instance));
  assert.equal(result.per_instance.changed_pixels.unrelated_surface,0,JSON.stringify(result.per_instance));
  assert.ok(!(await exported()).originals.some(source=>source.name.startsWith('material-v1-')&&JSON.parse(source.text).target.entity===secondWolf[0]));
  await view({mode:'clear'});
  assert.deepEqual(await exported(),twins,'Per-instance preview remains non-mutating');
  assert.deepEqual(result.errors,[]);
  fs.writeFileSync(out+'/browser.json',JSON.stringify(result,null,2));
} catch (error) {
  result.failure = String(error);
  if(page){result.last=await page.evaluate(()=>({state:window.aethyrforge?.snapshot(),text:document.body.innerText.slice(-3000)})).catch(()=>null);await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}
  fs.writeFileSync(out + "/browser.json", JSON.stringify(result, null, 2));
  throw error;
} finally {
  await context.close();
  await browser.close();
  await preview.close();
  await editor.close();
}
