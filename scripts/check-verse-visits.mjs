// Two synthetic account identities, real native access/storage, coder-sync Loro rows, and GPU players.
// This does not exercise a real sign-in provider, microphone, or human review.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
export async function checkVisits({page,context,base,config,out,errors,visitorKey}) {
  const hash=text=>createHash('sha256').update(text).digest('hex');
  const session=await (await context.request.get(base+'/v1/verse/studio/session')).json();
  const call=async(route,body)=>{const r=await context.request.post(base+'/v1/verse/studio/'+route,{headers:{origin:base},data:{...body,csrf:session.csrf}});const v=await r.json();assert.ok(r.ok(),JSON.stringify({status:r.status(),v}));return v;};
  const made=await call('projects',{name:'Isolated visitor courtyard'}),runtime=config.personal_runtime;
  const source=JSON.stringify({schema:'openagents.verse.planet.v1',seed:7,side_metres:64,spawn:[0,1.62,12],style:'soft-pastels',initial_hour:10,cycle_enabled:false,day_minutes:10,clock_anchor_unix_ms:Date.now()});
  const effect='fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> { return base; }';
  const document={schema:'openagents.verse.project.v1',namespace:'personal-planet',template:{world:runtime.template_world,engine_release:runtime.engine_release,asset_set_sha256:runtime.asset_set_sha256,scene_sha256:runtime.scene_sha256},source_revision:0,entities:{'visit-table':{asset:'quaternius:fantasy-standard:table_large',position:[4,0,4],yaw:0,scale:1}},rooms:{},removed_walls:[],materials:{},environment:{fogDensity:.003},source_draft:effect,active_source:effect,layers:[],originals:[{name:'planet-v1.json',text:source,sha256:hash(source)}]};
  const project=`projects/${made.id}/personal-planet/main`;
  const saved=await call(project,{command_id:'visit-save',expected_version:0,document,summary:'Visitor courtyard'});
  const published=await call(project+'/publications',{command_id:'visit-publish',expected_sequence:0,operation:'publish',saved_version:1,document_sha256:saved.saved.document_sha256,public:false,confirm_runtime_distribution:true,release:null});
  const projectsBefore=(await(await context.request.get(base+"/v1/verse/studio/projects")).json()).data.map(p=>p.id).sort();
  const create=`visits/${published.world}/${published.event.release}`;
  const invite=await call(create,{command_id:'anonymous-visit',anonymous:true,hours:1});
  const privateInvite=await call(create,{command_id:'signed-visit',anonymous:false,hours:1});
  const manage=`/new-lander/invite/${published.world}/${published.event.release}`;
  await page.goto('https://openagents.com'+manage);
  await page.selectOption('select[name="access"]','signed-in');await page.selectOption('select[name="hours"]','24');
  await page.click('button[type="submit"]');await page.waitForURL('https://openagents.com'+manage);await page.locator('a[href^="/new-lander/visit/"]').nth(2).waitFor();
  const visitor=await context.browser().newContext({viewport:{width:1280,height:850}});
  const anonymous=await context.browser().newContext({viewport:{width:390,height:844}});
  const others=[visitor,anonymous];
  const routeNative=async(ctx)=>{
    await ctx.grantPermissions(['local-network-access'],{origin:'https://openagents.com'});
    await ctx.route('https://openagents.com/**',async route=>{
      const req=route.request(),u=new URL(req.url());const headers={...req.headers(),host:new URL(base).host};delete headers['content-length'];for(const key of ['origin','referer'])if(headers[key]?.startsWith('https://openagents.com'))headers[key]=headers[key].replace('https://openagents.com',base);
      const r=await ctx.request.fetch(base+u.pathname+u.search,{method:req.method(),headers,data:req.postDataBuffer()||undefined,maxRedirects:0});const returned={...r.headers()};for(const key of ['content-length','content-encoding','transfer-encoding'])delete returned[key];
      if(r.status()>=300&&r.status()<400){const dest=new URL(returned.location,req.url());assert.equal(dest.origin,'https://openagents.com');delete returned.location;delete returned['content-security-policy'];returned['content-type']='text/html';await route.fulfill({status:200,headers:returned,body:`<script>location.replace(${JSON.stringify(dest.href)})</script>`});return;}
      await route.fulfill({status:r.status(),headers:returned,body:await r.body()});
    });
  };
  const ready=async p=>{console.log('Visit player opening',p.url());try{await p.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});await p.waitForFunction(()=>window.verseVisit&&!JSON.parse(window.verseVisit.snapshot()).busy);console.log('Visit player ready');}catch(error){const diagnostic=await p.evaluate(()=>({url:location.href,body:document.body.innerText,player:(()=>{try{return window.versePlayer?.snapshot()}catch(e){return String(e)}})(),visit:(()=>{try{return window.verseVisit?.snapshot()}catch(e){return String(e)}})()}));fs.writeFileSync(path.join(out,'visit-startup-failure.json'),JSON.stringify(diagnostic,null,2));await p.screenshot({path:path.join(out,'visit-startup-failure.png')});throw error;}};
  const settled=async p=>p.waitForFunction(()=>window.verseVisit&&!JSON.parse(window.verseVisit.snapshot()).busy);
  const show=async p=>{if(await p.locator('#visit-panel').isHidden())await p.click('#visit-toggle');};
  const joined=async p=>{await show(p);await p.locator('#visit-join').waitFor();await p.click('#visit-join');await p.waitForFunction(()=>JSON.parse(window.verseVisit.snapshot()).joined);};
  const releases=new Set();
  const deadline=(promise,label,ms=20000)=>Promise.race([promise,new Promise((_,reject)=>{const timer=setTimeout(()=>reject(Error(label+' timed out')),ms);promise.finally(()=>clearTimeout(timer)).catch(()=>{});})]);
  const hold=async(ctx,pattern,predicate)=>{
    let arrive,release;const arrived=new Promise(r=>arrive=r),released=new Promise(r=>release=r);
    await ctx.route(pattern,async route=>{if(!predicate(route.request()))return route.fallback();await ctx.unroute(pattern);const req=route.request(),u=new URL(req.url());const headers={...req.headers(),host:new URL(base).host};delete headers['content-length'];const response=await ctx.request.fetch(base+u.pathname+u.search,{method:req.method(),headers,data:req.postDataBuffer()||undefined,maxRedirects:0});arrive();await deadline(released,"Held visit response release");await route.fulfill({response});});const finish=()=>{release();releases.delete(finish);};releases.add(finish);return {arrived:deadline(arrived,"Visit poll arrival"),release:finish};
  };
  try {
    await Promise.all(others.map(routeNative));
    const form=await visitor.request.get(base+'/sign-in');const markup=await form.text();
    const fields=await page.evaluate(markup=>Object.fromEntries(new FormData(new DOMParser().parseFromString(markup,'text/html').querySelector('form[action="/sign-in"]'))),markup);
    const signed=await visitor.request.post(base+'/sign-in',{form:{...fields,credential:visitorKey},headers:{origin:base},maxRedirects:0});assert.equal(signed.status(),303);
    await visitor.addCookies((await visitor.cookies(base)).map(c=>({...c,domain:'openagents.com',secure:true})));
    const second=await visitor.newPage(),guest=await anonymous.newPage();for(const p of [second,guest]){p.on('pageerror',e=>errors.push(e.message));p.on('console',m=>{if(m.type()==='error')console.log('Visitor console:',m.text());});}
    await page.goto('https://openagents.com'+invite.link);await ready(page);await joined(page);
    await second.goto('https://openagents.com'+invite.link);await ready(second);await joined(second);
    await guest.goto('https://openagents.com'+invite.link);await ready(guest);await show(guest);assert.ok(await guest.locator('#visit-login').isVisible());assert.ok(await guest.locator('#visit-text').isDisabled());
    const manifest=await second.locator('#playable-release').textContent();assert.equal(JSON.parse(manifest).id,published.event.release);
    const studio=await visitor.request.get(base+'/v1/verse/studio/'+project);assert.equal(studio.status(),404);
    await page.fill('#visit-text','Hello visitor');await page.click('#visit-send');await page.locator('#visit-messages').getByText('Hello visitor',{exact:true}).waitFor();
    await second.locator('#visit-messages').getByText('Hello visitor',{exact:true}).waitFor({timeout:15000});
    await second.fill('#visit-text','Hello host');await second.click('#visit-send');await page.locator('#visit-messages').getByText('Hello host',{exact:true}).waitFor({timeout:15000});
    assert.match(await second.locator('#visit-messages li').first().innerText(),/^Host/);assert.match(await page.locator('#visit-messages li').nth(1).innerText(),/^Visitor [0-9a-f]{6}/);
    await second.click('#visit-toggle');assert.ok(await second.locator('#visit-panel').isHidden());assert.ok(await page.locator('#visit-panel').isVisible());
    await second.reload();await ready(second);assert.ok(await second.locator('#visit-panel').isHidden());await show(second);assert.equal(await second.locator('#visit-messages li').count(),2);
    await settled(second);await second.locator('#visit-messages button[data-action="report"]').first().focus();
    await second.evaluate(()=>{window.visitFocusNode=document.activeElement;window.visitStatusMutations=0;window.visitStatusObserver=new MutationObserver(()=>window.visitStatusMutations++);window.visitStatusObserver.observe(document.querySelector('#visit-status'),{childList:true,subtree:true,characterData:true});});
    await second.waitForTimeout(6000);await settled(second);assert.ok(await second.evaluate(()=>document.activeElement===window.visitFocusNode&&document.activeElement.isConnected));assert.equal(await second.evaluate(()=>window.visitStatusMutations),0,'Unchanged poll does not re-announce Connected');
    // An explicit Leave survives a held poll and its late joined response.
    const held=await hold(visitor,'https://openagents.com/v1/verse/studio/visits/'+invite.id,r=>r.method()==='GET');
    await held.arrived;await second.click('#visit-leave');assert.equal(await second.locator('#visit-messages li').count(),0);held.release();await settled(second);await second.waitForFunction(()=>!JSON.parse(window.verseVisit.snapshot()).joined);assert.equal(await second.locator('#visit-messages li').count(),0);await joined(second);
    // Capture exact Send intent while a poll is held; a newer draft must remain local.
    const heldSend=await hold(visitor,'https://openagents.com/v1/verse/studio/visits/'+invite.id,r=>r.method()==='GET');await heldSend.arrived;
    await page.locator('#visit-messages button[data-action="report"]').first().focus();await page.evaluate(()=>window.visitFocusNode=document.activeElement);
    await second.fill('#visit-text','Exact queued A');await second.click('#visit-send');await second.fill('#visit-text','Newer unsent B');heldSend.release();
    await page.locator('#visit-messages').getByText('Exact queued A',{exact:true}).waitFor({timeout:15000});assert.ok(await page.evaluate(()=>document.activeElement===window.visitFocusNode&&document.activeElement.isConnected),'An incoming message preserves the existing focused action');await settled(second);assert.equal(await second.locator('#visit-text').inputValue(),'Newer unsent B');assert.equal(await page.locator('#visit-messages').getByText('Newer unsent B',{exact:true}).count(),0);
    await page.locator('#visit-messages li').filter({hasText:'Hello host'}).getByText('Report message',{exact:true}).click();await settled(page);
    await second.locator('#visit-messages li').filter({hasText:'Hello visitor'}).getByText('Report message',{exact:true}).click();await settled(second);
    await page.locator('#visit-messages li[data-reported="true"]').filter({hasText:'Hello visitor'}).waitFor({timeout:15000});
    await second.locator('#visit-messages li').filter({hasText:'Hello visitor'}).getByText("Hide this person's messages",{exact:true}).click();await second.waitForFunction(()=>!document.querySelector('#visit-messages').textContent.includes('Hello visitor'));
    assert.ok(await page.locator('#visit-messages').getByText('Hello visitor',{exact:true}).isVisible());await second.click('#visit-unblock');await second.locator('#visit-messages').getByText('Hello visitor',{exact:true}).waitFor({timeout:15000});
    await second.click('#visit-mute');await second.waitForFunction(()=>JSON.parse(window.verseVisit.snapshot()).muted);
    await page.fill('#visit-text','Arrived while muted');await page.click('#visit-send');await settled(page);await second.waitForTimeout(6000);assert.equal(await second.locator('#visit-messages').getByText('Arrived while muted',{exact:true}).count(),0);
    await second.click('#visit-mute');await second.locator('#visit-messages').getByText('Arrived while muted',{exact:true}).waitFor({timeout:15000});
    await page.screenshot({path:path.join(out,'visits-host-desktop.png')});await guest.screenshot({path:path.join(out,'visits-guest-mobile.png')});
    // Controlled chat-SDK import failure leaves the separately pinned player usable.
    const failed=await context.browser().newContext({viewport:{width:390,height:844}});others.push(failed);await routeNative(failed);
    await failed.route('**/aethyrforge_host.js?chat-unavailable',route=>route.abort('failed'));
    await failed.route('https://openagents.com'+invite.link,async route=>{const response=await failed.request.get(base+invite.link);let html=await response.text();html=html.replace('data-sdk-url="'+config.personal_runtime.sdk_url+'"','data-sdk-url="'+config.personal_runtime.sdk_url+'?chat-unavailable"');await route.fulfill({response,body:html});});
    const failedPage=await failed.newPage();await failedPage.goto('https://openagents.com'+invite.link);
    await failedPage.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});
    assert.ok(await failedPage.locator('#engine').isVisible());assert.ok(await failedPage.locator('#visit-panel').isVisible());assert.ok(await failedPage.locator('#visit-retry').isVisible());assert.ok(await failedPage.locator('#visit-toggle').isHidden());assert.match(await failedPage.locator('#visit-status').innerText(),/could not connect/);
    // Disposing the exported controller invalidates the held callback.
    const disposed=await hold(visitor,'https://openagents.com/v1/verse/studio/visits/'+invite.id,r=>r.method()==='GET');await disposed.arrived;
    await second.evaluate(()=>{window.verseVisit.free();window.verseVisit=null;document.querySelector('#visit-status').textContent='Disposed check sentinel';});disposed.release();await second.waitForTimeout(600);assert.equal(await second.locator('#visit-status').innerText(),'Disposed check sentinel');
    // A known logged-out identity can keep anonymous play, but cannot keep private chat.
    await second.reload();await ready(second);await show(second);await visitor.clearCookies();
    await second.waitForFunction(()=>!JSON.parse(window.verseVisit.snapshot()).joined,null,{timeout:15000});assert.equal(await second.locator('#visit-messages li').count(),0);assert.ok(await second.locator('#engine').isVisible());
    // Sign-in-required visits close the player after confirmed logout.
    await context.addCookies((await context.cookies(base)).map(c=>({...c,domain:'openagents.com',secure:true})));
    await page.goto('https://openagents.com'+privateInvite.link);await ready(page);await show(page);await context.clearCookies();
    await page.waitForFunction(()=>JSON.parse(window.verseVisit.snapshot()).ended,null,{timeout:15000});assert.ok(await page.locator('#engine').isHidden());assert.ok(await page.locator('#world-retry').isHidden());assert.ok(await page.locator('#world-fullscreen').isHidden());
    // Restore only the synthetic host cookie to test host revocation on the original invitation.
    await context.addCookies([{name:'oa_cloud_session',value:'sess_'+'a'.repeat(64),url:base},{name:'oa_cloud_session',value:'sess_'+'a'.repeat(64),url:'https://openagents.com',secure:true}]);
    const hostSession=await(await context.request.get(base+'/v1/verse/studio/session')).json();assert.equal(hostSession.signed_in,true);
    const revoked=await context.request.post(base+'/v1/verse/studio/visits/'+invite.id,{headers:{origin:base},data:{csrf:hostSession.csrf,command_id:'end-visit',operation:'revoke'}});assert.equal(revoked.status(),200);
    await guest.waitForFunction(()=>JSON.parse(window.verseVisit.snapshot()).ended,null,{timeout:15000});assert.ok(await guest.locator('#engine').isHidden());assert.ok(await guest.locator('#world-retry').isHidden());
    const projectsAfter=(await(await context.request.get(base+"/v1/verse/studio/projects")).json()).data.map(p=>p.id).sort();assert.deepEqual(projectsAfter,projectsBefore,"Visits and rejoining create no additional project or personal planet");
    assert.deepEqual(errors,[]);
    const result={real_gpu_players:true,real_native_access_store:true,real_coder_sync_loro_postgres:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,private_captures:true,two_isolated_identities:true,pinned_private_version:true,anonymous_play_without_chat:true,native_invite_form:true,independent_visibility_reconnect_no_duplicates:true,unchanged_and_incoming_poll_preserve_focus:true,unchanged_status_not_reannounced:true,held_poll_leave:true,queued_send_exact_text_keeps_newer_draft:true,visible_host_report_target:true,per_viewer_block_mute:true,held_fetch_disposal:true,chat_sdk_failure_visible_player_stays_playable:true,logout_anonymous_keeps_play_private_chat_cleared:true,logout_required_ends_player:true,revocation_ends_open_player:true,visitor_has_no_editor_membership:true,visits_create_no_additional_projects:true,errors};
    fs.writeFileSync(path.join(out,'browser-visits.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  } finally {for(const release of releases)release();for(const ctx of others){await ctx.unrouteAll({behavior:'wait'});await ctx.close();}}
}
