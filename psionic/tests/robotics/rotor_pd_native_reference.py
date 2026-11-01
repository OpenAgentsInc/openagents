#!/usr/bin/env python3
"""New isolated-rotor native proof against cached 80-digit closed forms.

No product recurrence generates targets, no SDK/NN/old gate runs, and strict
Jury diagnostics are not control permissions or finite-precision certificates.
"""
from pathlib import Path
from fractions import Fraction
import argparse,hashlib,json,math,os,platform,struct,time
from panda_pose_ik_native_reference import Run,identity,sha,graph,finite,require
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/rotor-pd';SOURCE=ROOT/'psionic/lib/robotics/rotor-pd.coax';NATIVE=ROOT/'psionic/tests/robotics/rotor_pd_native.c'
EXPERIMENT_SHA='4e2924f589ea1c9a0b22c7f0c8ccd382f25702e057513b0f6b8449812d98036a'
ANALYSIS=struct.Struct('<q13d2q');EXPECTED_ANALYSIS=struct.Struct('<13d2q');RESPONSE=struct.Struct('<3q3d');RUN_INPUT=struct.Struct('<7dq')

def fixture_cap(p,x):
    a=(-p[2]*x[0]-(p[1]+p[3])*x[1])/(p[0]+p[4]*p[1]);v=x[1]+p[4]*a;e=x[0]+p[4]*v;finite([a,v,e],3,'independent guard fixture')
    return next((i for i,(value,cap)in enumerate(zip([a,v,e],[1e12,1e8,1e6]))if abs(value)>cap),-1)

def load_inputs(oracle):
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen isolated rotor experiment');e=json.loads((FIXTURE/'experiment.json').read_text());r=json.loads((oracle/'reference.json').read_text())
    require(r['passed'] and r['experiment']==identity(FIXTURE/'experiment.json') and r['budgets']==e['budgets'] and r['decimal_precision']==80 and not r['product_recurrence_used_as_oracle'] and r['Coax_calls']==r['SDK_calls']==0,'qualified independent closed-form definition')
    require(all(identity(oracle/name)==value for name,value in r['outputs'].items()) and all(sha(ROOT/name)==digest for name,digest in r['source_sha256'].items()),'all frozen expected/source artifacts')
    require(identity(FIXTURE/'profiles.f64')==e['profiles_file'] and identity(FIXTURE/'runs.bin')==e['runs_file'],'original parameter/run bits');profiles=(FIXTURE/'profiles.f64').read_bytes();runs=(FIXTURE/'runs.bin').read_bytes();require(len(profiles)==11*40 and len(runs)==22*64,'exact11profile/22run extents')
    exact=json.loads((oracle/'profiles-exact.json').read_text());index=json.loads((oracle/'response-index.json').read_text());require(len(exact)==11 and len(index)==22,'exact metadata extents');classes=[];margins=[]
    for i,p in enumerate(e['profiles']):
        raw=profiles[i*40:(i+1)*40];require(raw==struct.pack('<5Q',*p['parameter_u64'])==struct.pack('<5d',*p['parameters']),'exact profile input bits')
        M,B,Kp,Kd,h=map(Fraction.from_float,p['parameters']);damping=B+Kd;margin=4*M+2*h*B-2*h*Kd-h*h*Kp;kind=2 if damping<0 or margin<0 else 1 if damping==0 or margin==0 else 0
        require(kind==exact[i]['classification']==r['exact_Jury_classes'][i] and bool(kind==0)==exact[i]['strict_stable'],'independent Fraction strict-Jury class');classes.append(kind);margins.append(margin)
    require(classes==[0,0,0,1,2,0,1,0,2,0,0] and margins[6]==0 and margins[7]>0 and margins[8]<0,'dyadic Kd3 exact and one-ULP sides')
    require(e['profiles'][6]['parameters'][3]==3 and e['profiles'][7]['parameters'][3]==math.nextafter(3,-math.inf) and e['profiles'][8]['parameters'][3]==math.nextafter(3,math.inf),'actual IEEE neighbor fixtures')
    offset=0;total=0
    for i,run in enumerate(e['runs']):
        row=RUN_INPUT.unpack_from(runs,i*64);finite(row[:7],7,'run input');require(row[:5]==tuple(e['profiles'][run['profile']]['parameters']) and runs[i*64:(i+1)*64]==RUN_INPUT.pack(*run['parameters'],*run['initial_error_rate'],run['updates']),'run JSON/bit identity')
        require(0<=row[7]<=128 and index[i]['run']==i and index[i]['profile']==run['profile'] and index[i]['offset_records']==offset and index[i]['records']==row[7]+1 and index[i]['updates']==row[7] and index[i]['initial_acceleration_available']is False,'all response indices/horizons before product')
        offset+=row[7]+1;total+=row[7]
    require(offset==2054 and total==2032,'2032 updates and22 initial transport copies')
    # Verify each endpoint/endcap fixture BEFORE observing product results.
    for f in range(5):
        for value in e['parameter_bounds'][f]:
            p=[1.,0.,1e-6,0.,1e-6];p[f]=value;require(fixture_cap(p,[0.,0.])==-1,'parameter endpoint zero-state fixture')
    for sign in[-1.,1.]:
        p=[1.,0.,1e-6,0.,1e-6]
        require(fixture_cap(p,[sign*1e6,0.])==fixture_cap(p,[0.,sign*1e8])==-1,'inclusive state endpoint fixtures')
        p=[.01,0.,1e-6,100.,1e-6];require(fixture_cap(p,[0.,sign*1e8])==-1 and fixture_cap(p,[sign,sign*1e8])==0,'acceleration endpoint and sole late-A refusal')
        require(fixture_cap([1.,0.,500.,0.,1e-6],[-sign*1e6,sign*1e8])==1 and fixture_cap([10.,0.,1e-6,0.,.5],[sign*1e6,sign])==2,'isolated late-V/E refusals')
    return e,r,index,profiles,runs

