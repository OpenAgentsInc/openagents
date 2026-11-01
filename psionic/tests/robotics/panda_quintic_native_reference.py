#!/usr/bin/env python3
"""Offline rest-to-rest quintic query proof against new cached analytic targets.

No motion execution, collision/tracking certificate, SDK, NN or old gate.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext
import argparse,hashlib,json,math,os,platform,struct,time
from panda_pose_ik_native_reference import Run,identity,sha,graph,finite,require
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-quintic';SOURCE=ROOT/'psionic/lib/robotics/panda-quintic.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_quintic_native.c'
EXPERIMENT_SHA='642fbb94e348acf85f4594737ecbabf7e2619260cc2fe66954650613a45ffd99'
TIME=struct.Struct('<3q2d');SAMPLE=struct.Struct('<q21d')

def bits(x):return struct.unpack('<Q',struct.pack('<d',x))[0]
def rounded_sqrt(x):
    require(math.isfinite(x)and x>=0,'finite nonnegative sqrt input')
    if x==0:return 0.
    y=math.sqrt(x);F=Fraction.from_float;low=(F(math.nextafter(y,0))+F(y))/2;high=(F(y)+F(math.nextafter(y,math.inf)))/2;target=F(x);even=bits(y)%2==0
    require((low*low<target<high*high)or(even and target in[low*low,high*high]),'independently certified RN sqrt midpoint interval');return y

def policy(row):
    F=Fraction.from_float;maximum=0.;zero=all(row[j]==row[7+j]for j in range(7))
    for j in range(7):
        displacement=abs(float(F(row[7+j])-F(row[j])));velocity=float(F(float(F(1.875)*F(displacement)))/F(row[14+j]));acceleration=rounded_sqrt(float(F(float(F(5.773502691896258)*F(displacement)))/F(row[21+j])));maximum=max(maximum,velocity,acceleration)
    R=float(F(maximum)*500)
    if R>999999:return (5,0,0,0.,0.),R
    N=0 if zero else math.ceil(R)+1;return (0,N,N*2000000,float(Fraction(N,500)),0. if zero else maximum),R

def decode_time(raw):
    require(len(raw)==27*40,'exact27 complete40B timing rows');rows=list(TIME.iter_unpack(raw));finite([v for row in rows for v in row[3:]],54,'ALL actual/expected timing fields before maxima')
    for i,row in enumerate(rows):
        require(row[0]in[0,5],'timing status domain')
        if row[0]:require(raw[i*40+8:(i+1)*40]==bytes(32),'unavailable timing zero transport after C full atomicity assertions')
    return rows

def decode_samples(raw):
    require(len(raw)==238*176,'exact238 complete176B sample rows');rows=list(SAMPLE.iter_unpack(raw));finite([v for row in rows for v in row[1:]],4998,'ALL actual/expected sample fields before maxima')
    for i,row in enumerate(rows):
        require(row[0]in[0,5],'sample status domain')
        if row[0]:require(raw[i*176+8:(i+1)*176]==bytes(168),'unavailable sample zero transport after C full atomicity assertions')
    return rows

def frozen(oracle):
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'exact frozen27-edge experiment');e=json.loads((FIXTURE/'experiment.json').read_text());r=json.loads((oracle/'reference.json').read_text());require(r['passed']and r['decimal_precision']==80 and not r['product_recurrence']and r['SDK_calls']==r['Coax_calls']==r['NN_calls']==0 and not r['old_gates_repeated']and r['experiment']==identity(FIXTURE/'experiment.json')and r['budgets']==e['budgets'],'independent analytic reference identity/scope')
    require(all(identity(oracle/name)==value for name,value in r['outputs'].items())and all(sha(ROOT/name)==digest for name,digest in r['source_sha256'].items()),'all frozen oracle source/output identities');edges=(FIXTURE/'edges.f64').read_bytes();require(len(edges)==27*224 and identity(FIXTURE/'edges.f64')==e['edge_file']and edges==b''.join(struct.pack('<28d',*c['input_f64'])for c in e['edges']),'all frozen edge JSON/binary bits');finite(struct.unpack('<756d',edges),756,'ALL input words')
    require(e['constant_u64']==[bits(1.875),bits(5.773502691896258)]and e['maximum_steps']==1000000 and e['frequency_hertz']==500 and e['nanoseconds_per_tick']==2000000,'exact constants and integer clock')
    for c in e['edges']:
        x=c['input_f64'];require(all(e['joint_limits_radians'][j][0]<=x[j]<=e['joint_limits_radians'][j][1]and e['joint_limits_radians'][j][0]<=x[7+j]<=e['joint_limits_radians'][j][1]and .001<=x[14+j]<=20 and .001<=x[21+j]<=100 for j in range(7)),'independent input admission')
    plans=json.loads((oracle/'plans.json').read_text());wanted_time=decode_time((oracle/'timing-expected.bin').read_bytes());wanted_sample=decode_samples((oracle/'sample-expected.bin').read_bytes());require(len(plans)==27,'all analytic extrema plans');finite([v for p in plans for key in['continuous_peak_rates','continuous_peak_accelerations']for v in p[key]],378,'ALL cached analytic extrema before maxima')
    computed=[];epsilon=Decimal.from_float(e['timing_error_epsilon_ticks']);margin=[]
    for i,(c,p,wanted)in enumerate(zip(e['edges'],plans,wanted_time)):
        actual,R=policy(c['input_f64']);require(actual==wanted and p['status']==actual[0]and p['steps']==actual[1]and p['ns']==actual[2]and p['R']==R,'independent Fraction-RN+sqrt-certified policy '+str(i));computed.append(actual)
        real_ticks=Decimal(p['real_required_ticks']);tick_error=Decimal(p['absolute_tick_error']);require(real_ticks.is_finite()and tick_error.is_finite()and tick_error<epsilon,'finite cached real tick error before bound')
        with localcontext()as context:
            context.prec=90;require(abs(Decimal.from_float(R)-real_ticks)==tick_error,'cached highprecision diagnostic matches rounded policy R')
        if actual[0]==0:
            x=c['input_f64'];require(all(0<=p['continuous_peak_rates'][j]<=x[14+j]and 0<=p['continuous_peak_accelerations'][j]<=x[21+j]for j in range(7)),'cached continuous analytic extrema within original limits')
            if actual[1]>0:
                reserve=Decimal(p['reserve_ticks']);require(reserve.is_finite()and reserve>=1-epsilon,'cached declared real reserve on accepted nonzero edge');margin.append(str(reserve))
    require([x[1]for x in computed[17:20]]==[502,501,501]and computed[8][1]==1 and e['edges'][8]['input_f64'][0]!=e['edges'][8]['input_f64'][7],'near-grid operation policy and genuinely nonzero subnormal')
    require(plans[24]['R']==math.nextafter(999999.,math.inf)and computed[24][0]==5 and plans[25]['R']==999999. and plans[26]['R']==math.nextafter(999999.,0)and computed[25][1]==computed[26][1]==1000000,'independent cap/IEEE-neighbor fixture semantics')
    raw_queries=(oracle/'sample-queries.bin').read_bytes();require(len(raw_queries)==238*16,'exact238 edge/tick pairs');queries=list(struct.iter_unpack('<2q',raw_queries));metadata=json.loads((oracle/'sample-queries.json').read_text());require(len(metadata)==238 and raw_queries==b''.join(struct.pack('<2q',q['edge'],q['tick'])for q in metadata),'authoritative metadata/query bytes')
    for i,((edge,tick),q,wanted)in enumerate(zip(queries,metadata,wanted_sample)):
        require(q['index']==i and 0<=edge<27 and q['steps']==computed[edge][1]and q['status']==computed[edge][0]==wanted[0]and tick>=0 and (tick<=computed[edge][1]if computed[edge][0]==0 else tick==0),'all sample query statuses/headers/ticks')
    for edge,timing in enumerate(computed):
        ticks=[tick for owner,tick in queries if owner==edge];require(ticks==sorted(set(ticks)),'unique sorted sparse edge ticks')
        if timing[0]:require(ticks==[0],'duration-refusal propagation sample');continue
        N=timing[1];required={0,N,N//2,N//4,3*N//4}
        if N:
            k=math.floor(N*(3-math.sqrt(3))/6);L=Fraction(3)-Fraction(6*k,N);U=Fraction(3)-Fraction(6*(k+1),N);require(L>=0 and L*L>3 and(U<0 or U*U<3),'exact rational bracketing of irrational acceleration-extremum tick');required|={1,N-1,k,k+1,N-k-1,N-k}
        require(required<=set(ticks),'endpoints/quarters/midpoint/adjacent-extremum sparse coverage')
    require(r['timing_ok']==25 and r['duration_refusals']==2 and r['sample_queries']==238 and r['successful_samples']==236 and r['unavailable_samples']==2,'exact independent payload inventory');return e,r,edges,raw_queries,queries,wanted_time,wanted_sample

def mixed(pairs,atol,rtol,label):
    require(pairs and all(math.isfinite(a)and math.isfinite(b)for a,b in pairs),'finite comparison pairs '+label);errors=[abs(a-b)for a,b in pairs];limits=[atol+rtol*abs(b)for a,b in pairs];require(all(math.isfinite(x)and x<=y for x,y in zip(errors,limits)),label+' fixed per-scalar mixed budget');return dict(passed=True,values=len(pairs),max_abs_error=max(errors),maximum_budget_fraction=max(x/y for x,y in zip(errors,limits)),atol=atol,rtol=rtol)

def compare(timing_raw,sample_raw,wanted_time,wanted_sample,queries,e):
    timings=decode_time(timing_raw);samples=decode_samples(sample_raw);timepairs=[];positions=[];rates=[];accelerations=[];endpoints=0
    for a,b in zip(timings,wanted_time):
        require(a[:3]==b[:3],'exact timing status/N/integer-nanoseconds')
        if a[0]==0:timepairs.extend(zip(a[3:],b[3:]))
    for i,(a,b,(edge,tick))in enumerate(zip(samples,wanted_sample,queries)):
        require(a[0]==b[0],'exact sample status '+str(i))
        if a[0]:continue
        x=e['edges'][edge]['input_f64'];N=timings[edge][1];values=a[1:];positions.extend(zip(values[:7],b[1:8]));rates.extend(zip(values[7:14],b[8:15]));accelerations.extend(zip(values[14:21],b[15:22]));require(all(min(x[j],x[7+j])<=values[j]<=max(x[j],x[7+j])and abs(values[7+j])<=x[14+j]and abs(values[14+j])<=x[21+j]for j in range(7)),'actual interval/velocity/acceleration bounds')
        if tick in[0,N]:
            target=x[7:14]if tick==N else x[:7];require(sample_raw[i*176+8:i*176+64]==struct.pack('<7d',*target)and sample_raw[i*176+64:(i+1)*176]==bytes(112),'exact endpoint copied representation and positivezero derivatives');endpoints+=1
    budget=e['budgets'];return dict(passed=True,timing_ok=25,duration_refusals=2,sample_ok=236,unavailable_samples=2,all_actual_timing_finite_words=54,all_actual_sample_finite_words=4998,exact_endpoint_samples=endpoints,steps_nanoseconds_status_exact=True,metrics=dict(time=mixed(timepairs,budget['time_atol'],budget['time_rtol'],'time'),position=mixed(positions,budget['position_radians_atol'],0,'angle'),rate=mixed(rates,budget['derivatives_atol'],budget['derivatives_rtol'],'rate'),acceleration=mixed(accelerations,budget['derivatives_atol'],budget['derivatives_rtol'],'acceleration')))

def reader_guards(t,s,wt,ws):
    count=0
    for raw,decode in[(t,decode_time),(s,decode_samples),(wt,decode_time),(ws,decode_samples)]:
        bad=[raw[:-1],raw+b'\0']
        for v in[math.nan,math.inf,-math.inf]:
            value=bytearray(raw);struct.pack_into('<d',value,len(value)-8,v);bad.append(bytes(value))
        for value in bad:
            try:decode(value)
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted extent/lateIEEE poison')
    require(count==20,'all20 actual/expected reader poisons');return dict(passed=True,cases=count)

def native(args):
    require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile');oracle=args.oracle.resolve();e,reference,edges,raw_queries,queries,wanted_time,wanted_sample=frozen(oracle);out=args.output.resolve();require(not out.exists(),'preserve every previous/partial attempt');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-quintic.native.v1',passed=False,SDK_calls=0,NN_calls=0,old_gates_repeated=False,oracle_regenerated=False,motion_executed=False,experiment=identity(FIXTURE/'experiment.json'),reference=identity(oracle/'reference.json'),compiler=identity(compiler),commands=[],budgets=e['budgets']);runner=Run(out,record);runner.save();began=time.monotonic()
    try:
        dependencies=graph(SOURCE)
        for p in[NATIVE,Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_pose_ik_native_reference.py']+[p for directory in[FIXTURE,oracle]for p in directory.iterdir()if p.is_file()]:dependencies[p]=identity(p)
        record['frozen_inputs']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):value for p,value in sorted(dependencies.items())};record['independent_preflight']=dict(edges=27,sample_queries=238,Fraction_RN_operations=True,sqrt_RN_midpoint_certified=True,exact_cap_and_near_grid_fixtures=True,subnormal_nonzero_not_misclassified=True,cached_continuous_extrema_and_real_reserve_checked=True,no_motion_or_collision_claim=True);(out/'input.bin').write_bytes(struct.pack('<4I',0x31545150,27,238,1)+edges+raw_queries+struct.pack('<14d',*[x[0]for x in e['joint_limits_radians']],*[x[1]for x in e['joint_limits_radians']]));record['input']=identity(out/'input.bin');emissions={};runner.save()
        for mode,extra in[('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'panda-quintic.c';h=d/'panda-quintic.h';c.write_bytes(runner.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={m:dict(C=identity(c),header=identity(h))for m,(c,h)in emissions.items()};runner.save();outputs=[];common=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off']
        for mode,source_mode,flags in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/('native-'+mode);d.mkdir();c,h=emissions[source_mode];binary=d/'native';runner.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o']);runner.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*common,*flags,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',binary]);report=json.loads(runner.command('run-'+mode,[binary,out/'input.bin',d/'timing.bin',d/'sample.bin']));require(report['passed']and report['SDK_calls']==0 and report['edges']==27 and report['samples']==238 and report['time_record_bytes']==40 and report['sample_record_bytes']==176 and report['alias_cases']==24 and report['source_and_limit_endpoints']==56 and report['tick_refusals']==10 and report['duration_refused_edges']==2 and report['nonfinite_role_patterns']==140 and report['fpcr']==0,'all exact new native inventory');t=(d/'timing.bin').read_bytes();s=(d/'sample.bin').read_bytes();record.setdefault('modes',{})[mode]=dict(native=report,comparison=compare(t,s,wanted_time,wanted_sample,queries,e),binary=identity(binary),timing=identity(d/'timing.bin'),sample=identity(d/'sample.bin'));outputs.append((t,s));runner.save()
        require(outputs[0]==outputs[1]==outputs[2],'complete timing/sample files byte-identical across profiles');record['new_reader_guards']=reader_guards(*outputs[0],(oracle/'timing-expected.bin').read_bytes(),(oracle/'sample-expected.bin').read_bytes());require(all(identity(p)==v for p,v in dependencies.items())and identity(compiler)==record['compiler'],'all measured dependencies/compiler unchanged');record.update(passed=True,modes_byte_identical=True,timing_sha256=hashlib.sha256(outputs[0][0]).hexdigest(),sample_sha256=hashlib.sha256(outputs[0][1]).hexdigest())
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;record['partial_or_full_outputs']={str(p.relative_to(out)):identity(p)for pattern in['native-*/timing.bin','native-*/sample.bin']for p in out.glob(pattern)};runner.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),assertions={m:v['native']['assertions']for m,v in record['modes'].items()})))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--oracle',type=Path,default=FIXTURE);native(p.parse_args())
if __name__=='__main__':main()
