#!/usr/bin/env python3
"""Owned complete V2 BF16 models and independent cache-free prefix oracle."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[2]
S=dict(layers=2,width=4,vocab=9,dense_layers=1,dense_hidden=5,experts=3,selected=2,expert_hidden=3,shared=2,
       attention=dict(heads=2,nope=2,rope=2,value=2,latent=3,capacity=5))
SLOTS=['input_layernorm.weight','post_attention_layernorm.weight','self_attn.q_proj.weight','self_attn.kv_a_proj_with_mqa.weight','self_attn.kv_a_layernorm.weight','self_attn.kv_b_proj.weight','self_attn.o_proj.weight','mlp.gate.weight']
FFN=['gate_proj.weight','up_proj.weight','down_proj.weight']
TOKENS=[1,4,2,7,5]


def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]


def tensor_shapes(s):
    d,v=s['width'],s['vocab'];a=s['attention'];h=a['heads'];q=h*(a['nope']+a['rope']);z=a['latent'];r=a['rope']
    result={'model.embed_tokens.weight':[v,d],'model.norm.weight':[d],'lm_head.weight':[v,d]}
    def expert(prefix,hidden):
        for name,shape in zip(FFN,[[hidden,d],[hidden,d],[d,hidden]]):result[prefix+name]=shape
    for layer in range(s['layers']):
        p=f'model.layers.{layer}.'
        for name,shape in zip(SLOTS[:7],[[d],[d],[q,d],[z+r,d],[z],[h*(a['nope']+a['value']),z],[d,h*a['value']]]):result[p+name]=shape
        if layer<s['dense_layers']:expert(p+'mlp.',s['dense_hidden'])
        else:
            result[p+SLOTS[7]]=[s['experts'],d]
            for i in range(s['experts']):expert(p+f'mlp.experts.{i}.',s['expert_hidden'])
            expert(p+'mlp.shared_experts.',s['shared']*s['expert_hidden'])
    return result


def coefficients(dim,position):
    lo=max(math.floor(dim*math.log(4096/(32*2*math.pi))/(2*math.log(10000))),0)
    hi=min(math.ceil(dim*math.log(4096/(2*math.pi))/(2*math.log(10000))),dim-1)
    freq=[]
    for i in range(dim//2):
        ramp=min(max((i-lo)/(hi-lo),0),1);freq.append(10000**(-2*i/dim)*(1-ramp+ramp/40))
    return [f32(math.cos(position*f)) for f in freq]+[f32(math.sin(position*f)) for f in freq]


def reference(tensors,tokens,s,mode):
    d=s['width'];a=s['attention'];heads,nope,rope,value,latent=[a[k] for k in ['heads','nope','rope','value','latent']];qdim=nope+rope
    def matrix(name,x):
        w=tensors[name];n=len(x);return [f32(math.fsum(w[i*n+j]*x[j] for j in range(n))) for i in range(len(w)//n)]
    def norm(x,name):
        den=math.sqrt(math.fsum(v*v for v in x)/len(x)+1e-6);return [f32(v/den*w) for v,w in zip(x,tensors[name])]
    def rotate(x,position):
        c=coefficients(rope,position);pairs=list(zip(x[::2],x[1::2]))
        return [f32(av*c[i]-bv*c[rope//2+i]) for i,(av,bv) in enumerate(pairs)]+[f32(bv*c[i]+av*c[rope//2+i]) for i,(av,bv) in enumerate(pairs)]
    def ffn(x,prefix):
        gate=matrix(prefix+FFN[0],x);up=matrix(prefix+FFN[1],x)
        activation=[f32(g*(math.exp(-abs(g)) if g<0 else 1)/(1+math.exp(-abs(g)))*u) for g,u in zip(gate,up)]
        return matrix(prefix+FFN[2],activation)
    x=[tensors['model.embed_tokens.weight'][token*d:(token+1)*d] for token in tokens];trace=[];routes=[];cache=[]
    scale=(1+0.1*0.707*math.log(40))**2/math.sqrt(qdim)
    for layer in range(s['layers']):
        p=f'model.layers.{layer}.';normal=[norm(row,p+SLOTS[0]) for row in x];queries=[matrix(p+SLOTS[2],row) for row in normal];kv=[matrix(p+SLOTS[3],row) for row in normal]
        z=[norm(row[:latent],p+SLOTS[4]) for row in kv];key=[rotate(row[latent:],pos) for pos,row in enumerate(kv)]
        for pos,q in enumerate(queries):
            for h in range(heads):q[h*qdim+nope:(h+1)*qdim]=rotate(q[h*qdim+nope:(h+1)*qdim],pos)
        expanded=[matrix(p+SLOTS[5],row) for row in z];weights=tensors[p+SLOTS[5]];attended=[]
        for pos,q in enumerate(queries):
            output=[]
            for h in range(heads):
                wk=[weights[(h*(nope+value)+i)*latent:(h*(nope+value)+i+1)*latent] for i in range(nope+value)]
                absorbed=[f32(math.fsum(q[h*qdim+i]*wk[i][j] for i in range(nope))) for j in range(latent)]
                scores=[]
                for past in range(pos+1):
                    left,right=(absorbed,z[past]) if mode=='compressed' else (q[h*qdim:h*qdim+nope],expanded[past][h*(nope+value):h*(nope+value)+nope])
                    scores.append(math.fsum([*(v*w for v,w in zip(left,right)),*(q[h*qdim+nope+j]*key[past][j] for j in range(rope))])*scale)
                e=[math.exp(v-max(scores)) for v in scores];den=math.fsum(e)
                if mode=='compressed':
                    context=[f32(math.fsum(e[i]*z[i][j] for i in range(pos+1))/den) for j in range(latent)]
                    output.extend(f32(math.fsum(w*v for w,v in zip(wk[nope+j],context))) for j in range(value))
                else:output.extend(f32(math.fsum(e[i]*expanded[i][h*(nope+value)+nope+j] for i in range(pos+1))/den) for j in range(value))
            attended.append(output)
        residual=[[f32(v+w) for v,w in zip(row,matrix(p+SLOTS[6],att))] for row,att in zip(x,attended)];normal=[norm(row,p+SLOTS[1]) for row in residual];new=[];layer_routes=[]
        for row,normed in zip(residual,normal):
            if layer<s['dense_layers']:combined=ffn(normed,p+'mlp.');route=None
            else:
                logits=matrix(p+SLOTS[7],normed);exp=[math.exp(v-max(logits)) for v in logits];den=sum(exp);scores=[v/den for v in exp];order=sorted(range(s['experts']),key=lambda i:(-scores[i],i));ids=order[:s['selected']];parts=[ffn(normed,p+f'mlp.experts.{i}.') for i in ids]
                combined=[f32(math.fsum(scores[expert]*out[j] for expert,out in zip(ids,parts))) for j in range(d)];shared=ffn(normed,p+'mlp.shared_experts.');combined=[f32(v+w) for v,w in zip(combined,shared)]
                runner=order[s['selected']] if len(order)>s['selected'] else -1
                route=dict(ids=ids,weights=[scores[i] for i in ids],excluded=runner,margin=scores[ids[-1]]-scores[runner] if runner>=0 else 0,logits=logits)
            new.append([f32(v+w) for v,w in zip(row,combined)]);layer_routes.append(route)
        trace.extend(residual[-1]);trace.extend(new[-1]);routes.append(layer_routes[-1]);cache.append([(z[i] if mode=='compressed' else expanded[i])+key[i] for i in range(len(tokens))]);x=new
    return dict(logits=matrix('lm_head.weight',norm(x[-1],'model.norm.weight')),trace=trace,routes=routes,cache=cache)


def fixture(s):
    tensors={};header={};payload=bytearray()
    for index,(name,shape) in enumerate(tensor_shapes(s).items()):
        n=math.prod(shape);norm='layernorm' in name or name=='model.norm.weight'
        values=[1+((i+index)%3)/8 if norm else ((i*7+index*11)%23-11)/32 for i in range(n)]
        raw=b''.join(struct.pack('<H',struct.unpack('<I',struct.pack('<f',v))[0]>>16) for v in values)
        tensors[name]=[struct.unpack('<f',b'\0\0'+raw[i:i+2])[0] for i in range(0,len(raw),2)]
        header[name]=dict(dtype='BF16',shape=shape,data_offsets=[len(payload),len(payload)+len(raw)]);payload.extend(raw)
    encoded=json.dumps(header,separators=(',',':')).encode();encoded+=b' '*((-len(encoded))%8);begin=8+len(encoded);data=struct.pack('<Q',len(encoded))+encoded+payload
    refs={mode:[reference(tensors,TOKENS[:t+1],s,mode) for t in range(len(TOKENS))] for mode in ['compressed','expanded']}
    return data,header,begin,refs


def shape_code(s):
    return 'DvShape.{'+','.join(k+'='+str(v) for k,v in s.items() if k!='attention')+',attention=MlShape.{'+','.join(k+'='+str(v) for k,v in s['attention'].items())+'}}'


def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--wide',action='store_true');p.add_argument('--header',action='store_true');args=p.parse_args();s=S.copy();s['attention']=S['attention'].copy()
    label='tiny'
    if args.wide:label='wide';s.update(layers=3,width=6,vocab=11,dense_hidden=7,experts=4,selected=3,expert_hidden=5,shared=1);s['attention'].update(heads=3,nope=3,rope=4,value=2,latent=4)
    dest=ROOT/'psionic/fixtures/deepseek'/('v2-'+label);dest.mkdir(exist_ok=True);data,header,begin,refs=fixture(s);prefix='DVT' if not args.wide else 'DVW';code='// Owned complete V2 BF16 model and independent full-prefix references.\n'
    def arr(name,values,typ):return name+f' :: [{len(values)}]{typ}.['+','.join(str(x).lower()+('f32' if typ=='f32' else '') for x in values)+'];\n'
    code+=arr(prefix+'_DATA',data,'u8')+prefix+'_SHAPE :: '+shape_code(s)+';\n'
    code+=f'dv_{label}_model :: (data:[]u8,blocks:[]DvBlock,experts:[]DvExpert)->DvModel {{m:=DvModel.{{shape={prefix}_SHAPE}};\n'
    def view(name):
        lo,hi=header[name]['data_offsets'];return f'data[{begin+lo}..<{begin+hi}]'
    for i,name in enumerate(['model.embed_tokens.weight','model.norm.weight','lm_head.weight']):code+=f'm.p[{i}]={view(name)};\n'
    for layer in range(s['layers']):
        code+=f'blocks[{layer}]=DvBlock.{{}};\n';p=f'model.layers.{layer}.'
        for slot,name in enumerate(SLOTS[:7]):code+=f'blocks[{layer}].p[{slot}]={view(p+name)};\n'
        if layer<s['dense_layers']:
            for slot,name in enumerate(FFN):code+=f'blocks[{layer}].dense.p[{slot}]={view(p+"mlp."+name)};\n'
        else:
            code+=f'blocks[{layer}].p[7]={view(p+SLOTS[7])};\n';base=(layer-s['dense_layers'])*s['experts']
            for i in range(s['experts']):
                for slot,name in enumerate(FFN):code+=f'experts[{base+i}].p[{slot}]={view(p+f"mlp.experts.{i}."+name)};\n'
            code+=f'blocks[{layer}].experts=experts[{base}..<{base+s["experts"]}];\n'
            for slot,name in enumerate(FFN):code+=f'blocks[{layer}].shared.p[{slot}]={view(p+"mlp.shared_experts."+name)};\n'
    code+=f'm.blocks=blocks[0..<{s["layers"]}];return m;}}\n'+arr(prefix+'_TOKENS',TOKENS,'int')
    code+=prefix+'_PAYLOAD :: '+str(begin)+';\n'+prefix+'_PINS :: [1]DsArtifact.[DsArtifact.{name="model.safetensors",bytes='+str(len(data))+',sha="'+hashlib.sha256(data).hexdigest()+'"}];\n'
    code+=prefix+'_TENSORS :: ['+str(len(header))+']DvTensor.['+','.join('DvTensor.{file=0,name='+json.dumps(name)+',rank='+str(len(t['shape']))+',rows='+str(t['shape'][0])+',cols='+str(t['shape'][1] if len(t['shape'])==2 else 1)+',begin='+str(begin+t['data_offsets'][0])+',end='+str(begin+t['data_offsets'][1])+'}' for name,t in header.items())+'];\n'
    for mode in ['compressed','expanded']:
        for t,r in enumerate(refs[mode]):
            for key in ['logits','trace']:code+=arr(f'{prefix}_{mode.upper()}_{key.upper()}{t}',r[key],'f32')
            for layer,route in enumerate(r['routes']):
                if route:
                    code+=arr(f'{prefix}_{mode.upper()}_IDS{t}_{layer}',route['ids'],'int');code+=arr(f'{prefix}_{mode.upper()}_WEIGHTS{t}_{layer}',route['weights'],'f64')
        rowsize=(s['attention']['latent'] if mode=='compressed' else s['attention']['heads']*(s['attention']['nope']+s['attention']['value']))+s['attention']['rope']
        for t,r in enumerate(refs[mode]):code+=arr(f'{prefix}_{mode.upper()}_CACHE{t}',[v for layer in r['cache'] for row in layer for v in row],'f32')
        code+=f'''#test "complete V2 {label} {mode} matches independent full-prefix references" {{
    data:={prefix}_DATA;blocks:[{s['layers']}]DvBlock;experts:[{(s['layers']-s['dense_layers'])*s['experts']}]DvExpert;
    for layer:0..<{s['layers']} {{blocks[layer].p[7]=data[0..<1];blocks[layer].experts=experts[0..<1];for slot:0..<3 {{blocks[layer].dense.p[slot]=data[0..<1];blocks[layer].shared.p[slot]=data[0..<1];}}}}
    m:=dv_{label}_model(data[..],blocks[..],experts[..]);
    p:=dv_plan(m.shape,MlMode.{mode});cache:[{s['layers']*s['attention']['capacity']*rowsize}]f32;out:[{s['vocab']+1}]f32;trace:[{2*s['layers']*s['width']+1}]f32;routes:[{s['layers']+1}]DrRoute;
    work:[4096]f32;scores:[128]f64;acc:[{s['width']}]PsionicAccumulator;draft:[{s['layers']}]DrRoute;
    out[{s['vocab']}]=-7;trace[{2*s['layers']*s['width']}]=-9;routes[{s['layers']}].count=-11;for i:0..<{s['layers']} {{draft[i].count=17;routes[i].count=19;}}
'''
        for t in range(len(TOKENS)):
            code+=f'''    r{t}:=dv_step(m,MlMode.{mode},{TOKENS[t]},{t},cache[..],DvOutput.{{logits=out[..],trace=trace[..],routes=routes[..]}},DvWork.{{floats=work[0..<p.floats],scores=scores[0..<p.scores],accumulators=acc[..],routes=draft[..]}},false);
    #assert r{t}.status==DsStatus.ok && r{t}.written=={s['vocab']} && r{t}.next=={t+1};
    for i:0..<{s['vocab']} {{#assert abs(out[i]-{prefix}_{mode.upper()}_LOGITS{t}[i])<0.000002f32;}}
    for i:0..<{2*s['layers']*s['width']} {{#assert abs(trace[i]-{prefix}_{mode.upper()}_TRACE{t}[i])<0.000002f32;}}
    for layer:0..<{s['layers']} {{for pos:0..<{t+1} {{for j:0..<{rowsize} {{#assert abs(cache[(layer*{s['attention']['capacity']}+pos)*{rowsize}+j]-{prefix}_{mode.upper()}_CACHE{t}[(layer*{t+1}+pos)*{rowsize}+j])<0.000002f32;}}}}}}
'''
            for layer in range(s['layers']):
                if layer<s['dense_layers']:code+=f'    #assert routes[{layer}].count==0;\n'
                else:code+=f'    #assert routes[{layer}].count=={s["selected"]};for i:0..<{s["selected"]} {{#assert routes[{layer}].ids[i]=={prefix}_{mode.upper()}_IDS{t}_{layer}[i] && abs(routes[{layer}].weights[i]-{prefix}_{mode.upper()}_WEIGHTS{t}_{layer}[i])<1e-12;}}\n'
        code+=f'    #assert out[{s["vocab"]}]==-7 && trace[{2*s["layers"]*s["width"]}]==-9 && routes[{s["layers"]}].count==-11;\n}}\n'
    outputs={'model.safetensors':data,'model.coax':code.encode(),'reference.json':(json.dumps(refs,indent=2)+'\n').encode()}
    for name,raw in outputs.items():
        if args.write:(dest/name).write_bytes(raw)
        else:assert (dest/name).read_bytes()==raw,f'stale {dest/name}'
    if args.header:
        native=prefix.lower();print('static const int64_t '+native+'_tokens[] = {'+','.join(map(str,TOKENS))+'};')
        for key in ['logits','trace']:
            for mode in ['compressed','expanded']:
                values=[r[key] for r in refs[mode]];print(f'static const float {native}_{mode}_{key}[{len(TOKENS)}][{len(values[0])}] = {{'+','.join('{'+','.join(str(v)+'f' for v in row)+'}' for row in values)+'};')
        for mode in ['compressed','expanded']:
            cache_size=s['layers']*s['attention']['capacity']*((s['attention']['latent'] if mode=='compressed' else s['attention']['heads']*(s['attention']['nope']+s['attention']['value']))+s['attention']['rope'])
            rowsize=cache_size//(s['layers']*s['attention']['capacity']);records=[]
            for r in refs[mode]:
                cache=[0.0]*cache_size
                for layer,rows in enumerate(r['cache']):
                    for pos,row in enumerate(rows):cache[(layer*s['attention']['capacity']+pos)*rowsize:(layer*s['attention']['capacity']+pos+1)*rowsize]=row
                records.append(cache)
            print(f'static const float {native}_{mode}_cache[{len(TOKENS)}][{cache_size}] = {{'+','.join('{'+','.join(str(v)+'f' for v in row)+'}' for row in records)+'};')
            for key,typ in [('ids','int64_t'),('weights','double')]:
                print(f'static const {typ} {native}_{mode}_{key}[{len(TOKENS)}][{s["layers"]}][{s["selected"]}] = {{'+','.join('{'+','.join('{'+','.join(str(v) for v in (r['routes'][layer][key] if r['routes'][layer] else [0]*s['selected']))+'}' for layer in range(s['layers']))+'}' for r in refs[mode])+'};')
    else:print(f'V2 {label}: complete owned {s["layers"]}-layer model, both attention forms, {len(TOKENS)} prefix references; SHA256 {hashlib.sha256(data).hexdigest()}')


if __name__=='__main__':main()
