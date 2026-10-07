"""Use real installed binaries and an offline CLI fixture; never real provider accounts."""
import argparse,hashlib,json,os,re,signal,subprocess,sys,time
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--coder',required=True,type=Path,help='Absolute installed coder path; its companion openagents must be beside it.')
parser.add_argument('--commit',required=True,help='Exact clean source commit installed in both binaries.')
parser.add_argument('--root',required=True,type=Path,help='New directory under openagents scratch; all fixture state stays here.')
args=parser.parse_args()
assert args.coder.is_absolute() and args.root.is_absolute(), 'Use absolute binary and scratch paths.'
assert re.fullmatch('[0-9a-f]{40}',args.commit), 'Use the exact 40-character source commit.'
assert sys.platform=='darwin' and os.uname().machine=='arm64', 'This qualification selects macOS arm64 only.'
scratch_root=Path(os.environ.get('OPENAGENTS_SCRATCH_ROOT',Path.home()/'.openagents/scratch')).resolve()
assert args.root.resolve().is_relative_to(scratch_root), 'Keep fixture state under the configured durable scratch root.'
ROOT=args.root
ROOT.mkdir(mode=0o700)
os.umask(0o077)
state={'commit':args.commit}
CODER=args.coder

HOME=ROOT/'home';HOME.mkdir(mode=0o700)
BIN=ROOT/'bin';BIN.mkdir()
WORK=ROOT/'repository';WORK.mkdir()
AUTH=HOME/'.codex';AUTH.mkdir(mode=0o700)
LOGS=ROOT/'evidence';LOGS.mkdir(mode=0o700)
assert CODER.is_file(), 'Actual source installer must complete before first-task qualification.'
env={'HOME':str(HOME),'OPENAGENTS_HOME':str(HOME/'.openagents'),'CODEX_HOME':str(AUTH),'PATH':str(BIN)+':'+str(CODER.parent)+':/usr/bin:/bin:/usr/sbin:/sbin','LANG':'en_US.UTF-8','CLAUDE_BIN':str(ROOT/'unavailable-claude'),'CLAUDE_CONFIG_DIR':str(HOME/'.claude'),'OPENAGENTS_JEV_HOSTED':'off','CODER_CLOUD':'off','CODER_DELEGATE':'always','CODER_DELEGATE_AGENT':'codex','TMPDIR':str(ROOT/'tmp')}
Path(env['TMPDIR']).mkdir()
def run(args,tag,cwd=WORK,timeout=40):
 started=time.monotonic();r=subprocess.run(args,cwd=cwd,env=env,capture_output=True,text=True,timeout=timeout)
 (LOGS/(tag+'.stdout')).write_text(r.stdout);(LOGS/(tag+'.stderr')).write_text(r.stderr)
 return {'command':args,'exit':r.returncode,'elapsed_seconds':round(time.monotonic()-started,3),'stdout':r.stdout,'stderr':r.stderr}
for name in ['coder','openagents']:
 binary=CODER.parent/name
 version=run([str(binary),'--version'],name+'-version')
 assert version['exit']==0 and state['commit'][:10] in version['stdout'] and ' clean)' in version['stdout'],version
