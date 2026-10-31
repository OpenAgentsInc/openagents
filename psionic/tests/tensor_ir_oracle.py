#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent Decimal equations and logical coordinate storage for chapter 26a.

Only tooling: creates finite small graphs and a native expected-value header.
No Psionic code is called to infer shapes, plan storage, or calculate answers.
"""
from decimal import Decimal, localcontext
import hashlib, itertools, json, pathlib, struct, sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
OPS=['input','matmul','add','multiply','relu','sum','layernorm','view','reshape','gather']
def f32(x): return struct.unpack('<f',struct.pack('<f',float(x)))[0]
def coords(shape): return itertools.product(*(range(n) for n in shape))
def count(shape):
    n=1
    for d in shape:n*=d
    return n
def contiguous(shape):
    out=[];n=1
    for d in reversed(shape):out.append(n);n*=d
    return list(reversed(out))
def leaf(shape,base):return dict(op='input',shape=shape,strides=contiguous(shape),storage=count(shape),input_base=base)
def graph(name,nodes,inputs,output=None):return dict(name=name,nodes=nodes,inputs=inputs,output=len(nodes)-1 if output is None else output)
def cases():
    ff=[leaf([2,3],0),leaf([3,4],6),leaf([4],18),leaf([4,2],22),leaf([2],30),dict(op='matmul',a=0,b=1),dict(op='add',a=5,b=2),dict(op='relu',a=6),dict(op='matmul',a=7,b=3),dict(op='add',a=8,b=4)]
    raw=[1,2,3,-1,0,2,1,0,0,1,0,1,0,-1,0,0,1,1,1,-1,0,0,1,0,0,1,1,1,-1,1,.5,-.5]
    result=[graph('feed-forward',ff,raw)]
    result.append(graph('reverse-transpose-gather-sum',[leaf([2,3],0),dict(op='view',a=0,shape=[3,2],strides=[-1,3],offset=2),dict(op='gather',a=1,axis=0,indices=[2,0]),dict(op='sum',a=2,axis=1)],[1,2,3,4,5,6]))
    result.append(graph('reshape-scalar-broadcast-multiply',[leaf([2,3],0),dict(op='reshape',a=0,shape=[3,2]),leaf([],6),dict(op='multiply',a=1,b=2)],[1,-2,3,-4,5,-6,.5]))
    result.append(graph('layernorm-reduction',[leaf([2,4],0),dict(op='layernorm',a=0,epsilon=.25),dict(op='sum',a=1,axis=0)],[1,3,5,7,-8,-3,2,9]))
    result.append(graph('alias-lifetime',[leaf([2,2],0),dict(op='relu',a=0),dict(op='view',a=1,shape=[2,2],strides=[1,2]),dict(op='multiply',a=1,b=1),dict(op='add',a=3,b=2)],[-1,2,3,-4]))
    result.append(graph('empty-sum',[leaf([2,0],0),dict(op='sum',a=0,axis=1)],[]))
    result.append(graph('empty-matmul',[leaf([2,0],0),leaf([0,3],0),dict(op='matmul',a=0,b=1)],[]))
    result.append(graph('empty-gather',[leaf([2,3],0),dict(op='gather',a=0,axis=0,indices=[])],[1,2,3,4,5,6]))
    result.append(graph('scalar-output',[leaf([],0)],[3.25]))
    result.append(graph('view-output',[leaf([2,3],0),dict(op='view',a=0,shape=[3,2],strides=[1,3])],[1,2,3,4,5,6]))
    result.append(graph('broadcast-constant-view',[leaf([],0),dict(op='view',a=0,shape=[2,3],strides=[0,0]),dict(op='relu',a=1)],[-2]))
    m,k,h,o=32,48,64,32
    ns=[leaf([m,k],0),leaf([k,h],m*k),leaf([h],m*k+k*h),leaf([h,o],m*k+k*h+h),leaf([o],m*k+k*h+h+h*o)]+ff[5:]
    raw=[f32(((i*73+17)%101-50)/137) for i in range(m*k+k*h+h+h*o+o)]
    result.append(graph('larger-feed-forward',ns,raw))
    return result

def evaluate(g):
    values=[]
    def material(shape,data):return dict(shape=shape,root=data,strides=contiguous(shape),offset=0)
    def read(v,c):return v['root'][v['offset']+sum(a*b for a,b in zip(c,v['strides']))]
    def dec(x):return Decimal.from_float(x)
    with localcontext() as ctx:
        ctx.prec=100
        for n in g['nodes']:
            op=n['op']
            if op=='input':
                base=n['input_base'];values.append(dict(shape=n['shape'],root=[f32(v) for v in g['inputs'][base:base+n['storage']]],strides=n['strides'],offset=n.get('offset',0)));continue
            a=values[n['a']];ash=a['shape']
            if op=='view':values.append(dict(shape=n['shape'],root=a['root'],strides=n['strides'],offset=n.get('offset',0)));continue
            if op=='reshape':values.append(material(n['shape'],[read(a,c) for c in coords(ash)]));continue
            if op in ('add','multiply'):
                b=values[n['b']];bsh=b['shape'];rank=max(len(ash),len(bsh));aa=[1]*(rank-len(ash))+ash;bb=[1]*(rank-len(bsh))+bsh;shape=[max(x,y) for x,y in zip(aa,bb)];data=[]
                for c in coords(shape):
                    x=dec(read(a,tuple(0 if d==1 else c[i+rank-len(ash)] for i,d in enumerate(ash))));y=dec(read(b,tuple(0 if d==1 else c[i+rank-len(bsh)] for i,d in enumerate(bsh))));data.append(f32(x+y if op=='add' else x*y))
            elif op=='matmul':
                b=values[n['b']];shape=[ash[0],b['shape'][1]];data=[f32(sum((dec(read(a,(r,k)))*dec(read(b,(k,c))) for k in range(ash[1])),Decimal(0))) for r,c in coords(shape)]
            elif op=='relu':shape=ash;data=[max(0.0,read(a,c)) for c in coords(shape)]
            elif op=='sum':
                axis=n['axis'];shape=ash[:axis]+ash[axis+1:];data=[]
                for c in coords(shape):data.append(f32(sum((dec(read(a,c[:axis]+(j,)+c[axis:])) for j in range(ash[axis])),Decimal(0))))
            elif op=='gather':
                axis=n['axis'];shape=list(ash);shape[axis]=len(n['indices']);data=[read(a,c[:axis]+(n['indices'][c[axis]],)+c[axis+1:]) for c in coords(shape)]
            elif op=='layernorm':
                shape=ash;data=[]
                for outer in coords(ash[:-1]):
                    row=[dec(read(a,outer+(j,))) for j in range(ash[-1])];mean=sum(row)/len(row);variance=sum((x-mean)**2 for x in row)/len(row);den=(variance+dec(f32(n['epsilon']))).sqrt();data.extend(f32((x-mean)/den) for x in row)
            else:raise AssertionError(op)
            values.append(material(shape,data))
        v=values[g['output']];return [read(v,c) for c in coords(v['shape'])]

def array_literal(values,typ='float'):
    if typ=='float':return ','.join((float(x).hex()+'f') for x in values) or '0'
    return ','.join(map(str,values)) or '0'
def header():
    cs=cases();print('typedef struct {const char *name;int64_t inputs,outputs;const float *input,*expected;} TiOracle;')
    for i,g in enumerate(cs):
        answer=evaluate(g)
        if i==0:assert answer==[3.5,5.5,1.5,2.5]
        print(f'static const float oracle_input_{i}[]={{'+array_literal(g['inputs'])+'};')
        print(f'static const float oracle_output_{i}[]={{'+array_literal(answer)+'};')
    print('static const TiOracle oracles[]={'+','.join('{"%s",%d,%d,oracle_input_%d,oracle_output_%d}'%(g['name'],len(g['inputs']),len(evaluate(g)),i,i) for i,g in enumerate(cs))+'};')
    print('static coax_t_TiProgram oracle_program(int which){coax_t_TiProgram p={0};for(int i=0;i<32;i++){p.nodes.e[i].a=-1;p.nodes.e[i].b=-1;p.nodes.e[i].epsilon=0.00001f;}switch(which){')
    for i,g in enumerate(cs):
        print(f'case {i}:p.count={len(g["nodes"])};p.output={g["output"]};')
        for j,n in enumerate(g['nodes']):
            v=f'p.nodes.e[{j}]';print(f'{v}.op={OPS.index(n["op"])};')
            for key in ['a','b','storage','input_base','offset','axis','epsilon']:
                if key in n:print(f'{v}.{key}={float(n[key]).hex()+"f" if key=="epsilon" else n[key]};')
            if 'shape' in n:
                print(f'{v}.shape.rank={len(n["shape"])};')
                for d,size in enumerate(n['shape']):print(f'{v}.shape.dims.e[{d}]={size};')
            for key in ['strides','indices']:
                for d,size in enumerate(n.get(key,[])):print(f'{v}.{key}.e[{d}]={size};')
            if 'indices' in n:print(f'{v}.index_count={len(n["indices"])};')
        print('break;')
    print('default:abort();}return p;}')
    if '--compiler' in sys.argv:
        data=pathlib.Path(sys.argv[sys.argv.index('--compiler')+1]).read_bytes();digest=hashlib.sha256(data).digest();print('static const uint64_t qualified_compiler[4]={'+','.join('UINT64_C(0x'+digest[i:i+8].hex()+')' for i in range(0,32,8))+'};')
if '--header' in sys.argv:header()
else:
    cs=cases()
    for g in cs:evaluate(g)
    print(f'Tensor IR Decimal(100): {len(cs)} independent graph fixtures pass')
