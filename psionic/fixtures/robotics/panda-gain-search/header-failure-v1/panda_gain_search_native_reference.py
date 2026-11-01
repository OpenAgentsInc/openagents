#!/usr/bin/env python3
"""One-shot OWNED gain grid; cached scoring is a separate SDK/plant-free mode.

The four published baseline trajectories are carried by exact byte identity.
Only32 other rollouts may execute. All gain, score and winner policies are Coax.
"""
from pathlib import Path
import argparse,json,math,os,platform,struct
from panda_joint_pd_prefix_reference import Run,identity,sha
from panda_pose_ik_native_reference import graph
ROOT=Path(__file__).resolve().parents[3];F=ROOT/'psionic/fixtures/robotics/panda-gain-search';OLD=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking';SOURCE=ROOT/'psionic/tests/robotics/panda-pd-gain-search.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_gain_search_native.c';FORMAT=ROOT/'psionic/tests/robotics/panda_joint_pd_tracking_format.h';BASELINE=Path('/tmp/panda-joint-pd-tracking-native-first/native-debug/own-trace.bin');COMPILER=Path('/tmp/coax-spatial-fixture-fixes-reconstruction/coax');EXP='67c872e87b64041c9a0fde7014f828a345a0d48148d496fe087575d49fe38b31';COMPACT=struct.Struct('<18d8q');INDEX=struct.Struct('<6q');ROW=struct.Struct('<120d10q')
def require(ok,message):
 if not ok:raise RuntimeError(message)
def finite(values,n,label):require(len(values)==n and all(math.isfinite(v)for v in values),label+' complete/allfinite');return values
def verify(source):require(all(identity(ROOT/p)==v for p,v in source.items()),'frozen exactsources')
def frozen(baseline=None):
 require(sha(F/'experiment.json')==EXP,'frozenALL4terminal-goalstudy');e=json.loads((F/'experiment.json').read_text());b=json.loads((F/'baseline-carry.json').read_text());require(identity(F/'baseline-carry.json')==e['baseline_carry'],'baselinecarrycontextboundbyfrozenexperiment');verify(e['source']);verify(e['old_reference_data']);verify(b['source']);verify(b['original_data']);require(e['baseline_profile']==b['profile']==1 and e['new_rollouts']==32 and e['new_plant_steps']==24000 and e['baseline_new_plant_steps']==0 and e['forbidSDKloads'],'oneownedgrid/nobaselineexecution');require(all(identity(F/p)==v for p,v in e['input_files'].items()),'exactprofiles+heldoutfreeze');require((F/'profiles.f64').read_bytes()==b''.join(struct.pack('<64d',*c)for p in e['profiles']for c in p['case_inputs_f64']),'exactprofilegainbits');qualification=json.loads((OLD/'native-qualification.json').read_text());require(identity(OLD/'native-qualification.json')==b['previous_native_qualification']and qualification['passed']and qualification['output_sha256']==b['previous_trace']['sha256']and qualification['modes']['debug']['output']==b['previous_trace']and qualification['emissions']==b['old_emissions'],'originalqualifiedbaselinecompiler/numericcontext');verify(qualification['source'])
 if baseline is not None:require(identity(baseline)==b['previous_trace'],'EXACToldtrace,noresimulationfallback');raw=baseline.read_bytes();require(len(raw)==16+3000*1216 and struct.unpack_from('<4I',raw)==(0x314f5450,4,750,1216),'baselineoldfresh+shadowextent')
 return e,b