def decode_analysis(raw):
    require(len(raw)==11*128,'exact11 analysis actual records');rows=[ANALYSIS.unpack_from(raw,i*128)for i in range(11)];finite([v for row in rows for v in row[1:14]],143,'ALL actual analysis fields before maxima');require(all(row[0]==0 and row[14]in[0,1] and row[15]in[0,1,2]for row in rows),'exact analysis status/flag domains');return rows

def decode_response(raw,index):
    require(len(raw)==2054*48,'exact2054 response actual records');rows=[RESPONSE.unpack_from(raw,i*48)for i in range(2054)];finite([v for row in rows for v in row[3:]],6162,'ALL actual response fields before maxima')
    for run in index:
        for stage in range(run['records']):require(rows[run['offset_records']+stage][:3]==(run['run'],stage,0),'exact run/stage/status metadata')
    return rows

def expected(oracle):
    a=(oracle/'analysis-expected.bin').read_bytes();s=(oracle/'response-expected.bin').read_bytes();require(len(a)==11*120 and len(s)==2054*24,'exact all independent expected extents');ar=[EXPECTED_ANALYSIS.unpack_from(a,i*120)for i in range(11)];sr=finite(struct.unpack('<6162d',s),6162,'ALL expected response fields');finite([v for row in ar for v in row[:13]],143,'ALL expected analysis fields');require(all(row[13]in[0,1]and row[14]in[0,1,2]for row in ar),'independent flag/classification domains');return ar,[sr[i*3:(i+1)*3]for i in range(2054)]

def compare_values(actual,wanted,budget,label):
    finite(actual,len(wanted),label+' actual');finite(wanted,len(actual),label+' reference');require(actual,label+' nonempty');errors=[abs(x-y)for x,y in zip(actual,wanted)];limits=[budget['atol']+budget['rtol']*abs(v)for v in wanted];require(all(math.isfinite(x)and x<=lim for x,lim in zip(errors,limits)),label+' frozen absolute/relative error budget')
    return dict(passed=True,values=len(actual),max_abs_error=max(errors),maximum_budget_fraction=max(x/y for x,y in zip(errors,limits)),**budget)

def compare(araw,sraw,ea,es,e,index):
    a=decode_analysis(araw);s=decode_response(sraw,index);require(all(row[14:]==wanted[13:]for row,wanted in zip(a,ea)),'all11 exact strict-Jury/classification outputs')
    for i,margin in[(6,0.),(7,2.0**-51),(8,-2.0**-51)]:require(struct.pack('<d',a[i][13])==struct.pack('<d',ea[i][12])==struct.pack('<d',margin),'exact dyadic Jury margin output profile'+str(i))
    analysis=compare_values([v for row in a for v in row[1:14]],[v for row in ea for v in row[:13]],e['budgets']['analysis'],'analysis')
    metrics={name:compare_values([row[3+j]for row in s],[row[j]for row in es],e['budgets']['response'],name)for j,name in enumerate(['error_radians','rate_radians_per_second','effective_acceleration_radians_per_second_squared'])}
    for run in index:
        row=s[run['offset_records']];require(struct.pack('<3d',*row[3:])==struct.pack('<3d',*run['initial_error_rate'],0.),'initial state transport exactly copies input; acceleration unavailable')
    return dict(passed=True,analysis=analysis,response=metrics,exact_classification=True,applied_update_records=2032,initial_unapplied_transport_records=22,all_actual_and_expected_analysis_finite_words=143,all_actual_and_expected_response_finite_words=6162,diagnostic_is_permission=False)

