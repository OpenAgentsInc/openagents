#!/usr/bin/env python3
"""C/D effect lifecycle: prepare0steps, ONE SDK capture, ONE OWN set, cached modes.

Explicit SDK capture needs Root's separate GO. The cached default adapter is a
separate entry and never runs either physical collection or loads the SDK.
"""
from pathlib import Path
import argparse,json,math,os,platform,struct
from panda_joint_pd_prefix_reference import Run,identity,sha,assets,validate_step_model
from panda_pose_ik_native_reference import graph
ROOT=Path(__file__).resolve().parents[3];T=ROOT/'psionic/tests/robotics';F=ROOT/'psionic/fixtures/robotics/panda-scheduled-pd-validation';OLD=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking';SOURCE=T/'panda-scheduled-pd-validation.coax';NATIVE=T/'panda_scheduled_pd_validation_native.c';COLLECTOR=T/'panda_scheduled_pd_validation_sdk_collect.c';FORMAT=T/'panda_scheduled_pd_validation_format.h';COMPILER=Path('/tmp/coax-spatial-fixture-fixes-reconstruction/coax');EXP='32b5d8ae1cc6e07dba317677102681a831d5b94a19c5e7df78167b57f017f640';CSHA='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818';REF='5d881d8540f2e46f7e2a21f451339d33fee2258acfa8239c3c8b4e46ce2f1442';ROW=struct.Struct('<120d10q3dq');INDEX=struct.Struct('<6q');MAGIC={'sdk':0x31534443,'own':0x314f4443}
def require(ok,message):
 if not ok:raise RuntimeError(message)
def finite(values,n,label):require(len(values)==n and all(math.isfinite(x)for x in values),label+' complete/allfinite');return values
def verify(source):require(all(identity(ROOT/p)==v for p,v in source.items()),'all frozen source/input identities unchanged')
def frozen():
 require(sha(F/'experiment.json')==EXP,'C/D frozen before results');e=json.loads((F/'experiment.json').read_text());verify(e['metadata_inputs']);require(identity(F/'factory-cases.f64')==e['factory_input']and identity(F/'input.bin')==e['SDK_initial_input'],'exact8 context fixtures');require(e['contexts']==8 and e['ticks']==750 and e['budget']['SDK_steps_max']==e['budget']['OWN_steps_max']==6000 and e['budget']['shadow_trajectories']==0,'hard once-only step budget');require(sha(F/'reference-qualification.json')==REF,'original once-only new reference receipt');r=json.loads((F/'reference-qualification.json').read_text());require(r['passed']and r['experiment']==identity(F/'experiment.json')and all(identity(F/n)==v for n,v in r['outputs'].items()),'all new reference outputs');verify(r['source']);prior=json.loads((OLD/'sdk-reference.json').read_text());require(prior['passed']and identity(OLD/'sdk-reference.json')==e['SDK_admission']['prior_qualification']and identity(OLD/'sdk-model.json')==e['SDK_admission']['prior_model_metadata'],'exact prior SDK support/model proof');verify(prior['source']);validate_step_model(json.loads((OLD/'sdk-model.json').read_text()));q=json.loads((F/'source-qualification.json').read_text());require(q['passed']and q['actual_emitted_mutants']==263 and q['survived']==0,'new source proof carried without repeat');verify(q['source_files']);return e

