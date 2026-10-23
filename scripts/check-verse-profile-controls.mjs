// Hosted profile controls with a current native consumer and synthetic account service.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';

export async function checkProfileControls({page,context,base,config,out,errors}) {
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
  await page.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});
  const player=page.frames().find(f=>f.url().startsWith(runtime.engine_url));assert.ok(player);
  await absent(player);
  for(const width of [1440,390]){
    await page.setViewportSize({width,height:width===390?844:1000});
    for(const selector of ['#world-retry','#world-fullscreen'])await hit(page,selector);
    if(invite){await hit(page,'#visit-toggle');await page.click('#visit-toggle');await hit(page,'#visit-toggle');await page.click('#visit-toggle');}
    await page.screenshot({path:path.join(out,`profile-player-${personal?'personal':'house'}-${width}.png`)});
  }
  assert.deepEqual(errors,[]);
  const result={profile:personal?'personal':'house',hosted_editor:runtime.engine_url,hosted_preview:config.preview,real_gpu:true,current_native_consumer:true,synthetic_account_service:true,real_signin_provider:false,model_loaded:false,avatar_view_controls_absent:true,pinned_player_checked:true,mobile_desktop_native_controls_reachable:true,chat_toggle_reachable:!!invite,errors};
  fs.writeFileSync(path.join(out,'profile-controls.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}
