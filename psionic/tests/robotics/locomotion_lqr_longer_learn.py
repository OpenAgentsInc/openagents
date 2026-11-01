#!/usr/bin/env python3
"""Explicit versioned continuation from authenticated old128 checkpoints, using the ALREADY qualified native binary.

No compilation, fixture rerun, external simulator, SDK or original-weight model
is performed here. The owned scalar plant executes only in the explicit run.
Preparation requires fresh output and performs file identity work only. Each
one-shot fixed-action advance permits READY continuation and never recovers a
failed or running action; a reviewed frozen independent auditor is mandatory.
"""
from __future__ import annotations
import argparse,ast,copy,hashlib,importlib.util,json,os,platform,struct,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/locomotion-lqr-longer'
OLD=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
SOURCE=Path(__file__).resolve()
AUDITOR=ROOT/'psionic/tests/robotics/locomotion_lqr_longer_audit.py'
MATH_DRIVER=ROOT/'psionic/tests/robotics/locomotion_lqr_native_reference.py'
EXPERIMENT='9dc911e27fe377e4fa5f219f9ac577ca6c01a183c380ba40d200844d65e3ff87'
HEAD=struct.Struct('<4I');STATE=440;ITERATION=62696
MAGIC={'snapshot':0x3153584c,'iteration':0x3149584c,'coefficient':0x3143584c,'eval':0x3145584c,'quality':0x3147584c}
def need(c,t):
 if not c:raise ValueError(t)
def identity(p):
 b=Path(p).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def load(p):return json.loads(Path(p).read_text())
def write(p,x):Path(p).write_text(json.dumps(x,indent=2,allow_nan=False)+'\n')
def check_source(mapping):
 for name,h in mapping.items():need(identity(ROOT/name)==h,'immutable qualified source '+name)
def state(raw,seed,batch):
 need(seed in (0,1,7) and 128<=batch<=512,'canonical seed/extension batch domain');need(len(raw)==440,'field-wise state extent');K,m,v,t,skip=struct.unpack_from('<3d2q',raw);need(all(__import__('math').isfinite(x) for x in [K,m,v]) and .25<=K<=1 and v>=0,'state finite/domain');need((t,skip)==(4*batch,0),'state optimizer lineage');need(struct.unpack_from('<qQ',raw,424)==(batch,seed),'state batch/runseed')
 for w in range(16):
  x,s,c=struct.unpack_from('<d2Q',raw,40+w*24);need(__import__('math').isfinite(x) and abs(x)<=16 and (s,c)==(seed,131072+w*24576+(batch-128)*64),'world state/counter lineage')
 return K

def qualification(qdir):
 q=load(qdir/'record.json');need(q['passed'] is True and not q['plan_only'] and q['all_mode_output_bytes_exact'] and q['full_batches_plant_optimizer_RNG_tape']==0,'new guard-only qualification')
 need(q['experiment']==identity(F/'experiment.json') and q['experiment']['sha256']==EXPERIMENT,'sealed longer profile');check_source(q['source'])
 for path,h in q['frozen'].items():need(identity(path)==h,'frozen extension prerequisite')
 compiler=Path(next(x for x in q['commands'] if x['label']=='emit-release')['argv'][0]);need(identity(compiler)==q['compiler'],'qualified compiler image')
 mode=q['modes']['release'];binary=qdir/'native-release/native';need(identity(binary)==mode['binary'],'same qualified extension release binary')
 for name,leaf in [('C','locomotion-lqr-longer.c'),('header','locomotion-lqr-longer.h')]:need(identity(qdir/'release'/leaf)==mode[name],'qualified generated input')
 need(identity(qdir/'release/locomotion-lqr.h')==mode['header'],'exact private header alias')
 original=load(OLD/'native-qualification.json');check_source(original['source'])
 for name,h in load(OLD/'experiment.json')['carried'].items():need(identity(ROOT/name)==h,'carried old numeric helper')
 return q,binary,compiler

def auditor_binding(auditor):
 paths=[F/n for n in ['audit-freeze.json','audit-guard-qualification.json','numeric-carry.json','input-freeze.json']];freeze,guard,carry,_=map(load,paths)
 need(freeze['passed'] is True and freeze['auditor']==identity(auditor)==guard['producer'] and freeze['new_guard_qualification']==identity(paths[1]),'frozen new auditor and new-only guards')
 need(carry['passed'] is True and carry['new_auditor']==identity(auditor) and carry['old_auditor']==identity(ROOT/'psionic/tests/robotics/locomotion_lqr_learning_audit.py'),'old/new auditor carry')
 need(carry['numeric_chunk_suffix_byte_exact'] and carry['numeric_evaluation_suffix_byte_exact'],'numeric kernel carry')
 return paths