def flags():return ['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
def prepare(args,e,b):
 out=args.output.resolve();require(not out.exists(),'preservepreviousprepare');out.mkdir(parents=True);compiler=args.compiler.resolve();require(identity(compiler)==b['old_compiler'],'qualifiedimmutablea429');source={str(p.relative_to(ROOT)):identity(p)for p in set(graph(SOURCE))|{NATIVE,FORMAT,Path(__file__).resolve(),F/'experiment.json',F/'baseline-carry.json',F/'profiles.f64',F/'heldout-unscaled.f64',ROOT/'psionic/tests/robotics/panda_joint_pd_prefix_reference.py',ROOT/'psionic/tests/robotics/panda_pose_ik_native_reference.py'}|{ROOT/x for x in e['source']}|{ROOT/x for x in e['old_reference_data']}};r=dict(schema='psionic.panda-gain-search.preparation.v1',passed=False,prepared=False,commands=[],source=source,compiler=identity(compiler),experiment=identity(F/'experiment.json'),baseline=identity(args.baseline),SDK_calls=0,plant_steps=0,baseline_new_plant_steps=0,compiler_input_fingerprint=b['old_compiler_input_fingerprint'],numerical_execution_profile=b['old_numeric_profile']);run=Run(out,r);run.save()
 try:
  r['toolchain']=run.command('host-toolchain',[os.environ.get('CC','cc'),'--version']).decode();run.save()
  for mode,extra in[('debug',[]),('release',['--release'])]:
   d=out/mode;d.mkdir();c=d/'panda-pd-gain-search.c';h=d/'panda-pd-gain-search.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));r.setdefault('emissions',{})[mode]=dict(source=identity(c),header=identity(h))
  for mode,cmode,numeric in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   d=out/mode;d.mkdir(exist_ok=True);c=out/cmode/'panda-pd-gain-search.c';h=out/cmode/'panda-pd-gain-search.h';run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*flags(),'-Wno-unused-parameter',*numeric,'-c',c,'-o',d/'coax.o']);run.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*flags(),*numeric,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',d/'native']);env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');guard=json.loads(run.command('NEW-score-ABI-guards-'+mode,[d/'native','guards'],env));require(guard['passed']and guard['SDK_calls']==guard['plant_steps']==0 and guard['baseline_step_refused'],'newscoreronlyguardphase');r.setdefault('guards',{})[mode]=guard;r.setdefault('binaries',{})[mode]=identity(d/'native');run.save()
  report=json.loads(run.command('gain-metadata-once',[out/'release/native','gains',out/'gains.f64']));expected=b''.join(struct.pack('<14d',*c[39:53])for p in e['profiles']for c in p['case_inputs_f64']);require(report['passed']and report['plant_steps']==0 and(out/'gains.f64').read_bytes()==expected,'all36exactscaledgainvectors');verify(source);r.update(passed=True,prepared=True,gains=identity(out/'gains.f64'))
 except BaseException as error:r['failure']=repr(error);raise
 finally:run.save()
 print(json.dumps(dict(prepared=True,SDK_calls=0,plant_steps=0,record=str(out/'record.json'))))