def reader_guards(araw,sraw,ea,es,index):
    count=0
    for raw,decoder,offset in[(araw,decode_analysis,10*128+13*8),(sraw,lambda x:decode_response(x,index),len(sraw)-8)]:
        for bad in[raw[:-1],raw+b'\0']:
            try:decoder(bad)
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted wrong actual extent')
        for v in[math.nan,math.inf,-math.inf]:
            bad=bytearray(raw);struct.pack_into('<d',bad,offset,v)
            try:decoder(bytes(bad))
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted late actual IEEE')
    for values,n in[([v for row in ea for v in row[:13]],143),([v for row in es for v in row],6162)]:
        for v in[math.nan,math.inf,-math.inf]:
            bad=values[:];bad[-1]=v
            try:finite(bad,n,'poisoned expected')
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted late expected IEEE')
    require(count==16,'all16 actual/expected reader poisons');return dict(passed=True,cases=16,actual_and_expected=True)

def native(args):
    require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile');oracle=args.oracle.resolve();e,reference,index,profiles,runs=load_inputs(oracle);ea,es=expected(oracle);require(all(row[13]==int(kind==0)and row[14]==kind for row,kind in zip(ea,reference['exact_Jury_classes'])),'expected binary flags tied to independently verified Fraction classes');out=args.output.resolve();require(not out.exists(),'preserve every prior or failed attempt');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.rotor-pd.native.v1',passed=False,SDK_calls=0,NN_calls=0,old_gates_repeated=False,oracle_regenerated=False,experiment=identity(FIXTURE/'experiment.json'),reference=identity(oracle/'reference.json'),compiler=identity(compiler),commands=[],budgets=e['budgets'])
    runner=Run(out,record);runner.save();began=time.monotonic()
    try:
        deps=graph(SOURCE)
        for p in[NATIVE,Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_pose_ik_native_reference.py']+[p for directory in[FIXTURE,oracle]for p in directory.iterdir()if p.is_file()]:deps[p]=identity(p)
        record['frozen_inputs']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):value for p,value in sorted(deps.items())};record['independent_preflight']=dict(profiles=11,runs=22,updates=2032,initial_transport_records=22,exact_Fraction_Jury=True,Kd3_IEEE_neighbors_verified=True,endpoint_and_isolated_cap_fixtures_verified=True);runner.save();(out/'input.bin').write_bytes(struct.pack('<4I',0x31445052,11,22,1)+profiles+runs);record['input']=identity(out/'input.bin');emissions={}
        for mode,extra in[('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'rotor-pd.c';h=d/'rotor-pd.h';c.write_bytes(runner.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={mode:dict(C=identity(c),header=identity(h))for mode,(c,h)in emissions.items()};runner.save();outputs=[];common=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
        for mode,source_mode,flags in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/('native-'+mode);d.mkdir();c,h=emissions[source_mode];binary=d/'native';runner.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o']);runner.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*common,*flags,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',binary]);report=json.loads(runner.command('run-'+mode,[binary,out/'input.bin',d/'analysis.bin',d/'response.bin']))
            require(report['passed']and report['SDK_calls']==0 and report['profiles']==11 and report['runs']==22 and report['response_updates']==2032 and report['initial_transport_records']==22 and report['analysis_record_bytes']==128 and report['response_record_bytes']==48 and report['alias_cases']==14 and report['parameter_endpoints']==10 and report['state_acceleration_endpoints']==6 and report['late_cap_refusals']==6 and report['nonfinite_role_patterns']==35 and report['fpcr']==0,'exact new native inventory')
            ar=(d/'analysis.bin').read_bytes();sr=(d/'response.bin').read_bytes();record.setdefault('modes',{})[mode]=dict(native=report,comparison=compare(ar,sr,ea,es,e,index),binary=identity(binary),analysis=identity(d/'analysis.bin'),response=identity(d/'response.bin'));outputs.append((ar,sr));runner.save()
        require(outputs[0]==outputs[1]==outputs[2],'all complete analysis/response files bit-identical');record['new_reader_guards']=reader_guards(*outputs[0],ea,es,index);require(all(identity(p)==value for p,value in deps.items())and identity(compiler)==record['compiler'],'all source/reference/compiler bytes unchanged');record.update(passed=True,modes_byte_identical=True,analysis_sha256=hashlib.sha256(outputs[0][0]).hexdigest(),response_sha256=hashlib.sha256(outputs[0][1]).hexdigest())
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;record['partial_or_full_outputs']={str(p.relative_to(out)):identity(p)for pattern in['native-*/analysis.bin','native-*/response.bin']for p in out.glob(pattern)};runner.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),assertions={m:v['native']['assertions']for m,v in record['modes'].items()})))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--oracle',type=Path,default=FIXTURE);native(p.parse_args())
if __name__=='__main__':main()
