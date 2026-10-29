#!/usr/bin/env python3
"""Real sustained public-verification differential. Explicit test tooling only.

Default is an actual3600s after building/preflight, never simulated elapsed.
The separately labelled --preflight-seconds mode cannot satisfy acceptance.
"""
import argparse
from collections import Counter
import csv
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import random
import tempfile
import time
import schnorr_support as support
from oracle import NativeOracle,DEFAULT_LIBRARY,N,P
SEED=202610290340
MAX_CASES=4000000
MAX_MESSAGE=4096

def official():
 if support.sha(support.VECTORS)!=support.VECTOR_SHA:raise ValueError('BIP340 upstream corpus changed')
 return [{'message':bytes.fromhex(r['message']),'public':bytes.fromhex(r['public key']),'signature':bytes.fromhex(r['signature']),'expected':r['verification result']=='TRUE','index':int(r['index'])} for r in csv.DictReader(support.VECTORS.open(newline=''))]
def generated(rng,index,native,rows):
 category=index%12
 if category==9:
  row=rows[(index//12)%len(rows)];return row['message'],row['public'],row['signature'],'official'
 length=32 if category==1 else 0 if category==10 else rng.choice([0,1,17,32,100,128,255,512,1024,MAX_MESSAGE])
 message=rng.randbytes(length);key=rng.randrange(1,N).to_bytes(32,'big');aux=rng.randbytes(32);encoded=native.point(key)
 if encoded is None:raise ValueError('throwaway public-key creation failed')
 public=encoded[1:];signature=native.sign(message,key,aux)
 if signature is None:raise ValueError('throwaway signing oracle failed')
 name='valid'
 if category==2:
  data=bytearray(signature);at=rng.randrange(64);data[at]^=1<<rng.randrange(8);signature=bytes(data);name='signature-bit'
 elif category==3:
  data=bytearray(public);data[rng.randrange(32)]^=1<<rng.randrange(8);public=bytes(data);name='public-bit'
 elif category==4:message=message+b'\x01';name='changed-message'
 elif category==5:signature=P.to_bytes(32,'big')+signature[32:];name='r-boundary'
 elif category==6:signature=signature[:32]+N.to_bytes(32,'big');name='s-boundary'
 elif category==7:signature=signature[:63];name='short-signature'
 elif category==8:public=public+b'\0';name='long-public'
 elif category==11:signature=rng.randbytes(64);name='random-signature'
 return message,public,signature,name

def write_report(path,report):
 temp=path.with_suffix(path.suffix+'.tmp');temp.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');temp.replace(path)
def summary(samples):
 if not samples:return {}
 values=sorted(samples)
 return {name:values[min(len(values)-1,int((len(values)-1)*rank))] for name,rank in [('p50',.5),('p95',.95),('p99',.99)]}

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--seconds',type=int,default=3600);parser.add_argument('--preflight-seconds',type=int);parser.add_argument('--report',type=Path,required=True);parser.add_argument('--regressions',type=Path,required=True);args=parser.parse_args()
 acceptance=args.preflight_seconds is None;duration=args.seconds if acceptance else args.preflight_seconds
 if acceptance and not 3600<=duration<=7200:parser.error('acceptance duration must be3600..7200 actual seconds')
 if not acceptance and not 1<=duration<=60:parser.error('preflight duration must be1..60 seconds and is not acceptance')
 args.report.parent.mkdir(parents=True,exist_ok=True);args.regressions.parent.mkdir(parents=True,exist_ok=True)
 if args.regressions.exists():raise ValueError('refuse to overwrite an existing regression corpus')
 pins=support.source_pins();pins[str(Path(__file__).resolve().relative_to(support.ROOT))]=support.sha(Path(__file__))
 rows=official();native=NativeOracle(DEFAULT_LIBRARY);rng=random.Random(SEED);corpus=hashlib.sha256();categories=Counter();truth=Counter();samples={};timing={};count=0;nostr_count=0
 report={'schema':'coax.schnorr-hour.v1','state':'building','acceptance_hour':acceptance,'requested_seconds':duration,'seed':SEED,'scope':'public BIP340/general-byte verification correctness; no signing, constant-time, erasure, physical-device or production-secret approval','sources':pins,'check_inputs_at_build':support.check_inputs(),'official_sha256':support.VECTOR_SHA,'official_rows':len(rows),'official_message_lengths':sorted({len(r['message']) for r in rows}),'oracle_source_commit':json.loads((support.ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit'],'oracle_artifact_sha256':support.sha(DEFAULT_LIBRARY),'compiler':support.base.run(support.base.COMPILER,'version').stdout.strip(),'compiler_sha256':support.sha(support.base.COMPILER),'native_compiler':support.base.run('cc','--version').stdout.splitlines()[0],'native_target':support.base.run('cc','-dumpmachine').stdout.strip(),'budgets':{'max_cases':MAX_CASES,'max_generated_message_bytes':MAX_MESSAGE+1,'max_temporary_bytes':33554432,'max_samples_per_profile':4096,'checkpoint_seconds':30},'regressions_path':str(args.regressions),'timing_scope':'ctypes C call plus FFI; excludes argument copying and borrowed-input comparisons'}
 write_report(args.report,report)
 with tempfile.TemporaryDirectory(prefix='coax-schnorr-hour-') as work:
  libs,artifacts=support.build(work);report['artifacts']=artifacts
  for label,lib in libs:
   for row in rows:
    if native.verify(row['message'],row['public'],row['signature'])!=row['expected'] or support.verify(lib,row['message'],row['public'],row['signature'])!=row['expected'] or support.verify(lib,row['message'],row['public'],row['signature'],nostr=True)!=(row['expected'] and len(row['message'])==32):raise AssertionError('official verification preflight mismatch')
   samples[label]=[];timing[label]={'general_calls':0,'nostr_calls':0,'seconds':0.,'maximum_seconds':0.}
  support.check_pins(pins);start=time.monotonic();last=start;report.update({'state':'running','started_utc':datetime.now(timezone.utc).isoformat()});write_report(args.report,report)
  print(json.dumps({'state':'running','acceptance_hour':acceptance,'duration_seconds':duration,'official_rows':len(rows)}),flush=True)
  args.regressions.write_text('')
  try:
   while time.monotonic()-start<duration:
    if count>=MAX_CASES:raise RuntimeError('bounded case budget exhausted before requested duration')
    message,public,signature,category=generated(rng,count,native,rows);expected=native.verify(message,public,signature)
    corpus.update(len(message).to_bytes(4,'big')+len(public).to_bytes(2,'big')+len(signature).to_bytes(2,'big')+message+public+signature);categories[category]+=1;truth['valid' if expected else 'invalid']+=1
    for label,lib in libs:
     actual,elapsed=support.verify_timed(lib,message,public,signature)
     if actual!=expected:
      failure={'index':count,'seed':SEED,'mode':label,'entry':'general','category':category,'message':message.hex(),'public':public.hex(),'signature':signature.hex(),'expected':expected,'actual':actual};args.regressions.write_text(json.dumps(failure)+'\n');raise AssertionError('general differential mismatch; retained regression')
     timing[label]['general_calls']+=1;timing[label]['seconds']+=elapsed;timing[label]['maximum_seconds']=max(timing[label]['maximum_seconds'],elapsed)
     if len(samples[label])<4096:samples[label].append(elapsed)
     if count%17==0:
      actual,elapsed=support.verify_timed(lib,message,public,signature,nostr=True)
      if actual!=(expected and len(message)==32):
       failure={'index':count,'seed':SEED,'mode':label,'entry':'nostr','category':category,'message':message.hex(),'public':public.hex(),'signature':signature.hex(),'expected':expected and len(message)==32,'actual':actual};args.regressions.write_text(json.dumps(failure)+'\n');raise AssertionError('Nostr differential mismatch; retained regression')
      timing[label]['nostr_calls']+=1;timing[label]['seconds']+=elapsed;timing[label]['maximum_seconds']=max(timing[label]['maximum_seconds'],elapsed)
    if count%17==0:nostr_count+=1
    count+=1;now=time.monotonic()
    if now-last>=30:
     report.update({'elapsed_seconds':now-start,'cases':count,'nostr_cases':nostr_count,'categories':dict(categories),'expected':dict(truth),'corpus_sha256':corpus.hexdigest(),'native_profiles':timing});write_report(args.report,report);print(json.dumps({'state':'running','elapsed_seconds':round(now-start,1),'cases':count,'valid':truth['valid'],'invalid':truth['invalid']}),flush=True);last=now
   elapsed=time.monotonic()-start
   if acceptance and elapsed<3600:raise AssertionError('actual differential duration is below one hour')
   support.check_pins(pins)
   if support.sha(support.base.COMPILER)!=report['compiler_sha256']:raise ValueError('compiler binary changed during hour')
   used=sum(p.stat().st_size for p in Path(work).rglob('*') if p.is_file())
   if used>33554432:raise ValueError('temporary artifact budget exceeded')
   report.update({'state':'passed','elapsed_seconds':elapsed,'cases':count,'nostr_cases':nostr_count,'categories':dict(categories),'expected':dict(truth),'corpus_sha256':corpus.hexdigest(),'native_profiles':timing,'timing_first4096_general_quantiles':{label:summary(values) for label,values in samples.items()},'temporary_bytes':used,'regression_sha256':support.sha(args.regressions),'failures':0,'finished_utc':datetime.now(timezone.utc).isoformat()});write_report(args.report,report);print(json.dumps({'state':'passed','acceptance_hour':acceptance,'elapsed_seconds':elapsed,'cases':count,'failures':0}),flush=True)
  except BaseException as error:
   report.update({'state':'failed','elapsed_seconds':time.monotonic()-start,'cases':count,'error':str(error)});write_report(args.report,report);raise
  finally:native.close()
if __name__=='__main__':main()