def extract(out,baseline,e,b):
 rawbase=baseline.read_bytes();cache=bytearray(struct.pack('<6I',0x31434750,9,4,750,208,1));index=bytearray();entries=[];offset=0
 for p in range(9):
  for c in range(4):
   if p==1:
    records=[rawbase[16+(c*750+k)*1216:16+(c*750+k)*1216+1040]for k in range(750)];count=750;stage=code=0;raw_identity=b['previous_trace'];origin='carriedoldfresh-prefix-only'
   else:
    path=out/'raw'/('profile-%d-case-%d.bin'%(p,c));raw=path.read_bytes();require(len(raw)>=40,'retainedrawheader');profile,case,count,stage,code=struct.unpack_from('<5q',raw);require((profile,case)==(p,c)and 0<=count<=750 and len(raw)==40+count*1040 and ((count==750 and stage==code==0)or(count<750 and(stage!=0 or code!=0))),'exactrawpartial-or-completetrajectory');records=[raw[40+k*1040:40+(k+1)*1040]for k in range(count)];raw_identity=identity(path);origin='newownedtrajectory'
   previous=struct.pack('<18d',*e['profiles'][p]['case_inputs_f64'][c][:18])
   for k,rowraw in enumerate(records):
    row=ROW.unpack(rowraw);finite(row[:120],120,'new/carriedfreshfields');m=row[120:];require(m[:4]==(c,k,0,0)and rowraw[:144]==previous,'profile/case/timefreshstatecontinuity');post=rowraw[808:952];require(len(post)==144,'poststateexactfield-slice');cache+=post+struct.pack('<8q',p,c,k,m[2],m[3],m[4],m[8],m[9]);previous=post
   index+=INDEX.pack(p,c,offset,count,stage,code);entry=dict(profile=p,case_id=c,offset_records=offset,records=count,stage=stage,code=code,raw=raw_identity,origin=origin);entries.append(entry);offset+=count
 (out/'compact-cache.bin').write_bytes(cache);(out/'compact-index.bin').write_bytes(index);lineage=dict(schema='psionic.panda-gain-search.compact-lineage.v1',passed=True,experiment=identity(F/'experiment.json'),baseline_trace=b['previous_trace'],baseline_qualification=b['previous_native_qualification'],raw_trajectories=[x for x in entries if x['profile']!=1],all_trajectories=entries,compact=identity(out/'compact-cache.bin'),index=identity(out/'compact-index.bin'),records=offset,baseline_new_plant_steps=0,baseline_fresh_only=True,extraction='BytecopyPntfresh1040B poststate[808:952];intmetadata copied/insertsprofilefromheader. Allprestate bytes equalinitial/previouspost. Baselineshadow neverinterpreted; no physics/scorerecomputed.');(out/'lineage.json').write_text(json.dumps(lineage,indent=2)+'\n');return lineage

def prepared_unchanged(prepared,record):
 verify(record['source']);require(all(identity(prepared/m/'native')==v for m,v in record['binaries'].items()),'allpreparedbinariesunchanged')
 for mode,values in record['emissions'].items():require(identity(prepared/mode/'panda-pd-gain-search.c')==values['source']and identity(prepared/mode/'panda-pd-gain-search.h')==values['header'],'reusedC/Hidentitiesunchanged')

def progress_identity(path,strict=False):
 raw=path.read_bytes()if path.exists()else b'';events=[];partial_lines=0
 for line in raw.splitlines():
  try:events.append(json.loads(line))
  except(json.JSONDecodeError,UnicodeDecodeError):partial_lines+=1
 if strict:
  expected=[(p,c)for p in range(9)if p!=1 for c in range(4)];require(len(events)==64 and partial_lines==0,'complete64eventgridledger')
  for i,(p,c)in enumerate(expected):require(events[2*i]==dict(event='before_rollout',profile=p,case=c)and events[2*i+1]['event']=='after_rollout'and(events[2*i+1]['profile'],events[2*i+1]['case'])==(p,c),'exactprofile/caseledgerorder')
 return dict(file=identity(path)if path.exists()else None,parsed_events=len(events),before_rollout=sum(x.get('event')=='before_rollout'for x in events),after_rollout=sum(x.get('event')=='after_rollout'for x in events),partial_or_malformed_lines=partial_lines)

