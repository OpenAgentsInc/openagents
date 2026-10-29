#!/usr/bin/env python3
"""Independent scalar/Decimal hyper-connection fixtures. Tooling, never runtime.

F32 storage boundaries, compensated-wide projection/reduction profile. The
sigmoid and initial softmax exponentials use 80-digit Decimal arithmetic.
"""
import argparse
from decimal import Decimal,localcontext
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek'
SOURCE='https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/resolve/1bae0e1088993b9b82de08f59de5be5a34e42170/'
CONTRACT=dict(schema=1,profile='deepseek-hyper-cpu-wide-v1',sources={
    'model':dict(url=SOURCE+'inference/model.py',sha256='e5decd76fe70fd56ef2f10124d245c6f64fb6a0003cc9068a1d3182a168b8d2d'),
    'kernel':dict(url=SOURCE+'inference/kernel.py',sha256='1236c3507019ed176f5dba5e04bcea58867cf654818c6cf138ed4845398c2455'),
    'config':dict(url=SOURCE+'inference/config.json',sha256='2e84f45cf1dac8c7fcbb200e96667d4b913275690668ed496f24c7747207a809')},
    original=dict(width=5120,streams=4,mix_outputs=24,projection_dtype='F32',projection_shape=[24,20480],norm_epsilon=1e-20,sinkhorn_iterations=20,hc_epsilon=1e-6,initial_pre=[1,0,0,0],comb_axes=['source','destination']),
    semantics=dict(pre='sigmoid(affine pre) + hc_epsilon',post='2 * sigmoid(affine post)',comb='row softmax + hc_epsilon; column normalization; 19 row/column sweeps; add hc_epsilon to each normalization denominator',collapse='consume previous subblock pre, not current pre',carry='four residual streams plus the returned pre-mix; final head also consumes this carry'),
    numeric=dict(storage='F32',reductions='compensated binary64',projection='F32 dot result times F32 inverse RMS, rounded to F32',affine='binary64 multiply/add rounded to F32',softmax='F32 stored exponentials and coefficients, wide denominators',scope='CPU equation reference; no original-device/BF16 or whole-model execution claim'))


def f32(x):return struct.unpack('<f',struct.pack('<f',float(x)))[0]


def sigmoid(x):
    with localcontext() as ctx:
        ctx.prec=80;e=(-abs(Decimal.from_float(x))).exp()
        return (e if x<0 else Decimal(1))/(1+e)


def split(mixes,scale,base,iterations=20):
    affine=[f32(mixes[i]*scale[0 if i<4 else 1 if i<8 else 2]+base[i]) for i in range(24)]
    pre=[f32(sigmoid(x)+Decimal('0.000001')) for x in affine[:4]]
    post=[f32(2*sigmoid(x)) for x in affine[4:8]];comb=[]
    for j in range(4):
        row=affine[8+4*j:12+4*j];maximum=max(row)
        with localcontext() as ctx:
            ctx.prec=80;exps=[f32(Decimal.from_float(x-maximum).exp()) for x in row]
        den=math.fsum(exps);comb.append([f32(e/den+1e-6) for e in exps])
    def normalize(axis):
        sums=[math.fsum(comb[i][j] for i in range(4)) if axis==0 else math.fsum(comb[j]) for j in range(4)]
        return [[f32(comb[i][j]/(sums[j if axis==0 else i]+1e-6)) for j in range(4)] for i in range(4)]
    comb=normalize(0)
    for _ in range(iterations-1):comb=normalize(1);comb=normalize(0)
    return dict(pre=pre,post=post,comb=[x for row in comb for x in row])


def project(x,w):
    inv=f32(1/math.sqrt(math.fsum(v*v for v in x)/len(x)+1e-20))
    return [f32(f32(math.fsum(a*b for a,b in zip(row,x)))*inv) for row in w]


def collapse(x,previous,width):
    return [f32(math.fsum(previous[i]*x[i*width+j] for i in range(4))) for j in range(width)]


