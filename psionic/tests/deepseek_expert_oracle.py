#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Owned bit-exact original-format vectors and independent quantized FFN oracle.

Uses rational binary scalars, exhaustive nearest-even grid search, math.fsum
and Decimal sigmoid; never imports or executes upstream model/kernel code.
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


def f32(x):return struct.unpack('<f',struct.pack('<f',float(x)))[0]
def bits32(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def from32(x):return struct.unpack('<f',struct.pack('<I',x))[0]
def bits64(x):return struct.unpack('<Q',struct.pack('<d',x))[0]


def scalar(kind,code,scale):
    if scale==255 or (kind=='fp8' and code&127==127):return None
    if kind=='fp4':m=[0,.5,1,1.5,2,3,4,6][code&7];negative=bool(code&8)
    else:
        negative=bool(code&128);e=(code>>3)&15;f=code&7;m=f*2**-9 if e==0 else (1+f/8)*2**(e-7)
    value=math.ldexp(float(m),scale-127);return math.copysign(value,-1 if negative else 1)


FP8=[scalar('fp8',c,127) for c in range(127)]
def nearest(x):
    if not math.isfinite(x):return None
    negative=bool(bits32(x)&0x80000000);m=min(abs(x),448)
    code=min(range(127),key=lambda c:(abs(FP8[c]-m),c%2,c))
    return code|(128 if negative else 0)


def quantize(x):
    assert len(x)%32==0;out=[];codes=[];scales=[]
    for at in range(0,len(x),32):
        row=x[at:at+32];amax=max(max(abs(v) for v in row),f32(1e-4));fraction,exponent=math.frexp(f32(amax*f32(1/448)))
        power=exponent-1 if fraction==.5 else exponent;scale=power+127;scales.append(scale)
        for value in row:
            normalized=f32(math.ldexp(value,-power));code=nearest(normalized);codes.append(code);out.append(scalar('fp8',code,scale))
    return dict(values=out,codes=codes,scales=scales)


def matrix(kind,rows,cols,seed):
    codes=[]
    for r in range(rows):
        for c in range(cols):
            code=(r*7+c*3+seed+(c//32)*5)%16 if kind=='fp4' else (r*11+c*5+seed)%100+12
            if kind=='fp8' and (r+c+seed)%3==0:code|=128
            codes.append(code)
    data=[codes[i]|codes[i+1]<<4 for i in range(0,len(codes),2)] if kind=='fp4' else codes
    nr=rows if kind=='fp4' else (rows+31)//32
    scales=[(123 if kind=='fp4' else 118)+(r*3+b+seed)%5 for r in range(nr) for b in range(cols//32)]
    values=[]
    for i,code in enumerate(codes):
        r,c=divmod(i,cols);scale=scales[(r if kind=='fp4' else r//32)*(cols//32)+c//32];values.append(scalar(kind,code,scale))
    return dict(kind=kind,rows=rows,cols=cols,data=data,scales=scales,values=values)


def project(m,x):
    q=quantize(x);cols=m['cols'];return [f32(math.fsum(w*v for w,v in zip(m['values'][r*cols:(r+1)*cols],q['values']))) for r in range(m['rows'])]


def activate(gate,up,route,limit):
    out=[];gc=uc=0
    for g,u in zip(gate,up):
        if limit>0:
            gc+=g>limit;uc+=u>limit or u< -limit;g=min(g,limit);u=max(-limit,min(limit,u))
        with localcontext() as ctx:
            ctx.prec=80;e=(-abs(Decimal.from_float(g))).exp();sigmoid=(e if g<0 else Decimal(1))/(1+e)
            value=f32(Decimal.from_float(g)*sigmoid*Decimal.from_float(u))
        out.append(f32(value*route))
    return dict(values=out,gate_clamped=gc,up_clamped=uc)


def expert(kind,width,hidden,seed):
    return dict(gate=matrix(kind,hidden,width,seed),up=matrix(kind,hidden,width,seed+1),down=matrix(kind,width,hidden,seed+2))


def execute(e,x,route,limit=10):
    gate=project(e['gate'],x);up=project(e['up'],x);act=activate(gate,up,route,limit);out=project(e['down'],act['values'])
    return dict(gate=gate,up=up,activation=act['values'],output=out,gate_clamped=act['gate_clamped'],up_clamped=act['up_clamped'])


def arr(name,values,kind):
    if kind=='f32':texts=[repr(v)+'f32' for v in values]
    elif kind=='u64':texts=[hex(v) for v in values]
    else:texts=[str(v) for v in values]
    return name+' :: ['+str(len(values))+']'+kind+'.['+','.join(texts)+'];\n'


def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');a=p.parse_args()
    fp4bits=[bits64(scalar('fp4',c,s)) if s!=255 else 0 for s in range(256) for c in range(16)]
    fp8bits=[bits64(v) if (v:=scalar('fp8',c,s)) is not None else 0 for s in range(256) for c in range(256)]
    rounding=[]
    for c,value in enumerate(FP8):
        samples=[value]
        if c<126:
            midpoint=f32((value+FP8[c+1])/2);b=bits32(midpoint);samples+=[from32(b-1),midpoint,from32(b+1)]
        for x in samples:
            for negative in [False,True]:
                x=math.copysign(x,-1 if negative else 1);rounding.append([bits32(x),nearest(x)])
    rounding+=[[bits32(f32(3.4028234663852886e38)),126],[bits32(f32(-3.4028234663852886e38)),254]]
    scale_inputs=[];scale_codes=[]
    for power in range(-22,120):
        b=bits32(f32(math.ldexp(448.0,power)))
        for delta in [-1,0,1]:
            value=from32(b+delta);scale_inputs.append(b+delta);scale_codes.append(quantize([value]+[0.0]*31)['scales'][0])
    x=[f32(((i*17)%29-14)/8) for i in range(64)];experts=[expert('fp4',64,32,s) for s in [1,5,9]];shared=expert('fp8',64,32,2)
    weights=[f32(.137),f32(.719)];ids=[2,0];runs=[execute(e,x,f32(.137)) for e in experts];sr=execute(shared,x,1)
    routed=[execute(experts[i],x,w)['output'] for i,w in zip(ids,weights)]
    combined=[f32(f32(math.fsum(row[j] for row in routed))+sr['output'][j]) for j in range(64)]
    wrong=[f32(v*f32(.137)) for v in execute(experts[0],x,1)['output']];order_delta=max(abs(v-w) for v,w in zip(wrong,runs[0]['output']));assert order_delta>1e-4
    xwide=x[:32];wide=expert('fp4',32,64,4);wr=execute(wide,xwide,f32(.371))
    vectors=[x,[0.0,-0.0]+[from32(1),-from32(1)]+[0.0]*28,[f32(448),f32(-448),f32(448+2**-10)]+[0.0]*29,[from32(0x7f7fffff),-from32(0x7f7fffff)]+[0.0]*30]
    quant=[dict(input=v,**quantize(v)) for v in vectors]
    code='// Independent original-format weight/grid/FFN DATA. No remote runtime.\n'
    code+=arr('DEG_FP4_BITS',fp4bits,'u64');code+=arr('DEG_FP8_UNIT_BITS',fp8bits[127*256:128*256],'u64')
    code+=arr('DEG_ROUND_INPUT',[v[0] for v in rounding],'u32')+arr('DEG_ROUND_CODES',[v[1] for v in rounding],'u8')
    code+=arr('DEG_SCALE_INPUT',scale_inputs,'u32')+arr('DEG_SCALE_CODE',scale_codes,'u8')
    for i,v in enumerate(quant):
        for key in ['input','values']:code+=arr(f'DEG_QUANT_{i}_{key.upper()}',v[key],'f32' if key=='input' else 'f64')
        for key in ['codes','scales']:code+=arr(f'DEG_QUANT_{i}_{key.upper()}',v[key],'u8')
    code+=arr('DEG_INPUT',x,'f32')+arr('DEG_ROUTE_IDS',ids,'int')+arr('DEG_ROUTE_WEIGHTS',weights,'f32')+arr('DEG_COMBINED',combined,'f32')
    code+=arr('DEG_WIDE_INPUT',xwide,'f32')
    all_experts=[*experts,shared,wide];all_runs=[*runs,sr,wr]
    for i,(e,r) in enumerate(zip(all_experts,all_runs)):
        for field,m in e.items():
            code+=arr(f'DEG_{i}_{field.upper()}_DATA',m['data'],'u8')+arr(f'DEG_{i}_{field.upper()}_SCALES',m['scales'],'u8')
        for key in ['gate','up','activation','output']:code+=arr(f'DEG_{i}_{key.upper()}',r[key],'f32')
    # Constructor copies all input DATA into caller storage so native tests may
    # corrupt individual bytes without writing through read-only constants.
    for i,e in enumerate(all_experts):
        code+=f'de_fixture_{i} :: (bytes:[]u8)->DeExpert {{\n';offset=0
        fields=[]
        for field,m in e.items():
            for key in ['data','scales']:
                v=m[key];name=f'DEG_{i}_{field.upper()}_{key.upper()}';code+=f'for j:0..<{len(v)} {{bytes[{offset}+j]={name}[j];}}\n';fields.append((field,key,offset,offset+len(v)));offset+=len(v)
        code+='return DeExpert.{'+','.join(field+'=DeMatrix.{kind=DeKind.'+m['kind']+',rows='+str(m['rows'])+',columns='+str(m['cols'])+','+','.join(key+'=bytes['+str(lo)+'..<'+str(hi)+']' for f,key,lo,hi in fields if f==field)+'}' for field,m in e.items())+'};}\n'
        code+=f'DEG_{i}_BYTES :: {offset};\n'
    for i,v in enumerate(quant):
        n=len(v['input']);code+=f'''#test "expert activation grid {i} matches independent codes scales and wide values" {{
    x:=DEG_QUANT_{i}_INPUT;out:[{n+1}]f64;stage:[{n}]f64;out[{n}]=7;
    r:=de_quantize(x[..],out[0..<{n}],stage[..],false);#assert r.status==DsStatus.ok && r.written=={n} && out[{n}]==7;
    for j:0..<{n} {{#assert f64_bits(out[j])==f64_bits(DEG_QUANT_{i}_VALUES[j]);}}
    for block:0..<{n//32} {{s:=de_scale(x[32*block..<32*(block+1)]);#assert s.status==DsStatus.ok && s.code==DEG_QUANT_{i}_SCALES[block];}}
}}\n'''
    for i in range(5):
        d=all_experts[i]['gate']['cols'];h=all_experts[i]['gate']['rows'];input='DEG_WIDE_INPUT' if i==4 else 'DEG_INPUT';route=.371 if i==4 else 1.0 if i==3 else .137
        code+=f'''#test "complete quantized expert {i} matches independent pre-down weighted FFN" {{
    bytes:[8192]u8;for j:0..<8192 {{bytes[j]=165;}}e:=de_fixture_{i}(bytes[..]);x:={input};work:[512]f32;wide:[64]f64;out:[{d+1}]f32;out[{d}]=7;
    r:=de_expert(e,x[..],{route}f32,10,out[..],DeWork.{{floats=work[..],wide=wide[..]}},false);#assert r.status==DsStatus.ok && r.written=={d} && out[{d}]==7;
    #assert r.gate_clamped=={all_runs[i]['gate_clamped']} && r.up_clamped=={all_runs[i]['up_clamped']};
    for j:0..<{d} {{#assert abs(out[j]-DEG_{i}_OUTPUT[j])<=0.00002f32+0.000001f32*abs(DEG_{i}_OUTPUT[j]);}}
}}\n'''
        code+=f'''#test "expert fixture {i} copies every packed byte and paired scale exactly" {{
    bytes:[8192]u8;for j:0..<8192 {{bytes[j]=165;}}e:=de_fixture_{i}(bytes[..]);
'''
        for field,m in all_experts[i].items():
            for key in ['data','scales']:
                code+=f'    #assert e.{field}.{key}.count=={len(m[key])};for j:0..<{len(m[key])} {{#assert e.{field}.{key}[j]==DEG_{i}_{field.upper()}_{key.upper()}[j];}}\n'
        code+='}\n'
    code+='''#test "routed FP4 experts plus one shared FP8 expert match the independent mixture" {
    b0:[8192]u8;b1:[8192]u8;b2:[8192]u8;bs:[8192]u8;experts:=[3]DeExpert.[de_fixture_0(b0[..]),de_fixture_1(b1[..]),de_fixture_2(b2[..])];shared:=de_fixture_3(bs[..]);x:=DEG_INPUT;ids:=DEG_ROUTE_IDS;weights:=DEG_ROUTE_WEIGHTS;
    out:[65]f32;work:[512]f32;wide:[64]f64;acc:[64]PsionicAccumulator;out[64]=7;
    for j:0..<64 {acc[j]=PsionicAccumulator.{sum=17,correction=19};}
    r:=de_mixture(experts[..],shared,x[..],ids[..],weights[..],10,out[0..<64],DeWork.{floats=work[0..<352],wide=wide[..]},acc[..],false);#assert r.status==DsStatus.ok && r.written==64 && out[64]==7;
    for j:0..<64 {#assert abs(out[j]-DEG_COMBINED[j])<=0.00002f32+0.000001f32*abs(DEG_COMBINED[j]);}
}\n'''
    limited_shared=execute(shared,x,1,1);assert limited_shared['gate_clamped']>0
    counts={key:sum(execute(experts[i],x,w,1)[key] for i,w in zip(ids,weights))+limited_shared[key] for key in ['gate_clamped','up_clamped']}
    code+=f'''#test "mixture reports routed and shared clamping counts" {{
    b0:[8192]u8;b2:[8192]u8;bs:[8192]u8;experts:=[2]DeExpert.[de_fixture_0(b0[..]),de_fixture_2(b2[..])];shared:=de_fixture_3(bs[..]);x:=DEG_INPUT;ids:=[2]int.[1,0];weights:=DEG_ROUTE_WEIGHTS;
    out:[64]f32;work:[352]f32;wide:[64]f64;acc:[64]PsionicAccumulator;
    r:=de_mixture(experts[..],shared,x[..],ids[..],weights[..],1,out[..],DeWork.{{floats=work[..],wide=wide[..]}},acc[..],false);#assert r.status==DsStatus.ok && r.gate_clamped=={counts['gate_clamped']} && r.up_clamped=={counts['up_clamped']};
}}\n'''
    reference=dict(schema=1,profile='deepseek-expert-cpu-wide-v1',quant=quant,rounding=rounding,experts=all_experts,runs=all_runs,input=x,wide_input=xwide,ids=ids,route_weights=weights,combined=combined,post_down_wrong_order_max_abs=order_delta)
    rawbits=b'DEXBITS1'+struct.pack('<4096Q',*fp4bits)+struct.pack('<65536Q',*fp8bits)
    outputs={'expert-goldens.coax':code.encode(),'expert-goldens.json':(json.dumps(reference,separators=(',',':'))+'\n').encode(),'expert-bits.bin':rawbits}
    for name,data in outputs.items():
        if a.write:(DEST/name).write_bytes(data)
        else:assert (DEST/name).read_bytes()==data,'stale '+name
    if a.header:
        print('static const uint32_t deg_round_input[] = {'+','.join(hex(v[0])+'U' for v in rounding)+'};')
        print('static const uint8_t deg_round_codes[] = {'+','.join(str(v[1]) for v in rounding)+'};')
        print('static const uint32_t deg_scale_input[] = {'+','.join(hex(v)+'U' for v in scale_inputs)+'};')
        print('static const uint8_t deg_scale_codes[] = {'+','.join(map(str,scale_codes))+'};')
        print('static const float deg_input[] = {'+','.join(repr(v)+'f' for v in x)+'};')
        for i,r in enumerate(all_runs):
            for key in ['gate','up','activation','output']:print(f'static const float deg_{i}_{key}[] = {{'+','.join(repr(v)+'f' for v in r[key])+'};')
        print('static const float deg_combined[] = {'+','.join(repr(v)+'f' for v in combined)+'};')
    else:print('Expert oracle: all FP4/UE8M0 pairs, all FP8/UE8M0 pairs,',len(rounding),'rounding cases; pre/post-down difference',order_delta)


if __name__=='__main__':main()
