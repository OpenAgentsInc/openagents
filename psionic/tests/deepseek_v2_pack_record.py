#!/usr/bin/env python3
"""Author compact lossless evidence once after both original comparisons pass."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import struct
import sys
import numpy as np

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(Path(__file__).resolve().parent))
from deepseek_v2_reference import capture,compare
from deepseek_v2_record import xor


def reference_bytes(ref):
    out=bytearray(b'DV2RUN01'+struct.pack('<qqq',102400,110592,27))
    for pos,token in enumerate(ref['tokens']):
        out.extend(struct.pack('<qq',token,pos));out.extend(ref['logits'][pos].astype('<f4').tobytes());out.extend(ref['trace'][pos].astype('<f4').tobytes())
        for layer in range(27):
            out.extend(ref['ids'][pos,layer].astype('<i8').tobytes());out.extend(ref['weights'][pos,layer].astype('<f8').tobytes());out.extend(struct.pack('<d',ref['margins'][pos,layer]))
    return bytes(out)


def main():
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('destination',type=Path);a=p.parse_args();a.destination.mkdir(parents=True,exist_ok=False)
    candidate={m:(a.capture/f'{m}.bin').read_bytes() for m in ['compressed','expanded']};base=candidate['compressed'];files={};captures={}
    def save(name,raw):
        (a.destination/name).write_bytes(raw);files[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
    save('compressed-candidate.bin.gz',gzip.compress(base,mtime=0));save('expanded-candidate.xor.gz',gzip.compress(xor(base,candidate['expanded']),mtime=0))
    policy=(ROOT/'psionic/tests/v2-qualification-policy.json').read_bytes()
    cross=compare(capture(a.capture/'compressed.bin'),capture(a.capture/'expanded.bin'),json.loads(policy));assert cross['passed'];save('cross-cache-comparison.json',(json.dumps(cross,indent=2)+'\n').encode())
    for mode in ['compressed','expanded']:
        report=json.loads((a.capture/f'{mode}-comparison.json').read_text());assert report['passed'];reference=reference_bytes(np.load(a.capture/f'{mode}-reference.npz',allow_pickle=False))
        captures[mode]=dict(candidate_sha256=hashlib.sha256(candidate[mode]).hexdigest(),reference_sha256=hashlib.sha256(reference).hexdigest())
        save(f'{mode}-reference.xor.gz',gzip.compress(xor(candidate[mode],reference),mtime=0))
        for suffix in ['json','txt']:save(f'{mode}.{suffix}',(a.capture/f'{mode}.{suffix}').read_bytes())
        meta=json.loads((a.capture/f'{mode}.json').read_text());save(f'{mode}-generated.i64',struct.pack('<'+'q'*len(meta['generated']),*meta['generated']))
        save(f'{mode}-comparison.json',(a.capture/f'{mode}-comparison.json').read_bytes())
        save(f'{mode}-reference.log',(a.capture/f'reference-{mode}.log').read_bytes())
    save('build-identity.json',(a.capture/'build-identity.json').read_bytes());save('run.log',(a.capture/'run.log').read_bytes())
    rebuild={}
    for name,source in [('executed.c.gz',a.capture/'deepseek-v2.c'),('executed.h.gz',a.capture/'deepseek-v2.h'),('host.c.gz',ROOT/'psionic/tests/deepseek_v2_original.c'),('memory.c.gz',ROOT/'psionic/platform/deepseek-v2-memory.c'),('memory.h.gz',ROOT/'psionic/platform/deepseek-v2-memory.h'),('spans.h.gz',ROOT/'psionic/platform/spans.h')]:
        save(name,gzip.compress(source.read_bytes(),mtime=0));rebuild[name]=hashlib.sha256(source.read_bytes()).hexdigest()
    manifest=dict(schema=1,profile='deepseek-v2-lite-cpu-wide-v1',policy_sha256=hashlib.sha256(policy).hexdigest(),executed_c_path=str(a.capture/'deepseek-v2.c'),rebuild_sources=rebuild,captures=captures,files=files,scope='Complete original fixed Hi recipe in both cache forms. Wide CPU arithmetic; no Torch/BF16, GPU, throughput or general tokenizer parity claim.')
    (a.destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')


if __name__=='__main__':main()
