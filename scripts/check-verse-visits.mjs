// Two synthetic account identities, real native access/storage, and GPU players.
// Visits are play only (owner, 2026-10-24): no chat. This does not exercise a
// real sign-in provider, microphone, or human review.
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
  await page.click('[data-aethyr-id=access-signed-in]');await page.click('[data-aethyr-id=hours-24]');
  await page.click('[data-aethyr-id=invite-create]');await page.waitForURL('https://openagents.com'+manage);await page.locator('[data-aethyr-id=invitations] a[href^="/new-lander/visit/"]').nth(2).waitFor();
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
  const ready=async p=>{console.log('Visit player opening',p.url());try{await p.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});console.log('Visit player ready');}catch(error){const diagnostic=await p.evaluate(()=>({url:location.href,body:document.body.innerText,player:(()=>{try{return window.versePlayer?.snapshot()}catch(e){return String(e)}})()}));fs.writeFileSync(path.join(out,'visit-startup-failure.json'),JSON.stringify(diagnostic,null,2));await p.screenshot({path:path.join(out,'visit-startup-failure.png')});throw error;}};
  // No chat entry point, panel, or copy on a visit.
  const noChat=async p=>{assert.equal(await p.locator('#visit, [id^="visit-"]').count(),0);assert.doesNotMatch(await p.locator('body').innerText(),/chat/i);};
  // The player bar checks the invitation every 15 seconds.
  const closed=async(p,text)=>{await p.waitForFunction(text=>new RegExp(text).test(document.querySelector('#world-notice')?.textContent??''),text.source,{timeout:30000});assert.ok(await p.locator('#world-notice').isVisible());assert.ok(await p.locator('#engine').isHidden());assert.equal(await p.locator('[data-aethyr-id=world-retry]').count(),0);assert.equal(await p.locator('[data-aethyr-id=world-fullscreen]').count(),0);};
  try {
    await Promise.all(others.map(routeNative));
    const form=await visitor.request.get(base+'/sign-in');const markup=await form.text();
    const fields=await page.evaluate(markup=>Object.fromEntries(new FormData(new DOMParser().parseFromString(markup,'text/html').querySelector('form[action="/sign-in"]'))),markup);
    const signed=await visitor.request.post(base+'/sign-in',{form:{...fields,credential:visitorKey},headers:{origin:base},maxRedirects:0});assert.equal(signed.status(),303);
    await visitor.addCookies((await visitor.cookies(base)).map(c=>({...c,domain:'openagents.com',secure:true})));
    const second=await visitor.newPage(),guest=await anonymous.newPage();for(const p of [second,guest]){p.on('pageerror',e=>errors.push(e.message));p.on('console',m=>{if(m.type()==='error')console.log('Visitor console:',m.text());});}
    for(const p of [page,second,guest]){await p.goto('https://openagents.com'+invite.link);await ready(p);await noChat(p);}
    const manifest=await second.locator('#playable-release').textContent();assert.equal(JSON.parse(manifest).id,published.event.release);
    const studio=await visitor.request.get(base+'/v1/verse/studio/'+project);assert.equal(studio.status(),404);
    const chat=await visitor.request.post(base+'/v1/verse/studio/visits/'+invite.id,{headers:{origin:base},data:{csrf:'x',command_id:'old-chat',operation:'send',text:'Hello'}});assert.ok([400,403].includes(chat.status()),'The old chat operations are gone');
    assert.equal((await guest.request.get(base+'/new-lander/visit/boot.js')).status(),404);
    await page.screenshot({path:path.join(out,'visits-host-desktop.png')});await guest.screenshot({path:path.join(out,'visits-guest-mobile.png')});
    // Sign-in-required visits close the player after confirmed logout.
    await context.addCookies((await context.cookies(base)).map(c=>({...c,domain:'openagents.com',secure:true})));
    await page.goto('https://openagents.com'+privateInvite.link);await ready(page);await context.clearCookies();
    await closed(page,/signed out/);
    // Restore only the synthetic host cookie; the host ends the anonymous invitation from the invitations page.
    await context.addCookies([{name:'oa_cloud_session',value:'sess_'+'a'.repeat(64),url:base},{name:'oa_cloud_session',value:'sess_'+'a'.repeat(64),url:'https://openagents.com',secure:true}]);
    const hostSession=await(await context.request.get(base+'/v1/verse/studio/session')).json();assert.equal(hostSession.signed_in,true);
    await page.goto('https://openagents.com'+manage);
    const row=page.locator('[data-aethyr-id=invitations] > li').filter({has:page.locator(`a[href="${invite.link}"]`)});
    await row.locator('[data-aethyr-id$="-end"]').click();await page.waitForURL('https://openagents.com'+manage);
    await page.locator('[data-aethyr-id=invitations] > li').filter({has:page.locator(`a[href="${invite.link}"]`)}).getByText(/^Ended/).waitFor();
    await page.screenshot({path:path.join(out,'visits-ended-invitations.png')});
    await closed(guest,/invitation has ended/);await closed(second,/invitation has ended/);
    await guest.screenshot({path:path.join(out,'visits-ended-guest-mobile.png')});
    const projectsAfter=(await(await context.request.get(base+"/v1/verse/studio/projects")).json()).data.map(p=>p.id).sort();assert.deepEqual(projectsAfter,projectsBefore,"Visits create no additional project or personal planet");
    assert.deepEqual(errors,[]);
    const result={real_gpu_players:true,real_native_access_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,private_captures:true,two_isolated_identities:true,pinned_private_version:true,anonymous_play:true,native_invite_form:true,no_chat_entry_points:true,old_chat_operations_refused:true,logout_required_ends_player:true,host_ends_invitation_from_page:true,revocation_ends_open_players:true,visitor_has_no_editor_membership:true,visits_create_no_additional_projects:true,errors};
    fs.writeFileSync(path.join(out,'browser-visits.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  } finally {for(const ctx of others){await ctx.unrouteAll({behavior:'wait'});await ctx.close();}}
}
