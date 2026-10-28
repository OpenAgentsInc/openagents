#!/usr/bin/env python3
"""Bounded oracle/candidate comparison; never labels oracle smoke as Coax conformance."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import time
import tempfile
import resource
import shutil

from oracle import P,N,G,field,scalar,point_mul,encode_point,decode_point,raw_ecdh,schnorr_verify,NativeOracle,DEFAULT_LIBRARY

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]

def h(n):return format(n,'064x')
def result(value):return {'ok':True,'value':value.hex() if isinstance(value,bytes) else value}

def reference(request,native=None):
    module=request['module']
    if module in ('field','scalar'):
        fn=field if module=='field' else scalar
        try:return result(h(fn(request['op'],int(request['a'],16),int(request.get('b','0'),16))))
        except ValueError:return {'ok':False}
    if module=='group':
        k=int(request['scalar'],16);peer=bytes.fromhex(request['point']) if 'point' in request else encode_point(G)
        point=decode_point(peer)
        if point is None or not 0<=k<2**256:return {'ok':False}
        out=encode_point(point_mul(k,point))
        return result(out.hex() if out is not None else '00')
    if module=='ecdh':
        out=raw_ecdh(bytes.fromhex(request['secret']),bytes.fromhex(request['peer']))
        return {'ok':False} if out is None else result(out)
    if module=='schnorr' and request['op']=='verify':
        return {'ok':True,'valid':schnorr_verify(bytes.fromhex(request['message']),bytes.fromhex(request['public']),bytes.fromhex(request['signature']))}
    if module=='schnorr' and request['op']=='sign' and native is not None:
        out=native.sign(bytes.fromhex(request['message']),bytes.fromhex(request['secret']),bytes.fromhex(request['aux']))
        return {'ok':False} if out is None else result(out)
    raise ValueError('unsupported adapter operation')

def compare(request,actual,regressions,native=None):
    expected=reference(request,native)
    if actual==expected:return True
    path=Path(regressions);path.parent.mkdir(parents=True,exist_ok=True)
    record={'schema':'coax.secp-differential-regression.v1','request':request,'expected':expected,'actual':actual}
    with path.open('a') as stream:stream.write(json.dumps(record,sort_keys=True)+'\n')
    return False

def cases(seed,count):
    rng=random.Random(seed);out=[]
    edges=[0,1,2**26-1,2**26,2**32-1,2**32,2**64-1,P-1,P,N-1,N,2**256-1]
    for i in range(count):
        a=edges[i] if i<len(edges) else rng.getrandbits(256)
        b=edges[-1-i] if i<len(edges) else rng.getrandbits(256)
        for module in ('field','scalar'):
            for op in ('normalize','add','sub','mul','inverse'):
                out.append({'module':module,'op':op,'a':h(a),'b':h(b)})
        k=1+rng.randrange(N-1);peer=point_mul(1+rng.randrange(N-1),G)
        out.append({'module':'group','op':'mul','scalar':h(k),'point':encode_point(peer).hex()})
        out.append({'module':'ecdh','secret':h(k),'peer':h(peer[0])})
    with (ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv').open() as stream:
        for row in csv.DictReader(stream):
            out.append({'module':'schnorr','op':'verify','message':row['message'].lower(),'public':row['public key'].lower(),'signature':row['signature'].lower()})
            if row['secret key']:
                out.append({'module':'schnorr','op':'sign','message':row['message'].lower(),'secret':row['secret key'].lower(),'aux':row['aux_rand'].lower()})
    return out

def smoke(native,seed,count):
    requests=cases(seed,count);comparisons=0;failures=0
    # Field/scalar oracle checks arithmetic identities, not a nonexistent Coax
    # candidate; point/ECDH and signatures cross independent implementations.
    for request in requests:
        expected=reference(request,native);module=request['module']
        if module in ('field','scalar'):
            m=P if module=='field' else N;a=int(request['a'],16);b=int(request['b'],16)
            if request['op']=='mul':
                actual=result(h(sum(((a>>i)&1)*(b<<i) for i in range(256))%m))
            elif request['op']=='inverse':
                actual={'ok':False} if a%m==0 else result(h(pow(a,m-2,m)))
            else:
                # Distinct bounded carry/borrow/normalization identities; do
                # not count simply echoing the expected result as a check.
                value=a if request['op']=='normalize' else (a+b if request['op']=='add' else a-b)
                while value<0:value+=m
                while value>=m:value-=m
                actual=result(h(value))
        elif module=='group':
            data=native.point(bytes.fromhex(request['scalar']),bytes.fromhex(request['point']))
            actual={'ok':False} if data is None else result(data)
        elif module=='ecdh':
            data=native.ecdh(bytes.fromhex(request['secret']),bytes.fromhex(request['peer']))
            actual={'ok':False} if data is None else result(data)
        elif request['op']=='verify':
            actual={'ok':True,'valid':native.verify(bytes.fromhex(request['message']),bytes.fromhex(request['public']),bytes.fromhex(request['signature']))}
        else:
            data=native.sign(bytes.fromhex(request['message']),bytes.fromhex(request['secret']),bytes.fromhex(request['aux']))
            actual={'ok':False} if data is None else result(data)
        failures+=actual!=expected;comparisons+=1
    with (ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv').open() as stream:
        rows=list(csv.DictReader(stream))
    for row in rows:
        message=bytes.fromhex(row['message']);pk=bytes.fromhex(row['public key']);sig=bytes.fromhex(row['signature'])
        expected=row['verification result']=='TRUE'
        failures+=(native.verify(message,pk,sig)!=expected or schnorr_verify(message,pk,sig)!=expected);comparisons+=1
        if row['secret key']:
            failures+=native.sign(message,bytes.fromhex(row['secret key']),bytes.fromhex(row['aux_rand']))!=sig;comparisons+=1
    return {'mode':'oracle-plumbing-smoke','seed':seed,'generated_cases':len(requests),'comparisons':comparisons,'failures':failures,'candidate':None}

def candidate_run(native,command,seed,count,duration,regressions,timeout):
    deadline=time.monotonic()+duration if duration else None
    total=0;failures=0;rounds=0
    # One bounded JSON-line request per invocation prevents a candidate from
    # buffering forever or leaving a persistent deadlocked pipe. The adapter
    # may load its module once per request; long runs record this throughput.
    while True:
        for request in cases(seed+rounds,count):
            raw=(json.dumps(request,sort_keys=True)+'\n').encode()
            try:
                # POSIX test hosts only: regular-file outputs plus RLIMIT_FSIZE
                # cap output before it can accumulate in parent memory/disk.
                def limits():
                    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
                    resource.setrlimit(resource.RLIMIT_FSIZE,(1048576,1048576))
                with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
                    completed=subprocess.run(command,input=raw,stdout=stdout,stderr=stderr,timeout=timeout,check=False,preexec_fn=limits)
                    stdout.seek(0);data=stdout.read(1048577)
                if completed.returncode!=0 or len(data)>1048576:actual={'error':'candidate exit/output limit','exit':completed.returncode}
                else:actual=json.loads(data)
            except (subprocess.TimeoutExpired,ValueError):actual={'error':'candidate timeout or malformed response'}
            if not compare(request,actual,regressions,native):failures+=1
            total+=1
            if failures or (deadline and time.monotonic()>=deadline):break
        rounds+=1
        if failures or not deadline or time.monotonic()>=deadline:break
    return {'mode':'candidate-differential','seed':seed,'rounds':rounds,'comparisons':total,'failures':failures,'candidate':command}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--seed',type=int,default=20261028)
    p.add_argument('--count',type=int,default=32)
    p.add_argument('--seconds',type=int,default=0,help='explicit bounded milestone duration, e.g. 3600; not run by unit tests')
    p.add_argument('--timeout',type=float,default=5)
    p.add_argument('--library',type=Path,default=DEFAULT_LIBRARY)
    p.add_argument('--candidate-artifact',type=Path,action='append',default=[],help='additional candidate source/module/library bytes to identify in the report')
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--regressions',type=Path,default=HERE/'.cache/regressions.jsonl')
    p.add_argument('--candidate',nargs=argparse.REMAINDER)
    args=p.parse_args()
    if not 1<=args.count<=10000 or not 0<=args.seconds<=86400 or not 0<args.timeout<=60:p.error('outside bounded harness limits')
    candidate_files=[*args.candidate_artifact,*[Path(x) for x in (args.candidate or []) if Path(x).is_file()]]
    if args.candidate:
        executable=shutil.which(args.candidate[0])
        if executable:candidate_files.append(Path(executable))
    if any(not path.is_file() for path in candidate_files):p.error('missing candidate artifact')
    native=NativeOracle(args.library);start=time.monotonic()
    if args.candidate:value=candidate_run(native,args.candidate,args.seed,args.count,args.seconds,args.regressions,args.timeout)
    else:
        if args.seconds:p.error('--seconds requires a real --candidate; oracle smoke is not a milestone claim')
        value=smoke(native,args.seed,args.count)
    native.close()
    build=json.loads((args.library.parent/'build.json').read_text())
    value.update(schema='coax.secp-differential-report.v1',elapsed_seconds=time.monotonic()-start,build=build,
                 candidate_inputs={str(x.resolve()):hashlib.sha256(x.read_bytes()).hexdigest() for x in candidate_files},
                 harness_sha256={x.name:hashlib.sha256(x.read_bytes()).hexdigest() for x in [HERE/'oracle.py',HERE/'differential.py']},
                 vector_sha256=hashlib.sha256((ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv').read_bytes()).hexdigest(),
                 regressions_sha256=hashlib.sha256(args.regressions.read_bytes()).hexdigest() if args.regressions.is_file() else None)
    args.report.parent.mkdir(parents=True,exist_ok=True);args.report.write_text(json.dumps(value,indent=2)+'\n')
    print(json.dumps({k:value[k] for k in ('mode','comparisons','failures','elapsed_seconds')},sort_keys=True))
    raise SystemExit(1 if value['failures'] else 0)
