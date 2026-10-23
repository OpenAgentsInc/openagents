import assert from 'node:assert/strict';
import {chromium} from '@playwright/test';
import {writeFile} from 'node:fs/promises';
assert.ok(process.env.HAUNT_URL&&process.env.OPENAGENTS_CHROME_PORT);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
const context=browser.contexts()[0],decisions=[];
await context.route(url=>['/api/haunt/decision','/api/medieval/decision'].includes(url.pathname),route=>{decisions.push(new URL(route.request().url()).pathname);return route.abort('blockedbyclient');});
const report={browser:browser.version(),checks:[],passed:false};
const inspect=page=>page.evaluate(()=>{const s=window.hauntPreview.snapshot();return {project:s.project,canUndo:s.canUndo,canRedo:s.canRedo,accepted:s.accepted,draft:s.draft,hash:s.hash,device:s.deviceId,pose:s.pose,latest:s.latestResult,raw:window.kilnReadShader(),scene:window.kilnReadScene(),text:document.querySelector('#shader').value};});
async function waitSource(page,source){await page.waitForFunction(source=>window.hauntPreview.snapshot().project.materialDraft?.source===source&&window.kilnReadShader()===source&&document.querySelector('#shader').value===source,source,{timeout:20000});}
try {
 for(const path of ['', 'medieval/']) {
  const page=await context.newPage();
  try {
   const url=new URL(path,process.env.HAUNT_URL).href;
   await page.goto(url);await page.waitForFunction(()=>window.hauntPreview?.snapshot().accepted>0&&!document.querySelector('#ask').disabled,{}, {timeout:45000});
   const before=await inspect(page),original=await page.evaluate(()=>window.hauntPreview.original());
   assert.equal(before.project.materialDraft?.source,before.raw,'Opening must seed source history without a fake undo entry');
   assert.equal(before.text,before.raw);
   const source=original+'\n// manual history probe\n';
   if(path){await page.getByRole('button',{name:'Lab settings',exact:true}).click();await page.getByText('Edit the shader',{exact:true}).click();}else await page.getByText('Advanced · edit the shader',{exact:true}).click();
   await page.locator('#shader').fill(source);await page.locator('#save').click();await waitSource(page,source);await page.waitForFunction(accepted=>window.hauntPreview.snapshot().accepted>accepted&&window.hauntPreview.snapshot().latestResult.type==='accepted'&&!document.querySelector('#save').disabled,before.accepted);
   const first=await inspect(page);assert.equal(first.project.revision,before.project.revision+1);
   const tint=first.project.tint==='blue'?'orange':'blue';await page.evaluate(t=>window.hauntPreview.command({kind:'set_tint',tint:t}),tint);
   const bad=source+'\nthis is deliberately invalid WGSL';
   assert.equal(await page.evaluate(async s=>await window.kilnWrite(window.hauntPreview.snapshot().project.material,s),bad),0);
   await waitSource(page,bad);await page.waitForFunction(()=>{const s=window.hauntPreview.snapshot();return s.latestResult.type==='rejected'&&s.latestResult.revision===s.draft;});
   const invalid=await inspect(page);assert.equal(invalid.project.revision,first.project.revision+2);assert.equal(invalid.hash,first.hash);assert.equal(invalid.accepted,first.accepted);assert.ok(invalid.draft>first.draft);
   await page.evaluate(()=>window.hauntPreview.command({kind:'undo'}));await waitSource(page,source);
   let state=await inspect(page);assert.equal(state.project.tint,tint);
   await page.evaluate(()=>window.hauntPreview.command({kind:'undo'}));state=await inspect(page);assert.equal(state.project.tint,before.project.tint);
   await page.evaluate(()=>window.hauntPreview.command({kind:'undo'}));await waitSource(page,before.raw);
   await page.evaluate(()=>window.hauntPreview.command({kind:'redo'}));await waitSource(page,source);
   await page.evaluate(()=>window.hauntPreview.command({kind:'redo'}));await page.evaluate(()=>window.hauntPreview.command({kind:'redo'}));await waitSource(page,bad);await page.waitForFunction(()=>{const s=window.hauntPreview.snapshot();return s.latestResult.type==='rejected'&&s.latestResult.revision===s.draft;});
   const stable=await inspect(page);assert.deepEqual(stable.project.entities,before.project.entities);assert.deepEqual(stable.project.layout,before.project.layout);assert.deepEqual(stable.project.town,before.project.town);assert.equal(stable.device,before.device);assert.deepEqual(stable.pose,before.pose);
   const stale=structuredClone(stable.project);stale.revision=0;stale.tint='green';
   assert.equal(await page.evaluate(async p=>window.kilnWrite(p.id==='medieval-town-v1'?'medieval/scene.json':'haunt/scene.json',JSON.stringify(p)),stale),0);
   await page.waitForFunction(()=>document.querySelector('#scene-status').textContent.includes('scene changed'));
   assert.deepEqual((await inspect(page)).project,stable.project);
   const emptyResult=await page.evaluate(()=>window.hauntPreview.save(''));assert.equal(emptyResult.type,'rejected');
   await waitSource(page,'');const empty=await inspect(page);assert.equal(empty.latest.type,'rejected');assert.equal(empty.latest.revision,empty.draft);assert.ok(empty.draft>stable.draft);assert.equal(empty.project.revision,stable.project.revision+1);assert.equal(empty.accepted,stable.accepted);assert.equal(empty.hash,stable.hash);
   await page.reload();await page.waitForFunction(()=>window.hauntPreview?.snapshot().accepted>0&&!document.querySelector('#ask').disabled,{}, {timeout:45000});await waitSource(page,'');
   const reopened=await inspect(page);assert.deepEqual(reopened.project,empty.project);
   const padding=65536-new TextEncoder().encode(original+'\n/**/').length;const large=original+'\n/*'+String.fromCharCode(34,92).repeat(Math.floor(padding/2))+'x'.repeat(padding%2)+'*/';assert.equal(new TextEncoder().encode(large).length,65536);const escapedSourceBytes=new TextEncoder().encode(JSON.stringify(large)).length;assert.ok(escapedSourceBytes>65536);
   const largeResult=await page.evaluate(s=>window.hauntPreview.save(s),large);assert.equal(largeResult.type,'accepted');await waitSource(page,large);
   const saved=await inspect(page);assert.ok(new TextEncoder().encode(saved.scene).length>65536);assert.deepEqual(JSON.parse(saved.scene),saved.project);
   await page.reload();await page.waitForFunction(()=>window.hauntPreview?.snapshot().accepted>0&&!document.querySelector('#ask').disabled,{}, {timeout:45000});await waitSource(page,large);
   const final=await inspect(page);assert.deepEqual(final.project,saved.project);
   const legacy=structuredClone(final.project);delete legacy.materialDraft;
   const legacySource=original+'\n// legacy file migration\n';
   const setup=await page.evaluate(async ({project,source})=>{window.kilnShaderChanged=undefined;window.kilnSceneChanged=undefined;return [await window.kilnWrite(project.material,source),await window.kilnWrite(project.id==='medieval-town-v1'?'medieval/scene.json':'haunt/scene.json',JSON.stringify(project))];},{project:legacy,source:legacySource});assert.deepEqual(setup,[0,0]);
   await page.reload();await page.waitForFunction(()=>window.hauntPreview?.snapshot().accepted>0&&!document.querySelector('#ask').disabled,{}, {timeout:45000});await waitSource(page,legacySource);
   const migrated=await inspect(page);assert.deepEqual(migrated.project.entities,legacy.entities);assert.equal(migrated.project.revision,legacy.revision);assert.equal(JSON.parse(migrated.scene).materialDraft.source,legacySource);
   assert.equal(migrated.canUndo,false);const legacyEdit=legacySource+'\n// first edit after legacy restore\n';assert.equal((await page.evaluate(source=>window.hauntPreview.save(source),legacyEdit)).type,'accepted');await waitSource(page,legacyEdit);await page.evaluate(()=>window.hauntPreview.command({kind:'undo'}));await waitSource(page,legacySource);const legacyUndone=await inspect(page);assert.equal(legacyUndone.canUndo,false);assert.equal(legacyUndone.canRedo,true);assert.deepEqual(legacyUndone.project.layout,legacy.layout);assert.deepEqual(legacyUndone.project.town,legacy.town);
   const burst=[original+'\n// first queued file\n','invalid queued source',original+'\n// newest queued file\n'];
   const codes=await page.evaluate(async sources=>{const out=[];for(const source of sources)out.push(await window.kilnWrite(window.hauntPreview.snapshot().project.material,source));return out;},burst);assert.deepEqual(codes,[0,0,0]);await waitSource(page,burst[2]);
   const expectedHash=await page.evaluate(async source=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(source)))].map(n=>n.toString(16).padStart(2,'0')).join(''),burst[2]);
   await page.waitForFunction(hash=>window.hauntPreview.snapshot().hash===hash&&window.hauntPreview.snapshot().latestResult.type==='accepted',expectedHash);
   await page.waitForTimeout(100);const newest=await inspect(page);assert.equal(newest.project.revision,legacyUndone.project.revision+3);assert.deepEqual(newest.project.entities,migrated.project.entities);
   report.checks.push({lab:before.project.id,manualUndoRedo:true,rawUndoRedo:true,invalidLastGood:true,staleRejected:true,emptyReopen:true,largeReopen:true,legacyMigration:true,legacyFirstEditUndo:true,rapidRawNewest:true,layoutAndTownPreserved:true,escapedSourceBytes,bytes:new TextEncoder().encode(saved.scene).length,stableEntities:true,deviceAndPosePreserved:true});
  }finally{await page.evaluate(()=>window.hauntPreview?.dispose()).catch(()=>{});await page.close();}
 }
 assert.deepEqual(decisions,[]);report.passed=true;console.log(JSON.stringify(report,null,2));
}catch(error){report.error=String(error);throw error;}
finally{await browser.close();assert.deepEqual(decisions,[]);if(process.env.HAUNT_MATERIAL_REPORT)await writeFile(process.env.HAUNT_MATERIAL_REPORT,JSON.stringify(report,null,2));}