subprocess.run(['/usr/bin/git','init','--quiet',str(WORK)],env=env,check=True)
(WORK/'answer.txt').write_text('baseline\n')
subprocess.run(['/usr/bin/git','-C',str(WORK),'add','answer.txt'],env=env,check=True)
subprocess.run(['/usr/bin/git','-C',str(WORK),'-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','--quiet','-m','Freeze synthetic baseline'],env=env,check=True)
base=subprocess.check_output(['/usr/bin/git','-C',str(WORK),'rev-parse','HEAD'],env=env,text=True).strip()
fixture=r'''import json,sys,time,os
from pathlib import Path
if '--version' in sys.argv: print('codex offline-sales-qualification-fixture');sys.exit(0)
if sys.argv[1:3]==['login','status']:print('Offline synthetic login fixture');sys.exit(0)
if len(sys.argv)<2 or sys.argv[1]!='exec': print('Unsupported offline fixture command',file=sys.stderr);sys.exit(2)
prompt=sys.stdin.read()
def emit(v):print(json.dumps(v),flush=True)
emit({'type':'thread.started','thread_id':'0199aaaa-bbbb-7ccc-8ddd-offline00001'})
emit({'type':'turn.started'})
if 'QUALIFY_FAILURE' in prompt:
 emit({'type':'turn.failed','error':{'message':'Offline qualification fixture: provider transport failed'}});sys.exit(1)
if 'QUALIFY_STOP' in prompt:
 Path('partial.txt').write_text('partial synthetic effect\n')
 Path('partial-process.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp()}))
 emit({'type':'item.completed','item':{'id':'partial','type':'agent_message','text':'Partial effect retained; qualification fixture waiting.'}})
 time.sleep(120);sys.exit(3)
if 'QUALIFY_SUCCESS' not in prompt:
 emit({'type':'turn.failed','error':{'message':'Unrecognized synthetic fixture request'}});sys.exit(2)
Path('answer.txt').write_text('checked fixture result\n')
emit({'type':'item.completed','item':{'id':'change','type':'file_change','changes':[{'path':'answer.txt','kind':'update'}]}})
emit({'type':'item.completed','item':{'id':'reply','type':'agent_message','text':'Offline fixture wrote answer.txt. Independent acceptance remains required.'}})
emit({'type':'turn.completed','usage':{'input_tokens':30,'cached_input_tokens':0,'output_tokens':10}})
'''
(BIN/'codex').write_text('#!'+sys.executable+'\n'+fixture);os.chmod(BIN/'codex',0o700)
def turn(prompt,tag):return run([str(CODER),'-p','--json','--trace',str(LOGS/(tag+'.atif.jsonl')),prompt],tag)
missing=turn('QUALIFY_SUCCESS: change answer.txt to checked fixture result.','missing-login')
assert missing['exit']==1 and any(word in (missing['stdout']+missing['stderr']).lower() for word in ['credential','no codex login','not authenticated']),missing
# These strings are synthetic parser fixtures, never credentials for a real service.
(AUTH/'auth.json').write_text(json.dumps({'auth_mode':'chatgpt','tokens':{'access_token':'fixture.eyJleHAiOjQxMDI0NDQ4MDB9.fixture','account_id':'offline-fixture','refresh_token':'offline-fixture'}}));os.chmod(AUTH/'auth.json',0o600)
doctor=run([str(CODER),'doctor'],'doctor');assert doctor['exit']==0,doctor
executor=next(line for line in doctor['stdout'].splitlines() if line.startswith('executor'))
assert 'codex runs ' in executor and 'claude-code' not in executor,executor
assert 'OPENAGENTS_JEV_HOSTED=off' in doctor['stdout'],doctor
success=turn('QUALIFY_SUCCESS: change answer.txt to checked fixture result.','success')
assert success['exit']==0,success
summary=json.loads(success['stdout'].splitlines()[-1]);assert summary['outcome']=='answered',summary
# Freeze the exact patch, apply it to a separate clean worktree, and independently check it.
patch=subprocess.check_output(['/usr/bin/git','-C',str(WORK),'diff','--binary',base],env=env)
(LOGS/'candidate.patch').write_bytes(patch)
VERIFY=ROOT/'independent-candidate'
subprocess.run(['/usr/bin/git','-C',str(WORK),'worktree','add','--quiet','--detach',str(VERIFY),base],env=env,check=True)
subprocess.run(['/usr/bin/git','apply','--binary','-'],cwd=VERIFY,env=env,input=patch,check=True)
check=run([sys.executable,'-c',"from pathlib import Path; assert Path('answer.txt').read_text() == 'checked fixture result\\n'; print('Independent fixture check passed')"],'independent-check',cwd=VERIFY)
assert check['exit']==0,check
failure=turn('QUALIFY_FAILURE: leave the candidate unchanged.','failure');assert failure['exit']==1,failure
assert json.loads(failure['stdout'].splitlines()[-1])['outcome']=='failed',failure
stop_args=[str(CODER),'-p','--json','--trace',str(LOGS/'stopped.atif.jsonl'),'QUALIFY_STOP: retain a partial marker then wait.']
with (LOGS/'stopped.stdout').open('w') as out,(LOGS/'stopped.stderr').open('w') as err:
 p=subprocess.Popen(stop_args,cwd=WORK,env=env,stdout=out,stderr=err,start_new_session=True)
 deadline=time.monotonic()+35
 while not (WORK/'partial-process.json').exists() and p.poll() is None and time.monotonic()<deadline:time.sleep(.05)
 entered=(WORK/'partial.txt').exists() and (WORK/'partial-process.json').exists()
 if entered:
  child=json.loads((WORK/'partial-process.json').read_text())
  assert child['pid']>0 and child['pgid']>0 and child['pgid']!=os.getpgrp()
  command=subprocess.check_output(['/bin/ps','-p',str(child['pid']),'-o','command='],text=True)
  assert str(BIN/'codex') in command, command
  # The executor supervisor has its own process group; stop it as well as the caller.
  os.killpg(child['pgid'],signal.SIGTERM)
 if p.poll() is None:
  os.killpg(p.pid,signal.SIGTERM)
  try:p.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=5)
 assert entered, 'Fixture never entered stopped task'
 deadline=time.monotonic()+5
 while time.monotonic()<deadline:
  status=subprocess.run(['/bin/ps','-p',str(child['pid']),'-o','stat='],capture_output=True,text=True)
  if status.returncode or not status.stdout.strip() or status.stdout.strip().startswith('Z'):break
  time.sleep(.05)
 else:raise AssertionError('The synthetic executor remains live after interruption')
 stopped={'exit':p.returncode,'classification':'caller_stopped_partial_effect_unaccepted','partial_effect_retained':True,'executor_group_stopped':True,'command':stop_args}
