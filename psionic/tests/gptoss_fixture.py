#!/usr/bin/env python3
"""Owned tiny original-format model and a dense, cache-free scalar reference."""
import argparse,hashlib,json,math,random,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/gptoss/tiny'
S=dict(layers=2,experts=4,selected=2,vocab=16,width=32,hidden=32,heads=1,kv_heads=1,window=2,capacity=8)
SLOTS=['attn.norm.scale','attn.out.bias','attn.out.weight','attn.qkv.bias','attn.qkv.weight','attn.sinks','mlp.gate.bias','mlp.gate.weight','mlp.mlp1_bias','mlp.mlp2_bias','mlp.norm.scale','mlp.mlp1_weight.blocks','mlp.mlp1_weight.scales','mlp.mlp2_weight.blocks','mlp.mlp2_weight.scales']
TOKENS=[1,3,7,2]
LUT=[0,.5,1,1.5,2,3,4,6,-0.,-.5,-1,-1.5,-2,-3,-4,-6]
def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def bf16(x):return struct.pack('<H',struct.unpack('<I',struct.pack('<f',x))[0]>>16)
def shape(slot):
    return [[32],[32],[32,64],[192],[192,32],[1],[4],[4,32],[4,64],[4,32],[32],[4,64,1,16],[4,64,1],[4,32,1,16],[4,32,1]][slot]
