#!/usr/bin/env python3
"""Independent constant-rotor PD oracle: exact Jury, Decimal eigen responses.

Only new profiles are evaluated. Fraction treats each supplied binary64 as its
exact rational value. Eighty-digit complex roots and integer powers solve the
position recurrence in closed form; this is not the product's Euler step loop.
No Coax, Panda/SDK, downloaded script, NN or earlier numerical gate is run.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext,ROUND_HALF_EVEN
import argparse,hashlib,json,math,struct,time

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/rotor-pd'
ANALYSIS=struct.Struct('<13d2q')
RUN=struct.Struct('<7dq')
INITIALS=[[-1.,0.],[.375,-.5]]

def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(v)for v in values),label+' ALL finite and complete');return list(values)
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def rational(value):return Fraction.from_float(value)
def decimal(value):return Decimal(value.numerator)/Decimal(value.denominator)
def fraction_text(value):return str(value.numerator)+'/'+str(value.denominator)
def sign(value):return (value>0)-(value<0)
def cmul(a,b):return(a[0]*b[0]-a[1]*b[1],a[0]*b[1]+a[1]*b[0])
def cadd(a,b):return(a[0]+b[0],a[1]+b[1])
def csub(a,b):return(a[0]-b[0],a[1]-b[1])
def cscale(a,s):return(a[0]*s,a[1]*s)
def cdivide(a,b):
    denominator=b[0]*b[0]+b[1]*b[1];require(denominator!=0,'nonzero closed-form complex divisor');return((a[0]*b[0]+a[1]*b[1])/denominator,(a[1]*b[0]-a[0]*b[1])/denominator)
def cpower(value,exponent):
    result=(Decimal(1),Decimal(0));base=value
    while exponent:
        if exponent&1:result=cmul(result,base)
        exponent>>=1
        if exponent:base=cmul(base,base)
    return result
def algebra(parameters):
    m,b,kp,kd,h=map(rational,parameters);d=m+h*b;alpha=h*h*kp/d;beta=(m-h*kd)/d;t=1-alpha+beta;damping=b+kd;margin=4*m+2*h*b-2*h*kd-h*h*kp;disc=t*t-4*beta;continuous=damping*damping-4*m*kp
    require(m>0 and kp>0 and h>0 and b>=0 and kd>=0 and alpha>0,'physical signs and remaining Jury inequality');classification=0 if damping>0 and margin>0 else(2 if margin<0 else 1)
    return dict(m=m,b=b,kp=kp,kd=kd,h=h,d=d,alpha=alpha,beta=beta,t=t,damping=damping,margin=margin,disc=disc,continuous=continuous,classification=classification)
def freeze():
    FIXTURE.mkdir(parents=True,exist_ok=True);require(not(FIXTURE/'experiment.json').exists(),'preserve every frozen experiment')
    profiles=[('underdamped',[.01,.02,100.,.2,.002],128),('continuous-critical',[1.,0.,4.,4.,.002],128),('overdamped',[.01,.02,100.,4.,.002],128),('zero-damping',[.01,0.,100.,0.,.002],128),('derivative-unstable',[.01,.02,100.,12.,.002],24),('discrete-repeated',[1.,0.,64.,15.875,1/512],128),('jury-exact-boundary',[1.,0.,4.,3.,.5],32),('jury-stable-neighbor',[1.,0.,4.,math.nextafter(3.,-math.inf),.5],32),('jury-unstable-neighbor',[1.,0.,4.,math.nextafter(3.,math.inf),.5],32),('continuous-near-below',[1.,0.,4.,4.-2**-20,.002],128),('continuous-near-above',[1.,0.,4.,4.+2**-20,.002],128)]
    cases=[];runs=[]
    for index,(name,params,updates)in enumerate(profiles):
        finite(params,5,'profile parameters');cases.append(dict(index=index,name=name,parameters=params,parameter_u64=list(struct.unpack('<5Q',struct.pack('<5d',*params))),updates=updates,step_scope='primary .002-second model'if params[4]==.002 else'explicit isolated mathematical dyadic/coarse fixture'))
        for initial in INITIALS:runs.append(dict(index=len(runs),profile=index,parameters=params,initial_error_rate=initial,updates=updates))
    require(len(cases)==11 and len(runs)==22 and sum(r['updates']for r in runs)==2032,'11profiles/22runs/2032updates');(FIXTURE/'profiles.f64').write_bytes(struct.pack('<55d',*[v for p in cases for v in p['parameters']]));(FIXTURE/'runs.bin').write_bytes(b''.join(RUN.pack(*r['parameters'],*r['initial_error_rate'],r['updates'])for r in runs))
    experiment=dict(schema='psionic.rotor-pd.experiment.v1',frozen_before_closedform_generation_and_product_comparison=True,scope='Isolated constant rotor with explicit PD and implicit physical joint damping. No nonlinear Panda/contact/fluid/clipping/moving target/coupled inertia/clock/global/hardware stability claim.',parameter_order=['inertia','physical_damping','kp','kd','step'],parameter_units=['J*s^2/rad^2','J*s/rad^2','J/rad^2','J*s/rad^2','s'],parameter_bounds=[[.01,10],[0,100],[1e-6,500],[0,100],[1e-6,.5]],state_order=['error','rate'],state_units=['rad','rad/s'],state_absolute_caps=[1e6,1e8],effective_acceleration_absolute_cap=1e12,effective_acceleration_units='rad/s^2',maximum_updates=128,physics='D=M+hB; a=(-Kp*e-(B+Kd)*v)/D; vnew=v+h*a; enew=e+h*vnew. PhysicalB implicit; feedbackKd explicit.',normalized_state='[e,hv],both radians',transition='alpha=h^2*Kp/D,beta=(M-h*Kd)/D,A=[[1-alpha,beta],[-alpha,beta]]',continuous='omega=sqrt(Kp/M) in1/s,zeta=(B+Kd)/(2sqrt(M*Kp)); distinct from discrete roots/Jury',strict_Jury='For positiveM,Kp,h and nonnegativeB,Kd: B+Kd>0 AND4M+2hB-2hKd-h^2Kp>0; equality excluded. Classification0stable1boundary2unstable.',analysis_record='13F64 D,alpha,beta,A4rowmajor,omega,zeta,trace,algebraicdet(beta),jury_damping,jury_margin;I64strict_stable,I64classification=120B',analysis_units=['J*s^2/rad^2','1','1','1','1','1','1','1/s','1','1','1','J*s/rad^2','J*s^2/rad^2'],response_record='3F64 error,rate,effective_acceleration=24B; initial stage0 acceleration0 is unavailable input transport, not an applied product step',reference_method='Fraction exact IEEE input algebra and80-digit Decimal complex eigen roots/integer powers or repeated-root closed form. Velocity from consecutive analytic errors; acceleration from consecutive analytic velocities. No product step recurrence.',budgets=dict(response=dict(atol=2e-10,rtol=2e-12),analysis=dict(atol=2e-12,rtol=2e-12),classification='exact',strict_stable='exact',mode_bytes='exact'),profiles=cases,runs=runs,profile_count=11,run_count=22,product_updates=2032,initial_transport_records=22,response_records=2054,profiles_file=identity(FIXTURE/'profiles.f64'),runs_file=identity(FIXTURE/'runs.bin'),source_sha256={str(Path(__file__).resolve().relative_to(ROOT)):sha(__file__)},SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False)
    write(FIXTURE/'experiment.json',experiment);receipt=dict(schema='psionic.rotor-pd.input-freeze.v1',passed=True,experiment=identity(FIXTURE/'experiment.json'),profiles=identity(FIXTURE/'profiles.f64'),runs=identity(FIXTURE/'runs.bin'),expected_generated=False,product_comparisons=0,source=identity(__file__));write(FIXTURE/'input-freeze-v1.json',receipt);print(json.dumps(receipt))
def closed_errors(a,initial,count):
    e0,v0=map(lambda x:decimal(rational(x)),initial);h=decimal(a['h']);alpha=decimal(a['alpha']);beta=decimal(a['beta']);t=decimal(a['t']);e1=(1-alpha)*e0+beta*h*v0;zero=Decimal(0);maximum_imaginary=zero
    if a['disc']==0:
        root=t/2
        if root==0:errors=[e0,e1]+[zero]*max(0,count-1);branch='repeated-zero-nilpotent'
        else:errors=[(e0+Decimal(n)*(e1/root-e0))*(root**n)for n in range(count+1)];branch='repeated-root'
    else:
        if a['disc']>0:
            delta=decimal(a['disc']).sqrt();plus=((t+delta)/2,zero);minus=((t-delta)/2,zero);branch='distinct-real'
        else:
            delta=decimal(-a['disc']).sqrt();plus=(t/2,delta/2);minus=(t/2,-delta/2);branch='complex-conjugate'
        coefficient=cdivide(csub((e1,zero),cscale(minus,e0)),csub(plus,minus));other=csub((e0,zero),coefficient);errors=[]
        for n in range(count+1):
            value=cadd(cmul(coefficient,cpower(plus,n)),cmul(other,cpower(minus,n)));maximum_imaginary=max(maximum_imaginary,abs(value[1]));require(abs(value[1])<Decimal('1e-55')*max(Decimal(1),abs(value[0])),'real closed-form response after80digit conjugate cancellation');errors.append(value[0])
    require(len(errors)==count+1,'complete independent analytic response');velocities=[v0]+[(errors[n]-errors[n-1])/h for n in range(1,count+1)];accelerations=[zero]+[(velocities[n]-velocities[n-1])/h for n in range(1,count+1)];return errors,velocities,accelerations,branch,maximum_imaginary
def generate(out):
    require(not out.exists(),'preserve every original/failed reference');out.mkdir(parents=True);record=dict(schema='psionic.rotor-pd.independent-reference.v1',passed=False,Coax_calls=0,SDK_calls=0,NN_calls=0,old_gates_repeated=False,product_recurrence_used_as_oracle=False);began=time.monotonic()
    try:
        experiment=FIXTURE/'experiment.json';e=json.loads(experiment.read_text());freeze=json.loads((FIXTURE/'input-freeze-v1.json').read_text());require(freeze['passed']and freeze['experiment']==identity(experiment)and e['source_sha256']=={str(Path(__file__).resolve().relative_to(ROOT)):sha(__file__)},'exact pre-generation source/input freeze');require(e['profiles_file']==identity(FIXTURE/'profiles.f64')and e['runs_file']==identity(FIXTURE/'runs.bin'),'frozen binary transports');require(e['profile_count']==11 and e['run_count']==22 and e['product_updates']==2032 and e['response_records']==2054,'complete frozen new experiment');require((FIXTURE/'profiles.f64').read_bytes()==struct.pack('<55d',*[v for p in e['profiles']for v in p['parameters']]),'profile JSON/binary exact bits');require((FIXTURE/'runs.bin').read_bytes()==b''.join(RUN.pack(*r['parameters'],*r['initial_error_rate'],r['updates'])for r in e['runs']),'run JSON/binary exact bits');analysis=bytearray();response=bytearray();exact=[];indices=[];imaginary=Decimal(0)
        with localcontext()as ctx:
            ctx.prec=80;ctx.rounding=ROUND_HALF_EVEN
            for p in e['profiles']:
                a=algebra(p['parameters']);omega=(decimal(a['kp'])/decimal(a['m'])).sqrt();zeta=decimal(a['damping'])/(2*(decimal(a['m'])*decimal(a['kp'])).sqrt());values=[decimal(a['d']),decimal(a['alpha']),decimal(a['beta']),1-decimal(a['alpha']),decimal(a['beta']),-decimal(a['alpha']),decimal(a['beta']),omega,zeta,decimal(a['t']),decimal(a['beta']),decimal(a['damping']),decimal(a['margin'])];numeric=finite(list(map(float,values)),13,'ALL independent analysis fields');analysis+=ANALYSIS.pack(*numeric,int(a['classification']==0),a['classification']);exact.append(dict(index=p['index'],name=p['name'],classification=a['classification'],strict_stable=a['classification']==0,continuous_discriminant_sign=sign(a['continuous']),discrete_discriminant_sign=sign(a['disc']),fractions={name:fraction_text(a[name])for name in['d','alpha','beta','t','damping','margin','disc','continuous']}))
            require([x['classification']for x in exact]==[0,0,0,1,2,0,1,0,2,0,0],'predeclared strictJury classes including exact boundary andIEEEneighbors');require(exact[5]['discrete_discriminant_sign']==0 and exact[1]['continuous_discriminant_sign']==0,'separate exact discrete/continuous critical fixtures');require([sign(algebra(e['profiles'][i]['parameters'])['margin'])for i in[6,7,8]]==[0,1,-1],'exactFraction neighboring Jury signs')
            for run in e['runs']:
                profile=e['profiles'][run['profile']];a=algebra(profile['parameters']);errors,rates,accelerations,branch,imag=closed_errors(a,run['initial_error_rate'],run['updates']);imaginary=max(imaginary,imag);offset=len(response)//24;largest=[0.,0.,0.]
                for stage,(error,rate,accel)in enumerate(zip(errors,rates,accelerations)):
                    values=finite(list(map(float,[error,rate,accel])),3,'ALL independent response fields');require(abs(values[0])<=1e6 and abs(values[1])<=1e8 and abs(values[2])<=1e12,'predeclared trajectory remains inside numerical state/accel caps');response+=struct.pack('<3d',*values);largest=[max(x,abs(y))for x,y in zip(largest,values)]
                indices.append(dict(run=run['index'],profile=run['profile'],name=profile['name'],offset_records=offset,records=run['updates']+1,updates=run['updates'],initial_error_rate=run['initial_error_rate'],initial_acceleration_available=False,root_branch=branch,strict_stable=a['classification']==0,classification=a['classification'],maximum_absolute_error_rate_acceleration=largest,maximum_closedform_imaginary=str(imag)))
        require(len(analysis)==11*120 and len(response)==2054*24 and sum(x['updates']for x in indices)==2032,'exact complete independent expected extents');(out/'analysis-expected.bin').write_bytes(analysis);(out/'response-expected.bin').write_bytes(response);write(out/'profiles-exact.json',exact);write(out/'response-index.json',indices);require(sha(__file__)==e['source_sha256'][str(Path(__file__).resolve().relative_to(ROOT))],'executed oracle source unchanged');record.update(passed=True,experiment=identity(experiment),input_freeze=identity(FIXTURE/'input-freeze-v1.json'),source_sha256=e['source_sha256'],budgets=e['budgets'],profile_count=11,run_count=22,product_update_records=2032,initial_transport_records=22,total_response_records=2054,decimal_precision=80,decimal_rounding='ROUND_HALF_EVEN',maximum_closedform_imaginary=str(imaginary),exact_Jury_classes=[x['classification']for x in exact],continuous_discriminant_signs=[x['continuous_discriminant_sign']for x in exact],discrete_discriminant_signs=[x['discrete_discriminant_sign']for x in exact],root_branch_counts={name:sum(r['root_branch']==name for r in indices)for name in sorted({r['root_branch']for r in indices})},all_expected_analysis_finite_words=143,all_expected_response_finite_words=6162,outputs={name:identity(out/name)for name in['analysis-expected.bin','response-expected.bin','profiles-exact.json','response-index.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;write(out/'reference.json',record)
    print(json.dumps(dict(passed=True,reference=str(out/'reference.json'),profiles=11,runs=22,updates=2032,initials=22,classes=record['exact_Jury_classes'],root_branches=record['root_branch_counts'])))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--freeze',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args();require(args.freeze!=bool(args.output),'choose exactly freeze or generation');freeze()if args.freeze else generate(args.output.resolve())
if __name__=='__main__':main()
