#!/usr/bin/env python3
"""Explicit public-fixture and app tests, not a normal-build dependency."""
import hashlib,json,os,re,subprocess,sys,tempfile,time
from pathlib import Path
if not __debug__:raise RuntimeError('run this test without Python optimization')
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];COAX=ROOT/'coax/bin/coax'
sys.path.insert(0,str(ROOT/'tools/coax-nostr/fuzz'))
from oracle import schnorr_verify
def run(*args):
    result=subprocess.run(list(map(str,args)),text=True,capture_output=True)
    if result.returncode:raise RuntimeError(f'{args[0]} exited {result.returncode}:\n{result.stdout}\n{result.stderr}')
    return result.stdout
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def strings(p):return [json.loads(s) for s in re.findall(r'"(?:[^"\\]|\\.)*"',Path(p).read_text().split('::',1)[1])]
start=time.monotonic();cases=strings(HERE/'fixture.coax');capture=ROOT/'coax/lib/nostr/fixtures/coax_relay.coax'
assert sha(capture)=='fe9a9500458039ca208c5188ddddbfb049a531e2884940a6a1bba22385461046'
raw=json.loads(re.search(r'COAX_RELAY :: ("(?:[^"\\]|\\.)*");',capture.read_text()).group(1))
frame=next(row for row in raw.splitlines() if row.startswith('["EVENT","repo",'))
assert cases[0]==frame[len('["EVENT","repo",'):-1]
assert hashlib.sha256(cases[0].encode()).hexdigest()=='dcf32812f99618c23fa3dd2bff0569ef590f2b15d2d44b61bfb61f3257f00e93'
event=json.loads(cases[0]);preimage=json.dumps([0,event['pubkey'],event['created_at'],event['kind'],event['tags'],event['content']],ensure_ascii=False,separators=(',',':')).encode()
digest=hashlib.sha256(preimage).digest();assert digest.hex()==event['id'];assert schnorr_verify(digest,bytes.fromhex(event['pubkey']),bytes.fromhex(event['sig']))
report={'schema':'coax.real-event-probe.v1','scope':'local development public-event verification and display; no signing or production admission','compiler':run(COAX,'version').strip(),'compiler_sha256':sha(COAX),'captured_event_sha256':hashlib.sha256(cases[0].encode()).hexdigest(),'event_id':event['id'],'independent_sha256_and_bigint_bip340':True,'native':[]}
sources={}
for entry in ['probe.coax','verifier.coax']:
    for p in json.loads(run(COAX,'deps',HERE/entry))['sources']:sources[str(Path(p).relative_to(ROOT))]=sha(p)
for p in HERE.iterdir():
    if p.is_file() and p.suffix in ['.coax','.c','.cjs','.py','.sh','.html']:
        sources[str(p.relative_to(ROOT))]=sha(p)
sources['tools/coax-nostr/fuzz/oracle.py']=sha(ROOT/'tools/coax-nostr/fuzz/oracle.py')
for forbidden in ['secp_mult_base','schnorr_sign','nostr/keys','nip42','key_custody']:
    assert not any(forbidden in name.lower() for name in sources),('signing dependency',forbidden)
report['signing_dependency_absent']=True
with tempfile.TemporaryDirectory(prefix='coax-real-event-')as td:
    temp=Path(td)
    for mode,flags in [('checked',[]),('release',['--release'])]:
        for entry in ['probe','verifier']:
            (temp/(entry+'.c')).write_text(run(COAX,'c',HERE/(entry+'.coax'),'--library','--header',temp/(entry+'.h'),*flags))
        # Compile the scalar ABI declaration against its actual generated header.
        (temp/'abi.c').write_text('#include "verifier.h"\nextern void nv_run(int64_t mode,coax_v_s64 observation);\n')
        run('cc','-std=c11','-Werror','-I'+str(temp),'-c',temp/'abi.c','-o',temp/'abi.o')
        target=temp/(mode+'.test');run('cc','-std=c11','-O2','-I'+str(temp),temp/'probe.c',temp/'verifier.c',HERE/'probe-test.c','-lm','-o',target)
        result=run(target).strip();report['native'].append({'profile':mode,'result':result,'artifact_sha256':sha(target),'probe_c_sha256':sha(temp/'probe.c'),'verifier_c_sha256':sha(temp/'verifier.c')})
    report['wasm']=json.loads(run('node',HERE/'probe-test.cjs',HERE/'build/probe.wasm'));report['wasm']['artifact_sha256']=sha(HERE/'build/probe.wasm');report['wasm']['stack_bytes']=1048576
for p,want in sources.items():assert sha(ROOT/p)==want,('input changed',p)
assert sha(COAX)==report['compiler_sha256'];report['sources']=sources;report['elapsed_seconds']=time.monotonic()-start
text=json.dumps(report,indent=2,sort_keys=True)+'\n';print(text,end='')
if os.environ.get('COAX_REAL_EVENT_REPORT'):Path(os.environ['COAX_REAL_EVENT_REPORT']).write_text(text)
