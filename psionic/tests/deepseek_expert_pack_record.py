#!/usr/bin/env python3
"""Author small lossless original-component evidence, never redistribute weights."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]


def main():
    p=argparse.ArgumentParser();p.add_argument('pair',type=Path);p.add_argument('mixture',type=Path);p.add_argument('destination',type=Path);a=p.parse_args();a.destination.mkdir(parents=True,exist_ok=False);files={}
    def save(name,data):
        path=a.destination/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data);files[name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    for label,source in [('pair',a.pair),('mixture',a.mixture)]:
        report=json.loads((source/'comparison.json').read_text());assert report.get('passed',all(v['passed'] for v in report.get('experts',{}).values()))
        for file in sorted(source.iterdir()):
            if file.suffix in ['.f32','.f64']:save(label+'/'+file.name+'.gz',gzip.compress(file.read_bytes(),mtime=0))
            elif file.suffix in ['.json','.i64','.log']:save(label+'/'+file.name,file.read_bytes())
        for name,path in [('executed.c',source/'deepseek-expert.c'),('executed.h',source/'deepseek-expert.h'),('host.c',source/'executed-host.c' if label=='pair' else ROOT/'psionic/tests/deepseek_expert_original.c'),('reference.py',source/'executed-reference.py' if label=='pair' else ROOT/'psionic/tests/deepseek_expert_reference.py')]:save(label+'/'+name+'.gz',gzip.compress(path.read_bytes(),mtime=0))
    for name,path in [('oracle.py','psionic/tests/deepseek_expert_oracle.py'),('memory.c','psionic/platform/deepseek-expert-memory.c'),('memory.h','psionic/platform/deepseek-expert-memory.h'),('spans.h','psionic/platform/spans.h')]:save(name+'.gz',gzip.compress((ROOT/path).read_bytes(),mtime=0))
    manifest=dict(schema=1,profile='deepseek-expert-cpu-wide-v1',pair_policy_sha256=hashlib.sha256((ROOT/'psionic/tests/expert-qualification-policy.json').read_bytes()).hexdigest(),mixture_policy_sha256=hashlib.sha256((ROOT/'psionic/tests/expert-mixture-policy.json').read_bytes()).hexdigest(),pair_executed_c_path=str(a.pair/'deepseek-expert.c'),mixture_executed_c_path=str(a.mixture/'deepseek-expert.c'),files=files,scope='Completely authenticated original shard; two individual original FFNs and a complete 384/6 layer-0 MoE on one owned vector; CPU-wide profile, independent numerical reference, no whole-model/device/BF16 parity claim')
    (a.destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')


if __name__=='__main__':main()