def expand(x,residual,mix,width):
    # comb[source,target], exactly the original unsqueeze(-2)/sum(dim=2).
    return [f32(mix['post'][target]*x[j]+math.fsum(mix['comb'][4*source+target]*residual[source*width+j] for source in range(4))) for target in range(4) for j in range(width)]


def weights(width,seed):
    return [[f32(((i*19+j*7+seed*11)%37-18)/64) for j in range(4*width)] for i in range(24)]


def array(name,values):
    return name+' :: ['+str(len(values))+']f32.['+','.join(repr(x)+'f32' for x in values)+'];\n'


def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');a=p.parse_args()
    raw=[f32((i%7-3)/4) for i in range(24)];scale=[.75,-.5,1.25];base=[f32((i%9-4)/8) for i in range(24)]
    hard=[0.0]*8+[0.0,14.0,-14.0,-9.0,9.0,0.0,0.0,-14.0,-14.0,6.0,1.0,3.0,4.0,-14.0,0.0,-4.0]
    cases=[]
    for label,m,b in [('ordinary',raw,base),('slow',hard,[0.0]*24),('saturated',[-1200.0,1200.0,-0.0,.000001,1200.0,-1200.0,-1.0,1.0]+hard[8:],[0.0]*24)]:
        sc=scale if label=='ordinary' else [1.0]*3;result=split(m,sc,b)
        cases.append(dict(label=label,mixes=m,scale=sc,base=b,result=result))
    slow_delta=max(abs(x-y) for x,y in zip(split(hard,[1.0]*3,[0.0]*24,19)['comb'],cases[1]['result']['comb']));assert slow_delta>1e-5
    width=3;x=[-.5,.25,1.0]*4;previous=[1.0,0.0,0.0,0.0];chain=[]
    for step in range(6):
        w=weights(width,step);sc=[.5,-.75,1.25];b=[f32(((i*3+step)%11-5)/8) for i in range(24)]
        mixes=project(x,w);mix=split(mixes,sc,b);collapsed=collapse(x,previous,width);sublayer=[f32(f32(v*.75)+f32((step+1)/16)) for v in collapsed];out=expand(sublayer,x,mix,width)
        chain.append(dict(input=x,previous=previous,projection=[v for row in w for v in row],scale=sc,base=b,mixes=mixes,mix=mix,collapsed=collapsed,sublayer=sublayer,output=out));x=out;previous=mix['pre']
    code='// Independent Decimal/scalar hyper-connection DATA, no remote runtime.\n'
    for i,c in enumerate(cases):
        for key in ['mixes','scale','base']:code+=array(f'DHG_SPLIT_{i}_{key.upper()}',c[key])
        for key in ['pre','post','comb']:code+=array(f'DHG_SPLIT_{i}_{key.upper()}',c['result'][key])
    for i,c in enumerate(chain):
        for key in ['input','previous','projection','scale','base','mixes','collapsed','sublayer','output']:code+=array(f'DHG_CHAIN_{i}_{key.upper()}',c[key])
        for key in ['pre','post','comb']:code+=array(f'DHG_CHAIN_{i}_{key.upper()}',c['mix'][key])
    final_collapsed=collapse(x,previous,width);code+=array('DHG_FINAL_COLLAPSED',final_collapsed)
    transposed=dict(chain[1]['mix']);transposed['comb']=[chain[1]['mix']['comb'][4*j+i] for i in range(4) for j in range(4)]
    wrong_orientation=expand(chain[1]['sublayer'],chain[1]['input'],transposed,width)
    orientation_delta=max(abs(a-b) for a,b in zip(wrong_orientation,chain[1]['output']));assert orientation_delta>1e-4
    for i,c in enumerate(cases):
        code+=f'''#test "hyper split {c['label']} matches independent Decimal 20-iteration reference" {{
    r:=dh_split(DHG_SPLIT_{i}_MIXES,DHG_SPLIT_{i}_SCALE,DHG_SPLIT_{i}_BASE);#assert r.status==DsStatus.ok;
    for j:0..<4 {{#assert abs(r.mix.pre[j]-DHG_SPLIT_{i}_PRE[j])<0.00000012f32 && abs(r.mix.post[j]-DHG_SPLIT_{i}_POST[j])<0.00000024f32;}}
    for j:0..<16 {{#assert abs(r.mix.comb[j]-DHG_SPLIT_{i}_COMB[j])<0.00000012f32;}}
}}\n'''
    code+='''#test "six hyper subblocks use previous pre-mix and preserve the complete temporal carry" {
    state:=DHG_CHAIN_0_INPUT;carry:=dh_initial();collapsed:[3]f32;out:[12]f32;work:[12]f32;
'''
    for i in range(len(chain)):
        code+=f'''    w{i}:=DHG_CHAIN_{i}_PROJECTION;
    r{i}:=dh_prepare(3,state[..],carry,DhWeights.{{projection=w{i}[..],scale=DHG_CHAIN_{i}_SCALE,base=DHG_CHAIN_{i}_BASE}},collapsed[..],work[0..<3],false);#assert r{i}.status==DsStatus.ok;
    for j:0..<3 {{#assert abs(collapsed[j]-DHG_CHAIN_{i}_COLLAPSED[j])<0.000001f32;}}
    for j:0..<4 {{#assert abs(r{i}.mix.pre[j]-DHG_CHAIN_{i}_PRE[j])<0.00000012f32 && abs(r{i}.mix.post[j]-DHG_CHAIN_{i}_POST[j])<0.00000024f32;}}
    for j:0..<16 {{#assert abs(r{i}.mix.comb[j]-DHG_CHAIN_{i}_COMB[j])<0.00000012f32;}}
    y{i}:[3]f32;for j:0..<3 {{y{i}[j]=collapsed[j]*0.75f32+{(i+1)/16}f32;}}
    f{i}:=dh_finish(3,state[..],y{i}[..],r{i}.mix,out[..],work[..],false);#assert f{i}.status==DsStatus.ok && f{i}.written==12;
    for j:0..<12 {{#assert abs(out[j]-DHG_CHAIN_{i}_OUTPUT[j])<0.000002f32;}}
    for j:0..<4 {{#assert f{i}.carry.pre[j]==r{i}.mix.pre[j];}}
    state=out;carry=f{i}.carry;
'''
    code+='''    head:=dh_collapse(3,state[..],carry,collapsed[..],work[0..<3],false);#assert head.status==DsStatus.ok && head.written==3;
    for j:0..<3 {#assert abs(collapsed[j]-DHG_FINAL_COLLAPSED[j])<0.000002f32;}
}\n'''
    reference=dict(schema=1,scope='Four-stream, 20-iteration wide CPU component equations, not original full-model/BF16 parity',split=cases,chain=chain,final_collapsed=final_collapsed,slow_iteration_19_20_max_abs=slow_delta,orientation_transpose_max_abs=orientation_delta)
    outputs={'hyper-goldens.coax':code.encode(),'hyper-goldens.json':(json.dumps(reference,indent=2)+'\n').encode(),'hyper-contract.json':(json.dumps(CONTRACT,indent=2)+'\n').encode()}
    for name,data in outputs.items():
        if a.write:(DEST/name).write_bytes(data)
        else:assert (DEST/name).read_bytes()==data,'stale '+name
    if a.header:
        print('static const float dhg_final_collapsed[] = {'+','.join(repr(v)+'f' for v in final_collapsed)+'};')
        for i,c in enumerate(chain):
            for key in ['input','previous','projection','scale','base','mixes','collapsed','sublayer','output']:
                print('static const float dhg_chain_'+str(i)+'_'+key+'[] = {'+','.join(repr(v)+'f' for v in c[key])+'};')
            for key in ['pre','post','comb']:print('static const float dhg_chain_'+str(i)+'_'+key+'[] = {'+','.join(repr(v)+'f' for v in c['mix'][key])+'};')
    else:print('Hyper-connections: 3 split/Sinkhorn references and 6 complete temporal handoffs; SHA256',hashlib.sha256(outputs['hyper-goldens.coax']).hexdigest())


if __name__=='__main__':main()
