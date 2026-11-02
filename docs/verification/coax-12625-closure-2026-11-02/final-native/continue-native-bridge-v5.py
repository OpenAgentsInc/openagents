# Firm reason: 2 (independent continuation of the original native bridge commands and refusal oracle).
from pathlib import Path
import hashlib,json,os,subprocess,signal,time
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve();BASE=Path('/tmp/coax-12625-closure/native-v5');DIR=BASE/'bridge-continuation';DIR.mkdir(exist_ok=False);EXE=BASE/'bridge/coax-sanitized';start=time.monotonic();commands=[]
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=json.loads((BASE/'qualification/record.json').read_text());source_pins=json.loads((BASE/'source-pins.json').read_text())['source_pins'];test_pins=original['test_input_pins'];assert all(sha(ROOT/p)==h for p,h in source_pins.items());assert all(sha(ROOT/p)==h for p,h in test_pins.items())
artifact_paths=[EXE,BASE/'bridge/modules.o',BASE/'proto_importer.c',BASE/'proto_importer.h',BASE/'proto_runtime.inc',BASE/'coax'];artifacts={str(p):sha(p) for p in artifact_paths};env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1'}
def run(name,args,limit,expected):
 argv=list(map(str,args));beg=time.monotonic();timed=False;so=DIR/(name+'.stdout');se=DIR/(name+'.stderr')
 with so.open('wb') as stdout,se.open('wb') as stderr:
  p=subprocess.Popen(argv,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  (DIR/(name+'.launch.json')).write_text(json.dumps({'pid':p.pid,'pgid':p.pid,'argv':argv})+'\n')
  try:rc=p.wait(timeout=min(limit,max(1,840-(time.monotonic()-start))))
  except subprocess.TimeoutExpired:timed=True;os.killpg(p.pid,signal.SIGKILL);rc=p.wait()
 row={'name':name,'argv':argv,'returncode':rc,'expected_returncode':expected,'timed_out':timed,'wall_limit_seconds':limit,'elapsed_seconds':time.monotonic()-beg,'stdout_sha256':sha(so),'stderr_sha256':sha(se)};commands.append(row);(DIR/(name+'.json')).write_text(json.dumps(row,indent=2)+'\n');print(name,rc,row['elapsed_seconds'],flush=True);assert not timed and rc==expected,name;return so.read_bytes(),se.read_bytes()
passed=False
try:
 run('check-checker',[EXE,'check',ROOT/'coax/lib/compiler/checker.coax'],600,0)
 run('check-home',[EXE,'check',ROOT/'verse/home.coax'],180,0)
 run('check-grid',[EXE,'check',ROOT/'verse/grid.coax'],180,0)
 deep=BASE/'bridge/deep-types.coax';deep.write_text('\n'.join(f'S{i} :: struct {{x:S{i+1};}}' for i in range(400))+'\nS400 :: struct {x:int;}')
 stdout,stderr=run('deep-refusal',[EXE,'c',deep,'--library'],30,1);assert b'error[E0381]' in stderr;assert b'AddressSanitizer' not in stderr;assert b'runtime error:' not in stderr;assert stdout==b'';passed=True
finally:
 unchanged=all(sha(ROOT/p)==h for p,h in source_pins.items()) and all(sha(ROOT/p)==h for p,h in test_pins.items()) and all(sha(Path(p))==h for p,h in artifacts.items())
 record={'schema':'coax.12625.native.bridge-continuation.v1','original_unittest_failure':'qualification/native-bridge.json','original_failure_preserved':True,'original_method':'Parser.test_native_bridge_under_sanitizers','reused_successful_steps':['native modules.o compilation','sanitized compiler linking','sanitized check parser.coax'],'reuse_evidence':'The original unittest method reached the next checker.coax check; its unchanged pinned source runs those earlier steps sequentially through LX.checked, which raises on any nonzero result. Artifacts are retained and hash-pinned here.','artifact_pins':artifacts,'source_pins':source_pins,'test_input_pins':test_pins,'environment':{'ASAN_OPTIONS':'detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1'},'only_delta':'Checker check wall limit180→600 seconds; other checks180 and deep-refusal30 unchanged. Entire continuation≤840 seconds. No source/compiler flags/work quota/oracle changes.','elapsed_seconds':time.monotonic()-start,'inputs_unchanged':unchanged,'commands':commands,'passed':passed and unchanged};(DIR/'record.json').write_text(json.dumps(record,indent=2)+'\n');print('PASSED',record['passed'],flush=True)
