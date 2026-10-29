#!/usr/bin/env python3
"""Pack inert reviewed V2-Lite header intervals into Coax; never run remote code."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from deepseek_v2_fixture import tensor_shapes,shape_code
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek/v2-original'
SHAPE=dict(layers=27,width=2048,vocab=102400,dense_layers=1,dense_hidden=10944,experts=64,selected=6,expert_hidden=1408,shared=2,
           attention=dict(heads=16,nope=128,rope=64,value=128,latent=512,capacity=128))


def main():
    p=argparse.ArgumentParser();p.add_argument('--original',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
    pins=json.loads((DEST/'pins.json').read_text())['files'];weights=[x for x in pins if x['name'].endswith('.safetensors')];other=[x for x in pins if not x['name'].endswith('.safetensors')];pins=weights+other
    config=json.loads((DEST/'config.json').read_text())
    profile=dict(hidden_size=2048,vocab_size=102400,num_hidden_layers=27,first_k_dense_replace=1,intermediate_size=10944,n_routed_experts=64,num_experts_per_tok=6,moe_intermediate_size=1408,n_shared_experts=2,num_attention_heads=16,qk_nope_head_dim=128,qk_rope_head_dim=64,v_head_dim=128,kv_lora_rank=512,q_lora_rank=None,rms_norm_eps=1e-6,rope_theta=10000,max_position_embeddings=163840,norm_topk_prob=False,routed_scaling_factor=1.0,scoring_func='softmax',topk_method='greedy',n_group=1,topk_group=1,attention_bias=False,hidden_act='silu',tie_word_embeddings=False,moe_layer_freq=1,bos_token_id=100000,eos_token_id=100001)
    assert all(config[key]==value for key,value in profile.items())
    assert config['rope_scaling']==dict(beta_fast=32,beta_slow=1,factor=40,mscale=.707,mscale_all_dim=.707,original_max_position_embeddings=4096,type='yarn')
    if a.original:
        headers=[]
        for pin in weights:
            file=a.original/pin['name'];assert file.stat().st_size==pin['bytes']
            with file.open('rb') as f:n=struct.unpack('<Q',f.read(8))[0];raw=f.read(n)
            headers.append(dict(name=pin['name'],payload=8+n,header=json.loads(raw)))
    else:headers=json.loads((DEST/'headers.json').read_text())
    expected=tensor_shapes(SHAPE);found={};tensors=[];payloads=[]
    for file,item in enumerate(headers):
        assert item['name']==weights[file]['name'];start=item['payload'];payloads.append(start);cursor=0
        entries=sorted(((name,h) for name,h in item['header'].items() if name!='__metadata__'),key=lambda x:x[1]['data_offsets'][0])
        for name,h in entries:
            assert name in expected and name not in found and h['dtype']=='BF16' and h['shape']==expected[name],name
            lo,hi=h['data_offsets'];shape=h['shape'];assert lo==cursor and hi-lo==2*__import__('math').prod(shape);cursor=hi;found[name]=file
            tensors.append(dict(file=file,name=name,rank=len(shape),rows=shape[0],cols=shape[1] if len(shape)==2 else 1,begin=start+lo,end=start+hi))
        assert start+cursor==weights[file]['bytes']
    assert set(found)==set(expected)
    index=json.loads((DEST/'model.safetensors.index.json').read_text())['weight_map'];assert set(index)==set(found)
    for name,file in found.items():assert index[name]==weights[file]['name']
    for pin in other:
        data=(DEST/pin['name']).read_bytes();assert len(data)==pin['bytes'] and hashlib.sha256(data).hexdigest()==pin['sha256'];payloads.append(0)
    code='// Pinned original V2-Lite metadata DATA. Remote source is never executed.\n'
    code+='DV_ORIGINAL_SHAPE :: '+shape_code(SHAPE)+';\n'
    code+='DV_ORIGINAL_PINS :: ['+str(len(pins))+']DsArtifact.['+','.join('DsArtifact.{name='+json.dumps(x['name'])+',bytes='+str(x['bytes'])+',sha='+json.dumps(x['sha256'])+'}' for x in pins)+'];\n'
    code+='DV_ORIGINAL_PAYLOAD :: ['+str(len(payloads))+']int.['+','.join(map(str,payloads))+'];\n'
    code+='DV_ORIGINAL_TENSORS :: ['+str(len(tensors))+']DvTensor.['+','.join('DvTensor.{'+','.join(k+'='+json.dumps(v) for k,v in t.items())+'}' for t in tensors)+'];\n'
    outputs={'headers.json':(json.dumps(headers,separators=(',',':'))+'\n').encode(),'targets.coax':code.encode()}
    for name,data in outputs.items():
        if a.write:(DEST/name).write_bytes(data)
        else:assert (DEST/name).read_bytes()==data,f'stale {name}'
    print(f'V2 original metadata: {len(tensors)} exact BF16 intervals in {len(headers)} complete shard payloads; {len(pins)} artifact pins')


if __name__=='__main__':main()
