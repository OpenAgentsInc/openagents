#!/usr/bin/env python3
"""Build fixed original-model descriptors from pinned immutable source headers."""
import argparse,hashlib,importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/gptoss/original'
spec=importlib.util.spec_from_file_location('owned_fixture',ROOT/'psionic/tests/gptoss_fixture.py');fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
def shape(cfg,capacity=131072):return dict(layers=cfg['num_hidden_layers'],experts=cfg['num_experts'],selected=cfg['experts_per_token'],vocab=cfg['vocab_size'],width=cfg['hidden_size'],hidden=cfg['intermediate_size'],heads=cfg['num_attention_heads'],kv_heads=cfg['num_key_value_heads'],window=cfg['sliding_window'],capacity=capacity)
def shape_text(s):return 'GShape.{'+','.join(k+'='+str(v) for k,v in s.items())+'}'
def output(prefix,files,headers,s):
    artifacts=[];descriptors=[];lines=[]
    for index,f in enumerate(files):
        header=headers.get(f['name']);offset=0 if header is None else 8+len(header)
        artifacts.append(dict(**f,header=offset))
        if header is not None:
            for name,t in json.loads(header).items():
                assert name!='__metadata__' and t['dtype'] in ('BF16','U8') and len(t['shape'])<=4
                a,b=t['data_offsets'];descriptors.append(dict(name=name,dtype=0 if t['dtype']=='BF16' else 1,rank=len(t['shape']),dims=t['shape']+[0]*(4-len(t['shape'])),begin=offset+a,end=offset+b,file=index))
    assert len(descriptors)==15*s['layers']+3
    lines.append(prefix+'_SHAPE :: '+shape_text(s)+';')
    lines.append(prefix+f'_ARTIFACTS :: [{len(artifacts)}]GArtifact.[')
    for f in artifacts:lines.append('GArtifact.{name='+json.dumps(f['name'])+',bytes='+str(f['bytes'])+',sha='+json.dumps(f['sha256'])+',header='+str(f['header'])+'},')
    lines.append('];\n'+prefix+f'_SPECS :: [{len(descriptors)}]GSpec.[')
    for d in descriptors:lines.append('GSpec.{name='+json.dumps(d['name'])+',dtype='+str(d['dtype'])+',rank='+str(d['rank'])+',dims=[4]int.['+','.join(map(str,d['dims']))+'],begin='+str(d['begin'])+',end='+str(d['end'])+',file='+str(d['file'])+'},')
    lines.append('];')
    return '\n'.join(lines)+'\n'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--write',action='store_true');args=ap.parse_args();pins=json.loads((DEST/'pins.json').read_text());code='// Fixed offset profiles derived only from SHA-256-pinned original files.\n'
    for model,source in pins['sources'].items():
        cache=Path.home()/'.cache/psionic/gptoss-original'/model/source['revision'];files=source['files'];headers={}
        for f in files:
            if f['name'].endswith('.safetensors'):headers[f['name']]=(DEST/model/(Path(f['name']).name+'.header.json')).read_bytes()[:-1]
            else:
                data=((cache/f['name']) if args.write else (DEST/model/Path(f['name']).name)).read_bytes();assert len(data)==f['bytes'] and hashlib.sha256(data).hexdigest()==f['sha256'];target=DEST/model/Path(f['name']).name
                if args.write:target.write_bytes(data)
                assert target.read_bytes()==data
        cfg=json.loads((DEST/model/'config.json').read_text());assert cfg['hidden_size']==2880 and cfg['intermediate_size']==2880 and cfg['head_dim']==64 and cfg['swiglu_limit']==7.0 and cfg['rope_theta']==150000 and cfg['rope_scaling_factor']==32 and cfg['initial_context_length']==4096 and cfg['rope_ntk_beta']==32 and cfg['rope_ntk_alpha']==1
        assert cfg['num_hidden_layers']==(24 if model=='20b' else 36) and cfg['num_experts']==(32 if model=='20b' else 128)
        code+=output('G20' if model=='20b' else 'G120',files,headers,shape(cfg))
    data,header,_,_=fixture.build();tiny=[dict(name='tiny.safetensors',bytes=len(data),sha256=hashlib.sha256(data).hexdigest())];n=int.from_bytes(data[:8],'little')
    code+=output('GOWN',tiny,{'tiny.safetensors':data[8:8+n]},fixture.S)
    target=DEST/'targets.coax'
    if args.write:target.write_text(code)
    assert target.read_text()==code,'original profile descriptor drift'
    print('Fixed original profiles: 363 20b tensors, 543 120b tensors and 33 owned tiny tensors verify')
if __name__=='__main__':main()
