#!/usr/bin/env python3
"""First-order native timing probe for #89db with positive controls.

Explicit test tooling, not secret-use approval. Classes: the fixed valid
official vector-0 candidate against seeded random 64-byte candidates for the
same public message and key, interleaved. Fixtures: 0 the staged fixed-work
self-verifier, 1 a fixed-work control, 2 a deliberately leaky branch control
that must be detected, 3 the variable-time fast verifier the signer used.
One full verification costs about a millisecond, so the default sample count
stays below the timing model's 1M minimum (its status is then SHORT) and the
Welch t is computed here from the raw CT01 rows, raw and trimmed at the 95th
percentile of all observations. |t| > 10 is the detection threshold.
"""
import argparse
import json
import math
import platform
import statistics
import struct
import time
from pathlib import Path
import secp_field as base
from secp_field_mul import file_sha
ROOT=base.ROOT
GCC='gcc'

def welch(a,b):
 return (statistics.fmean(a)-statistics.fmean(b))/math.sqrt(statistics.variance(a)/len(a)+statistics.variance(b)/len(b))
def decode(path):
 data=Path(path).read_bytes()
 if data[:4]!=b'CT01':raise ValueError('raw version')
 groups=[[],[]]
 for group,ns in struct.iter_unpack('<BQ',data[12:]):groups[group].append(ns)
 cut=sorted(groups[0]+groups[1])[int(0.95*(len(groups[0])+len(groups[1])))]
 trimmed=[[x for x in g if x<cut] for g in groups]
 return {'n0':len(groups[0]),'n1':len(groups[1]),'median0':statistics.median(groups[0]),'median1':statistics.median(groups[1]),'t':welch(*groups),'t_trimmed_p95':welch(*trimmed),'p95_ns':cut}

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--samples',type=int,default=60000);parser.add_argument('--timeout',type=int,default=840);args=parser.parse_args()
 d=args.output.resolve();d.mkdir(parents=True,exist_ok=True)
 source=ROOT/'coax/tests/nostr/schnorr_self_verify_timing.coax';driver=ROOT/'coax/tests/nostr/schnorr_self_verify_timing.c';c=d/'timing.c';header=d/'schnorr_self_verify_timing.h'
 c.write_text(base.run(base.COMPILER,'c',source,'--library','--header',header,'--release','--run-steps','250000000').stdout)
 exe=d/'timing';flags=['-std=c11','-O2'];base.run(GCC,*flags,'-I'+str(d),c,driver,'-lm','-o',exe)
 report={'schema':'coax.ssv-timing.v1','scope':'first-order Welch observations, fixed valid candidate versus seeded random candidates, this native target on a shared loaded machine; no object/JIT audit, no address probe beyond the source schedule, not production-secret approval','compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'cc':base.run(GCC,'--version').stdout.splitlines()[0],'flags':flags,'target':platform.machine()+' '+platform.system()+' '+platform.release(),'generated_c_sha256':file_sha(c),'binary_sha256':file_sha(exe),'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in [source,driver,Path(__file__),ROOT/'coax/lib/crypto/schnorr_self_verify.coax']},'samples_per_fixture':args.samples,'threshold_abs_t':10,'runs':[]}
 for index,name in [(2,'branch-control'),(1,'fixed-control'),(3,'vartime-fast-verifier'),(0,'fixed-work-self-verifier')]:
  raw=d/(name+'.bin');start=time.time();observed=json.loads(base.run(exe,index,raw,args.samples,timeout=args.timeout).stdout)
  report['runs'].append({'fixture':name,'coax':observed,'python':decode(raw),'raw_sha256':file_sha(raw),'wall_s':round(time.time()-start,1)})
  (d/'report.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
 print(json.dumps(report,indent=2,sort_keys=True))
if __name__=='__main__':main()
