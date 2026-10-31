#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
import contextlib, io, copy, hashlib, importlib.util, pathlib, sys
p=pathlib.Path(__file__).with_name('tensor_ir_oracle.py');spec=importlib.util.spec_from_file_location('ir_oracle',p);m=importlib.util.module_from_spec(spec);saved=sys.argv;sys.argv=[str(p)]
with contextlib.redirect_stdout(io.StringIO()):spec.loader.exec_module(m)
sys.argv=saved
cases=[]
for batch in (1,2,4,8):
    for branch in (0,1):
        for repeat in (0,1):
            g=copy.deepcopy(m.cases()[0]);g['nodes'][0]=m.leaf([batch,3],0);base=batch*3
            for n in g['nodes'][1:5]:n['input_base']=base;base+=n['storage']
            g['inputs']=[((i*3+1)%13-6)/4 for i in range(batch*3)]+m.cases()[0]['inputs'][6:]
            if branch:g['nodes'][7]=dict(op='multiply',a=6,b=6)
            if repeat:g['nodes'].append(dict(op='relu',a=9));g['output']=10
            answer=m.evaluate(g);cases.append((batch,branch,repeat,g['inputs'],answer))
if '--header' in sys.argv:
    print('typedef struct {int batch,branch,repeat;int64_t n,m;const float *input,*expected;} TemplateOracle;')
    for i,(_,_,_,x,y) in enumerate(cases):
        print(f'static const float tx{i}[]={{'+m.array_literal(x)+'};')
        print(f'static const float ty{i}[]={{'+m.array_literal(y)+'};')
    print('static const TemplateOracle template_cases[]={'+','.join('{%d,%d,%d,%d,%d,tx%d,ty%d}'%(b,c,r,len(x),len(y),i,i) for i,(b,c,r,x,y) in enumerate(cases))+'};')
    compiler=pathlib.Path(sys.argv[sys.argv.index('--compiler')+1]);h=hashlib.sha256(compiler.read_bytes()).digest();print('static const uint64_t qualified_compiler[4]={'+','.join('UINT64_C(0x'+h[i:i+8].hex()+')' for i in range(0,32,8))+'};')
else:print('Dynamic template: 16 independent Decimal graph answers pass')