stopped_check=run([sys.executable,'-c',"from pathlib import Path; assert Path('partial.txt').read_text() == 'partial synthetic effect\\n'; print('Independent stopped-effect inspection passed; no result acceptance or automatic replay')"],'stopped-effect-check')
assert stopped_check['exit']==0,stopped_check
stopped['independent_inspection']='evidence/stopped-effect-check.stdout'
restart=turn('QUALIFY_SUCCESS: independently inspected prior partial marker; start a new attempt.','restart');assert restart['exit']==0,restart
assert (WORK/'partial.txt').read_text()=='partial synthetic effect\n'
recheck=run([sys.executable,'-c',"from pathlib import Path; assert Path('answer.txt').read_text() == 'checked fixture result\\n'; assert Path('partial.txt').exists(); print('Restart independent check passed; partial effect preserved')"],'restart-check');assert recheck['exit']==0,recheck
# Never persist even synthetic auth values in published qualification evidence.
(AUTH/'auth.json').unlink()
artifacts={str(p.relative_to(ROOT)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in LOGS.iterdir() if p.is_file()}
proof={'schema':'openagents.sales.first-task-qualification.v1','source_commit':state['commit'],'platform':'macOS arm64','provider':'offline Codex CLI protocol fixture; no network or real login','actual_installed_binary_sha256':hashlib.sha256(CODER.read_bytes()).hexdigest(),'baseline_commit':base,'candidate_sha256':hashlib.sha256(patch).hexdigest(),'missing_login':missing,'doctor':doctor,'success':success,'independent_check':check,'failure':failure,'stopped':stopped,'restart':restart,'restart_check':recheck,'artifacts':artifacts,'real_customer_qualified':False,'actual_provider_qualified':False,'network_configuration':'Hosted decisions and cloud disabled; no decision profile, provider keys, or live CLI; only exact offline Codex fixture selected','hosted_decisions':'OPENAGENTS_JEV_HOSTED=off'}
(LOGS/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps({'source_commit':state['commit'],'checks':'actual installed binary, missing login refusal, doctor, success, independent candidate check, explicit failure, stopped partial work, new-process restart check','artifacts':len(artifacts),'real_provider_qualified':False},indent=2))