def actions():
 return [*({'kind':'import','seed':s} for s in [0,1,7]),*({'kind':'canonical','seed':s,'first_batch':b,'batches':16} for s in [0,1,7] for b in range(128,512,16)),*({'kind':'replay','seed':s,'first_batch':256,'batches':1} for s in [0,1,7]),*({'kind':'evaluation','label':f'seed-{s}','seed':s} for s in [0,1,7])]

def byte_identity(raw):return {'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}

def admit_original(e,original_context,seed,raw,audit_raw):
 need(seed in (0,1,7),'canonical original seed');item=e['resume_sources'][str(seed)]
 need(byte_identity(raw)==item['snapshot'] and byte_identity(audit_raw)==item['audit'],'exact original learned endpoint/audit')
 need(len(raw)==488 and HEAD.unpack_from(raw)==(0x3153514c,1,440,0) and raw[16:48]==original_context,'original wire context')
 need(original_context==bytes.fromhex(e['original_descriptor']['sha256']),'original sealed context')
 need(struct.unpack_from('<qQ',raw,48+424)==(128,seed) and struct.unpack_from('<2q',raw,48+24)==(512,0),'original endpoint labels')
 original=json.loads(audit_raw);need(original['schema']=='psionic.locomotion-lqr.learning-audit.v1' and original['passed'] is True,'original PASSED audit')
 need(original['auditor']==e['original_auditor'] and original['inputs']['descriptor']==e['original_descriptor'],'original auditor/context')
 need(original['inputs']['after']==item['snapshot'] and original['result']['seed']==seed and original['result']['first_batch']==112 and original['result']['batches']==16,'original independently audited endpoint')

def verify_originals(e,original_context):
 need(original_context==bytes.fromhex(identity(OLD/'learning-execution/descriptor.json')['sha256']),'original context bytes')
 for seed in [0,1,7]:
  item=e['resume_sources'][str(seed)];admit_original(e,original_context,seed,(ROOT/item['snapshot_path']).read_bytes(),(ROOT/item['audit_path']).read_bytes())

def descriptor_policy(d,e):
 need(d['schema']=='psionic.locomotion-lqr-longer.learning-descriptor.v1','versioned continuation descriptor')
 need(d['actions']==actions()==e['actions'] and len(d['actions'])==81,'exact sealed81 actions')
 need(d['training']==e['training'] and d['resume_sources']==e['resume_sources'],'same training/resume policy')
 need(d['native_binary']==d['release_binary'],'single qualified binary identity')

def ready_inventory(record,d):
 need(record['completed_actions']==d['actions'][:len(record['completed_actions'])],'monotonic completed prefix')
 for kind,key in [('import','imports'),('canonical','chunks'),('replay','replays'),('evaluation','evaluations')]:
  expected=[a for a in record['completed_actions'] if a['kind']==kind];items=record.get(key,[])
  need(len(items)==len(expected),'complete action-to-artifact inventory '+kind)
  for item,action in zip(items,expected):
   need(item['seed']==action['seed'],'completed action seed '+kind)
   if kind in ('canonical','replay'):need((item['first_batch'],item['batches'])==(action['first_batch'],action['batches']),'completed action batch '+kind)
   if kind=='evaluation':need(item['label']==action['label'],'completed evaluation label')

def admit_endpoint(item,audit,seed,batch,descriptor_id,auditor_id,checkpoint_id):
 first=128 if batch==128 else batch-16;count=0 if batch==128 else 16
 directory=f'import/seed-{seed}' if batch==128 else f'canonical/seed-{seed}/batch-{first:03d}'
 need(item['seed']==seed and item['directory']==directory and item['audit']['directory']==directory+'/audit','exact previous action directory/seed')
 if batch!=128:need((item['first_batch'],item['batches'])==(first,16),'exact previous completed chunk')
 need(item['artifacts']['checkpoint.bin']==checkpoint_id,'previous captured checkpoint identity')
 need(audit['schema']=='psionic.locomotion-lqr-longer.learning-audit.v1' and audit['passed'] is True and audit['auditor']==auditor_id,'previous PASSED same-source audit')
 need(audit['inputs']['descriptor']==descriptor_id and audit['inputs']['after']==checkpoint_id,'previous context/independently audited endpoint')
 r=audit['result'];need((r['seed'],r['first_batch'],r['batches'])==(seed,first,count),'previous contiguous audit labels')
 if batch==128:need(r['import_only'] is True,'first chunk requires actual counter-only import')

def authenticated_endpoint(record,out,d,seed,batch,context):
 items=record['imports'] if batch==128 else record['chunks']
 selected=[x for x in items if x['seed']==seed and (batch==128 or x['first_batch']==batch-16)]
 need(len(selected)==1,'unique authenticated predecessor');item=selected[0]
 path=out/item['directory']/'checkpoint.bin';ap=out/item['audit']['directory']/'record.json';need(identity(ap)==item['audit']['record'],'previous audit bytes immutable')
 cp_id=identity(path);admit_endpoint(item,load(ap),seed,batch,identity(out/'descriptor.json'),d['live_auditor'],cp_id)
 state(parse_file(path,MAGIC['snapshot'],440,0,context,1),seed,batch)
 return path,ap

def admit_replay_target(item,audit,seed,before_id,descriptor_id,auditor_id):
 directory=f'canonical/seed-{seed}/batch-256'
 need((item['seed'],item['first_batch'],item['batches'],item['directory'])==(seed,256,16,directory),'exact canonical replay target')
 need(audit['schema']=='psionic.locomotion-lqr-longer.learning-audit.v1' and audit['passed'] is True and audit['auditor']==auditor_id,'replay PASSED same-source target audit')
 need(audit['inputs']['descriptor']==descriptor_id and audit['inputs']['before']==before_id,'replay target authenticated after256 checkpoint')
 need((audit['result']['seed'],audit['result']['first_batch'],audit['result']['batches'])==(seed,256,16),'replay target labels')
 for name,key in [('iterations.bin','iterations'),('coefficients.bin','coefficients')]:need(item['artifacts'][name]==audit['inputs'][key],'replay target artifacts independently audited')

def authenticated_replay(record,out,d,seed,context):
 initial,_=authenticated_endpoint(record,out,d,seed,256,context)
 items=[x for x in record['chunks'] if x['seed']==seed and x['first_batch']==256];need(len(items)==1,'unique replay target');item=items[0]
 ap=out/item['audit']['directory']/'record.json';need(identity(ap)==item['audit']['record'],'replay target audit immutable')
 admit_replay_target(item,load(ap),seed,identity(initial),identity(out/'descriptor.json'),d['live_auditor'])
 return initial,out/item['directory']

def lifecycle_guard_files():
 paths=[F/'lifecycle-guard-qualification.json',F/'lifecycle-selection-carry.json'];guard,carry=map(load,paths)
 need(guard['passed'] is True and guard['producer']==identity(SOURCE) and guard['product_calls']==0,'new lifecycle guard qualification')
 need(carry['passed'] is True and carry['new_driver']==identity(SOURCE) and carry['select_action_byte_exact'],'old selector carry')
 return paths

def prepare(qdir,out,auditor):
 need(not out.exists(),'fresh longer preparation');q,binary,compiler=qualification(qdir);paths=auditor_binding(auditor)+lifecycle_guard_files();e=load(F/'experiment.json');old_context=bytes.fromhex(identity(OLD/'learning-execution/descriptor.json')['sha256']);verify_originals(e,old_context);out.mkdir(parents=True)
 descriptor={'schema':'psionic.locomotion-lqr-longer.learning-descriptor.v1','frozen_before_import_learning':True,'experiment':identity(F/'experiment.json'),'contract':e['contract'],'original_experiment':identity(OLD/'experiment.json'),'original_learning_record':identity(OLD/'learning-execution/record.json'),'original_descriptor':identity(OLD/'learning-execution/descriptor.json'),'math_qualification':identity(OLD/'native-qualification.json'),'independent_reference':identity(OLD/'reference.json'),'extension_qualification':identity(qdir/'record.json'),'qualified_directory':str(qdir),'native_binary':identity(binary),'release_binary':identity(binary),'compiler':identity(compiler),'live_auditor':identity(auditor),'live_auditor_path':str(auditor),'source':{str(x.relative_to(ROOT)):identity(x) for x in [SOURCE,auditor,*paths]},'actions':actions(),'training':e['training'],'resume_sources':e['resume_sources'],'wire':{'header':'<4I+32B descriptorSHA','snapshot':[MAGIC['snapshot'],1,440,0],'iteration':[MAGIC['iteration'],1,62696,16],'coefficient':[MAGIC['coefficient'],1,88,32768],'evaluation':[MAGIC['eval'],1,72,6],'quality':[MAGIC['quality'],1,200,1]},'planned_new_effects':{'imports':3,'canonical_plant_steps':589824,'canonical_adam_updates':4608,'canonical_tape_pairs':36864,'canonical_sample_gradients':2359296,'canonical_recomputed_coefficients':2359296,'replay_plant_steps':1536,'replay_adam_updates':12,'replay_tape_pairs':96,'replay_sample_gradients':6144,'deterministic_evaluation_steps':2304,'old_prefix_and_baseline_reruns':0},'failure_policy':'ONE fixed action then independent audit; READY only on pass; stop/preserve RUNNING or FAILED, never retry or retune.','persistence':'File fflush/fsync iteration and coefficients before checkpoint temp fsync+rename; no directory-fsync/crash-durability claim.','effect_executions':0}
 descriptor_policy(descriptor,e);write(out/'descriptor.json',descriptor);context=bytes.fromhex(identity(out/'descriptor.json')['sha256']);(out/'context.bin').write_bytes(context);(out/'original-context.bin').write_bytes(old_context)
 r={'schema':'psionic.locomotion-lqr-longer.preparation.v1','passed':True,'descriptor':identity(out/'descriptor.json'),'context':identity(out/'context.bin'),'original_context':identity(out/'original-context.bin'),'source':descriptor['source'],'effect_calls':0,'commands':[]};write(out/'prepared.json',r);print(json.dumps({'passed':True,'prepared':str(out/'prepared.json'),'descriptor':identity(out/'descriptor.json')}))

def parse_file(path,magic,width,cap,context,count):
 raw=Path(path).read_bytes();need(len(raw)==48+width*count,'exact complete wire extent '+str(path));need(HEAD.unpack_from(raw)==(magic,1,width,cap) and raw[16:48]==context,'wire/context header '+str(path));return raw[48:]
def extent(path,width):
 if not path.exists():return {'exists':False}
 n=path.stat().st_size;return {'exists':True,'bytes':n,'complete_records':max(0,n-48)//width,'trailing_bytes':max(0,n-48)%width,'header_complete':n>=48,'identity':identity(path)}

def select_action(record,descriptor):
 need(record['lifecycle_state']=='ready' and not record['passed'],'only READY work may advance')
 index=len(record['completed_actions']);need(record['completed_actions']==descriptor['actions'][:index] and index<len(descriptor['actions']),'fixed next action without replay');return index,descriptor['actions'][index]

def run(out,metadata_only=False):
 prepared=load(out/'prepared.json');d=load(out/'descriptor.json');need(prepared['passed'] and identity(out/'descriptor.json')==prepared['descriptor'] and identity(out/'context.bin')==prepared['context'],'immutable reviewed preparation');check_source(prepared['source']);descriptor_policy(d,load(F/'experiment.json'));qdir=Path(d['qualified_directory']);q,binary,compiler=qualification(qdir);need(identity(qdir/'record.json')==d['extension_qualification'] and identity(binary)==d['release_binary'],'prepared release context');context=(out/'context.bin').read_bytes();need(identity(out/'original-context.bin')==prepared['original_context'],'original context immutable');verify_originals(load(F/'experiment.json'),(out/'original-context.bin').read_bytes());need(platform.system()=='Darwin' and platform.machine()=='arm64' and sys.byteorder=='little','named scalar host')
 if metadata_only:
  record=load(out/'record.json') if (out/'record.json').exists() else {'lifecycle_state':'ready','passed':False,'completed_actions':[]}
  ready_inventory(record,d);index,action=select_action(record,d);print(json.dumps({'passed':True,'metadata_only':True,'next_index':index,'next_action':action,'product_calls':0}));return
 lock=out/'.advance.lock';lockfd=os.open(lock,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.close(lockfd)
 marker=Path('/tmp/locomotion-lqr-longer-learning-'+EXPERIMENT+'.one-shot.json')
 if not(out/'record.json').exists():
  fd=os.open(marker,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.write(fd,(json.dumps({'output':str(out),'descriptor':prepared['descriptor']})+'\n').encode());os.close(fd)
  r={'schema':'psionic.locomotion-lqr-longer.learning.v1','passed':False,'lifecycle_state':'ready','descriptor':prepared['descriptor'],'prepared':identity(out/'prepared.json'),'qualified_math':identity(OLD/'native-qualification.json'),'extension_qualification':identity(qdir/'record.json'),'binary':identity(binary),'source':prepared['source'],'imports':[],'chunks':[],'replays':[],'evaluations':[],'completed_actions':[],'commands':[],'old_math_compile_model_SDK_calls':0,'seconds_total':0.}
 else:
  r=load(out/'record.json');need(r['lifecycle_state']=='ready' and not r['passed'],'only explicitly READY work may advance; never recover failed/running actions');need(load(marker)=={'output':str(out),'descriptor':prepared['descriptor']},'same one-shot experiment')
 need(r['descriptor']==prepared['descriptor'] and r['prepared']==identity(out/'prepared.json'),'unchanged monotonic lifecycle context')
 for group in ['imports','chunks','replays','evaluations']:
  for item in r[group]:
   for name,h in item['artifacts'].items():need(identity(out/item['directory']/name)==h,'completed artifact immutable')
   need(identity(out/item['audit']['directory']/'record.json')==item['audit']['record'],'completed audit immutable')
 ready_inventory(r,d);index,action=select_action(r,d);r['lifecycle_state']='running';r['active_action']=action;write(out/'record.json',r);began=time.monotonic()
 def command(label,argv,dir):
  item={'label':label,'argv':list(map(str,argv)),'directory':str(dir.relative_to(out))};r['commands'].append(item);t=time.monotonic();write(out/'record.json',r)
  try:
   remaining=840-(time.monotonic()-began);need(remaining>0,'840-second whole-run cap');z=subprocess.run(item['argv'],cwd=ROOT,capture_output=True,timeout=min(180,remaining));(dir/'stdout').write_bytes(z.stdout);(dir/'stderr').write_bytes(z.stderr);item.update(exit=z.returncode,seconds=time.monotonic()-t);need(z.returncode==0 and not z.stderr,label+' successful/empty stderr');report=json.loads(z.stdout);need(report['passed'] is True,label+' native report');return report
  except subprocess.TimeoutExpired as x:
   (dir/'stdout').write_bytes(x.stdout or b'');(dir/'stderr').write_bytes(x.stderr or b'');item.update(timed_out=True,seconds=time.monotonic()-t);raise
  finally:
   item['partial_artifacts']={n:extent(dir/n,w) for n,w in [('iterations.bin',62696),('coefficients.bin',88)]};write(out/'record.json',r)
 def chunk(seed,first,n,initial,dir,phase):
  qualification(qdir);check_source(prepared['source']);previous=parse_file(initial,MAGIC['snapshot'],440,0,context,1);state(previous,seed,first);dir.mkdir(parents=True);report=command(f'{phase}-{seed}-{first}',[binary,'chunk',initial,dir,n,out/'context.bin'],dir);need(report['committed_batches']==n and report['last_batch']==first+n and report['status']==0,'exact completed chunk metadata')
  packets=parse_file(dir/'iterations.bin',MAGIC['iteration'],62696,16,context,n);parse_file(dir/'coefficients.bin',MAGIC['coefficient'],88,32768,context,n*2048);cp=parse_file(dir/'checkpoint.bin',MAGIC['snapshot'],440,0,context,1);state(cp,seed,first+n);need(cp==packets[-440:],'packet/checkpoint exact final state')
  events=[json.loads(x) for x in (dir/'progress.jsonl').read_text().splitlines()];need(events==[x for b in range(first,first+n) for x in [{'event':'before','batch':b},{'event':'committed','batch':b+1}]],'exact complete progress ledger')
  for i in range(n):
   p=packets[i*62696:(i+1)*62696];state(p[-440:],seed,first+i+1);need(struct.unpack_from('<3q',p)==(16,32,512),'fixed source batch shape');need(struct.unpack_from('<d',p,24)[0]==struct.unpack_from('<d',previous)[0],'rollout-start K lineage');previous=p[-440:]
  obj={'seed':seed,'first_batch':first,'batches':n,'directory':str(dir.relative_to(out)),'native':report,'artifacts':{name:identity(dir/name) for name in ['iterations.bin','coefficients.bin','checkpoint.bin','progress.jsonl','stdout','stderr']}};r['chunks' if phase=='canonical' else 'replays'].append(obj);write(out/'record.json',r);return dir/'checkpoint.bin'
 def audit(dir,argv):
  logs=dir/'audit-command';logs.mkdir();target=dir/'audit';report=command('audit-'+str(index),[sys.executable,d['live_auditor_path'],'--descriptor',out/'descriptor.json','--math-record',OLD/'native-qualification.json','--extension-record',qdir/'record.json',*argv,'--output',target],logs);need(load(target/'record.json')['passed'] is True,'independent new action audit');return {'directory':str(target.relative_to(out)),'record':identity(target/'record.json')}
 try:
  if action['kind']=='import':
   seed=action['seed'];item=d['resume_sources'][str(seed)];original=ROOT/item['snapshot_path'];need(identity(original)==item['snapshot'],'exact original before import');dir=out/f'import/seed-{seed}';dir.mkdir(parents=True);report=command(f'import-{seed}',[binary,'import',original,dir,out/'original-context.bin',out/'context.bin'],dir);need(report['status']==0 and report['batch']==128 and report['seed']==seed and report['full_batches']==0,'exact import metadata');raw=parse_file(dir/'checkpoint.bin',MAGIC['snapshot'],440,0,context,1);state(raw,seed,128);need({'bytes':440,'sha256':hashlib.sha256(raw).hexdigest()}==item['remapped_raw'],'exact sealed counter-only result')
   obj={'seed':seed,'directory':str(dir.relative_to(out)),'native':report,'artifacts':{n:identity(dir/n) for n in ['checkpoint.bin','stdout','stderr']}};r['imports'].append(obj);obj['audit']=audit(dir,['--import-state','--before',original,'--after',dir/'checkpoint.bin','--seed',seed])
  elif action['kind']=='canonical':
   seed,first=action['seed'],action['first_batch'];initial,prior=authenticated_endpoint(r,out,d,seed,first,context);dir=out/f'canonical/seed-{seed}/batch-{first:03d}';after=chunk(seed,first,16,initial,dir,'canonical');r['chunks'][-1]['audit']=audit(dir,['--before',initial,'--after',after,'--iterations',dir/'iterations.bin','--coefficients',dir/'coefficients.bin','--progress',dir/'progress.jsonl','--seed',seed,'--first-batch',first,'--count',16,'--previous-audit',prior])
  elif action['kind']=='replay':
   seed=action['seed'];initial,original=authenticated_replay(r,out,d,seed,context);dir=out/f'replay/seed-{seed}';after=chunk(seed,256,1,initial,dir,'replay')
   for name,width,rows in [('iterations.bin',62696,1),('coefficients.bin',88,2048)]:need((dir/name).read_bytes()[48:]==(original/name).read_bytes()[48:48+width*rows],'exact declared next batch257 '+name)
   r['replays'][-1]['exact_original_batch257_bytes']=True;r['replays'][-1]['audit']=audit(dir,['--before',initial,'--after',after,'--iterations',dir/'iterations.bin','--coefficients',dir/'coefficients.bin','--progress',dir/'progress.jsonl','--seed',seed,'--first-batch',256,'--count',1,'--canonical-audit',original/'audit/record.json','--canonical-iterations',original/'iterations.bin','--canonical-coefficients',original/'coefficients.bin'])
  else:
   label,seed=action['label'],action['seed'];snapshot_path,prior=authenticated_endpoint(r,out,d,seed,512,context);dir=out/'evaluation'/label;dir.mkdir(parents=True);report=command('evaluation-'+label,[binary,'eval',snapshot_path,dir,out/'context.bin'],dir);need(report['steps']==768 and report['evaluations']==6,'named final-only deterministic eval count');parse_file(dir/'evaluations.bin',MAGIC['eval'],72,6,context,6);parse_file(dir/'quality.bin',MAGIC['quality'],200,1,context,1)
   obj={'label':label,'seed':seed,'directory':str(dir.relative_to(out)),'native':report,'artifacts':{n:identity(dir/n) for n in ['evaluations.bin','quality.bin','stdout','stderr']}};r['evaluations'].append(obj);obj['audit']=audit(dir,['--after',snapshot_path,'--evaluations',dir/'evaluations.bin','--quality',dir/'quality.bin','--seed',seed,'--previous-audit',prior])
  qualification(qdir);check_source(prepared['source']);need(identity(compiler)==d['compiler'] and identity(binary)==d['release_binary'],'final qualified binary/compiler');r['completed_actions'].append(action);r.pop('active_action',None);complete=len(r['completed_actions'])==len(d['actions']);r['lifecycle_state']='complete' if complete else 'ready';r['passed']=complete
  if complete:r.update(completed_seeds=[0,1,7],new_canonical_batches=1152,canonical_steps=589824,canonical_adam_updates=4608,combined_historical_canonical_steps=786432,combined_historical_adam_updates=6144,replay_steps=1536,replay_adam_updates=12,deterministic_eval_steps=2304,old_prefix_and_baseline_reruns=0,goal_status='All per-action math audits passed; retain each final goal result without seed selection')
 except BaseException as x:r['lifecycle_state']='failed';r['error']=type(x).__name__+': '+str(x);raise
 finally:r['last_advance_seconds']=time.monotonic()-began;r['seconds_total']+=r['last_advance_seconds'];write(out/'record.json',r);lock.unlink()
 print(json.dumps({'action_passed':True,'complete':r['passed'],'completed_actions':len(r['completed_actions']),'record':str(out/'record.json'),'identity':identity(out/'record.json')}))

def lifecycle_guards(output):
 need(not output.exists(),'preserve FIRST lifecycle-guard context');output.mkdir(parents=True)
 positives=0;poisons=0
 def yes(fn):
  nonlocal positives
  fn();positives+=1
 def no(fn):
  nonlocal poisons
  try:fn()
  except (ValueError,KeyError,IndexError,struct.error):poisons+=1;return
  raise ValueError('new metadata poison unexpectedly admitted')
 e=load(F/'experiment.json');oldctx=bytes.fromhex(e['original_descriptor']['sha256'])
 for seed in (0,1,7):
  item=e['resume_sources'][str(seed)];raw=(ROOT/item['snapshot_path']).read_bytes();ar=(ROOT/item['audit_path']).read_bytes()
  yes(lambda:admit_original(e,oldctx,seed,raw,ar))
  no(lambda:admit_original(e,bytes(32),seed,raw,ar))
  no(lambda:admit_original(e,oldctx,seed,raw[:-1],ar))
  no(lambda:admit_original(e,oldctx,seed,raw,ar+b' '))
  for offset,fmt,value in [(0,'<I',MAGIC['snapshot']),(48+24,'<q',511),(48+32,'<q',1),(48+424,'<q',127),(48+432,'<Q',13)]:
   bad=bytearray(raw);struct.pack_into(fmt,bad,offset,value);ee=copy.deepcopy(e);ee['resume_sources'][str(seed)]['snapshot']=byte_identity(bad)
   no(lambda b=bytes(bad),ee=ee:admit_original(ee,oldctx,seed,b,ar))
  for path,value in [(('schema',),'wrong'),(('passed',),False),(('auditor',),{'bytes':1,'sha256':'00'*32}),(('inputs','descriptor'),{'bytes':1,'sha256':'00'*32}),(('result','first_batch'),111),(('result','batches'),15)]:
   bad=json.loads(ar);target=bad
   for key in path[:-1]:target=target[key]
   target[path[-1]]=value;br=json.dumps(bad).encode();ee=copy.deepcopy(e);ee['resume_sources'][str(seed)]['audit']=byte_identity(br)
   no(lambda br=br,ee=ee:admit_original(ee,oldctx,seed,raw,br))
  remapped=bytearray(raw[48:])
  for batch in (128,129,256,511,512):
   struct.pack_into('<q',remapped,24,4*batch);struct.pack_into('<q',remapped,424,batch)
   for w in range(16):struct.pack_into('<Q',remapped,56+24*w,131072+w*24576+(batch-128)*64)
   yes(lambda r=bytes(remapped),b=batch:state(r,seed,b))
  no(lambda:state(bytes(remapped),13,512));no(lambda:state(bytes(remapped),seed,513))
  for offset,fmt,value in [(24,'<q',2047),(32,'<q',1),(424,'<q',511),(432,'<Q',13),(56+24*15,'<Q',524289),(40+24*15,'<d',16.001),(0,'<d',float('nan')),(16,'<d',-1)]:
   bad=bytearray(remapped);struct.pack_into(fmt,bad,offset,value);no(lambda b=bytes(bad):state(b,seed,512))
 for index in (0,3,4,75,78,80):
  d={'actions':actions()};prefix=d['actions'][:index];r={'lifecycle_state':'ready','passed':False,'completed_actions':copy.deepcopy(prefix),'imports':[],'chunks':[],'replays':[],'evaluations':[]}
  for a in prefix:
   key={'import':'imports','canonical':'chunks','replay':'replays','evaluation':'evaluations'}[a['kind']];r[key].append({k:v for k,v in a.items() if k!='kind'})
  yes(lambda:ready_inventory(r,d));yes(lambda:need(select_action(r,d)==(index,d['actions'][index]),'new81 next action'))
  bad=copy.deepcopy(r);bad['lifecycle_state']='running';no(lambda:select_action(bad,d))
  if index:
   bad=copy.deepcopy(r);bad['imports'][0]['seed']=13;no(lambda:ready_inventory(bad,d));bad=copy.deepcopy(r);bad['completed_actions'][0]['seed']=13;no(lambda:ready_inventory(bad,d))
 desc={'bytes':100,'sha256':'ab'*32};auditor=identity(AUDITOR);cp={'bytes':488,'sha256':'cd'*32}
 def endpoint(seed,batch):
  first=128 if batch==128 else batch-16;count=0 if batch==128 else 16;di=f'import/seed-{seed}' if batch==128 else f'canonical/seed-{seed}/batch-{first:03d}'
  item={'seed':seed,'directory':di,'audit':{'directory':di+'/audit'},'artifacts':{'checkpoint.bin':cp}}
  if count:item.update(first_batch=first,batches=count)
  a={'schema':'psionic.locomotion-lqr-longer.learning-audit.v1','passed':True,'auditor':auditor,'inputs':{'descriptor':desc,'after':cp},'result':{'seed':seed,'first_batch':first,'batches':count,'import_only':not count}}
  return item,a
 for batch in (128,144,256,512):
  item,a=endpoint(0,batch);yes(lambda:admit_endpoint(item,a,0,batch,desc,auditor,cp))
  for path,value in [(('schema',),'old'),(('passed',),False),(('auditor',),desc),(('inputs','descriptor'),cp),(('inputs','after'),desc),(('result','seed'),7),(('result','first_batch'),0),(('result','batches'),1)]:
   bad=copy.deepcopy(a);target=bad
   for key in path[:-1]:target=target[key]
   target[path[-1]]=value;no(lambda bad=bad:admit_endpoint(item,bad,0,batch,desc,auditor,cp))
  bad=copy.deepcopy(item);bad['directory']='canonical/seed-0/batch-256';no(lambda:admit_endpoint(bad,a,0,batch,desc,auditor,cp))
 item={'seed':0,'first_batch':256,'batches':16,'directory':'canonical/seed-0/batch-256','artifacts':{'iterations.bin':cp,'coefficients.bin':cp}}
 a={'schema':'psionic.locomotion-lqr-longer.learning-audit.v1','passed':True,'auditor':auditor,'inputs':{'descriptor':desc,'before':cp,'iterations':cp,'coefficients':cp},'result':{'seed':0,'first_batch':256,'batches':16}}
 yes(lambda:admit_replay_target(item,a,0,cp,desc,auditor))
 for path,value in [(('passed',),False),(('auditor',),desc),(('inputs','before'),desc),(('inputs','descriptor'),cp),(('inputs','iterations'),desc),(('inputs','coefficients'),desc),(('result','seed'),1),(('result','first_batch'),240)]:
  bad=copy.deepcopy(a);target=bad
  for key in path[:-1]:target=target[key]
  target[path[-1]]=value;no(lambda bad=bad:admit_replay_target(item,bad,0,cp,desc,auditor))
 # Filesystem lineage bridge: synthetic states only, no Coax/plant/optimizer calls.
 sandbox=output/'synthetic';sandbox.mkdir();write(sandbox/'descriptor.json',{'synthetic_metadata':True});context=bytes.fromhex(identity(sandbox/'descriptor.json')['sha256']);d={'live_auditor':auditor}
 record={'imports':[],'chunks':[]};old=(ROOT/e['resume_sources']['0']['snapshot_path']).read_bytes()[48:]
 for batch in (128,256,512):
  raw=bytearray(old);struct.pack_into('<q',raw,24,4*batch);struct.pack_into('<q',raw,424,batch)
  for w in range(16):struct.pack_into('<Q',raw,56+24*w,131072+w*24576+(batch-128)*64)
  item,a=endpoint(0,batch);di=sandbox/item['directory'];(di/'audit').mkdir(parents=True);wire=HEAD.pack(MAGIC['snapshot'],1,440,0)+context+raw;(di/'checkpoint.bin').write_bytes(wire)
  cp_id=identity(di/'checkpoint.bin');item['artifacts']['checkpoint.bin']=cp_id;a['inputs']={'descriptor':identity(sandbox/'descriptor.json'),'after':cp_id};write(di/'audit/record.json',a);item['audit']['record']=identity(di/'audit/record.json');record['imports' if batch==128 else 'chunks'].append(item)
  yes(lambda b=batch:authenticated_endpoint(record,sandbox,d,0,b,context));saved=(di/'checkpoint.bin').read_bytes();(di/'checkpoint.bin').write_bytes(saved[:-1]+bytes([saved[-1]^1]));no(lambda b=batch:authenticated_endpoint(record,sandbox,d,0,b,context));(di/'checkpoint.bin').write_bytes(saved)
 r={'schema':'psionic.locomotion-lqr-longer.lifecycle-guards.v1','passed':True,'producer':identity(SOURCE),'experiment':identity(F/'experiment.json'),'positive_cases':positives,'poisons_refused':poisons,'product_calls':0,'old_math_or_reader_tests_repeated':0,'scope':'New81-action selection, canonical import,128..512 state lineage, same-context/source previous endpoints, direct256 replay and filesystem checkpoint authentication only. Synthetic metadata faults; no training or old numerical oracle execution.'}
 write(output/'record.json',r);print(json.dumps(r))

def main():
 p=argparse.ArgumentParser();g=p.add_mutually_exclusive_group(required=True);g.add_argument('--lifecycle-guards',type=Path);g.add_argument('--prepare',action='store_true');g.add_argument('--run-prepared',type=Path);g.add_argument('--plan-advance',type=Path);p.add_argument('--qualified',type=Path);p.add_argument('--output',type=Path);p.add_argument('--auditor',type=Path,default=AUDITOR);a=p.parse_args()
 if a.lifecycle_guards is not None:lifecycle_guards(a.lifecycle_guards.resolve());return
 if a.prepare:need(a.qualified is not None and a.output is not None,'prepare requires qualified/output');prepare(a.qualified.resolve(),a.output.resolve(),a.auditor.resolve())
 else:
  target=a.run_prepared if a.run_prepared is not None else a.plan_advance
  if a.qualified is not None:need(str(a.qualified.resolve())==load(target/'descriptor.json')['qualified_directory'],'requested qualified directory matches preparation')
  run(target.resolve(),metadata_only=a.plan_advance is not None)
if __name__=='__main__':main()
