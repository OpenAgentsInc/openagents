#!/usr/bin/env python3
"""Independent full-prefix projected-attention oracle; upstream is data only."""
import argparse
from decimal import Decimal,localcontext
import gzip
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek'
spec=importlib.util.spec_from_file_location('owned_cache_oracle',ROOT/'psionic/tests/deepseek_cache_oracle.py')
cache=importlib.util.module_from_spec(spec);spec.loader.exec_module(cache)
def f32(x):return struct.unpack('<f',struct.pack('<f',float(x)))[0]
def bits(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def bf(x):
    b=bits(f32(x));return ((b+32767+((b>>16)&1))>>16)&65535
def frombf(b):return struct.unpack('<f',struct.pack('<I',b<<16))[0]
def config():
    raw=gzip.decompress((DEST/'cache-reference-config.txt.gz').read_bytes())
    pin=json.loads((DEST/'cache-source.json').read_text())['sources']['config.json']['sha256']
    assert hashlib.sha256(raw).hexdigest()==pin
    return json.loads(raw)
def links(c):
    out=[]
    for layer in range(c['n_layers']+c['n_mtp_layers']):
        ratio=c['compress_ratios'][layer]
        kv=max((i for i in c['kv_source_layers'] if i<=layer and c['compress_ratios'][i]==ratio),default=-1) if ratio else -1
        index=max((i for i in c['index_source_layers'] if i<=layer and c['compress_ratios'][i]==ratio),default=-1) if ratio else -1
        candidate=c['candidate_source_layer'] if ratio and layer>=c['candidate_source_layer'] else -1
        out.append([ratio,kv,index,candidate])
    return out
def select(scores,visible,mask,k):
    return sorted(sorted((i for i in range(visible) if mask[i]),key=lambda i:(-scores[i],i))[:k])
def candidates(scores,visible,block,k):
    n=len(scores);out=[0]*n
    if not visible:return out
    newest=(visible-1)//block
    blocks=range(newest+1)
    ranked=sorted(blocks,key=lambda b:(0 if b==newest else 1,-max(scores[b*block:min((b+1)*block,visible)]),b))
    chosen=set(ranked[:k])
    return [int(i//block in chosen) for i in range(n)]
def policy_fixtures():
    out=[]
    for n in [1,7,8,9,17,65]:
        scores=[f32(((i*17)%23-11)/8) for i in range(n)]
        for visible in sorted({0,1,n//2,n}):
            for block in [1,3,8]:
                for k in [1,2,5]:
                    mask=candidates(scores,visible,block,k)
                    out.append(dict(scores=scores,visible=visible,block=block,k=k,mask=mask,selected=select(scores,visible,mask,5)))
    return out
def normalized(values,weight):
    xs=[Decimal.from_float(frombf(bf(v))) for v in values]
    inverse=1/(sum(x*x for x in xs)/len(xs)+Decimal('1e-20')).sqrt()
    return [bf(f32(f32(float(x*inverse))*frombf(w))) for x,w in zip(xs,weight)]
def pool_fixture(ratio,width,steps):
    xs=[f32(((t*19+i*11)%47-23)/16) for t in range(steps) for i in range(width)]
    gs=[f32(((t*13+i*7)%31-15)/8) for t in range(steps) for i in range(width)]
    weight=[bf((i%5+2)/4) for i in range(width)];out=[]
    with localcontext() as ctx:
        ctx.prec=80
        for t in range(steps):
            if (t+1)%ratio:out+=[0]*width;continue
            if ratio==1:row=xs[t*width:(t+1)*width]
            else:
                row=[]
                for i in range(width):
                    at=t*width+i;prev=at-width;a=Decimal.from_float(gs[prev]);b=Decimal.from_float(gs[at]);top=max(a,b);pa=(a-top).exp();pb=(b-top).exp()
                    row.append(f32(float((Decimal.from_float(xs[prev])*pa+Decimal.from_float(xs[at])*pb)/(pa+pb))))
            out+=normalized(row,weight)
    return dict(ratio=ratio,width=width,steps=steps,kv=xs,gate=gs,norm=weight,output=out)
def coeffs(compressed,rope,position):
    base=160000 if compressed else 10000;out=[]
    low=math.floor(rope*math.log(65536/(32*2*math.pi))/(2*math.log(base))) if compressed else 0
    high=math.ceil(rope*math.log(65536/(2*math.pi))/(2*math.log(base))) if compressed else 1
    for j in range(rope//2):
        exponent=f32(2*j/rope);frequency=f32(1/f32(math.pow(base,exponent)))
        if compressed:
            ramp=f32(max(0,min(1,f32(f32(j-low)/f32(high-low)))))
            smooth=f32(1-ramp);frequency=f32(f32(f32(frequency/16)*f32(1-smooth))+f32(frequency*smooth))
        angle=f32(position*frequency);out +=[f32(math.cos(angle)),f32(math.sin(angle))]
    return out
def rotate(compressed,position,heads,width,rope,inverse,x):
    co=coeffs(compressed,rope,position);out=list(x)
    for h in range(heads):
        base=h*width+width-rope
        for j in range(rope//2):
            a=frombf(x[base+2*j]);b=frombf(x[base+2*j+1]);c,s=co[2*j:2*j+2]
            if inverse:s=-s
            out[base+2*j]=bf(f32(a*c-b*s));out[base+2*j+1]=bf(f32(b*c+a*s))
    return out
def rotate_fixture(c,inverse,heads=2,width=80,rope=64,position=731):
    x=[bf(((i*13)%37-18)/8) for i in range(heads*width)]
    return dict(compressed=c,inverse=inverse,heads=heads,width=width,rope=rope,position=position,input=x,output=rotate(c,position,heads,width,rope,inverse,x))
def packed(kind,rows,width):
    flat=[bf(v) for row in rows for v in row];result=cache.matrix(kind,flat,width)
    return dict(data=result['data'],scales=result['scales'],values=[frombf(b) for b in result['output']])
def index_values(heads,width,q,keys,weights):
    scaled=[frombf(bf(f32(frombf(w)*f32(1/math.sqrt(heads*width))))) for w in weights];out=[]
    for row in range(len(keys)//width):
        terms=[]
        for h in range(heads):
            dot=frombf(bf(f32(math.fsum(a*b for a,b in zip(q[h*width:(h+1)*width],keys[row*width:(row+1)*width])))))
            terms.append(frombf(bf(f32(max(0,dot)*scaled[h]))))
        out.append(frombf(bf(f32(math.fsum(terms)))))
    return out
def index_fixture(width=32,heads=2,rows=7):
    q=packed(2,[[((h*7+j*3)%19-9)/8 for j in range(width)] for h in range(heads)],width)
    k=packed(2,[[((r*11+j*5)%23-11)/8 for j in range(width)] for r in range(rows)],width)
    weights=[bf((h%5-2)/8) for h in range(heads)]
    return dict(heads=heads,width=width,rows=rows,q=q,keys=k,weights=weights,output=index_values(heads,width,q['values'],k['values'],weights))
def attention_values(q,keys,sinks,width):
    out=[]
    for h,sink in enumerate(sinks):
        logits=[math.fsum(a*b for a,b in zip(q[h*width:(h+1)*width],row))/math.sqrt(width) for row in keys]
        maximum=max([sink,*logits]);weights=[math.exp(v-maximum) for v in logits];denom=math.fsum([math.exp(sink-maximum),*weights])
        out +=[bf(f32(math.fsum(w*row[j] for w,row in zip(weights,keys))/denom)) for j in range(width)]
    return out
def attention_fixture(width=32,heads=2,position=7,window=3,ratio=1):
    rows=(position+1)//ratio
    chronological=[[(t*7+j*3)%29/16-0.75 for j in range(width)] for t in range(position+1)]
    physical=[None]*window
    for t,row in enumerate(chronological):physical[t%window]=row
    w=packed(0,physical,width);c=packed(1,[[((t*17+j*11)%31-15)/16 for j in range(width)] for t in range(rows)],width)
    q=[bf(((h*3+j*13)%17-8)/8) for h in range(heads) for j in range(width)];sinks=[f32((h%5-2)/4) for h in range(heads)];ids=[0,3,rows-1]
    keys=[w['values'][(t%window)*width:(t%window+1)*width] for t in range(position-window+1,position+1)]+[c['values'][i*width:(i+1)*width] for i in ids]
    return dict(width=width,heads=heads,position=position,window=window,rows=rows,ratio=ratio,q=q,window_cache=w,compressed=c,ids=ids,sinks=sinks,output=attention_values([frombf(b) for b in q],keys,sinks,width))
def chain_fixture():
    steps=19;layers=43;width=32;heads=2;window=3;rope=8;c=config();mapping=links(c)
    data=dict(steps=steps,layers=layers,width=width,heads=heads,window=window,rope=rope)
    data['window_input']=[bf(((p*7+l*11+j*3)%29-14)/8) for p in range(steps) for l in range(layers) for j in range(width)]
    data['query']=[bf(((p*13+l*17+j*5)%31-15)/8) for p in range(steps) for l in range(layers) for j in range(heads*width)]
    data['index_query']=[bf(((p*17+l*19+j*7)%37-18)/8) for p in range(steps) for l in range(layers) for j in range(heads*width)]
    data['weights']=[bf(((p+l+h*3)%7-2)/8) for p in range(steps) for l in range(layers) for h in range(heads)]
    data['kv']=[f32(((p*19+l*23+j*11)%47-23)/16) for p in range(steps) for l in range(layers) for j in range(width)]
    data['gate']=[f32(((p*13+l*7+j*5)%31-15)/8) for p in range(steps) for l in range(layers) for j in range(width)]
    data['norm']=[bf((j%5+2)/4) for j in range(width)];unit=[bf(1)]*width
    compressed={};indexkeys={};windows={};stored_comp={};stored_keys={}
    with localcontext() as ctx:
        ctx.prec=80
        for l in range(layers):
            ratio,owner,index,candidate=mapping[l];rows=[]
            for p in range(steps):
                x=data['window_input'][(p*layers+l)*width:(p*layers+l+1)*width]
                normalized_row=normalized([frombf(v) for v in x],unit)
                rotated=rotate(bool(ratio),p,1,width,rope,False,normalized_row)
                rows.append(packed(0,[[frombf(v) for v in rotated]],width))
            windows[l]=rows
            if l not in c['kv_source_layers']:continue
            latents=[];keys=[];stored_comp[l]=dict(data=[],scales=[]);stored_keys[l]=dict(data=[],scales=[])
            for p in range(ratio-1,steps,ratio):
                if ratio==1:row=data['kv'][(p*layers+l)*width:(p*layers+l+1)*width]
                else:
                    row=[]
                    for j in range(width):
                        a=((p-1)*layers+l)*width+j;b=(p*layers+l)*width+j;ga=Decimal.from_float(data['gate'][a]);gb=Decimal.from_float(data['gate'][b]);m=max(ga,gb);pa=(ga-m).exp();pb=(gb-m).exp()
                        row.append(f32(float((Decimal.from_float(data['kv'][a])*pa+Decimal.from_float(data['kv'][b])*pb)/(pa+pb))))
                latent=normalized(row,data['norm']);first=p+1-ratio
                key=normalized([frombf(v) for v in latent],unit)
                key=rotate(True,first,1,width,rope,False,key)
                kp=packed(2,[[frombf(v) for v in key]],width);keys+=kp['values']
                for role in ['data','scales']:stored_keys[l][role]+=kp[role]
                latent=rotate(True,first,1,width,rope,False,latent)
                cp=packed(1,[[frombf(v) for v in latent]],width);latents+=cp['values']
                for role in ['data','scales']:stored_comp[l][role]+=cp[role]
            compressed[l]=latents;indexkeys[l]=keys
    output=[];selected=[];candidate_trace=[]
    for p in range(steps):
        picks={};candidate_mask=None
        for l in range(layers):
            ratio,owner,index,candidate=mapping[l];visible=(p+1)//ratio if ratio else 0;ids=[]
            if ratio and index==l:
                if visible:
                    q=data['index_query'][(p*layers+l)*heads*width:(p*layers+l+1)*heads*width]
                    q=rotate(True,p,heads,width,rope,False,q);q=packed(2,[[frombf(v) for v in q[h*width:(h+1)*width]] for h in range(heads)],width)['values']
                    weights=data['weights'][(p*layers+l)*heads:(p*layers+l+1)*heads]
                    scores=index_values(heads,width,q,indexkeys[owner][:visible*width],weights)
                    if l==c['candidate_source_layer']:candidate_mask=candidates(scores,visible,3,2)
                    mask=candidate_mask if candidate>=0 else [1]*visible
                    picks[l]=select(scores,visible,mask,3)
                else:picks[l]=[]
            if ratio:ids=picks[index]
            selected+=ids+[-1]*(3-len(ids))
            query=data['query'][(p*layers+l)*heads*width:(p*layers+l+1)*heads*width]
            query=rotate(bool(ratio),p,heads,width,rope,False,query)
            keys=[windows[l][t]['values'] for t in range(max(0,p-window+1),p+1)]
            if ratio:keys +=[compressed[owner][i*width:(i+1)*width] for i in ids]
            sinks=[f32((l%5+h-2)/4) for h in range(heads)]
            answer=attention_values([frombf(b) for b in query],keys,sinks,width)
            answer=rotate(bool(ratio),p,heads,width,rope,True,answer)
            answer=packed(0,[[frombf(v) for v in answer[h*width:(h+1)*width]] for h in range(heads)],width)
            output +=[bf(v) for v in answer['values']]
        candidate_trace+=(candidate_mask or [])+[0]*(steps-len(candidate_mask or []))
    data['output']=output;data['selected']=selected;data['candidates']=candidate_trace
    for name in ['final_window_data','final_window_scales','final_comp_data','final_comp_scales','final_keys_data','final_keys_scales','final_tail','final_gate']:data[name]=[]
    for l in range(layers):
        physical=[None]*window
        for p in range(steps):physical[p%window]=windows[l][p]
        for row in physical:
            data['final_window_data']+=row['data'];data['final_window_scales']+=row['scales']
        for name,stored,scale_per_row in [('comp',stored_comp,2),('keys',stored_keys,1)]:
            row=stored.get(l,dict(data=[],scales=[]))
            data['final_'+name+'_data']+=row['data']+[0]*(steps*width//2-len(row['data']))
            data['final_'+name+'_scales']+=row['scales']+[0]*(steps*scale_per_row-len(row['scales']))
        tail=l in c['kv_source_layers'] and steps%mapping[l][0]!=0
        for role in ['tail','gate']:data['final_'+role]+=data['kv' if role=='tail' else 'gate'][((steps-1)*layers+l)*width:((steps-1)*layers+l+1)*width] if tail else [0.0]*width
    return data
def emit_coax(j):
    s='// Owned sort oracle, independently derived from the pinned configuration.\n'
    for field,name in enumerate(['RATIO','KV','INDEX','CANDIDATE']):
        s+=f'DAS_{name} :: [43]int.['+','.join(str(r[field]) for r in j['links'])+'];\n'
    s+='DasCase :: struct {n:int;visible:int;block:int;keep:int;count:int;scores:[65]f32;mask:[65]u8;ids:[5]int;}\n'
    s+=f'DAS_CASES :: [{len(j["policy"])}]DasCase.[\n'
    for f in j['policy']:
        scores=f['scores']+[0.0]*(65-len(f['scores']));mask=f['mask']+[0]*(65-len(f['mask']));ids=f['selected']+[0]*(5-len(f['selected']))
        s+='DasCase.{n='+str(len(f['scores']))+',visible='+str(f['visible'])+',block='+str(f['block'])+',keep='+str(f['k'])+',count='+str(len(f['selected']))+',scores=[65]f32.['+','.join(str(v)+'f32' for v in scores)+'],mask=[65]u8.['+','.join(map(str,mask))+'],ids=[5]int.['+','.join(map(str,ids))+']},\n'
    s+='];\n'
    for i,r in enumerate(j['rotate']):
        if len(r['input'])!=160:continue
        for key in ['input','output']:
            s+=f'DAS_ROTATE_{i}_{key.upper()} :: [160]u16.['+','.join(map(str,r[key]))+'];\n'
    ix=j['index'][0];at=j['attention'][0]
    vectors=[('INDEX_Q','u8',ix['q']['data']),('INDEX_QS','u8',ix['q']['scales']),('INDEX_K','u8',ix['keys']['data']),('INDEX_KS','u8',ix['keys']['scales']),('INDEX_W','u16',ix['weights']),('INDEX_Y','f32',ix['output']),('ATT_Q','u16',at['q']),('ATT_W','u8',at['window_cache']['data']),('ATT_WS','u8',at['window_cache']['scales']),('ATT_C','u8',at['compressed']['data']),('ATT_CS','u8',at['compressed']['scales']),('ATT_Y','u16',at['output'])]
    for name,typ,values in vectors:s+=f'DAS_{name} :: [{len(values)}]{typ}.['+','.join(str(v)+('f32' if typ=='f32' else '') for v in values)+'];\n'
    for fixture in j['pool']:
        if fixture['width']!=32:continue
        prefix='DAS_POOL_'+str(fixture['ratio'])
        for key,typ in [('kv','f32'),('gate','f32'),('norm','u16'),('output','u16')]:
            values=fixture[key];s+=f'{prefix}_{key.upper()} :: [{len(values)}]{typ}.['+','.join(str(v)+('f32' if typ=='f32' else '') for v in values)+'];\n'
    s+='DasRopeCase :: struct {compressed:bool;rope:int;position:int;coeff:[64]u32;}\n'
    s+=f'DAS_ROPE :: [{len(j["rope"])}]DasRopeCase.[\n'
    for r in j['rope']:
        values=[bits(v) for v in r['coeff']]+[0]*(64-r['rope'])
        s+='DasRopeCase.{compressed='+str(r['compressed']).lower()+',rope='+str(r['rope'])+',position='+str(r['position'])+',coeff=[64]u32.['+','.join(map(str,values))+']},\n'
    s+='];\n'
    return s
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');a=p.parse_args()
    j=json.loads(gzip.decompress((DEST/'sparse-goldens.json.gz').read_bytes())) if a.header else dict(links=links(config()),policy=policy_fixtures(),pool=[pool_fixture(r,w,t) for w,t in [(32,9),(512,5)] for r in [1,2]],rope=[dict(compressed=c,rope=d,position=p,coeff=coeffs(c,d,p)) for c in [False,True] for d in [2,8,64] for p in [0,1,127,65535,1048575]],rotate=[rotate_fixture(c,i) for c in [False,True] for i in [False,True]]+[rotate_fixture(True,False,64,512,64,65535)],index=[index_fixture(),index_fixture(128,32,9)],attention=[attention_fixture(),attention_fixture(512,64),attention_fixture(32,2,129,128,2),attention_fixture(512,64,129,128,2)],chain=chain_fixture())
    if a.header:
        print('#include <stdint.h>')
        def arr(name,typ,values):
            print(f'static const {typ} {name}[]={{'+','.join(repr(float(v))+'f' if typ=='float' else str(v) for v in values)+'};')
        print('typedef struct {int64_t n,visible,block,k,count;float scores[65];uint8_t mask[65];int64_t ids[5];} DasPolicy;')
        print('static const DasPolicy das_policy[]={')
        for f in j['policy']:
            print('{'+','.join(map(str,[len(f['scores']),f['visible'],f['block'],f['k'],len(f['selected'])]))+',{'+','.join(str(v)+'f' for v in f['scores'])+'},{'+','.join(map(str,f['mask']))+'},{'+','.join(map(str,f['selected']))+'}},')
        print('};')
        for i,f in enumerate(j['rotate']):
            for name in ['input','output']:arr(f'das_rotate_{i}_{name}','uint16_t',f[name])
        print('typedef struct {int compressed,inverse;int64_t heads,width,rope,position;const uint16_t *input,*output;} DasRotateFixture;\nstatic const DasRotateFixture das_rotate[]={')
        for i,f in enumerate(j['rotate']):print('{'+','.join(map(str,[int(f['compressed']),int(f['inverse']),f['heads'],f['width'],f['rope'],f['position']]))+f',das_rotate_{i}_input,das_rotate_{i}_output'+'},')
        print('};')
        print('typedef struct {int compressed;int64_t rope,position;uint32_t coeff[64];} DasRopeFixture;\nstatic const DasRopeFixture das_rope[]={')
        for f in j['rope']:print('{'+str(int(f['compressed']))+','+str(f['rope'])+','+str(f['position'])+',{'+','.join(map(str,[bits(v) for v in f['coeff']]))+'}},')
        print('};')
        for i,f in enumerate(j['pool']):
            for key in ['kv','gate','norm','output']:arr(f'das_pool_{i}_{key}','float' if key in ['kv','gate'] else 'uint16_t',f[key])
        print('typedef struct {int64_t ratio,width,steps;const float *kv,*gate;const uint16_t *norm,*output;} DasPool;\nstatic const DasPool das_pools[]={')
        for i,f in enumerate(j['pool']):print('{'+','.join(map(str,[f['ratio'],f['width'],f['steps']]))+','+','.join(f'das_pool_{i}_{k}' for k in ['kv','gate','norm','output'])+'},')
        print('};')
        for i,f in enumerate(j['index']):
            for name,v in [('q',f['q']['data']),('qs',f['q']['scales']),('k',f['keys']['data']),('ks',f['keys']['scales'])]:arr(f'das_ix_{i}_{name}','uint8_t',v)
            arr(f'das_ix_{i}_weights','uint16_t',f['weights']);arr(f'das_ix_{i}_output','float',f['output'])
        print('typedef struct {int64_t heads,width,rows;const uint8_t *q,*qs,*k,*ks;const uint16_t *weights;const float *output;} DasIndex;\nstatic const DasIndex das_indices[]={')
        for i,f in enumerate(j['index']):print('{'+','.join(map(str,[f['heads'],f['width'],f['rows']]))+','+','.join(f'das_ix_{i}_{k}' for k in ['q','qs','k','ks','weights','output'])+'},')
        print('};')
        for i,f in enumerate(j['attention']):
            for name,v in [('w',f['window_cache']['data']),('ws',f['window_cache']['scales']),('c',f['compressed']['data']),('cs',f['compressed']['scales'])]:arr(f'das_at_{i}_{name}','uint8_t',v)
            for name,typ in [('q','uint16_t'),('output','uint16_t'),('ids','int64_t'),('sinks','float')]:arr(f'das_at_{i}_{name}',typ,f[name])
        print('typedef struct {int64_t heads,width,window,position,rows,ratio;const uint8_t *w,*ws,*c,*cs;const uint16_t *q,*output;const int64_t *ids;const float *sinks;} DasAttentionFixture;\nstatic const DasAttentionFixture das_attention[]={')
        for i,f in enumerate(j['attention']):print('{'+','.join(map(str,[f['heads'],f['width'],f['window'],f['position'],f['rows'],f['ratio']]))+','+','.join(f'das_at_{i}_{k}' for k in ['w','ws','c','cs','q','output','ids','sinks'])+'},')
        print('};')
        for name in ['window_input','query','index_query','weights','norm','kv','gate','output','selected','candidates','final_window_data','final_window_scales','final_comp_data','final_comp_scales','final_keys_data','final_keys_scales','final_tail','final_gate']:
            arr('das_chain_'+name,'float' if name in ['kv','gate','final_tail','final_gate'] else 'int64_t' if name=='selected' else 'uint8_t' if name=='candidates' or name.startswith('final_') else 'uint16_t',j['chain'][name])
        return
    files={'sparse-goldens.coax':emit_coax(j).encode(),'sparse-goldens.json.gz':(json.dumps(j,separators=(',',':'))+'\n').encode()}
    for name,data in files.items():
        path=DEST/name
        if a.write:path.write_bytes(gzip.compress(data,mtime=0) if name.endswith('.gz') else data)
        else:assert (gzip.decompress(path.read_bytes()) if name.endswith('.gz') else path.read_bytes())==data,name
    print('Sparse oracle: pinned 43-layer owners and independent candidate/position selections agree')
if __name__=='__main__':main()