def build():
    rng=random.Random(45001);values={};tensors={}
    for l in range(2):
        for slot,name in enumerate(SLOTS):
            dims=shape(slot);n=math.prod(dims);key=f'block.{l}.{name}';dtype='U8' if slot>=11 else 'BF16'
            if slot in (12,14):data=bytes(rng.choice([123,124]) for _ in range(n))
            elif slot in (11,13):data=bytes(rng.randrange(256) for _ in range(n))
            else:
                if slot in (0,10):v=[1+rng.randrange(-2,3)/16 for _ in range(n)]
                elif slot in (2,4,7):v=[rng.randrange(-4,5)/32 for _ in range(n)]
                else:v=[rng.randrange(-4,5)/64 for _ in range(n)]
                data=b''.join(map(bf16,v))
            tensors[key]=(dtype,dims,data)
    for name,dims in [('embedding.weight',[16,32]),('norm.scale',[32]),('unembedding.weight',[16,32])]:
        n=math.prod(dims);v=[1]*n if name=='norm.scale' else [rng.randrange(-8,9)/(4 if name.startswith('embedding') else 16) for _ in range(n)]
        tensors[name]=('BF16',dims,b''.join(map(bf16,v)))
    payload=bytearray();header={}
    for name,(dtype,dims,data) in sorted(tensors.items()):
        begin=len(payload);payload.extend(data);header[name]=dict(dtype=dtype,shape=dims,data_offsets=[begin,len(payload)])
    raw=json.dumps(header,separators=(',',':')).encode();raw+=b' '*((-len(raw))%8)
    data=len(raw).to_bytes(8,'little')+raw+payload;base=8+len(raw)
    spans={k:(base+v['data_offsets'][0],base+v['data_offsets'][1]) for k,v in header.items()}
    for name,(dtype,dims,raw) in tensors.items():
        if dtype=='BF16':values[name]=[struct.unpack('<f',struct.pack('<I',w[0]<<16))[0] for w in struct.iter_unpack('<H',raw)]
        else:values[name]=list(raw)
    for l in range(2):
        for part in (1,2):
            prefix=f'block.{l}.mlp.mlp{part}_weight';packed=values[prefix+'.blocks'];scales=values[prefix+'.scales']
            values[prefix]=[math.ldexp(LUT[(byte>>(4*half))&15],scales[i//16]-127) for i,byte in enumerate(packed) for half in range(2)]
    return data,header,spans,values

def project(weights,rows,columns,x,bias=None):
    y=[f32(math.fsum(weights[r*columns+c]*x[c] for c in range(columns))) for r in range(rows)]
    return [f32(v+bias[i]) for i,v in enumerate(y)] if bias is not None else y

def norm(x,scale):
    divisor=math.sqrt(math.fsum(v*v for v in x)/len(x)+1e-5)
    return [f32(v/divisor*w) for v,w in zip(x,scale)]
def rotate(x,p):
    # Direct-power scalar derivation, independent of the Coax exponential form.
    lo=32*math.log(4096/(32*2*math.pi))/math.log(150000);hi=32*math.log(4096/(2*math.pi))/math.log(150000);concentration=1+.1*math.log(32)
    out=[0.]*len(x)
    for head in range(len(x)//64):
        for i in range(32):
            ramp=min(1,max(0,(i-lo)/(hi-lo)));angle=p/150000**(i/32)*(1-ramp+ramp/32);co=f32(math.cos(angle)*concentration);si=f32(math.sin(angle)*concentration)
            a,b=x[head*64+i],x[head*64+i+32];out[head*64+i]=f32(a*co-b*si);out[head*64+i+32]=f32(b*co+a*si)
    return out

def reference(values,tokens):
    # Entire sequence traverses one layer at a time. No candidate ring/cache used.
    xs=[values['embedding.weight'][t*32:(t+1)*32] for t in tokens];traces=[];routes=[];coefficients=[];keys=[];vals=[]
    for l in range(2):
        p=f'block.{l}.';normalized=[norm(x,values[p+'attn.norm.scale']) for x in xs]
        all_qkv=[project(values[p+'attn.qkv.weight'],192,32,x,values[p+'attn.qkv.bias']) for x in normalized]
        qs=[rotate(x[:64],t) for t,x in enumerate(all_qkv)];ks=[rotate(x[64:128],t) for t,x in enumerate(all_qkv)];vs=[x[128:] for x in all_qkv];keys.append(ks);vals.append(vs)
        next_x=[]
        for t,x in enumerate(xs):
            visible=range(max(0,t-1) if l%2==0 else 0,t+1);scores=[math.fsum(a*b for a,b in zip(qs[t],ks[j]))/8 for j in visible];sink=values[p+'attn.sinks'][0]
            maximum=max(scores+[sink]);weights=[math.exp(s-maximum) for s in scores];total=math.fsum(weights+[math.exp(sink-maximum)])
            attn=[f32(math.fsum(w*vs[j][d] for w,j in zip(weights,visible))/total) for d in range(64)]
            result=project(values[p+'attn.out.weight'],32,64,attn,values[p+'attn.out.bias']);residual=[f32(a+b) for a,b in zip(x,result)]
            n=norm(residual,values[p+'mlp.norm.scale']);logits=project(values[p+'mlp.gate.weight'],4,32,n,values[p+'mlp.gate.bias']);ids=sorted(range(4),key=lambda i:(-logits[i],i))[:2]
            exp=[math.exp(logits[i]-logits[ids[0]]) for i in ids];den=math.fsum(exp);mix=[f32(v/den) for v in exp];experts=[]
            for identity in ids:
                w=values[p+'mlp.mlp1_weight'][identity*2048:(identity+1)*2048];b=values[p+'mlp.mlp1_bias'][identity*64:(identity+1)*64];up=project(w,64,32,n,b);activated=[]
                for gate,linear in zip(up[::2],up[1::2]):
                    gate=min(gate,7);linear=max(-7,min(linear,7));z=1.702*gate;sigmoid=1/(1+math.exp(-z)) if z>-700 else 0;activated.append(f32(gate*sigmoid*(linear+1)))
                w=values[p+'mlp.mlp2_weight'][identity*1024:(identity+1)*1024];b=values[p+'mlp.mlp2_bias'][identity*32:(identity+1)*32];experts.append(project(w,32,32,activated,b))
            result=[f32(residual[i]+math.fsum(w*e[i] for w,e in zip(mix,experts))) for i in range(32)];next_x.append(result)
            if t==len(xs)-1:traces.extend(residual+result);routes.extend(ids);coefficients.extend(mix)
        xs=next_x
    logits=project(values['unembedding.weight'],16,32,norm(xs[-1],values['norm.scale']))
    cache=[0.]*1280
    for l in range(2):
        capacity=2 if l==0 else 8;base=0 if l==0 else 256
        for t in range(len(tokens)-1):
            slot=t%capacity;cache[base+slot*128:base+slot*128+64]=keys[l][t];cache[base+slot*128+64:base+(slot+1)*128]=vals[l][t]
    return dict(logits=logits,trace=traces,routes=routes,weights=coefficients,cache=cache)

def array(name,xs,typ='f32'):
    if typ=='f32':items=','.join(repr(float(x))+'f32' for x in xs)
    else:items=','.join(map(str,xs))
    return f'{name} :: [{len(xs)}]{typ}.['+items+'];\n'

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--write',action='store_true');ap.add_argument('--header',action='store_true');args=ap.parse_args();data,header,spans,values=build();DEST.mkdir(parents=True,exist_ok=True)
    refs=[reference(values,TOKENS[:i+1]) for i in range(len(TOKENS))]
    if args.header:
        print('#include <stdint.h>')
        print('static const coax_t_GShape tiny_shape={'+','.join('.'+k+'='+str(v) for k,v in S.items())+'};')
        print('static coax_t_GArtifact tiny_artifacts[]={{.name={"tiny.safetensors",16},.bytes='+str(len(data))+',.sha={"'+hashlib.sha256(data).hexdigest()+'",64},.header='+str(8+int.from_bytes(data[:8],'little'))+'}};')
        print('static coax_t_GSpec tiny_specs[]={')
        for name,v in header.items():
            dims=v['shape']+[0]*(4-len(v['shape']));a,b=spans[name]
            print('{.name={"'+name+'",'+str(len(name))+'},.dtype='+str(int(v['dtype']=='U8'))+',.rank='+str(len(v['shape']))+',.dims={{'+','.join(map(str,dims))+'}},.begin='+str(a)+',.end='+str(b)+',.file=0},')
        print('};')
        print('static const int64_t tiny_tokens[]={'+','.join(map(str,TOKENS))+'};')
        for field in ('logits','trace','weights','routes'):
            typ='int64_t' if field=='routes' else 'float';rows=[]
            for r in refs:rows.append('{'+','.join(map(str,r[field]))+'}')
            print(f'static const {typ} tiny_{field}[4][{len(refs[0][field])}]={{'+','.join(rows)+'};')
        return
    code='// Owned tiny original-layout model and independent dense scalar references.\n'+array('GT_FILE',data,'u8')
    code+='GT_SHAPE :: GShape.{'+','.join(k+'='+str(v) for k,v in S.items())+'};\n'
    code+='gt_model :: (data:[]u8,blocks:[]GBlock)->GModel {\n'
    for l in range(2):
        for slot,name in enumerate(SLOTS):
            a,b=spans[f'block.{l}.{name}'];code+=f'    blocks[{l}].p[{slot}]=data[{a}..<{b}];\n'
    code+='    return GModel.{shape=GT_SHAPE,blocks=blocks,p=[3][]u8.['+','.join(f'data[{spans[n][0]}..<{spans[n][1]}]' for n in ['embedding.weight','norm.scale','unembedding.weight'])+']};\n}\n'
    metadata_code=code
    code='#load "data.coax";\n'
    for j,r in enumerate(refs):
        for field in ['cache','logits','trace','routes','weights']:code+=array('GT_'+field.upper()+str(j),r[field],'int' if field=='routes' else 'f32')
        code+=array('GT_AFTER'+str(j),reference(values,TOKENS[:j+1]+[1])['cache'])
        code+=f'''#test "dense reference versus original-format model step {j}" {{
    data:=GT_FILE;blocks:[2]GBlock;m:=gt_model(data[..],blocks[..]);p:=g_plan(m.shape);
    cache:=GT_CACHE{j};logits:=[17]f32;trace:[129]f32;routes:[5]int;weights:[5]f32;
    floats:[1792]f32;scores:[8]f64;sums:[32]PsionicAccumulator;work_routes:[4]int;
    logits[16]=-7;trace[128]=-9;routes[4]=-11;weights[4]=-13;
    out:=GOutput.{{logits=logits[..],trace=trace[..],routes=routes[..],weights=weights[..]}};
    work:=GWork.{{floats=floats[..],scores=scores[..],sums=sums[..],routes=work_routes[..]}};
    r:=g_forward(m,{TOKENS[j]},{j},g_macs(m.shape,{j}),cache[..],out,work,false);
    #assert p.status==GStatus.ok && p.floats==1792 && r.status==GStatus.ok && r.written==16 && r.next=={j+1};
    #assert r.macs==g_macs(m.shape,{j});for i:0..<1280 {{#assert cache[i]==GT_AFTER{j}[i];}}
    for i:0..<16 {{#assert abs(logits[i]-GT_LOGITS{j}[i])<0.00002f32*(1+abs(GT_LOGITS{j}[i]));}}
    for i:0..<128 {{#assert abs(trace[i]-GT_TRACE{j}[i])<0.00002f32*(1+abs(GT_TRACE{j}[i]));}}
    for i:0..<4 {{#assert routes[i]==GT_ROUTES{j}[i] && abs(weights[i]-GT_WEIGHTS{j}[i])<0.0000002f32;}}
    #assert logits[16]== -7 && trace[128]== -9 && routes[4]== -11 && weights[4]== -13;
}}
'''.replace('logits:=[17]f32;','logits:[17]f32;')
    outputs={'model.safetensors':data,'data.coax':metadata_code.encode(),'model.coax':code.encode(),'reference.json':(json.dumps({'shape':S,'tokens':TOKENS,'references':refs,'source':'owned dense whole-sequence scalar math.fsum reference; no candidate outputs or upstream runtime'},indent=2)+'\n').encode()}
    for name,blob in outputs.items():
        target=DEST/name
        if args.write:target.write_bytes(blob)
        assert target.read_bytes()==blob,'fixture drift '+name
    print('Tiny gpt-oss: original BF16/MXFP4 bytes and four dense full-prefix references verify')
if __name__=='__main__':main()