def collect(args,e,b):
 prepared=args.prepared.resolve();pr=json.loads((prepared/'record.json').read_text());require(pr['prepared']and pr['passed']and pr['baseline']==identity(args.baseline),'exactadmittedpreparation');prepared_unchanged(prepared,pr);out=args.output.resolve();require(not out.exists(),'ONEgridonly,preserveearlierattempt');out.mkdir(parents=True);(out/'raw').mkdir();r=dict(schema='psionic.panda-gain-search.owned-grid.v1',passed=False,collection_passed=False,SDK_calls=0,commands=[],source=pr['source'],experiment=identity(F/'experiment.json'),baseline_trace=identity(args.baseline),baseline_new_plant_steps=0,preparation=identity(prepared/'record.json'),canonical_profile='release/O2,scalar-libm,FPCR0,nofastmath/contraction');run=Run(out,r);run.save()
 try:
  marker=prepared/'ONE_GRID_STARTED.json';require(not marker.exists(),'NEVERrepeatstartedcanonicalgrid');marker.write_text(json.dumps(dict(experiment=identity(F/'experiment.json'),preparation=identity(prepared/'record.json'),output=str(out),intent='32newrolloutsONCE;baseline1NEVER'))+'\n');r['one_shot_marker']=identity(marker)
  source_input=OLD/'input.bin';r['initial_input']=identity(source_input);report=json.loads(run.command('ONE-NEW-grid',[prepared/'release/native','run',source_input,out/'raw',out/'progress.jsonl']));require(report['passed']and report['SDK_calls']==0 and report['new_trajectories']==32 and report['baseline_plant_steps']==0 and report['new_plant_steps']<=24000 and report['attempted_steps']>=report['new_plant_steps'],'exactboundednewgrid');r['native']=report;r['progress']=progress_identity(out/'progress.jsonl',True);lineage=extract(out,args.baseline,e,b);require(sum(x['records']for x in lineage['raw_trajectories'])==report['new_plant_steps'],'rawcommittedstepcount');r.update(collection_passed=True,lineage=identity(out/'lineage.json'),compact=identity(out/'compact-cache.bin'),index=identity(out/'compact-index.bin'),attempted_steps=report['attempted_steps'],committed_steps=report['new_plant_steps']);run.save()
  for mode in['debug','release','sanitized']:
   d=out/mode;d.mkdir();env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');score=json.loads(run.command('CACHED-score-'+mode,[prepared/mode/'native','score',out/'compact-cache.bin',out/'compact-index.bin',d/'case-scores.bin',d/'candidates.bin',d/'selection.bin'],env));require(score['passed']and score['SDK_calls']==score['plant_steps']==0,'scoringneversteps');r.setdefault('scores',{})[mode]=dict(native=score,outputs={n:identity(d/n)for n in['case-scores.bin','candidates.bin','selection.bin']});run.save()
  require(r['scores']['debug']['outputs']==r['scores']['release']['outputs']==r['scores']['sanitized']['outputs'],'allnewscorermodebytesexact');prepared_unchanged(prepared,pr);require(identity(args.baseline)==b['previous_trace'],'baselineunchanged');r.update(passed=True,scorer_modes_byte_identical=True,physics_grid_repeated=False)
 except BaseException as error:r['failure']=repr(error);raise
 finally:
  partial={}
  for path in(out/'raw').glob('*.bin'):
   raw=path.read_bytes();n=max(0,len(raw)-40);entry=dict(file=identity(path),has_full_header=len(raw)>=40,complete_payload_rows=n//1040,partial_payload_bytes=n%1040)
   if len(raw)>=40:entry['observed_header_5i64']=list(struct.unpack_from('<5q',raw))
   partial[path.name]=entry
  r['partial_or_complete_raw_files']=partial;r['final_progress_observation']=progress_identity(out/'progress.jsonl');r['observed_complete_payload_rows']=sum(x['complete_payload_rows']for x in partial.values());r['attempted_steps_unknown_on_abnormal_exit']=not r.get('collection_passed',False);run.save()
 print(json.dumps(dict(passed=True,record=str(out/'record.json'),new_steps=r['committed_steps'],baseline_steps=0,selection=struct.unpack('<4q2d2q',(out/'debug/selection.bin').read_bytes()))))

def main():
 p=argparse.ArgumentParser(description=__doc__);m=p.add_mutually_exclusive_group(required=True);m.add_argument('--prepare',action='store_true');m.add_argument('--collect-once',action='store_true');p.add_argument('--output',type=Path,required=True);p.add_argument('--prepared',type=Path);p.add_argument('--compiler',type=Path,default=COMPILER);p.add_argument('--baseline',type=Path,default=BASELINE);a=p.parse_args();require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualifiedDarwinarm64');e,b=frozen(a.baseline);(prepare if a.prepare else collect)(a,e,b)
if __name__=='__main__':main()
