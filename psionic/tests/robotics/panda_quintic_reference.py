#!/usr/bin/env python3
"""New independent quintic timing oracle, no product recurrence or old gate.

Fraction rounded-operation model fixes binary64 timing policy. Decimal80 real
extrema and direct polynomials independently check reserve and sparse samples.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext,ROUND_FLOOR
import argparse,hashlib,json,math,struct,time

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-quintic'
CV=1.875;CA=5.773502691896258;CAP=999999.;EPS=2.**-20
TIME=struct.Struct('<3q2d');SAMPLE=struct.Struct('<q21d')
SOURCES={'psionic/fixtures/robotics/panda-fk/sdk-model.json':'4e8f3aca4884775f5d6a9a5d7c186de70ee3798fa46ed72032d657ac60e6a027','psionic/fixtures/robotics/panda-fk/sdk-cases.f64':'482fb40c4c7bac35ff733a1cdb9de071f534959d552a69d89d94dd551787186b'}
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def identity(p):return dict(bytes=Path(p).stat().st_size,sha256=sha(p))
def write(p,x):p.write_text(json.dumps(x,indent=2)+'\n')
def finite(x,n,label):
    require(len(x)==n and all(math.isfinite(v)for v in x),label+' ALL finite/complete');return list(x)
def f(x):return Fraction.from_float(x)
def dec(x):return Decimal(x.numerator)/Decimal(x.denominator)
def rn(x):return float(x)
def mul(a,b):return rn(f(a)*f(b))
def div(a,b):return rn(f(a)/f(b))
def square_root(x):
    if x==0:return 0.
    with localcontext()as ctx:ctx.prec=80;y=float(dec(f(x)).sqrt())
    lo=(f(math.nextafter(y,-math.inf))+f(y))/2;hi=(f(math.nextafter(y,math.inf))+f(y))/2
    require(lo*lo<f(x)<hi*hi or(f(x)in[lo*lo,hi*hi]and struct.unpack('<Q',struct.pack('<d',y))[0]%2==0),'independent correctly rounded sqrt interval');return y
def policy(row):
    q0,q1,v,a=[row[i*7:(i+1)*7]for i in range(4)]
    if all(x==y for x,y in zip(q0,q1)):return dict(status=0,steps=0,ns=0,duration=0.,estimate=0.,R=0.,zero=True)
    candidates=[]
    for x,y,vl,al in zip(q0,q1,v,a):
        d=abs(rn(f(y)-f(x)));candidates.extend([div(mul(CV,d),vl),square_root(div(mul(CA,d),al))])
    estimate=max(candidates);ratio=mul(estimate,500.)
    if ratio>CAP:return dict(status=5,steps=0,ns=0,duration=0.,estimate=0.,R=ratio,zero=False,unpublished_estimate=estimate)
    r=f(ratio);n=(r.numerator+r.denominator-1)//r.denominator+1;require(1<=n<=1000000,'policy integer headroom');return dict(status=0,steps=n,ns=n*2000000,duration=rn(Fraction(n,500)),estimate=estimate,R=ratio,zero=False)
def rng(seed):
    while True:seed=(1664525*seed+1013904223)&0xffffffff;yield(seed+.5)/2**32
def source_identity():
    result=dict(SOURCES);result[str(Path(__file__).resolve().relative_to(ROOT))]=sha(__file__);require(all(sha(ROOT/p)==h for p,h in result.items()),'immutable original source identities');return result
def freeze():
    bound=source_identity();FIXTURE.mkdir(parents=True,exist_ok=True);require(not(FIXTURE/'experiment.json').exists(),'never overwrite frozen inputs');model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text());limits=[j['range']for j in model['arm_joints']];home=list(struct.unpack('<7d',(ROOT/'psionic/fixtures/robotics/panda-fk/sdk-cases.f64').read_bytes()[:56]));edges=[]
    def add(name,q0,q1,v=None,a=None):
        row=finite(list(q0)+list(q1)+(v if v is not None else[2.]*7)+(a if a is not None else[10.]*7),28,'edge input');require(all(lo<=x<=hi for values in[q0,q1]for x,(lo,hi)in zip(values,limits))and all(.001<=x<=20 for x in row[14:21])and all(.001<=x<=100 for x in row[21:]),'source and limit domains');edges.append(dict(index=len(edges),name=name,input_f64=row))
    dense=[lo+.37*(hi-lo)for lo,hi in limits];upper=[hi for lo,hi in limits];lower=[lo for lo,hi in limits]
    add('zero-home',home,home);add('zero-dense',dense,dense);add('zero-exact-upper',upper,upper)
    for name,j,delta,vl,al in[('single-velocity-positive',0,.3,.05,100.),('single-velocity-negative',2,-.4,.1,100.),('single-acceleration-positive',1,.1,20.,.01),('single-acceleration-negative',5,-.25,20.,.02),('single-small-elbow',3,.01,1.,1.),('single-subnormal',0,math.ulp(0.),20.,100.)]:
        target=home[:];target[j]+=delta;v=[20.]*7;a=[100.]*7;v[j]=vl;a[j]=al;add(name,home,target,v,a)
    random=rng(0x5155494e)
    for k in range(6):
        q0=[lo+(.1+.8*next(random))*(hi-lo)for lo,hi in limits];q1=[lo+(.1+.8*next(random))*(hi-lo)for lo,hi in limits];v=[.05+2.95*next(random)for _ in range(7)];a=[.1+29.9*next(random)for _ in range(7)];add('dense-'+str(k),q0,q1,v,a)
    add('whole-box-forward',lower,upper);add('whole-box-reverse',upper,lower)
    target=home[:];target[0]=1.
    for name,value in[('below',math.nextafter(CV,0)),('equal',CV),('above',math.nextafter(CV,math.inf))]:v=[20.]*7;v[0]=value;add('velocity-integer-'+name,home,target,v,[100.]*7)
    with localcontext()as ctx:ctx.prec=80;center=float((Decimal(10)/Decimal(3).sqrt())*Decimal('.25')/(Decimal('.2')**2))
    target=home[:];target[0]=.25
    for name,value in[('below',math.nextafter(center,0)),('center',center),('above',math.nextafter(center,math.inf))]:a=[100.]*7;a[0]=value;add('acceleration-integer-'+name,home,target,[20.]*7,a)
    add('valid-input-duration-refusal',lower,upper,[.001]*7,[1.]*7)
    start=home[:];start[0]=-1.;target=home[:];target[0]=1.;vcenter=float(Fraction(1875,999999));candidates=[vcenter]
    for sign in[-math.inf,math.inf]:
        value=vcenter
        for _ in range(8):value=math.nextafter(value,sign);candidates.append(value)
    exact=[]
    for value in candidates:
        row=start+target+[value]+[20.]*6+[100.]*7
        if policy(row)['R']==999999.:exact.append(value)
    require(exact,'pre-freeze cap fixture with exact rounded R999999');capcenter=min(exact,key=lambda x:abs(x-vcenter));construction=[]
    for name,value in[('below-velocity',math.nextafter(capcenter,0)),('exact',capcenter),('above-velocity',math.nextafter(capcenter,math.inf))]:
        v=[20.]*7;v[0]=value;add('duration-cap-'+name,start,target,v,[100.]*7);construction.append(dict(name=name,velocity=value,policy=policy(edges[-1]['input_f64'])))
    require(len(edges)==27 and any(x['policy']['status']==5 for x in construction)and any(x['policy']['steps']==1000000 for x in construction),'27edges and both duration-cap outcomes before freeze');(FIXTURE/'edges.f64').write_bytes(struct.pack('<756d',*[x for c in edges for x in c['input_f64']]))
    e=dict(schema='psionic.panda-quintic.experiment.v1',frozen_before_expected_generation_and_product_comparison=True,scope='Rest-to-rest quintic QUERY, no executed motion,collision,tracking,jerk bound,global/grid-minimal time or compiler clock/bounded certificate.',joint_limits_radians=limits,velocity_limits=[.001,20],acceleration_limits=[.001,100],maximum_steps=1000000,frequency_hertz=500,nanoseconds_per_tick=2000000,Cv=CV,Ca=CA,constant_u64=list(struct.unpack('<2Q',struct.pack('<2d',CV,CA))),timing_recipe='RN dq=q1-q0; Tv=RN(RN(Cv*abs(dq))/v); Ta=RN_sqrt(RN(RN(Ca*abs(dq))/a)); Tmax=max14 candidates; R=RN(Tmax*500); rejectR>999999 beforecast; numericzeroedgeN0 otherwiseN=ceil(R)+1; duration=RN(N/500),ns=N*2000000.',zero_edge='All endpoint angles numerically equal after all input admission;N0 sampletick0 copiesq1,including signed-zero representation.',sample_method='Decimal80 direct analytic polynomial and exact integer t/T; not product half-evaluator or recurrence. Exact endpoint copies,positivezero endpoint derivatives.',timing_error_epsilon_ticks=EPS,numerical_assumptions='binary64 RN,gradualunderflow,correctsqrt,noFMA/reassociation/fastmath. Real requirement usesexactendpointdifference and10/sqrt3.',budgets=dict(position_radians_atol=2e-12,derivatives_atol=2e-10,derivatives_rtol=2e-12,time_atol=2e-12,time_rtol=2e-12,steps='exact rounded-operation policy',nanoseconds='exact',status='exact',endpoint_bits='exact',mode_bytes='exact'),edges=edges,edge_count=27,edge_file=identity(FIXTURE/'edges.f64'),cap_input_construction=construction,source_sha256=bound,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False)
    write(FIXTURE/'experiment.json',e);record=dict(schema='psionic.panda-quintic.input-freeze.v1',passed=True,experiment=identity(FIXTURE/'experiment.json'),edges=identity(FIXTURE/'edges.f64'),construction_policy_probes_only=True,full_expected_generated=False,product_comparisons=0);write(FIXTURE/'input-freeze-v1.json',record);print(json.dumps(record))
def real_requirement(row):
    q0,q1,v,a=[row[i*7:(i+1)*7]for i in range(4)];ca=Decimal(10)/Decimal(3).sqrt();tv=[];ta=[]
    for x,y,vl,al in zip(q0,q1,v,a):d=abs(dec(f(y)-f(x)));tv.append(Decimal('1.875')*d/dec(f(vl)));ta.append((ca*d/dec(f(al))).sqrt())
    return max(tv+ta),tv,ta
def analytic_sample(row,n,k):
    q0,q1,_,_=[row[i*7:(i+1)*7]for i in range(4)]
    if k==n:return list(q1)+[0.]*14
    if k==0:return list(q0)+[0.]*14
    u=Decimal(k)/Decimal(n);t=Decimal(n)/500;s=10*u**3-15*u**4+6*u**5;s1=30*u**2-60*u**3+30*u**4;s2=60*u-180*u**2+120*u**3;positions=[];rates=[];accelerations=[]
    for x,y in zip(q0,q1):d=dec(f(y)-f(x));positions.append(float(dec(f(x))+d*s));rates.append(float(d*s1/t));accelerations.append(float(d*s2/(t*t)))
    return finite(positions+rates+accelerations,21,'ALL analytic samples')
def generate(out):
    require(not out.exists(),'preserve every original/failed reference');out.mkdir(parents=True);record=dict(schema='psionic.panda-quintic.independent-reference.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False,product_recurrence=False);began=time.monotonic()
    try:
        bound=source_identity();e=json.loads((FIXTURE/'experiment.json').read_text());freeze=json.loads((FIXTURE/'input-freeze-v1.json').read_text());require(freeze['passed']and freeze['experiment']==identity(FIXTURE/'experiment.json')and e['source_sha256']==bound,'frozen source/input recipe');require(identity(FIXTURE/'edges.f64')==e['edge_file']and(FIXTURE/'edges.f64').read_bytes()==struct.pack('<756d',*[x for c in e['edges']for x in c['input_f64']]),'all27input bits');timings=bytearray();samples=bytearray();query_bytes=bytearray();queries=[];plans=[];maxerror=Decimal(0);minreserve=None
        with localcontext()as ctx:
            ctx.prec=80;ca=Decimal(10)/Decimal(3).sqrt();critical=(Decimal(3)-Decimal(3).sqrt())/6
            for edge in e['edges']:
                row=edge['input_f64'];p=policy(row);real,tv,ta=real_requirement(row);rreal=real*500;error=abs(Decimal.from_float(p['R'])-rreal);maxerror=max(maxerror,error);require(error<Decimal.from_float(EPS),'declared absolute tick error bound,including subnormal');timings+=TIME.pack(p['status'],p['steps'],p['ns'],p['duration'],p['estimate']);n=p['steps'];peaks_v=[0.]*7;peaks_a=[0.]*7
                if p['status']==0 and not p['zero']:
                    reserve=Decimal(n)-rreal;require(reserve>=1-Decimal.from_float(EPS),'conservative reserve tick against REAL requirement');minreserve=reserve if minreserve is None else min(minreserve,reserve);t=Decimal(n)/500
                    for j in range(7):d=abs(dec(f(row[7+j])-f(row[j])));peaks_v[j]=float(Decimal('1.875')*d/t);peaks_a[j]=float(ca*d/(t*t));require(Decimal('1.875')*d/t<=dec(f(row[14+j]))and ca*d/(t*t)<=dec(f(row[21+j])),'continuous derivative extrema meet every limit')
                else:reserve=None
                plans.append(dict(edge=edge['index'],name=edge['name'],**p,real_required_seconds=str(real),real_required_ticks=str(rreal),absolute_tick_error=str(error),reserve_ticks=str(reserve)if reserve is not None else None,continuous_peak_rates=peaks_v,continuous_peak_accelerations=peaks_a))
                if p['status']!=0:ticks=[0]
                elif n==0:ticks=[0]
                else:
                    ticks={0,1,n//2,n-1,n,n//4,(3*n)//4}
                    for point in[critical,1-critical]:z=point*n;floor=int(z.to_integral_value(rounding=ROUND_FLOOR));ticks.update([max(0,min(n,floor)),max(0,min(n,floor+1))])
                    ticks=sorted(ticks)
                for k in ticks:
                    status=p['status'];values=analytic_sample(row,n,k)if status==0 else[0.]*21;samples+=SAMPLE.pack(status,*values);query_bytes+=struct.pack('<2q',edge['index'],k);queries.append(dict(index=len(queries),edge=edge['index'],tick=k,status=status,steps=n))
        require(len(timings)==27*40 and len(samples)==len(queries)*176 and len(query_bytes)==len(queries)*16,'complete timing/sample extents')
        for i in range(27):finite(TIME.unpack_from(timings,i*40)[3:],2,'ALL accepted/unavailable timing fields')
        for i in range(len(queries)):finite(SAMPLE.unpack_from(samples,i*176)[1:],21,'ALL accepted/unavailable sample fields')
        (out/'timing-expected.bin').write_bytes(timings);(out/'sample-expected.bin').write_bytes(samples);(out/'sample-queries.bin').write_bytes(query_bytes);write(out/'sample-queries.json',queries);write(out/'plans.json',plans);require(source_identity()==bound,'all executed source identities unchanged');record.update(passed=True,experiment=identity(FIXTURE/'experiment.json'),input_freeze=identity(FIXTURE/'input-freeze-v1.json'),source_sha256=bound,budgets=e['budgets'],decimal_precision=80,edges=27,timing_ok=sum(p['status']==0 for p in plans),duration_refusals=sum(p['status']==5 for p in plans),zero_edges=sum(p['zero']for p in plans),sample_queries=len(queries),successful_samples=sum(q['status']==0 for q in queries),unavailable_samples=sum(q['status']!=0 for q in queries),maximum_absolute_tick_error=str(maxerror),minimum_real_reserve_ticks=str(minreserve),all_expected_timing_finite_words=54,all_expected_sample_finite_words=len(queries)*21,outputs={name:identity(out/name)for name in['timing-expected.bin','sample-expected.bin','sample-queries.bin','sample-queries.json','plans.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;write(out/'reference.json',record)
    print(json.dumps(dict(passed=True,reference=str(out/'reference.json'),edges=27,queries=record['sample_queries'],duration_refusals=record['duration_refusals'],zero_edges=record['zero_edges'])))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--freeze',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args();require(args.freeze!=bool(args.output),'choose freeze or generation');freeze()if args.freeze else generate(args.output.resolve())
if __name__=='__main__':main()
