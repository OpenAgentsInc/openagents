// Personal-world handler checks. Recognition events and account identity are synthetic.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
export async function checkPersonalWorld({page,context,base,config,out,errors}) {
  assert.ok(config.personal_runtime);
  const session=await (await context.request.get(base+'/v1/verse/studio/session')).json();
  const call=async (route,body)=>{
    const response=await context.request.post(base+'/v1/verse/studio/'+route,{headers:{origin:base},data:{...body,csrf:session.csrf}});
    const value=await response.json();assert.equal(response.status(),200,JSON.stringify(value));return value;
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
  await page.waitForFunction(()=>document.querySelector('#planet-status').textContent.includes('ready'));
  const first=await exported();assert.equal(Object.keys(first.entities).length,24);assert.equal(recipe(first).seed,42);
  await page.screenshot({path:path.join(out,'personal-day-desktop.png')});
  // Advance only the host's measurement clock to test the actual request handler;
  // the private renderer keeps its real advancing clock.
  const anchor=recipe(first).clock_anchor_unix_ms;
  await page.evaluate(now=>{window.fixtureDateNow=Date.now;Date.now=()=>now;},anchor+30000);
  await edit('request');await page.click('#edit-voice-start');
  const phrase=async text=>{
    await page.waitForFunction(()=>window.speechFixture.last&&!window.speechFixture.last.aborted);
    await page.evaluate(text=>window.speechFixture.last.onresult({results:[[{transcript:text}]]}),text);
  };
  await phrase('use bold fantasy');await page.locator('#edit-auth').waitFor({state:'visible'});await phrase('confirm change');await page.locator('#edit-auth').waitFor({state:'hidden'});
  let current=await exported();assert.equal(recipe(current).style,'bold-fantasy');assert.equal(recipe(current).clock_anchor_unix_ms,anchor,'Style-only handler preserves the clock anchor');
  await close();await page.screenshot({path:path.join(out,'personal-bold-desktop.png')});
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
  await page.fill('#planet-hour','22');await page.selectOption('#planet-style','soft-pastels');await page.click('#planet-apply');await confirm();
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
  await page.keyboard.press('Escape');
  await edit('history');await page.click('#project-save-copy');await confirm();
  await page.waitForFunction(()=>document.querySelector('#project-status').textContent.includes('Saved to project'));
  const kept=await exported();const savedPose=(await snapshot()).status.checkpoint;
  await close();await page.setViewportSize({width:390,height:844});await page.screenshot({path:path.join(out,'personal-mobile.png')});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.setViewportSize({width:1440,height:1000});await page.reload();await ready();editor=page.frames().find(f=>f.url().startsWith(config.editor));
  await edit('history');await page.click('#project-open');await confirm();
  const reopened=await exported();assert.deepEqual(reopened.entities,kept.entities);assert.deepEqual(recipe(reopened),recipe(kept));
  const restoredPose=(await snapshot()).status.checkpoint;
  assert.equal(restoredPose.mode,savedPose.mode);
  for(const key of ['eye','target'])assert.ok(restoredPose[key].every((v,i)=>Math.abs(v-savedPose[key][i])<.03),'Saved personal pose reopens');
  assert.deepEqual(errors,[]);
  const result={real_gpu_engine:true,real_native_store:true,synthetic_account_service:true,real_signin_provider:false,real_speech_provider:false,synthetic_recognition:true,microphone_used:false,native_only_tutorial_precondition:true,generate_cancel_retry:true,spoken_style_preserves_clock:true,elapsed_pause_freezes_current_hour:true,settings_preserve_layout_seed:true,night_and_day_captures:true,control_cycle_resume:true,submitted_renderer_cycle_advances_at_stable_revision_pose:true,paused_renderer_lighting_fixed:true,both_supported_styles_captured:true,walks_finite_courtyard:true,save_reopen_same_planet:true,private_captures:true,errors};
  fs.writeFileSync(path.join(out,'browser-personal.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}
