#!/usr/bin/env python3
"""Prepare an original routed packet from the chapter-95 admitted artifact.
Full-shard admission is reused, not redundantly rerun. Reads six bounded spans.
"""
import argparse,gzip,hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[2];D=R/'psionic/fixtures/deepseek'
def main():
    p=argparse.ArgumentParser();p.add_argument('--weights',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();pin=json.loads((D/'expert-original.json').read_text());assert a.weights.stat().st_size==pin['pin']['bytes'];a.out.mkdir(parents=True,exist_ok=True)
    header=json.loads(gzip.decompress((D/'expert-original-header.json.gz').read_bytes()));chunks=[];ranges=[]
    with a.weights.open('rb') as f:
        for role in ['w1','w3','w2']:
            for leaf in ['weight','scale']:
                name=f'layers.0.ffn.experts.0.{role}.{leaf}';b,e=header[name]['data_offsets'];f.seek(pin['payload']+b);raw=f.read(e-b);assert len(raw)==e-b;chunks.append(raw);ranges.append(dict(name=name,offset=pin['payload']+b,bytes=e-b,sha256=hashlib.sha256(raw).hexdigest()))
    packet=b''.join(chunks);assert len(packet)==18800640;(a.out/'packet.bin').write_bytes(packet)
    old=D/'expert-cpu-original/pair'
    for name,source in [('input.f32','routed-input.f32.gz'),('expected.f32','routed-reference-output.f32.gz')]:
        raw=gzip.decompress((old/source).read_bytes());assert len(raw)==20480;(a.out/name).write_bytes(raw)
    result=dict(schema=1,source_pin=pin['pin'],admission_reused_from='docs/verification/psionic-ch63-2026-10-29.md',scope='Six original byte intervals from the previously admitted unchanged local shard. Complete-shard hash was not redundantly rerun.',source_stat=dict(bytes=a.weights.stat().st_size,mtime_ns=a.weights.stat().st_mtime_ns),ranges=ranges,packet_sha256=hashlib.sha256(packet).hexdigest())
    (a.out/'source.json').write_text(json.dumps(result,indent=2)+'\n');print('Original packet:',len(packet),'bytes, six lossless original intervals;',result['packet_sha256'])
if __name__=='__main__':main()