def flags():return ['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
def source_map(e):
 paths=set(graph(SOURCE))|{NATIVE,COLLECTOR,FORMAT,T/'panda_joint_pd_tracking_format.h',T/'panda_actuation_sdk_collect.c',T/'panda_joint_pd_prefix_reference.py',T/'panda_euler_sdk_reference.py',T/'panda_actuation_sdk_reference.py',T/'panda_actuation_reference.py',T/'panda_pose_ik_native_reference.py',T/'panda_scheduled_pd_validation_reference.py',Path(__file__).resolve(),F/'experiment.json',F/'factory-cases.f64',F/'input.bin',F/'reference-qualification.json',F/'source-qualification.json',F/'unit-refusals.json'}|{ROOT/p for p in e['metadata_inputs']}
 ref=json.loads((F/'reference-qualification.json').read_text());paths|={F/n for n in ref['outputs']};return{str(p.relative_to(ROOT)):identity(p)for p in sorted(paths)}
def effect_context(a,e):
 scene=a.scene.resolve();sdk=a.sdk.resolve();library=sdk/'libmujoco.3.3.1.dylib';s=e['SDK_admission'];headers={str(p.relative_to(sdk)):identity(p)for p in sorted((sdk/'include/mujoco').glob('*.h'))};decl,unique=assets(scene);require(identity(scene)==s['source_XML']and identity(library)==s['SDK_library']and headers==s['SDK_headers']and unique==s['assets']and decl==s['asset_declarations'],'exact SDK/XML/headers/85assets/86declarations');return scene,sdk,dict(source_XML=identity(scene),SDK_library=identity(library),SDK_headers=headers,assets=unique,asset_declarations=decl,qualified_model=identity(OLD/'sdk-model.json'))
def decode(raw,index,e,kind):
 require(len(raw)>=24 and struct.unpack_from('<6I',raw)==(MAGIC[kind],2,4,750,1072,1)and len(raw)==24+6000*1072 and len(index)==384,'complete pose-aware trace/index extents');rows=[ROW.unpack_from(raw,24+i*1072)for i in range(6000)];finite([x for r in rows for x in r[:120]],720000,'ALL fresh observations before anymax');finite([x for r in rows for x in r[130:133]],18000,'ALL schedule fields before anymax')
 for c in range(8):
  require(INDEX.unpack_from(index,c*48)==(c//4,c%4,c*750,750,0,0),'all8 canonical complete context index');case=e['cases'][c];previous=struct.pack('<18d',*case['initial_q9'],*case['initial_v9'])
  for k in range(750):
   r=rows[c*750+k];require(r[120:124]==(c%4,k,0,0)and struct.pack('<18d',*r[:18])==previous,'context/scenario/tick/status/prestate lineage');require(0<=r[124]<=7 and r[125]==min(k,r[126])and 0<=r[126]<=1000000 and r[127]in[0,1]and 0<=r[128]<=9 and 0<=r[129]<=2 and r[133]in[0,1,2],'all metadata domains');require(abs(r[119]-(k+1)/500.)<=e['numeric']['clock_atol_seconds'],'clock phase');require(r[131]>=0 and r[131]<=40 and 4<=r[132]<=20,'schedule domains');pulse=c%4==2 and 200<=k<250;external=case['pulse9']if pulse else[0.]*9;require(list(r[83:92])==external and r[127]==int(pulse)and list(r[81:83])==case['finger_targets2'],'signed pulse and perpose fingers');limits=[80.]*5+[12.]*2;require(all(v==min(L,max(-L,p))for p,v,L in zip(r[60:67],r[67:74],limits))and r[124]==sum(p< -L or p>L for p,L in zip(r[60:67],limits)),'Pjp clipping exact');previous=struct.pack('<18d',*r[101:119])
 return rows

def progress(path,kind,strict=False):
 events=[];bad=0
 if path.exists():
  for line in path.read_bytes().splitlines():
   try:events.append(json.loads(line))
   except(ValueError,UnicodeError):bad+=1
 if strict:
  triples=[]
  if kind=='sdk':triples=[(-1,-1,'before_model_load'),(-1,-1,'model_admitted')]
  for c in range(8):
   triples +=[(c,-1,'before_initial_forward'),(c,-1,'after_initial_forward')]if kind=='sdk'else[(c,-1,'initial_admitted')]
   for k in range(750):triples +=[(c,k,'before_step'),(c,k,'after_step'),(c,k,'record_committed')]if kind=='sdk'else[(c,k,'before_step'),(c,k,'record_committed')]
  require(not bad and len(events)==len(triples)and all((r['context'],r['tick'],r['event'])==want and r['pose']==(-1 if want[0]<0 else want[0]//4)and r['scenario']==(-1 if want[0]<0 else want[0]%4)and r['stage']==r['code']==0 for r,want in zip(events,triples)),'exact one-shot lifecycle ledger')
 return dict(events=len(events),malformed_lines=bad,model_loads_started=sum(r['event']=='before_model_load'for r in events),initial_forwards_started=sum(r['event']=='before_initial_forward'for r in events),initial_forwards_completed=sum(r['event']=='after_initial_forward'for r in events),steps_started=sum(r['event']=='before_step'for r in events),SDK_steps_completed=sum(r['event']=='after_step'for r in events),records_committed=sum(r['event']=='record_committed'for r in events))
def fixture_compare(actual,e):
 require((actual/'factory-cases.f64').read_bytes()==(F/'factory-cases.f64').read_bytes()and(actual/'input.bin').read_bytes()==(F/'input.bin').read_bytes(),'all8 authored fixture/reset bits exact');raw=(actual/'references.bin').read_bytes();want=(F/'reference-samples.bin').read_bytes();require(len(raw)==len(want)==6008*184,'all6008 reference records');av=[struct.unpack_from('<21d2q',raw,i*184)for i in range(6008)];wv=[struct.unpack_from('<21d2q',want,i*184)for i in range(6008)];finite([x for row in av for x in row[:21]],126168,'ALL native targetfields');finite([x for row in wv for x in row[:21]],126168,'ALL new analytic targetfields');m=[0.,0.]
 for a,b in zip(av,wv):
  require(a[21:]==b[21:],'all reference tick/N exact')
  for j in range(21):
   d=abs(a[j]-b[j]);cap=e['numeric']['reference_angle_atol']if j<7 else e['numeric']['reference_derivative_atol']+e['numeric']['reference_derivative_rtol']*abs(b[j]);require(d<=cap,'new reference per-scalar budget');m[j>=7]=max(m[j>=7],d)
 return dict(passed=True,contexts=8,frames=6008,angle_max_abs_error=m[0],derivative_max_abs_error=m[1],all_metadata_exact=True)

def prepare(a,e):
 out=a.output.resolve();require(not out.exists(),'preserve every prepare attempt');out.mkdir(parents=True);compiler=a.compiler.resolve();require(sha(compiler)==CSHA,'qualified immutable a429');scene,sdk,context=effect_context(a,e);r=dict(schema='psionic.panda-scheduled-pd-validation.preparation.v1',passed=False,prepared=False,commands=[],source=source_map(e),compiler_path=str(compiler),compiler=identity(compiler),compiler_input_fingerprint='580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036',scene_path=str(scene),sdk_path=str(sdk),effect_context=context,experiment=identity(F/'experiment.json'),metadata_admission_carried_from=identity(OLD/'sdk-reference.json'),SDK_loads=0,SDK_steps=0,owned_steps=0);run=Run(out,r);run.save()
 try:
  r['toolchain']=run.command('toolchain',[os.environ.get('CC','cc'),'--version']).decode()
  for mode,extra in[('debug',[]),('release',['--release'])]:
   d=out/mode;d.mkdir();c=d/'panda-scheduled-pd-validation.c';h=d/'panda-scheduled-pd-validation.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));alias=d/'panda-joint-pd-tracking.h';alias.write_bytes(h.read_bytes());r.setdefault('emissions',{})[mode]=dict(source=identity(c),header=identity(h),alias=identity(alias));run.save()
  for mode,cmode,opts in[('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']),('release','release',['-O2']),('debug','debug',['-O0','-g'])]:
   d=out/mode;d.mkdir(exist_ok=True);inc=out/cmode;run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*flags(),'-Wno-unused-parameter',*opts,'-c',inc/'panda-scheduled-pd-validation.c','-o',d/'coax.o']);run.command('compile-native-'+mode,[os.environ.get('CC','cc'),*flags(),*opts,'-I'+str(inc),NATIVE,d/'coax.o','-lm','-o',d/'native']);r.setdefault('binaries',{})[mode]=identity(d/'native');run.save()
   if mode=='sanitized':
    run.command('compile-SDK-collector',[os.environ.get('CC','cc'),*flags(),*opts,'-I'+str(inc),'-I'+str(sdk/'include'),COLLECTOR,d/'coax.o',sdk/'libmujoco.3.3.1.dylib','-Wl,-rpath,'+str(sdk),'-lm','-o',out/'collect']);r['collector_binary']=identity(out/'collect');env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');guard=json.loads(run.command('NEW-zero-plant-ABI-guards-once',[d/'native','guards'],env));require(guard['passed']and guard['new_plant_steps']==guard['SDK_calls']==0,'new interfaces only; no extra step');r['new_ABI_guards']=guard;report=json.loads(run.command('NEW-fixture-reference-export-once',[d/'native','fixtures',out/'factory-cases.f64',out/'input.bin',out/'references.bin'],env));require(report['passed']and report['plant_steps']==report['SDK_calls']==0,'fixture/reference phase zero plant');r['fixture_reference_comparison']=fixture_compare(out,e);r['fixture_reference_native']=report;r['input']=identity(out/'input.bin');run.save()
  verify(r['source']);require(identity(compiler)==r['compiler'],'compiler unchanged');_,_,after=effect_context(a,e);require(after==context,'SDK/XML/assets stayed unchanged without loading SDK');r.update(passed=True,prepared=True)
 except BaseException as error:r['failure']=repr(error);raise
 finally:run.save()
 print(json.dumps(dict(prepared=True,SDK_loads=0,plant_steps=0,record=str(out/'record.json'),guards=r.get('new_ABI_guards'))))

def checked_prepared(a,e):
 d=a.prepared.resolve();r=json.loads((d/'record.json').read_text());require(r['passed']and r['prepared']and r['experiment']==identity(F/'experiment.json'),'qualified preparation');verify(r['source']);require(identity(Path(r['compiler_path']))==r['compiler']and identity(d/'collect')==r['collector_binary']and identity(d/'input.bin')==r['input'],'compiler/collector/input unchanged');require(all(identity(d/m/'native')==v for m,v in r['binaries'].items()),'all native binaries unchanged')
 for mode,value in r['emissions'].items():require(identity(d/mode/'panda-scheduled-pd-validation.c')==value['source']and identity(d/mode/'panda-scheduled-pd-validation.h')==value['header']and identity(d/mode/'panda-joint-pd-tracking.h')==value['header'],'both C/H and private aliases unchanged')
 return d,r

def capture(a,e):
 d,pr=checked_prepared(a,e);scene,sdk,context=effect_context(a,e);require(context==pr['effect_context'],'actual capture context exact');out=a.output.resolve();require(not out.exists(),'one SDK capture; preserve earlier effects');out.mkdir(parents=True);r=dict(schema='psionic.panda-scheduled-pd-validation.sdk-reference.v1',passed=False,capture_started=False,commands=[],source=pr['source'],compiler=pr['compiler'],preparation=identity(d/'record.json'),experiment=identity(F/'experiment.json'),effect_context=context,SDK_runtime_uninstrumented=True,collector_and_Coax_sanitized=True,NN_calls=0,render_calls=0,shadow_steps=0,post_step_forwards=0);env=dict(os.environ,DYLD_LIBRARY_PATH=str(sdk),ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');run=Run(out,r,env);run.save()
 try:
  marker=d/'ONE_SDK_CAPTURE_STARTED.json';require(not marker.exists(),'no SDK retry');marker.write_text(json.dumps(dict(experiment=identity(F/'experiment.json'),output=str(out)))+'\n');r['capture_started']=True;r['one_shot_marker']=identity(marker);run.save();report=json.loads(run.command('ONE-SDK-6000-step-capture',[d/'collect',scene,d/'input.bin',out/'sdk-trace.bin',out/'sdk-index.bin',out/'sdk-model.json',OLD/'sdk-model.json',out/'progress.jsonl']));require(report['passed']and report['contexts']==8 and report['steps']==6000 and report['explicit_forwards']==8 and report['SDK_model_loads']==1 and report['post_step_forwards']==report['shadow_steps']==report['NN_calls']==report['render_calls']==0 and report['sparse_moment_rows']==report['generalized_force_equalities']==54000,'exact new SDK scope');require(identity(out/'sdk-model.json')==e['SDK_admission']['prior_model_metadata'],'actual model admitted before firstforward');validate_step_model(json.loads((out/'sdk-model.json').read_text()));rows=decode((out/'sdk-trace.bin').read_bytes(),(out/'sdk-index.bin').read_bytes(),e,'sdk');r.update(progress(out/'progress.jsonl','sdk',True));checked_prepared(a,e);_,_,after=effect_context(a,e);require(after==context,'effect context unchanged after capture');r.update(passed=True,native=report,trace=identity(out/'sdk-trace.bin'),index=identity(out/'sdk-index.bin'),model=identity(out/'sdk-model.json'),progress_identity=identity(out/'progress.jsonl'),SDK_steps=6000,all_actual_finite_words=len(rows)*123)
 except BaseException as error:r['failure']=repr(error);raise
 finally:
  r['partial_progress']=progress(out/'progress.jsonl','sdk');path=out/'sdk-trace.bin'
  if path.exists():n=path.stat().st_size;r['partial_or_complete_trace']=identity(path);r['complete_payload_rows']=max(0,n-24)//1072;r['partial_payload_bytes']=max(0,n-24)%1072
  if(out/'sdk-index.bin').exists():r['partial_or_complete_index']=identity(out/'sdk-index.bin')
  run.save()
 print(json.dumps(dict(passed=True,SDK_steps=6000,trace=r['trace'],record=str(out/'record.json'))))

def owned(a,e):
 d,pr=checked_prepared(a,e);capture_dir=a.capture.resolve();sr=json.loads((capture_dir/'record.json').read_text());require(sr['passed']and sr['SDK_steps']==6000 and sr['preparation']==identity(d/'record.json')and identity(capture_dir/'sdk-trace.bin')==sr['trace']and identity(capture_dir/'sdk-index.bin')==sr['index'],'completed single SDK capture before OWN');out=a.output.resolve();require(not out.exists(),'one owned collection; preserve earlier attempt');out.mkdir(parents=True);r=dict(schema='psionic.panda-scheduled-pd-validation.native-reference.v1',passed=False,commands=[],source=pr['source'],compiler=pr['compiler'],preparation=identity(d/'record.json'),capture_record=identity(capture_dir/'record.json'),capture_trace=sr['trace'],capture_index=sr['index'],experiment=identity(F/'experiment.json'),SDK_calls=0,shadow_steps=0,old_original_case_steps=0);run=Run(out,r);run.save()
 try:
  marker=d/'ONE_OWN_COLLECTION_STARTED.json';require(not marker.exists(),'no OWN retry');marker.write_text(json.dumps(dict(experiment=identity(F/'experiment.json'),capture=identity(capture_dir/'record.json'),output=str(out)))+'\n');r['one_shot_marker']=identity(marker);report=json.loads(run.command('ONE-OWN-6000-step-set',[d/'release/native','own',d/'input.bin',out/'own-trace.bin',out/'own-index.bin',out/'progress.jsonl']));require(report['passed']and report['contexts']==8 and report['owned_steps']==6000 and report['shadow_steps']==report['SDK_calls']==0,'exact new OWN budget');rows=decode((out/'own-trace.bin').read_bytes(),(out/'own-index.bin').read_bytes(),e,'own');r.update(progress(out/'progress.jsonl','own',True));r.update(native=report,trace=identity(out/'own-trace.bin'),index=identity(out/'own-index.bin'),progress_identity=identity(out/'progress.jsonl'),owned_steps=6000,all_actual_finite_words=len(rows)*123,physical_collection_completed=True);run.save()
  for mode in['debug','release','sanitized']:
   md=out/mode;md.mkdir();env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');reports={};outputs={};reports['comparison']=json.loads(run.command('CACHED-pair-compare-'+mode,[d/mode/'native','compare',capture_dir/'sdk-trace.bin',capture_dir/'sdk-index.bin',out/'own-trace.bin',out/'own-index.bin',md/'parity.bin'],env));outputs['parity.bin']=identity(md/'parity.bin')
   for kind,parent in[('sdk',capture_dir),('own',out)]:
    reports[kind]=json.loads(run.command('CACHED-'+kind+'-score-'+mode,[d/mode/'native','score',parent/(kind+'-trace.bin'),parent/(kind+'-index.bin'),md/(kind+'-case-scores.bin'),md/(kind+'-pose-scores.bin'),md/(kind+'-verdict.bin')],env));require(reports[kind]['passed']and reports[kind]['plant_steps']==reports[kind]['SDK_calls']==0,'cached reducer never steps');outputs.update({kind+'-'+n:identity(md/(kind+'-'+n))for n in['case-scores.bin','pose-scores.bin','verdict.bin']})
   r.setdefault('modes',{})[mode]=dict(native=reports,outputs=outputs);run.save()
  require(r['modes']['debug']['outputs']==r['modes']['release']['outputs']==r['modes']['sanitized']['outputs'],'all cached comparison/reduction bytes identical');checked_prepared(a,e);require(identity(capture_dir/'record.json')==r['capture_record']and identity(capture_dir/'sdk-trace.bin')==r['capture_trace']and identity(capture_dir/'sdk-index.bin')==r['capture_index'],'single original capture unchanged');r.update(passed=True,cached_modes_byte_identical=True,physics_repeated=False)
 except BaseException as error:r['failure']=repr(error);raise
 finally:
  r['partial_progress']=progress(out/'progress.jsonl','own');path=out/'own-trace.bin'
  if path.exists():n=path.stat().st_size;r['partial_or_complete_trace']=identity(path);r['complete_payload_rows']=max(0,n-24)//1072;r['partial_payload_bytes']=max(0,n-24)%1072
  if(out/'own-index.bin').exists():r['partial_or_complete_index']=identity(out/'own-index.bin')
  run.save()
 print(json.dumps(dict(passed=True,owned_steps=6000,SDK_calls=0,verdicts={kind:struct.unpack('<2q',(out/'debug'/(kind+'-verdict.bin')).read_bytes())for kind in['sdk','own']},record=str(out/'record.json'))))

def main():
 p=argparse.ArgumentParser(description=__doc__);m=p.add_mutually_exclusive_group(required=True);m.add_argument('--prepare',action='store_true');m.add_argument('--capture-prepared',action='store_true');m.add_argument('--own-prepared',action='store_true');p.add_argument('--output',type=Path,required=True);p.add_argument('--prepared',type=Path);p.add_argument('--capture',type=Path);p.add_argument('--compiler',type=Path,default=COMPILER);p.add_argument('--scene',type=Path,default=Path.home()/'.cache/psionic/libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000/scene.xml');p.add_argument('--sdk',type=Path,default=Path.home()/'.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco');a=p.parse_args();require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualified Darwin-arm64');e=frozen();(prepare if a.prepare else capture if a.capture_prepared else owned)(a,e)
if __name__=='__main__':main()
