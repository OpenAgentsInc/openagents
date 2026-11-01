#!/usr/bin/env python3
"""Explicit one-shot release learning, using the ALREADY qualified native binary.

No compilation, fixture rerun, external simulator, SDK or original-weight model
is performed here. The owned scalar plant executes only in the explicit run.
Preparation requires fresh output and performs file identity work only. Each
one-shot fixed-action advance permits READY continuation and never recovers a
failed or running action; a reviewed frozen independent auditor is mandatory.
"""
from __future__ import annotations
import argparse,ast,hashlib,importlib.util,json,os,platform,struct,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
SOURCE=Path(__file__).resolve()
AUDITOR=ROOT/'psionic/tests/robotics/locomotion_lqr_learning_audit.py'
MATH_DRIVER=ROOT/'psionic/tests/robotics/locomotion_lqr_native_reference.py'
EXPERIMENT='8f002e3f4192a29f249e3a0e3ec68ef50d29542eb91197b96eaaec5bf117194c'
HEAD=struct.Struct('<4I');STATE=440;ITERATION=62696
MAGIC={'snapshot':0x3153514c,'iteration':0x3149514c,'coefficient':0x3143514c,'eval':0x3145514c,'quality':0x3147514c}
def need(c,t):
 if not c:raise ValueError(t)
def identity(p):
 b=Path(p).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def load(p):return json.loads(Path(p).read_text())
def write(p,x):Path(p).write_text(json.dumps(x,indent=2,allow_nan=False)+'\n')
def check_source(mapping):
 for name,h in mapping.items():need(identity(ROOT/name)==h,'immutable qualified source '+name)
def state(raw,seed,batch):
 need(len(raw)==440,'field-wise state extent');K,m,v,t,skip=struct.unpack_from('<3d2q',raw);need(all(__import__('math').isfinite(x) for x in [K,m,v]) and .25<=K<=1 and v>=0,'state finite/domain');need((t,skip)==(4*batch,0),'state optimizer lineage');need(struct.unpack_from('<qQ',raw,424)==(batch,seed),'state batch/runseed')
 for w in range(16):
  x,s,c=struct.unpack_from('<d2Q',raw,40+w*24);need(__import__('math').isfinite(x) and abs(x)<=16 and (s,c)==(seed,w*8192+batch*64),'world state/counter lineage')
 return K

def qualification(qdir):
 q=load(qdir/'record.json');need(q['passed'] is True and not q['plan_only'] and q['all_mode_output_bytes_exact'] and q['learning_calls']==0,'first fixture qualification')
 need(q['experiment']==identity(F/'experiment.json') and q['experiment']['sha256']==EXPERIMENT,'sealed experiment');check_source(q['source'])
 for p,h in q['frozen'].items():need(identity(p)==h,'qualified frozen evidence '+p)
 emit=next(x for x in q['commands'] if x['label']=='emit-release');compiler=Path(emit['argv'][0]);need(identity(compiler)==q['compiler'],'actual qualified compiler')
 mode=q['modes']['release'];binary=qdir/'native-release/native';need(identity(binary)==mode['binary'],'same qualified release binary')
 for n,p in [('C',qdir/'release/locomotion-lqr.c'),('header',qdir/'release/locomotion-lqr.h')]:need(identity(p)==mode[n],'qualified generated '+n)
 for seed in [0,1,7]:need(identity(qdir/f'native-release/initial-{seed}.bin')==mode['outputs'][f'initial-{seed}.bin'],'qualified initial factory')
 return q,binary,compiler

def auditor_binding(auditor):
 guard=F/'learning-reader-qualification.json';freeze=F/'learning-audit-freeze.json';follow=F/'learning-audit-followup-v1.json';lineage=F/'learning-lineage-qualification.json';archive=F/'learning-audit-original-v1.py.txt';g,f,n=load(guard),load(follow),load(lineage)
 need(g['passed'] is True and f['passed'] is True and n['passed'] is True,'passed carried/new audit guards')
 need(f['old_source']==identity(archive)==g['producer'] and f['new_source']==identity(auditor)==n['producer'],'old/new whole auditor association')
 need(f['carried_reader_receipt']==identity(guard) and f['historical_audit_freeze']==identity(freeze) and f['new_lineage_receipt']==identity(lineage),'audit guard history association')
 need(n['math_qualification']==identity(F/'native-qualification.json'),'new lineage qualified factory association')
 for path in [archive,auditor]:
  source=path.read_text();decls={x.name:ast.get_source_segment(source,x).encode() for x in ast.parse(source).body if isinstance(x,ast.FunctionDef)}
  for name,h in f['unchanged_declarations'].items():need({'bytes':len(decls[name]),'sha256':hashlib.sha256(decls[name]).hexdigest()}==h,'carried unchanged decoder declaration '+name)
 need(f['chunk_and_evaluation_numeric_AST_unchanged_after_removing_ONLY_lineage_branches'] is True,'guard-only numeric body carry')
 return guard,freeze,follow,lineage,archive

