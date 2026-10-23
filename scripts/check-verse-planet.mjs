// Personal-world handler checks. Recognition events and account identity are synthetic.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
export async function checkPersonalWorld({page,context,base,config,out,errors}) {
  assert.ok(config.personal_runtime);
  const session=await (await context.request.get(base+'/v1/verse/studio/session')).json();
  const call=async (route,body)=>{
    const response=await context.request.post(base+'/v1/verse/studio/'+route,{headers:{origin:base},data:{...body,csrf:session.csrf}});
    const value=await response.json();assert.ok([200,201].includes(response.status()),JSON.stringify({status:response.status(),value}));return value;
  };
  // Native-only tutorial setup is a precondition, not rendered Haunted House journey evidence.
  const project=await call('projects',{name:'Synthetic tutorial precondition'});
  const r=config.publish_runtime;
  const effect='fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> { return base; }';
  const document={schema:'openagents.verse.project.v1',namespace:'haunted-house',template:{world:r.template_world,engine_release:r.engine_release,asset_set_sha256:r.asset_set_sha256,scene_sha256:r.scene_sha256},source_revision:0,entities:{},rooms:{},removed_walls:[],materials:{},environment:{},source_draft:effect,active_source:effect,layers:[],originals:[]};
  const saved=await call(`projects/${project.id}/haunted-house/main`,{command_id:'precondition-save',expected_version:0,document,summary:'Synthetic tutorial precondition'});
  const publication=await call(`projects/${project.id}/haunted-house/main/publications`,{command_id:'precondition-publish',expected_sequence:0,operation:'publish',saved_version:1,document_sha256:saved.saved.document_sha256,public:true,confirm_runtime_distribution:true,release:null});
  await call('tutorial',{operation:'play',world:publication.world,release:publication.event.release});
  await call('tutorial',{operation:'return'});
  const award=await call('tutorial',{operation:'allocate'});
  const planet=award.progress.planet, url='https://openagents.com/new-lander/planet/'+planet.id+'/world';
  const ready=async()=>page.waitForFunction(()=>window.hauntDemo&&JSON.parse(window.hauntDemo.snapshot()).phase==='open'&&JSON.parse(window.hauntDemo.snapshot()).status.authoring,null,{timeout:125000});
  await page.goto(url);await ready();
  await page.waitForFunction(id=>document.querySelector('#project-picker').value===id,planet.project);
  let editor=page.frames().find(f=>f.url().startsWith(config.editor));assert.ok(editor);
  const exported=()=>editor.evaluate(()=>window.aethyrforgeEditor.service.projectDocument());
  const recipe=d=>JSON.parse(d.originals.find(s=>s.name==='planet-v1.json').text);
  const confirm=async()=>{await page.locator('#edit-confirm').waitFor({state:'visible'});await page.click('#edit-confirm');await page.locator('#edit-auth').waitFor({state:'hidden'});};
  const edit=async tab=>{if(await page.locator('#edit-panel').isHidden())await page.click('#world-edit');await page.click('#tab-'+tab);};
  const close=async()=>{if(await page.locator('#edit-close').isVisible())await page.click('#edit-close');};
  await page.fill('#planet-seed','42');await page.fill('#planet-day','2');await page.click('#planet-generate');
  await page.locator('#edit-auth').waitFor({state:'visible'});await page.click('#edit-cancel');
  assert.equal((await exported()).originals.length,0,'Cancel keeps the ungenerated base');
  await page.click('#planet-generate');await confirm();
  await page.waitForFunction(()=>document.querySelector('#planet-status').textContent.includes('ready')||document.querySelector('#voice-response').textContent.includes('Sign in and confirm'));
  assert.match(await page.locator('#planet-status').innerText(),/ready/);
  const first=await exported();assert.equal(Object.keys(first.entities).length,24);assert.equal(recipe(first).seed,42);
  await page.screenshot({path:path.join(out,'personal-day-desktop.png')});
  let releaseGrant, arrived;
  const held=new Promise(resolve=>arrived=resolve), released=new Promise(resolve=>releaseGrant=resolve);
  await context.route('https://openagents.com/v1/verse/studio/grants',async route=>{arrived();await released;await route.fallback();},{times:1});
  await page.fill('#planet-seed','99');await page.click('#planet-generate');
  try { await page.locator('#edit-confirm').waitFor({state:'visible'});await page.click('#edit-confirm');await held;
    await page.click('#edit-cancel');
  } finally {releaseGrant();}
  await page.waitForTimeout(700);
  assert.deepEqual(await exported(),first,'Cancel while permission response is held keeps the working world');
  await page.fill('#planet-seed','42');

  // Advance only the host's measurement clock to test the actual request handler;
  // the private renderer keeps its real advancing clock.
  const anchor=recipe(first).clock_anchor_unix_ms;
  await page.evaluate(now=>{window.fixtureDateNow=Date.now;Date.now=()=>now;},anchor+30000);
  await edit('request');await page.click('#edit-voice-start');
  const phrase=async text=>{
    await page.waitForFunction(()=>window.speechFixture.last&&!window.speechFixture.last.aborted);
    await page.evaluate(text=>{const r=window.speechFixture.last;r.onspeechstart?.({});r.onresult({results:[[{transcript:text}]]});},text);
  };
  await phrase('use bold fantasy');await page.locator('#edit-auth').waitFor({state:'visible'});await phrase('confirm change');await page.locator('#edit-auth').waitFor({state:'hidden'});
  let current=await exported();assert.equal(recipe(current).style,'bold-fantasy');assert.equal(recipe(current).clock_anchor_unix_ms,anchor,'Style-only handler preserves the clock anchor');
  await close();await page.screenshot({path:path.join(out,'personal-bold-desktop.png')});
  await phrase('undo');await page.locator('#edit-auth').waitFor({state:'visible'});await phrase('confirm change');await page.locator('#edit-auth').waitFor({state:'hidden'});
  assert.equal(recipe(await exported()).style,'soft-pastels');assert.equal(await page.locator('#planet-style').inputValue(),'soft-pastels');
  await page.click('#planet-apply');await page.locator('#edit-auth').waitFor({state:'visible'});
  const reviewedUndo=await page.evaluate(()=>JSON.parse(window.hauntDemo.pending_change()).pending.action.document);
  assert.equal(recipe(reviewedUndo).style,'soft-pastels','Review after Undo uses the accepted style');await page.click('#edit-cancel');

  await phrase('redo');await page.locator('#edit-auth').waitFor({state:'visible'});await phrase('confirm change');await page.locator('#edit-auth').waitFor({state:'hidden'});
  assert.equal(recipe(await exported()).style,'bold-fantasy');assert.equal(await page.locator('#planet-style').inputValue(),'bold-fantasy');

  await phrase('pause day cycle');await page.locator('#edit-auth').waitFor({state:'visible'});await phrase('confirm change');await page.locator('#edit-auth').waitFor({state:'hidden'});
  current=await exported();assert.ok(Math.abs(recipe(current).initial_hour-14)<.001,'Elapsed 8 to 14 pauses at 14');assert.equal(recipe(current).cycle_enabled,false);
  assert.deepEqual(current.entities,first.entities);
  const runtime=()=>editor.evaluate(()=>window.aethyrforgeEditor.snapshot().status);
  await editor.waitForFunction(()=>Math.abs((window.aethyrforgeEditor.snapshot().status.renderedEnvironment?.settings.sunAzimuth??0)-210)<.01);
  const pausedA=await runtime();await page.waitForTimeout(1300);const pausedB=await runtime();
  assert.ok(pausedB.renderedEnvironment.frame>pausedA.renderedEnvironment.frame);
  assert.deepEqual(pausedA.renderedEnvironment.settings,pausedB.renderedEnvironment.settings,'Paused lighting stays fixed across submitted frames');
  assert.equal(pausedA.revision,pausedB.revision);assert.deepEqual(pausedA.view.eye,pausedB.view.eye);

  await page.evaluate(()=>{Date.now=window.fixtureDateNow;});
  await phrase('stop');await close();
  await page.fill('#planet-seed','999');await page.click('#planet-apply');await confirm();
  assert.equal(recipe(await exported()).seed,42,'Review settings does not relabel the accepted layout seed');
  await page.fill('#planet-hour','22');await page.selectOption('#planet-style','soft-pastels');await page.click('#planet-apply');await page.locator('#edit-auth').waitFor({state:'visible'});assert.match(await page.locator('#planet-status').innerText(),/22:00/);await confirm();
  const night=await exported();assert.equal(recipe(night).initial_hour,22);assert.equal(recipe(night).cycle_enabled,false);assert.deepEqual(night.entities,first.entities);
  await page.screenshot({path:path.join(out,'personal-night-desktop.png')});
  // Resume through controls and observe real renderer time, without a wall-clock mock.
  await page.check('#planet-cycle');await page.click('#planet-apply');await confirm();
  const running=recipe(await exported());assert.equal(running.initial_hour,22);
  const runningA=await runtime();await page.waitForTimeout(1800);const runningB=await runtime();
  assert.equal(runningA.revision,runningB.revision);assert.deepEqual(runningA.view.eye,runningB.view.eye);
  assert.notEqual(runningA.renderedEnvironment.settings.sunAzimuth,runningB.renderedEnvironment.settings.sunAzimuth,'Submitted renderer lighting advances at a stable authored revision and pose');
  assert.ok(runningB.renderedEnvironment.frame>runningA.renderedEnvironment.frame);
  fs.writeFileSync(path.join(out,'personal-rendered-cycle.json'),JSON.stringify({paused:[pausedA,pausedB],running:[runningA,runningB]},null,2));
  const preview=page.frames().find(f=>f.url().startsWith(config.preview));assert.ok(preview);
  await page.bringToFront();await preview.locator('#world').click();
  await preview.waitForFunction(()=>document.pointerLockElement?.id==='world');
  const snapshot=()=>page.evaluate(()=>JSON.parse(window.hauntDemo.snapshot()));
  const pose=(await snapshot()).status.checkpoint;
  await page.keyboard.down('w');await page.waitForTimeout(900);await page.keyboard.up('w');
  await page.waitForFunction(before=>JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye.some((v,i)=>Math.abs(v-before[i])>.1),pose.eye);
  const walked=(await snapshot()).status.checkpoint;assert.equal(walked.mode,'walk');assert.ok(Math.abs(walked.eye[0])<31&&Math.abs(walked.eye[2])<31);
  await editor.evaluate(()=>window.aethyrforgeEditor.service.cameraPose([0,1.62,-30.5],[0,1.62,-31],undefined,'walk'));
  await page.waitForFunction(()=>JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye[2]<-30);
  await page.keyboard.down('w');await page.waitForTimeout(1200);await page.keyboard.up('w');
  const bounded=(await snapshot()).status.checkpoint;assert.ok(bounded.eye[2]>-31.5,'Visible perimeter wall stops walking');
  await editor.evaluate(p=>window.aethyrforgeEditor.service.cameraPose(p.eye,p.target,undefined,p.mode),walked);
  await page.waitForFunction(p=>JSON.parse(window.hauntDemo.snapshot()).status.checkpoint.eye.every((v,i)=>Math.abs(v-p[i])<.03),walked.eye);
  await page.keyboard.press('Escape');
  await edit('history');await page.click('#project-save-copy');await confirm();
  await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('Saved to project'));
  const kept=await exported();const savedPose=(await snapshot()).status.checkpoint;
  await close();await page.setViewportSize({width:390,height:844});
  const hit=async selector=>{await page.locator(selector).scrollIntoViewIfNeeded();const box=await page.locator(selector).boundingBox();assert.ok(box);assert.ok(await page.locator(selector).evaluate((e,b)=>{const hit=document.elementFromPoint(b.x+b.width/2,b.y+b.height/2);return hit===e||e.contains(hit);},box),selector+' is reachable');};
  for(const selector of ['#planet-settings > details > summary','#planet-hour','#planet-generate','#planet-apply','#edit-voice-start','#edit-voice-stop'])await hit(selector);
  await page.click('#edit-voice-start');await page.waitForFunction(()=>document.querySelector('#edit-voice-status').textContent.includes('Listening'));
  await hit('#edit-voice-stop');await page.click('#edit-voice-stop');assert.match(await page.locator('#edit-voice-status').innerText(),/off/);
  await page.locator('#planet-settings > details > summary').scrollIntoViewIfNeeded();await page.click('#planet-settings > details > summary');
  await hit('#edit-voice-start');await hit('#edit-voice-stop');
  await page.screenshot({path:path.join(out,'personal-mobile.png')});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.setViewportSize({width:1440,height:1000});await page.reload();await ready();editor=page.frames().find(f=>f.url().startsWith(config.editor));
  await edit('history');await page.click('#project-open');await confirm();
  const reopened=await exported();assert.deepEqual(reopened.entities,kept.entities);assert.deepEqual(recipe(reopened),recipe(kept));
  const restoredPose=(await snapshot()).status.checkpoint;
  assert.equal(restoredPose.mode,savedPose.mode);
  for(const key of ['eye','target'])assert.ok(restoredPose[key].every((v,i)=>Math.abs(v-savedPose[key][i])<.03),'Saved personal pose reopens');
  await page.click('#project-save-copy');await confirm();await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('Saved to project'));
  await edit('compile');await page.selectOption('#publish-visibility','public');await edit('request');await page.click('#request-options');await page.click('#project-publish');
  await page.locator('#publish-review[open]').waitFor();await page.check('#publish-confirm-rights');await page.click('#publish-confirm');await page.locator('#publish-review').waitFor({state:'hidden'});
  const playablePath=await page.locator('#published-link').getAttribute('href');assert.ok(playablePath);
  const guest=await context.browser().newContext({viewport:{width:1280,height:800}});
  try {
    await guest.grantPermissions(['local-network-access'],{origin:'https://openagents.com'});
    await guest.route('https://openagents.com/**',async route=>{const u=new URL(route.request().url());const response=await guest.request.get(base+u.pathname+u.search,{maxRedirects:0});const headers={...response.headers()};for(const k of ['content-length','content-encoding','transfer-encoding'])delete headers[k];await route.fulfill({status:response.status(),headers,body:response.status()>=300&&response.status()<400?Buffer.alloc(0):await response.body()});});
    const player=await guest.newPage();await player.goto('https://openagents.com'+playablePath);
    await player.waitForFunction(()=>window.versePlayer&&JSON.parse(window.versePlayer.snapshot()).engine.receipts.some(r=>r.outcome.message==='Playable version opened.'),null,{timeout:125000});
    const privatePlayer=player.frames().find(f=>f.url().startsWith(config.editor));assert.ok(privatePlayer);
    const manifest=await player.evaluate(()=>JSON.parse(document.querySelector('#playable-release').textContent).manifest);
    assert.deepEqual(recipe(manifest.content.document),recipe(kept));assert.deepEqual(manifest.content.document.entities,kept.entities);
    const played=await privatePlayer.evaluate(()=>window.aethyrforgeEditor.snapshot().status);
    assert.equal(played.authoring.active_source,kept.active_source);assert.ok(played.instances>0);assert.equal(played.view.mode,'walk');
    assert.deepEqual(played.personalWorld.entities,kept.entities,'Applied authored layout matches the saved world');
    assert.equal(played.personalWorld.recipeSha256,kept.originals.find(s=>s.name==='planet-v1.json').sha256);
    assert.ok(played.renderedEnvironment.frame>0);
    await player.screenshot({path:path.join(out,'personal-playable-guest.png')});
  } finally {await guest.unrouteAll({behavior:'wait'});await guest.close();}
  const progress=await (await context.request.get(base+'/v1/verse/studio/tutorial')).json();assert.equal(progress.progress.planet.project,planet.project);
  const finalPose=(await snapshot()).status.checkpoint;
  for(const key of ['eye','target'])assert.ok(finalPose[key].every((v,i)=>Math.abs(v-restoredPose[key][i])<.03));
  assert.deepEqual(errors,[]);
  const result={real_gpu_engine:true,real_native_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,synthetic_recognition:true,microphone_used:false,native_only_tutorial_precondition:true,generate_cancel_retry:true,cancel_during_delayed_permission_keeps_working_world:true,spoken_style_preserves_clock:true,elapsed_pause_freezes_current_hour:true,settings_preserve_layout_seed:true,night_and_day_captures:true,control_cycle_resume:true,submitted_renderer_cycle_advances_at_stable_revision_pose:true,paused_renderer_lighting_fixed:true,both_supported_styles_captured:true,walks_finite_courtyard:true,save_reopen_same_planet:true,perimeter_collision:true,undo_redo_recipe_controls_agree:true,normal_clock_review:true,mobile_mic_and_settings_hit_tests:true,personal_pinned_guest_player:true,private_captures:true,errors};
  fs.writeFileSync(path.join(out,'browser-personal.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}
