#!/usr/bin/env python3
"""Independent dense scalar reference and central differences, tooling only.
No Coax/native implementation is called to build these expected values."""
import argparse,itertools,json,math,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
NAMES='add subtract multiply divide negate relu exponential logarithm equal less sum mean maximum softmax take update concat sort topk matmul conv2d maxpool2d avgpool2d nearest2d bilinear2d mse cross_entropy'.split()
BINARY={'add','subtract','multiply','divide','equal','less','update','concat','matmul','conv2d','mse'}
FAMILIES=['elementwise']*10+['reduction']*3+['normalization','indexing','indexing','joining','ordering','ordering','matrix','convolution','pooling','pooling','interpolation','interpolation','loss','loss']
RULES=['broadcast(A,B)']*4+['A']*4+['broadcast(A,B)']*2+['reduce(A,axis,keep)']*3+['A','A with axis=len(indices)','A; B=A with axis=len(indices)','A with axis=A.axis+B.axis; other axes equal','A; stable axis permutation','A with axis=k; 0<=k<=A.axis','[A.rows,B.columns]; rank2 and A.columns=B.rows','[N,O,H-KH+1,W-KW+1]; NCHW/OIHW valid stride1 groups1','[N,C,floor((H-KH)/stride)+1,floor((W-KW)/stride)+1]','[N,C,floor((H-KH)/stride)+1,floor((W-KW)/stride)+1]','[N,C,height,width]; floor(dst*src/dst_size)','[N,C,height,width]; clipped half-pixel align_corners=false','scalar; equal nonempty shapes; mean squared error','scalar; A=[rows,classes], nonempty; int64 row labels; mean stable log-softmax loss']
def manifest():
    return dict(schema=1,profile='CPU-f32-wide-views-v1',shape_inference='to_plan',dispatch='to_forward',vjp='to_vjp',storage='borrowed f32 views, rank 0..4, <=1048576 logical elements per input/output',work_limit=1048576,gradient_work='(na+nb)*outputs*branch_factor <= 1048576; plus the admitted forward validation',forward_tolerance='abs(got-ref)/(1+abs(ref)) <= 2e-6',gradient_tolerance='abs(got-central_difference)/(1+abs(central_difference)) <= 3e-5',gradient_semantics='wide value equations at f32 inputs before final narrowing; discrete indices/parameters have no derivative; frozen subgradients at kinks',arithmetic='f32 operands; f64 products and ordered compensated reductions; one checked f32 narrowing; no FMA/reassociation',publication='public outputs publish together; arithmetic refusal may dirty scratch; all public counts zero on refusal',operators=[dict(id=i,name=n,family=FAMILIES[i],arity=2 if n in BINARY else 1,shape_rule=RULES[i],value_dtype='f32',aux_dtype='i64' if n in ('sort','topk') else None,backends=['cpu'],gradient='analytical_value_vjp' if n not in ('equal','less') else 'not_differentiable',indices_gradient=False,parameters_gradient=False,automatic_tape=False) for i,n in enumerate(NAMES)],conventions=dict(sort='stable value order, equal values lowest original axis index; indices are axis-local',maximum='lowest axis position on ties',maxpool2d='first row-major window position on ties',relu='zero derivative at zero',update='last indexed update wins; overwritten updates have zero value gradient',empty='sum identity and empty outputs supported; mean/max/softmax reduced width and mean losses must be nonempty',gradient_coordinates='dense logical input coordinates; backing-storage alias/view pullbacks and tape integration are separate',outputs='fresh/unpublished caller-owned result spans; use tensor-view scatter to replace a registered tensor'))
def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def coords(shape):return itertools.product(*(range(x) for x in shape))
def flat(shape,c):
    n=0
    for d,i in zip(shape,c):n=n*d+i
    return n
def at(a,s,c):return a[flat(s,c)]
def broadcast(a,b):
    rank=max(len(a),len(b));aa=(1,)*(rank-len(a))+a;bb=(1,)*(rank-len(b))+b
    assert all(x==y or x==1 or y==1 for x,y in zip(aa,bb))
    return tuple(y if x==1 else x for x,y in zip(aa,bb))
