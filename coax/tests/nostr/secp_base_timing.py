#!/usr/bin/env python3
"""Declared first-order timing profile, not independent security approval."""
import argparse
import json
import os
from pathlib import Path
import platform
import sys
import secp_field as base
from secp_field_mul import file_sha
ROOT=base.ROOT
sys.path.insert(0,str(ROOT/'tools/coax-nostr/timing'))
from run import verify

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'coax/bin/nostr-fixed-base-timing');args=parser.parse_args();directory=args.output.resolve();directory.mkdir(parents=True,exist_ok=True)
 source=ROOT/'coax/tests/nostr/secp_base_timing.coax';driver=ROOT/'coax/tests/nostr/secp_base_timing.c';c=directory/'timing.c';header=directory/'secp_base_timing.h'
 c.write_text(base.run(base.COMPILER,'c',source,'--library','--header',header,'--release','--run-steps','250000000').stdout)
 exe=directory/'timing';base.run('cc','-std=c11','-O2','-I'+str(directory),c,driver,'-lm','-o',exe)
 paths=[source,driver,Path(__file__),ROOT/'tools/coax-nostr/timing/model.coax',ROOT/'tools/coax-nostr/timing/fixtures.coax',ROOT/'tools/coax-nostr/timing/run.py']+[ROOT/'coax/lib/crypto'/n for n in ['secp_mult_base.coax','secp_mult_base_web.coax','secp_mult_base_native.coax','secp_group.coax','secp_field.coax','secp_scalar.coax','bytes.coax']]
 report={'schema':'coax.secp-fixed-base-timing.v1','scope':'first-order Welch observations of public fixed-zero versus seeded256-bit inputs on this named native target only; no source guard, object/JIT, independent audit, erasure or production-secret admission','compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in paths},'build_inputs':{'run_steps':250000000,'release':True,'native_flags':['-std=c11','-O2']},'generated_c_sha256':file_sha(c),'object_sha256':file_sha(exe),'target':{'architecture':platform.machine(),'os':platform.system()+' '+platform.release(),'cpu':base.run('sysctl','-n','machdep.cpu.brand_string').stdout.strip(),'compiler':base.run('cc','--version').stdout.splitlines()[0],'environment':'normal shared developer machine; no affinity/governor/process changes'},'samples_per_fixture':1000000,'seed':20261029,'threshold_abs_t':10,'raw_retention':'CT01 raw rows and exact native object in ignored output directory; hashes retained here','runs':[]}
 names=['web','native','fixed-control','branch-control']
 for index,name in enumerate(names):
  raw=directory/(name+'.bin');observed=json.loads(base.run(exe,index,raw,timeout=1800).stdout);independent=verify(raw,observed)
  report['runs'].append({'fixture':name,'coax':observed,'independent':independent,'raw_sha256':file_sha(raw),'raw_file':raw.name})
  (directory/'report.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
 report['artifact_bytes']=sum(p.stat().st_size for p in directory.iterdir() if p.is_file());(directory/'report.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
 if report['artifact_bytes']>128*1024*1024:raise AssertionError('artifact budget128MiB exceeded')
 for row in report['runs']:
  expected=1 if row['fixture']=='branch-control' else 2
  if row['coax']['status']!=expected:raise AssertionError('retained timing alarm/control failure: '+row['fixture'])
 print(directory/'report.json')
if __name__=='__main__':main()
