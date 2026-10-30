#!/usr/bin/env python3
"""Retain reviewed original shard/header DATA and borrow exact selected tensors."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek'
PIN=dict(repo='deepseek-ai/DeepSeek-V4.1-Flash-1017',revision='1bae0e1088993b9b82de08f59de5be5a34e42170',name='model-00003-of-00048.safetensors',bytes=7389759032,sha256='41611994fc9b27829b757d72257570d65a976fe65b8c6df8641adcd139d65ded')


def main():
    p=argparse.ArgumentParser();p.add_argument('--original',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
    if a.original:
        assert a.original.stat().st_size==PIN['bytes']
        with a.original.open('rb') as f:n=struct.unpack('<Q',f.read(8))[0];raw=f.read(n)
        header=json.loads(raw);cursor=0;entries=sorted((v for k,v in header.items() if k!='__metadata__'),key=lambda v:v['data_offsets'][0])
        for h in entries:
            lo,hi=h['data_offsets'];assert lo==cursor and hi>lo;cursor=hi
        assert cursor+n+8==PIN['bytes'];tensors={}
        for prefix in [*[f'layers.0.ffn.experts.{i}.' for i in range(384)],'layers.0.ffn.shared_experts.']:
            for branch in ['w1','w3','w2']:
                for suffix in ['weight','scale']:name=prefix+branch+'.'+suffix;tensors[name]=header[name]
        for suffix in ['weight','bias']:name='layers.0.ffn.gate.'+suffix;tensors[name]=header[name]
        record=dict(schema=1,pin=PIN,payload=n+8,raw_header_sha256=hashlib.sha256(raw).hexdigest(),whole_header_tensors=len(entries),reviewed=tensors)
    else:
        record=json.loads((DEST/'expert-original.json').read_text());assert record['pin']==PIN;raw=gzip.decompress((DEST/'expert-original-header.json.gz').read_bytes());header=json.loads(raw)
    assert hashlib.sha256(raw).hexdigest()==record['raw_header_sha256'] and len(raw)+8==record['payload']
    entries=sorted((v for k,v in header.items() if k!='__metadata__'),key=lambda v:v['data_offsets'][0]);assert len(entries)==record['whole_header_tensors'];cursor=0
    widths={'F32':4,'BF16':2,'F8_E4M3':1,'F8_E8M0':1,'I8':1}
    for h in entries:
        lo,hi=h['data_offsets'];assert lo==cursor and hi-lo==__import__('math').prod(h['shape'])*widths[h['dtype']];cursor=hi
    assert cursor+record['payload']==PIN['bytes']
    for name,h in record['reviewed'].items():assert header[name]==h,name
    def family(prefix,shared):
        matrices=[]
        for slot,branch in enumerate(['w1','w3','w2']):
            weight=record['reviewed'][prefix+branch+'.weight'];scale=record['reviewed'][prefix+branch+'.scale'];rows,cols=weight['shape'];cols*=1 if shared else 2
            assert [rows,cols]==([5120,2304] if slot==2 else [2304,5120])
            assert weight['dtype']==('F8_E4M3' if shared else 'I8') and scale['dtype']=='F8_E8M0'
            assert scale['shape']==[(rows+31)//32 if shared else rows,cols//32]
            lo,hi=weight['data_offsets'];sl,sh=scale['data_offsets'];assert hi-lo==rows*cols//(1 if shared else 2)
            assert sh-sl==__import__('math').prod(scale['shape'])
            matrices.append(dict(kind='fp8' if shared else 'fp4',rows=rows,columns=cols,data_begin=record['payload']+lo,data_end=record['payload']+hi,scale_begin=record['payload']+sl,scale_end=record['payload']+sh))
        return matrices
    matrices=family('layers.0.ffn.experts.0.',False)+family('layers.0.ffn.shared_experts.',True)
    all_matrices=[m for i in range(384) for m in family(f'layers.0.ffn.experts.{i}.',False)]
    code='// Inert original tensor intervals, derived from the completely hashed shard.\n'
    code+='DeSourceMatrix :: struct {kind:DeKind;rows:int;columns:int;data_begin:int;data_end:int;scale_begin:int;scale_end:int;}\n'
    code+='DE_ORIGINAL_PIN :: DsArtifact.{name='+json.dumps(PIN['name'])+',bytes='+str(PIN['bytes'])+',sha='+json.dumps(PIN['sha256'])+'};\n'
    code+='DE_ORIGINAL_MATRICES :: [6]DeSourceMatrix.['+','.join('DeSourceMatrix.{'+','.join(k+'='+(('DeKind.'+v) if k=='kind' else str(v)) for k,v in m.items())+'}' for m in matrices)+'];\n'
    code+='DE_ORIGINAL_ALL :: [1152]DeSourceMatrix.['+','.join('DeSourceMatrix.{'+','.join(k+'='+(('DeKind.'+v) if k=='kind' else str(v)) for k,v in m.items())+'}' for m in all_matrices)+'];\n'
    for suffix,shape,dtype in [('weight',[384,5120],'BF16'),('bias',[384],'F32')]:
        h=record['reviewed']['layers.0.ffn.gate.'+suffix];assert h['shape']==shape and h['dtype']==dtype;lo,hi=h['data_offsets'];code+=f'DE_ORIGINAL_GATE_{suffix.upper()} :: [2]int.[{record["payload"]+lo},{record["payload"]+hi}];\n'
    for name,data in {'expert-original.json':(json.dumps(record,indent=2)+'\n').encode(),'expert-original-targets.coax':code.encode(),'expert-original-header.json.gz':gzip.compress(raw,mtime=0)}.items():
        if a.write:(DEST/name).write_bytes(data)
        else:assert (DEST/name).read_bytes()==data,'stale '+name
    print('Original expert DATA:',record['whole_header_tensors'],'header entries cover the full payload;',len(all_matrices)+3,'reviewed FP4/FP8 matrices and the original BF16/F32 gate')


if __name__=='__main__':main()