def bcoord(s,c):return tuple(0 if d==1 else i for d,i in zip(s,c[len(c)-len(s):]))
def reference(case,aa=None,bb=None):
    name=case['name'];a=case['a'] if aa is None else aa;b=case['b'] if bb is None else bb;s=case['ashape'];t=case['bshape'];axis=case.get('axis',0);idx=case.get('indices',[]);aux=[]
    if name in ('add','subtract','multiply','divide','equal','less'):
        o=broadcast(s,t);v=[]
        for c in coords(o):
            x=at(a,s,bcoord(s,c));y=at(b,t,bcoord(t,c))
            v.append({'add':lambda:x+y,'subtract':lambda:x-y,'multiply':lambda:x*y,'divide':lambda:x/y,'equal':lambda:float(x==y),'less':lambda:float(x<y)}[name]())
        return o,v,aux
    if name in ('negate','relu','exponential','logarithm'):
        fn={'negate':lambda x:-x,'relu':lambda x:max(0.,x),'exponential':math.exp,'logarithm':math.log}[name];return s,[fn(x) for x in a],aux
    if name in ('sum','mean','maximum'):
        o=s[:axis]+((1,) if case.get('keep') else ())+s[axis+1:];v=[]
        for c in coords(o):
            base=list(c)
            if case.get('keep'):base.pop(axis)
            xs=[at(a,s,tuple(base[:axis]+[j]+base[axis:])) for j in range(s[axis])]
            value=math.fsum(xs) if name!='maximum' else max(xs)
            if name=='mean':value/=len(xs)
            v.append(value)
        return o,v,aux
    if name=='softmax':
        v=[]
        for c in coords(s):
            row=[at(a,s,c[:axis]+(j,)+c[axis+1:]) for j in range(s[axis])];m=max(row);v.append(math.exp(at(a,s,c)-m)/math.fsum(math.exp(x-m) for x in row))
        return s,v,aux
    if name=='take':
        o=s[:axis]+(len(idx),)+s[axis+1:];return o,[at(a,s,c[:axis]+(idx[c[axis]],)+c[axis+1:]) for c in coords(o)],aux
    if name=='update':
        v=list(a)
        for c in coords(t):v[flat(s,c[:axis]+(idx[c[axis]],)+c[axis+1:])]=at(b,t,c)
        return s,v,aux
    if name=='concat':
        o=s[:axis]+(s[axis]+t[axis],)+s[axis+1:];v=[]
        for c in coords(o):v.append(at(a,s,c) if c[axis]<s[axis] else at(b,t,c[:axis]+(c[axis]-s[axis],)+c[axis+1:]))
        return o,v,aux
    if name in ('sort','topk'):
        k=s[axis] if name=='sort' else case['k'];o=s[:axis]+(k,)+s[axis+1:];v=[]
        for c in coords(o):
            order=sorted(range(s[axis]),key=lambda j:((-1 if case.get('descending',True) else 1)*at(a,s,c[:axis]+(j,)+c[axis+1:]),j))
            j=order[c[axis]];v.append(at(a,s,c[:axis]+(j,)+c[axis+1:]));aux.append(j)
        return o,v,aux
    if name=='matmul':return (s[0],t[1]),[math.fsum(a[i*s[1]+k]*b[k*t[1]+j] for k in range(s[1])) for i in range(s[0]) for j in range(t[1])],aux
    if name=='conv2d':
        o=(s[0],t[0],s[2]-t[2]+1,s[3]-t[3]+1);v=[]
        for n,oc,y,x in coords(o):v.append(math.fsum(at(a,s,(n,ic,y+h,x+w))*at(b,t,(oc,ic,h,w)) for ic in range(s[1]) for h in range(t[2]) for w in range(t[3])))
        return o,v,aux
    if name in ('maxpool2d','avgpool2d'):
        h,w=case['height'],case['width'];stride=case.get('stride',1);o=(s[0],s[1],(s[2]-h)//stride+1,(s[3]-w)//stride+1);v=[]
        for n,c,y,x in coords(o):
            cells=[at(a,s,(n,c,y*stride+ky,x*stride+kx)) for ky in range(h) for kx in range(w)];v.append(max(cells) if name=='maxpool2d' else math.fsum(cells)/len(cells))
        return o,v,aux
    if name in ('nearest2d','bilinear2d'):
        o=(s[0],s[1],case['height'],case['width']);v=[]
        for n,c,y,x in coords(o):
            if name=='nearest2d':v.append(at(a,s,(n,c,y*s[2]//o[2],x*s[3]//o[3])));continue
            yy=max(0.,min(s[2]-1.,(y+.5)*s[2]/o[2]-.5));xx=max(0.,min(s[3]-1.,(x+.5)*s[3]/o[3]-.5));y0=int(yy);x0=int(xx);y1=min(y0+1,s[2]-1);x1=min(x0+1,s[3]-1);fy=yy-y0;fx=xx-x0
            v.append(math.fsum([at(a,s,(n,c,y0,x0))*(1-fy)*(1-fx),at(a,s,(n,c,y0,x1))*(1-fy)*fx,at(a,s,(n,c,y1,x0))*fy*(1-fx),at(a,s,(n,c,y1,x1))*fy*fx]))
        return o,v,aux
    if name=='mse':return (),[math.fsum((x-y)**2 for x,y in zip(a,b))/len(a)],aux
    if name=='cross_entropy':
        losses=[]
        for row in range(s[0]):
            xs=a[row*s[1]:(row+1)*s[1]];m=max(xs);losses.append(m-xs[idx[row]]+math.log(math.fsum(math.exp(x-m) for x in xs)))
        return (),[math.fsum(losses)/s[0]],aux
    raise AssertionError(name)
def case(name,s,a,t=(0,),b=(),**kw):return dict(name=name,ashape=tuple(s),a=[f32(x) for x in a],bshape=tuple(t),b=[f32(x) for x in b],**kw)
def cases():
    rows=[]
    for n in NAMES[:4]+NAMES[8:10]:rows.append(case(n,(2,3),[1,-2,3,4,-5,6],(3,),[2,3,4]))
    for n in NAMES[4:8]:rows.append(case(n,(3,),[.25,1,4] if n=='logarithm' else [-1,.5,2]))
    for n in ['sum','mean','maximum','softmax']:rows.append(case(n,(2,3),[.5,2,-1,4,1.5,-2],axis=1))
    rows += [case('sum',(2,3),[1,2,3,4,5,6],axis=0,keep=True),case('softmax',(2,3),[1,-1,2,3,2,-2],axis=0)]
    rows += [case('take',(3,2),[1,2,3,4,5,6],indices=[2,0,2]),case('update',(3,2),[1,2,3,4,5,6],(3,2),[10,20,30,40,50,60],indices=[2,0,2]),case('concat',(2,2),[1,2,3,4],(2,1),[5,6],axis=1)]
    rows += [case('sort',(2,4),[3,-1,4,2,8,6,5,7],axis=1,descending=False),case('topk',(2,4),[3,-1,4,2,8,6,5,7],axis=1,k=2),case('matmul',(2,3),[1,2,-1,3,-2,4],(3,2),[2,-1,3,4,-2,1])]
    rows += [case('conv2d',(1,2,3,3),[(i-8)/4 for i in range(18)],(2,2,2,2),[(i-7)/8 for i in range(16)])]
    for n in ['maxpool2d','avgpool2d']:rows.append(case(n,(1,1,3,4),[i+1 for i in range(12)],height=2,width=2,stride=1))
    rows += [case('nearest2d',(1,1,2,3),[1,2,3,4,5,6],height=3,width=2),case('bilinear2d',(1,1,2,2),[1,2,3,4],height=3,width=3),case('mse',(2,3),[1,-2,3,4,1,2],(2,3),[2,1,-1,3,0,4]),case('cross_entropy',(2,3),[.5,-1,2,3,-.5,1],indices=[2,0])]
    rows += [case('add',(),[2],(2,3),[1,2,3,4,5,6]),case('add',(0,3),[],(1,3),[1,2,3]),case('sum',(2,0),[],axis=1),case('matmul',(2,0),[],(0,3),[]),case('take',(2,3),[1,2,3,4,5,6],axis=1,indices=[]),case('concat',(2,0),[],(2,3),[1,2,3,4,5,6],axis=1),case('topk',(3,),[1,2,3],k=0),case('conv2d',(1,0,3,3),[],(2,0,2,2),[]),case('bilinear2d',(1,1,1,1),[3],height=3,width=4)]
    return rows
def gradients(c,dy):
    def loss(a,b):return math.fsum(x*y for x,y in zip(reference(c,a,b)[1],dy))
    results=[]
    for field in ['a','b']:
        out=[]
        for i,x in enumerate(c[field]):
            h=1e-5*(1+abs(x));plus=list(c[field]);minus=list(c[field]);plus[i]+=h;minus[i]-=h
            if field=='a':g=(loss(plus,c['b'])-loss(minus,c['b']))/(2*h)
            else:g=(loss(c['a'],plus)-loss(c['a'],minus))/(2*h)
            out.append(g)
        results.append(out)
    return results

def cfloat(x):
    v=format(x,'.9g');return v+('' if '.' in v or 'e' in v else '.0')+'f'
def array(xs,fn=str):return '{'+','.join(fn(x) for x in xs)+'}' if xs else '{0}'
def shape_c(s):return '{'+str(len(s))+',{{'+','.join(str(x) for x in s+(0,)*(4-len(s)))+'}}}'
def header(rows):
    out=['/* Independent scalar/central-difference fixtures. Generated by tensor_ops_oracle.py. */','typedef struct {const char *name;coax_t_ToRequest request;coax_t_TvShape a_shape,b_shape,out_shape;int na,nb,n,ni,aux,differentiable;float a[64],b[64],dy[64];int64_t indices[16],expected_aux[64];double expected[64],ga[64],gb[64];} ToOracle;','static const ToOracle to_oracles[]={']
    for c in rows:
        os,ys,aux=reference(c);n=len(ys);dy=[((i%7)-3)*.25 for i in range(n)];diff=c['name'] not in ['equal','less'];ga,gb=gradients(c,dy) if diff else ([],[])
        assert all(len(v)<=64 for v in [c['a'],c['b'],ys,ga,gb]);assert math.prod(os)==n
        r='{.op='+str(NAMES.index(c['name']))+',.axis='+str(c.get('axis',0))+',.keep='+str(int(c.get('keep',False)))+',.descending='+str(int(c.get('descending',True)))+',.k='+str(c.get('k',0))+',.height='+str(c.get('height',0))+',.width='+str(c.get('width',0))+',.stride='+str(c.get('stride',1))+'}'
        fields=[json.dumps(c['name']),r,shape_c(c['ashape']),shape_c(c['bshape']),shape_c(os),str(len(c['a'])),str(len(c['b'])),str(n),str(len(c.get('indices',[]))),str(len(aux)),str(int(diff)),array(c['a'],cfloat),array(c['b'],cfloat),array(dy,cfloat),array(c.get('indices',[])),array(aux),array(ys,repr),array(ga,repr),array(gb,repr)]
        out.append('{'+','.join(fields)+'},')
    out += ['};','static const char *to_manifest_names[]={'+','.join(json.dumps(x) for x in NAMES)+'};','static const int to_manifest_arities[]={'+','.join(str(2 if n in BINARY else 1) for n in NAMES)+'};']
    return '\n'.join(out)+'\n'
def main():
    p=argparse.ArgumentParser();p.add_argument('--header',action='store_true');p.add_argument('--write-manifest',action='store_true');a=p.parse_args();path=ROOT/'psionic/fixtures/tensor-ops/operators.json'
    if a.write_manifest:path.write_text(json.dumps(manifest(),indent=2)+'\n')
    assert json.loads(path.read_text())==manifest(),'operator manifest drift'
    rows=cases();assert set(c['name'] for c in rows)==set(NAMES)
    if a.header:print(header(rows),end='')
    else:print(f'operator manifest: {len(NAMES)} CPU operators; independent scalar/central-difference cases: {len(rows)}')
if __name__=='__main__':main()
