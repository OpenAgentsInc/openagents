#!/usr/bin/env python3
"""Real checked-arithmetic refusal and native/WASI runtime controls."""
import hashlib,json,resource,shutil,signal,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];HERE=Path(__file__).resolve().parent
SOURCE=HERE/'secret_checked_arithmetic_fixture.coax';COMPILER=ROOT/'coax/bin/coax'
CLANG='/opt/homebrew/opt/llvm/bin/clang';SYSROOT='/opt/homebrew/opt/wasi-libc/share/wasi-sysroot';RESOURCE='/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(*args):return subprocess.run([str(x) for x in args],capture_output=True,text=True,timeout=180,preexec_fn=lambda:resource.setrlimit(resource.RLIMIT_CORE,(0,0)))
def require(result):
 if result.returncode:raise AssertionError((result.args,result.returncode,result.stdout,result.stderr))
 return result.stdout
profiles={'scalar':'tools/coax-nostr/secret-check/check.py','legacy':'tools/coax-nostr/secret-check/aggregate.py','public':'tools/coax-nostr/public-view-check/aggregate.py','large':'tools/coax-nostr/large-source-audit/aggregate.py'}
entries={'checked_add':'unsupported','checked_sub':'unsupported','checked_mul':'unsupported','checked_neg':'unsupported','compound':'unsupported','secret_shift':'secret-shift','secret_div':'unsupported','wrapping_add':'accepted'}
expected_trap={0:"integer overflow in '+'",1:"integer overflow in '-'",2:"integer overflow in '*'",3:"integer overflow in unary '-'",4:"integer overflow in '+'",5:'shift amount out of range',6:"division by zero",9:"integer overflow in '+'"}
tracked=[SOURCE,HERE/'secret_checked_arithmetic_fixture.c',HERE/'secret_checked_cast_wasm.cjs',COMPILER,ROOT/'coax/lib/crypto/secret-flow.coax']+[ROOT/'tools/coax-nostr'/p/'aggregate.coax' for p in ('secret-check','public-view-check','large-source-audit')]
initial={str(p.resolve()):sha(p) for p in tracked};results=[]
tool_inputs={};target_tools={name:str(Path(path).resolve()) for name,path in {'cc':shutil.which('cc'),'clang':CLANG,'node':shutil.which('node')}.items()}
target_hashes={name:sha(path) for name,path in target_tools.items()}
with tempfile.TemporaryDirectory(prefix='coax-checked-arithmetic-') as td:
 d=Path(td);report=d/'report.json'
 for profile,tool in profiles.items():
  tests=dict(entries)
  if profile!='scalar':tests['array_compound']='unsupported'
  for entry,expected in tests.items():
   result=run(sys.executable,ROOT/tool,SOURCE,'--entry',entry,'--report',report);data=json.loads(report.read_text());accepted=expected=='accepted'
   if (result.returncode!=(0 if accepted else 1) or data['accepted'] is not accepted or data['error_name']!=expected or data['source_sha256']!={str(SOURCE.resolve()):sha(SOURCE)} or data['compiler_sha256']!=sha(COMPILER) or (accepted and data['return_dependency_mask']!=1)):
    raise AssertionError((profile,entry,result.returncode,data))
   inputs=data['audit_input_sha256']
   if profile not in tool_inputs:
    if any(sha(path)!=digest for path,digest in inputs.items()):raise AssertionError('audit input identity')
    tool_inputs[profile]=inputs
   elif inputs!=tool_inputs[profile]:raise AssertionError('audit input drift')
   results.append([profile,entry,expected])
 for mode in ('checked','release'):
  c=d/'arithmetic.c';h=d/'arithmetic.h';mapping=d/'map.json';native=d/'native';wasm=d/'arithmetic.wasm'
  c.write_text(require(run(COMPILER,'c',SOURCE,'--library','--header',h,'--source-map',mapping,*(['--release'] if mode=='release' else []))))
  if json.loads(mapping.read_text())['source_hashes']!={str(SOURCE.resolve()):sha(SOURCE)}:raise AssertionError('source map identity')
  require(run('cc','-std=c11','-O2','-I'+str(d),c,HERE/'secret_checked_arithmetic_fixture.c','-lm','-o',native))
  require(run(CLANG,'--target=wasm32-wasip1','--sysroot='+SYSROOT,'-resource-dir='+RESOURCE,'-std=c11','-O2','-I'+str(d),c,HERE/'secret_checked_arithmetic_fixture.c','-lm','-o',wasm))
  for target in ('native','wasm'):
   for case in range(10):
    args=[native,case] if target=='native' else ['node',HERE/'secret_checked_cast_wasm.cjs',wasm,case]
    result=run(*args);trap=case in (5,6) or (mode=='checked' and case in (0,1,2,3,4,9))
    if trap:
     if result.returncode==0 or expected_trap[case] not in result.stderr or (target=='native' and result.returncode!=-signal.SIGABRT):raise AssertionError((mode,target,case,result.returncode,result.stderr))
    elif result.returncode:raise AssertionError((mode,target,case,result.returncode,result.stderr))
   results.append([mode,target,10,'passed'])
  results.append([mode,'sha256',{'c':sha(c),'map':sha(mapping),'native':sha(native),'wasm':sha(wasm)}])
 if any(sha(p)!=h for p,h in initial.items()) or any(sha(p)!=h for inputs in tool_inputs.values() for p,h in inputs.items()):raise AssertionError('input drift')
 if any(sha(path)!=target_hashes[name] for name,path in target_tools.items()):raise AssertionError('target tool drift')
print(json.dumps({'inputs':initial,'audit_inputs':tool_inputs,'target_tools':{name:{'path':path,'sha256':target_hashes[name]} for name,path in target_tools.items()},'results':results},indent=2))