def actions():
 return [*({'kind':'canonical','seed':s,'first_batch':b,'batches':16} for s in [0,1,7] for b in range(0,128,16)),*({'kind':'replay','seed':s,'first_batch':64,'batches':1} for s in [0,1,7]),*({'kind':'evaluation','label':label,'seed':seed} for label,seed in [('baseline',0),('seed-0',0),('seed-1',1),('seed-7',7)])]

def prepare(qdir,out,auditor):
 need(not out.exists(),'fresh learning preparation');q,binary,compiler=qualification(qdir);need(auditor.is_file(),'independent live auditor must be authored/frozen');guard,freeze,follow,lineage,archive=auditor_binding(auditor);out.mkdir(parents=True)
 e=load(F/'experiment.json');descriptor={'schema':'psionic.locomotion-lqr.learning-descriptor.v1','frozen_before_learning':True,'experiment':identity(F/'experiment.json'),'contract':e['contract'],'qualified_math_record':identity(qdir/'record.json'),'math_qualification':identity(qdir/'record.json'),'independent_reference':identity(F/'reference.json'),'live_auditor':identity(auditor),'live_auditor_path':str(auditor),'live_reader_qualification':identity(guard),'live_audit_freeze':identity(freeze),'live_audit_followup':identity(follow),'live_lineage_qualification':identity(lineage),'actions':actions(),'native_binary':identity(binary),'qualified_directory':str(qdir),'release_binary':identity(binary),'compiler':identity(compiler),'source':{str(SOURCE.relative_to(ROOT)):identity(SOURCE),str(auditor.relative_to(ROOT)):identity(auditor),str(guard.relative_to(ROOT)):identity(guard),str(freeze.relative_to(ROOT)):identity(freeze),str(follow.relative_to(ROOT)):identity(follow),str(lineage.relative_to(ROOT)):identity(lineage),str(archive.relative_to(ROOT)):identity(archive)},'training':e['training'],'wire':{'header':'<4I +32B descriptorSHA','snapshot':[MAGIC['snapshot'],1,440,0],'iteration':[MAGIC['iteration'],1,62696,16],'coefficient':[MAGIC['coefficient'],1,88,32768],'coefficient_scope':'source-bound RECOMPUTED observation, batch/epoch/row order; not internal instrumentation','evaluation':[MAGIC['eval'],1,72,6],'quality':[MAGIC['quality'],1,200,1]},'planned_effects':{'canonical_plant_steps':196608,'canonical_adam_updates':1536,'canonical_tape_pairs':12288,'canonical_sample_gradients':786432,'canonical_coefficient_probe_calls':786432,'replay_plant_steps':1536,'replay_adam_updates':12,'replay_tape_pairs':96,'replay_sample_gradients':6144,'replay_coefficient_probe_calls':6144,'deterministic_evaluation_steps':3072},'failure_policy':'Stop and preserve complete/partial records; never retry a batch or recover a packet/checkpoint disagreement automatically. No tuning or best-seed selection.','persistence':'C fflush and file fsync both iteration/coefficient batches before checkpoint temp fsync+rename; no directory fsync/crash-durability claim.','learning_executions':0}
 write(out/'descriptor.json',descriptor);context=bytes.fromhex(identity(out/'descriptor.json')['sha256']);(out/'context.bin').write_bytes(context)
 for seed in [0,1,7]:
  raw=(qdir/f'native-release/initial-{seed}.bin').read_bytes();state(raw,seed,0);(out/f'initial-{seed}.bin').write_bytes(HEAD.pack(MAGIC['snapshot'],1,440,0)+context+raw)
 r={'schema':'psionic.locomotion-lqr.learning-preparation.v1','passed':True,'descriptor':identity(out/'descriptor.json'),'context':identity(out/'context.bin'),'source':descriptor['source'],'initials':{f'initial-{s}.bin':identity(out/f'initial-{s}.bin') for s in [0,1,7]},'learning_calls':0,'commands':[]};write(out/'prepared.json',r);print(json.dumps({'passed':True,'prepared':str(out/'prepared.json'),'descriptor':identity(out/'descriptor.json')}))

def parse_file(path,magic,width,cap,context,count):
 raw=Path(path).read_bytes();need(len(raw)==48+width*count,'exact complete wire extent '+str(path));need(HEAD.unpack_from(raw)==(magic,1,width,cap) and raw[16:48]==context,'wire/context header '+str(path));return raw[48:]
