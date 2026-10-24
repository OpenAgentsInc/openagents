// Hosted profile controls with a current native consumer and synthetic account service.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';

export async function checkProfileControls({page,context,base,config,out,errors}) {
  if(process.env.AETHYRFORGE_PROFILE_ZERO_START==='1')await context.addInitScript(()=>{
    const post=Worker.prototype.postMessage;
    Worker.prototype.postMessage=function(value,...rest){
      if(value?.type==='attach')value={...value,width:0,height:0};
      return post.call(this,value,...rest);
    };
  });
  if(process.env.AETHYRFORGE_PROFILE_GEOMETRY==='1')page.on('console',message=>{if(message.text().startsWith('Profile viewport ')||message.text().startsWith('Profile parent '))console.log(message.text());});
  if(process.env.AETHYRFORGE_PROFILE_GEOMETRY==='1')await context.addInitScript(()=>{
    const geometry=e=>{if(!e)return null;const r=e.getBoundingClientRect(),s=getComputedStyle(e);return {width:r.width,height:r.height,display:s.display,visibility:s.visibility,hidden:e.hidden};};
    const post=Worker.prototype.postMessage;
    Worker.prototype.postMessage=function(value,...rest){
      if(value?.type==='attach'||value?.type==='resize')console.log('Profile viewport '+JSON.stringify({url:location.href,type:value.type,width:value.width,height:value.height,inner:[innerWidth,innerHeight],canvas:geometry(document.querySelector('canvas')),body:geometry(document.body),ready:document.readyState}));
      return post.call(this,value,...rest);
    };
    let last='';const timer=setInterval(()=>{const e=document.querySelector('#engine,#preview-frame');if(!e)return;const value=JSON.stringify({url:location.href,inner:[innerWidth,innerHeight],engine:geometry(e),body:geometry(document.body),main:geometry(document.querySelector('main')),sheets:[...document.styleSheets].map(s=>s.href),ready:document.readyState});if(value!==last){last=value;console.log('Profile parent '+value);}},20);setTimeout(()=>clearInterval(timer),15000);
  });
  await context.addInitScript(()=>{
    let last='';setInterval(()=>{
      const status=window.aethyrforgeEditor?.snapshot().status;
      if(status?.phase==='error'&&status.message!==last){last=status.message;console.error('Profile renderer failure: '+JSON.stringify({message:status.message,phase:status.phase}));}
    },20);
  });
  const personal=process.env.AETHYRFORGE_PROFILE_CONTROLS==='personal';
  const runtime=personal?config.personal_runtime:config.publish_runtime;
  const session=await(await context.request.get(base+'/v1/verse/studio/session')).json();
  const call=async(route,body)=>{
    const response=await context.request.post(base+'/v1/verse/studio/'+route,{headers:{origin:base},data:{...body,csrf:session.csrf}});
    const value=await response.json();assert.ok(response.ok(),JSON.stringify({status:response.status(),value}));return value;
  };
  let studioPath='/new-lander/house';
  if(personal) {
    // Native-only tutorial setup; this is not rendered tutorial journey evidence.
    const p=await call('projects',{name:'Profile tutorial precondition'}),r=config.publish_runtime;
    const effect='fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> { return base; }';
    const document={schema:'openagents.verse.project.v1',namespace:'haunted-house',template:{world:r.template_world,engine_release:r.engine_release,asset_set_sha256:r.asset_set_sha256,scene_sha256:r.scene_sha256},source_revision:0,entities:{},rooms:{},removed_walls:[],materials:{},environment:{},source_draft:effect,active_source:effect,layers:[],originals:[]};
    const route=`projects/${p.id}/haunted-house/main`;
    const saved=await call(route,{command_id:'profile-precondition-save',expected_version:0,document,summary:'Profile tutorial precondition'});
    const publication=await call(route+'/publications',{command_id:'profile-precondition-publish',expected_sequence:0,operation:'publish',saved_version:1,document_sha256:saved.saved.document_sha256,public:true,confirm_runtime_distribution:true,release:null});
    await call('tutorial',{operation:'play',world:publication.world,release:publication.event.release});await call('tutorial',{operation:'return'});
    const award=await call('tutorial',{operation:'allocate'});studioPath='/new-lander/planet/'+award.progress.planet.id+'/world';
  }
  await page.goto('https://openagents.com'+studioPath);
  await page.waitForFunction(()=>window.hauntDemo&&JSON.parse(window.hauntDemo.snapshot()).phase==='open'&&JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:125000});
  const editor=page.frames().find(f=>f.url().startsWith(runtime.engine_url));assert.ok(editor);
  const absent=async(frame)=>{
    await frame.waitForFunction(()=>window.aethyrforgeEditor?.snapshot().status.phase==='ready');
    const status=await frame.evaluate(()=>window.aethyrforgeEditor.snapshot().status);
    assert.equal(status.character.modelLoaded,false);
    assert.equal(await frame.locator('nav[aria-label="Play controls"]').count(),0);
    const preview=page.frames().find(f=>f.url().startsWith(personal?config.personal_preview:config.preview));
    if(preview){await preview.locator('#world').focus();await page.keyboard.press('v');}
    assert.equal(await frame.locator('button[aria-label*="third person"]').count(),0);
  };
  await absent(editor);
  const hit=async(p,selector)=>{
    const node=p.locator(selector);await node.scrollIntoViewIfNeeded();const box=await node.boundingBox();assert.ok(box,selector);
    assert.ok(await node.evaluate((e,b)=>{const target=document.elementFromPoint(b.x+b.width/2,b.y+b.height/2);return target===e||e.contains(target);},box),selector+' is reachable');
  };
  for(const width of [1440,390]){
    await page.setViewportSize({width,height:width===390?844:1000});
    for(const selector of ['#world-edit','#edit-voice-start','#edit-voice-stop'])await hit(page,selector);
    await page.screenshot({path:path.join(out,`profile-${personal?'personal':'house'}-${width}.png`)});
  }
  if(process.env.AETHYRFORGE_PROFILE_STUDIO_ONLY==='1') {
    const result={profile:personal?'personal':'house',hosted_editor:runtime.engine_url,real_gpu:true,current_native_consumer:true,synthetic_account_service:true,real_signin_provider:false,native_only_tutorial_precondition:personal,model_loaded:false,avatar_view_controls_absent:true,mobile_desktop_native_controls_reachable:true,pinned_player_checked:false,pinned_player_pending:'Hidden-frame zero viewport renderer failure; tracked separately.'};
    fs.writeFileSync(path.join(out,'profile-controls.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));return;
  }
  const project=await call('projects',{name:'Profile control check'});
  const effect='fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> { return base; }';
  const source=JSON.stringify({schema:'openagents.verse.planet.v1',seed:7,side_metres:64,spawn:[0,1.62,12],style:'soft-pastels',initial_hour:10,cycle_enabled:false,day_minutes:10,clock_anchor_unix_ms:Date.now()});
  const document={schema:'openagents.verse.project.v1',namespace:personal?'personal-planet':'haunted-house',template:{world:runtime.template_world,engine_release:runtime.engine_release,asset_set_sha256:runtime.asset_set_sha256,scene_sha256:runtime.scene_sha256},source_revision:0,entities:{},rooms:{},removed_walls:[],materials:{},environment:{},source_draft:effect,active_source:effect,layers:[],originals:personal?[{name:'planet-v1.json',text:source,sha256:createHash('sha256').update(source).digest('hex')}]:[]};
  const route=`projects/${project.id}/${document.namespace}/main`;
  const saved=await call(route,{command_id:'profile-save',expected_version:0,document,summary:'Profile controls'});
  const publication=await call(route+'/publications',{command_id:'profile-publish',expected_sequence:0,operation:'publish',saved_version:1,document_sha256:saved.saved.document_sha256,public:true,confirm_runtime_distribution:true,release:null});
  const invite=personal?await call(`visits/${publication.world}/${publication.event.release}`,{command_id:'profile-invite',anonymous:true,hours:1}):null;
  await page.goto('https://openagents.com'+(invite?.link??publication.url));
  await page.waitForFunction(()=>window.versePlayer&&(()=>{const e=JSON.parse(window.versePlayer.snapshot()).engine;return e.error||e.receipts.some(r=>r.outcome.message==='Playable version opened.');})(),null,{timeout:125000});
  assert.equal((await page.evaluate(()=>JSON.parse(window.versePlayer.snapshot()))).engine.error??null,null,'Pinned player must open');
  const player=page.frames().find(f=>f.url().startsWith(runtime.engine_url));assert.ok(player);
  await absent(player);
  const fullSizes=[];
  const fullSize=async(frame)=>{
    const outer=await page.locator('#engine').boundingBox();assert.ok(outer&&outer.width>0&&outer.height>0);
    await page.waitForTimeout(250);
    const size=await frame.evaluate(async()=>{
      const blob=await window.aethyrforgeEditor.service.captureBlob();const image=await createImageBitmap(blob);
      const size={width:image.width,height:image.height,innerWidth,innerHeight};image.close();return size;
    });
    assert.equal(size.width,Math.round(outer.width));assert.equal(size.height,Math.round(outer.height));fullSizes.push(size);
  };
  for(const width of [1440,390]){
    await page.setViewportSize({width,height:width===390?844:1000});
    await fullSize(player);
    for(const selector of ['[data-aethyr-id=world-retry]','[data-aethyr-id=world-fullscreen]'])await hit(page,selector);
    if(invite)assert.equal(await page.locator('#visit, [id^="visit-"]').count(),0,'Visits have no chat');
    await page.screenshot({path:path.join(out,`profile-player-${personal?'personal':'house'}-${width}.png`)});
  }
  const before=await page.evaluate(()=>JSON.parse(window.versePlayer.snapshot()));
  await page.locator('#engine').evaluate(frame=>{frame.hidden=true;frame.style.display='none';});
  await page.waitForTimeout(300);
  assert.equal((await page.evaluate(()=>JSON.parse(window.versePlayer.snapshot()))).engine.phase,'open');
  await page.locator('#engine').evaluate(frame=>{frame.style.removeProperty('display');frame.hidden=false;});
  await page.setViewportSize({width:1280,height:900});await fullSize(player);
  const recovered=await page.evaluate(()=>JSON.parse(window.versePlayer.snapshot()));
  assert.equal(recovered.engine.session,before.engine.session);assert.equal(recovered.release,before.release);
  assert.equal(recovered.engine.status.revision,before.engine.status.revision);
  await page.reload();
  await page.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});
  const reloaded=page.frames().find(f=>f.url().startsWith(runtime.engine_url));assert.ok(reloaded);await absent(reloaded);await fullSize(reloaded);
  assert.deepEqual(errors,[]);
  const result={profile:personal?'personal':'house',hosted_editor:runtime.engine_url,hosted_preview:personal?config.personal_preview:config.preview,real_gpu:true,current_native_consumer:true,synthetic_account_service:true,real_signin_provider:false,model_loaded:false,avatar_view_controls_absent:true,pinned_player_checked:true,mobile_desktop_native_controls_reachable:true,chat_toggle_reachable:!!invite,forced_zero_attach:process.env.AETHYRFORGE_PROFILE_ZERO_START==='1',hidden_zero_then_visible_same_session_revision:true,reload_same_version:true,rendered_sizes:fullSizes,errors};
  fs.writeFileSync(path.join(out,'profile-controls.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}
