// Public conformance check. Set NODE_PATH to a Playwright installation.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS || `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
const context = browser.contexts()[0];
const page = await context.newPage();
const errors=[];
page.on('pageerror',e=>errors.push(e.message));
const host = process.env.AETHYRFORGE_TEST_HOST || 'http://127.0.0.1:55306';
const engine = process.env.AETHYRFORGE_TEST_ENGINE || process.env.AETHYRFORGE_TEST_MOCK || 'http://127.0.0.1:55307/mock.html';
const privateEngine=!!process.env.AETHYRFORGE_TEST_ENGINE;
const config=new URLSearchParams({engine,release:privateEngine?'local-conformance-release':'fixture-release',abi:privateEngine?'aethyrforge.browser.v1':'fixture-abi',project:privateEngine?'medieval-town-lab':'fixture-world'});
await page.goto(`${host}/?${config}`);
try { await page.waitForFunction(()=>window.host && JSON.parse(window.host.snapshot()).phase==='open',null,{timeout:18000}); } catch(e) {console.error('Connection diagnostics',errors,await page.evaluate(()=>window.host?.snapshot()),page.frames().map(f=>f.url()));throw e;}
const snapshot=()=>page.evaluate(()=>JSON.parse(window.host.snapshot()));
await page.click('#observe');await page.waitForFunction(()=>JSON.parse(window.host.snapshot()).status.frame>3);
const before=await snapshot();
await page.click('#panel-toggle');await page.click('#panel-toggle');await page.click('#inspect');
await page.waitForFunction(frame=>JSON.parse(window.host.snapshot()).status.frame>frame,before.status.frame);
let after=await snapshot();assert.equal(after.session,before.session);assert.equal(after.status.generation,before.status.generation);
// An unsolicited window message cannot enter the scoped channel.
await page.evaluate(()=>window.postMessage(JSON.stringify({protocol:'openagents.aethyrforge.host',version:1,session:'forged',sequence:999,message:{type:'revoked',message:'forged'}}),location.origin));
assert.equal((await snapshot()).phase,'open');
const engineFrame=page.frames().find(f=>f.url().startsWith(engine));assert.ok(engineFrame);
if(privateEngine)await engineFrame.evaluate(()=>window.aethyrforge.cameraPose([3,4,6],[3,4,5],undefined,'fly'));
else await engineFrame.locator('body').click();
await page.waitForFunction(x=>JSON.parse(window.host.snapshot()).status.checkpoint.eye[0]!==x,before.status.checkpoint.eye[0]);
const checkpoint=(await snapshot()).status.checkpoint;assert.ok(checkpoint);
await page.click('#reload');
await page.waitForFunction(session=>{const s=JSON.parse(window.host.snapshot());return s.phase==='open'&&s.session!==session},before.session,{timeout:30000});
after=await snapshot();assert.notEqual(after.status.generation,before.status.generation);
for(let i=0;i<3;i++){assert.ok(Math.abs(after.status.checkpoint.eye[i]-checkpoint.eye[i])<0.05);assert.ok(Math.abs(after.status.checkpoint.target[i]-checkpoint.target[i])<0.05)}
assert.equal(after.status.checkpoint.mode,checkpoint.mode);
await page.evaluate(()=>window.host.request(JSON.stringify({type:'close'})));
await page.waitForFunction(()=>JSON.parse(window.host.snapshot()).phase==='closed');
await page.click('#leave');assert.equal(await page.locator('iframe').count(),0);
await page.waitForFunction(()=>document.querySelector('iframe')===null);
if(privateEngine)assert.equal(page.workers().length,0);
// Pinned release mismatch must never open.
const invalid=new URLSearchParams(config);invalid.set('release','unapproved-release');
await page.goto(`${host}/?${invalid}`);await page.waitForFunction(()=>JSON.parse(window.host.snapshot()).error!==null,null,{timeout:30000});assert.notEqual((await snapshot()).phase,'open');
await page.evaluate(()=>window.host.close());
for(const variant of ['session','version','payload','bytes','object']) {
  const badEngine=new URL(`forged-${variant}.html`,process.env.AETHYRFORGE_TEST_MOCK || 'http://127.0.0.1:55307/mock.html').href;
  await page.goto(`${host}/?${new URLSearchParams({engine:badEngine,release:'fixture-release',abi:'fixture-abi',project:'fixture-world'})}`);
  await page.waitForFunction(()=>JSON.parse(window.host.snapshot()).error!==null,null,{timeout:10000});
  assert.notEqual((await snapshot()).phase,'open');
  await page.evaluate(()=>window.host.close());
}
assert.deepEqual(errors,[]);
console.log(JSON.stringify({engine:privateEngine?'private-artifact':'public-mock',checks:['ready/open/inspect/observe/close','overlay continuity','window forgery ignored','reload checkpoint','release mismatch rejected','frame cleanup','forged session/version/payload/bytes/transport rejected'],before,after},null,2));
await page.close();await browser.close();