def extent(path,width):
 if not path.exists():return {'exists':False}
 n=path.stat().st_size;return {'exists':True,'bytes':n,'complete_records':max(0,n-48)//width,'trailing_bytes':max(0,n-48)%width,'header_complete':n>=48,'identity':identity(path)}

def verify_initials(out,prepared,qdir,context):
 for seed in [0,1,7]:
  path=out/f'initial-{seed}.bin';need(identity(path)==prepared['initials'][path.name],'prepared initial identity')
  raw=parse_file(path,MAGIC['snapshot'],440,0,context,1);need(raw==(qdir/f'native-release/initial-{seed}.bin').read_bytes(),'exact qualified factory initial bytes');state(raw,seed,0)

def select_action(record,descriptor):
 need(record['lifecycle_state']=='ready' and not record['passed'],'only READY work may advance')
 index=len(record['completed_actions']);need(record['completed_actions']==descriptor['actions'][:index] and index<len(descriptor['actions']),'fixed next action without replay');return index,descriptor['actions'][index]

def run(out,metadata_only=False):
 prepared=load(out/'prepared.json');d=load(out/'descriptor.json');need(prepared['passed'] and identity(out/'descriptor.json')==prepared['descriptor'] and identity(out/'context.bin')==prepared['context'],'immutable reviewed preparation');check_source(prepared['source']);qdir=Path(d['qualified_directory']);q,binary,compiler=qualification(qdir);need(identity(qdir/'record.json')==d['qualified_math_record'] and identity(binary)==d['release_binary'],'prepared release context');context=(out/'context.bin').read_bytes();verify_initials(out,prepared,qdir,context);need(platform.system()=='Darwin' and platform.machine()=='arm64' and sys.byteorder=='little','named scalar host')
 if metadata_only:
  record=load(out/'record.json') if (out/'record.json').exists() else {'lifecycle_state':'ready','passed':False,'completed_actions':[]}
  index,action=select_action(record,d);print(json.dumps({'passed':True,'metadata_only':True,'next_index':index,'next_action':action,'product_calls':0}));return
 lock=out/'.advance.lock';lockfd=os.open(lock,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.close(lockfd)
 marker=Path('/tmp/locomotion-lqr-learning-'+EXPERIMENT+'.one-shot.json')
 if not(out/'record.json').exists():
  fd=os.open(marker,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.write(fd,(json.dumps({'output':str(out),'descriptor':prepared['descriptor']})+'\n').encode());os.close(fd)
  r={'schema':'psionic.locomotion-lqr.learning.v1','passed':False,'lifecycle_state':'ready','descriptor':prepared['descriptor'],'prepared':identity(out/'prepared.json'),'qualified_math':identity(qdir/'record.json'),'binary':identity(binary),'source':prepared['source'],'chunks':[],'replays':[],'evaluations':[],'completed_actions':[],'commands':[],'old_math_compile_model_SDK_calls':0,'seconds_total':0.}
 else:
  r=load(out/'record.json');need(r['lifecycle_state']=='ready' and not r['passed'],'only explicitly READY work may advance; never recover failed/running actions');need(load(marker)=={'output':str(out),'descriptor':prepared['descriptor']},'same one-shot experiment')
 need(r['descriptor']==prepared['descriptor'] and r['prepared']==identity(out/'prepared.json'),'unchanged monotonic lifecycle context')
 for group in ['chunks','replays','evaluations']:
  for item in r[group]:
   for name,h in item['artifacts'].items():need(identity(out/item['directory']/name)==h,'completed artifact immutable')
   need(identity(out/item['audit']['directory']/'record.json')==item['audit']['record'],'completed audit immutable')
 index,action=select_action(r,d);r['lifecycle_state']='running';r['active_action']=action;write(out/'record.json',r);began=time.monotonic()
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
  logs=dir/'audit-command';logs.mkdir();target=dir/'audit';report=command('audit-'+str(index),[sys.executable,d['live_auditor_path'],'--descriptor',out/'descriptor.json','--math-record',qdir/'record.json',*argv,'--output',target],logs);need(load(target/'record.json')['passed'] is True,'independent new action audit');return {'directory':str(target.relative_to(out)),'record':identity(target/'record.json')}
 try:
  if action['kind']=='canonical':
   seed,first=action['seed'],action['first_batch'];initial=out/f'initial-{seed}.bin' if first==0 else out/f'canonical/seed-{seed}/batch-{first-16:03d}/checkpoint.bin';dir=out/f'canonical/seed-{seed}/batch-{first:03d}';after=chunk(seed,first,16,initial,dir,'canonical')
   r['chunks'][-1]['audit']=audit(dir,['--before',initial,'--after',after,'--iterations',dir/'iterations.bin','--coefficients',dir/'coefficients.bin','--progress',dir/'progress.jsonl','--seed',seed,'--first-batch',first,'--count',16,*(['--previous-audit',out/f'canonical/seed-{seed}/batch-{first-16:03d}/audit/record.json'] if first else [])])
  elif action['kind']=='replay':
   seed=action['seed'];initial=out/f'canonical/seed-{seed}/batch-048/checkpoint.bin';original=out/f'canonical/seed-{seed}/batch-064';dir=out/f'replay/seed-{seed}';after=chunk(seed,64,1,initial,dir,'replay')
   for name,width,rows in [('iterations.bin',62696,1),('coefficients.bin',88,2048)]:need((dir/name).read_bytes()[48:]==(original/name).read_bytes()[48:48+width*rows],'exact declared checkpoint65 '+name)
   r['replays'][-1]['exact_original_batch65_bytes']=True;r['replays'][-1]['audit']=audit(dir,['--before',initial,'--after',after,'--iterations',dir/'iterations.bin','--coefficients',dir/'coefficients.bin','--progress',dir/'progress.jsonl','--seed',seed,'--first-batch',64,'--count',1,'--canonical-audit',original/'audit/record.json','--canonical-iterations',original/'iterations.bin','--canonical-coefficients',original/'coefficients.bin'])
  else:
   label,seed=action['label'],action['seed'];snapshot_path=out/'initial-0.bin' if label=='baseline' else out/f'canonical/seed-{seed}/batch-112/checkpoint.bin';dir=out/'evaluation'/label;dir.mkdir(parents=True);report=command('evaluation-'+label,[binary,'eval',snapshot_path,dir,out/'context.bin'],dir);need(report['steps']==768 and report['evaluations']==6,'named deterministic eval count');parse_file(dir/'evaluations.bin',MAGIC['eval'],72,6,context,6);parse_file(dir/'quality.bin',MAGIC['quality'],200,1,context,1)
   obj={'label':label,'directory':str(dir.relative_to(out)),'native':report,'artifacts':{n:identity(dir/n) for n in ['evaluations.bin','quality.bin','stdout','stderr']}};r['evaluations'].append(obj);obj['audit']=audit(dir,['--after',snapshot_path,'--evaluations',dir/'evaluations.bin','--quality',dir/'quality.bin','--seed',seed,*(['--baseline'] if label=='baseline' else ['--previous-audit',out/f'canonical/seed-{seed}/batch-112/audit/record.json'])])
  qualification(qdir);check_source(prepared['source']);need(identity(compiler)==d['compiler'] and identity(binary)==d['release_binary'],'final qualified binary/compiler');r['completed_actions'].append(action);r.pop('active_action',None);complete=len(r['completed_actions'])==len(d['actions']);r['lifecycle_state']='complete' if complete else 'ready';r['passed']=complete
  if complete:r.update(completed_seeds=[0,1,7],canonical_batches=384,canonical_steps=196608,canonical_adam_updates=1536,replay_steps=1536,replay_adam_updates=12,deterministic_eval_steps=3072,goal_status='All per-action math audits passed; retain each final goal result without seed selection')
 except BaseException as x:r['lifecycle_state']='failed';r['error']=type(x).__name__+': '+str(x);raise
 finally:r['last_advance_seconds']=time.monotonic()-began;r['seconds_total']+=r['last_advance_seconds'];write(out/'record.json',r);lock.unlink()
 print(json.dumps({'action_passed':True,'complete':r['passed'],'completed_actions':len(r['completed_actions']),'record':str(out/'record.json'),'identity':identity(out/'record.json')}))

def main():
 p=argparse.ArgumentParser();g=p.add_mutually_exclusive_group(required=True);g.add_argument('--prepare',action='store_true');g.add_argument('--run-prepared',type=Path);g.add_argument('--plan-advance',type=Path);p.add_argument('--qualified',type=Path);p.add_argument('--output',type=Path);p.add_argument('--auditor',type=Path,default=AUDITOR);a=p.parse_args()
 if a.prepare:need(a.qualified is not None and a.output is not None,'prepare requires qualified/output');prepare(a.qualified.resolve(),a.output.resolve(),a.auditor.resolve())
 else:
  target=a.run_prepared if a.run_prepared is not None else a.plan_advance
  if a.qualified is not None:need(str(a.qualified.resolve())==load(target/'descriptor.json')['qualified_directory'],'requested qualified directory matches preparation')
  run(target.resolve(),metadata_only=a.plan_advance is not None)
if __name__=='__main__':main()
