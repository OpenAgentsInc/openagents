#!/usr/bin/env python3
"""Freeze and independently evaluate NEW GAE/PPO scalar-foundation fixtures.

No learner, environment, RNG/model/SDK/runtime or old gate executes. GAE's
primary oracle explicitly sums future residuals; it never copies the reverse
production recurrence. PPO's operation/branch oracle uses exact rationals.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext
import argparse,hashlib,json,math,struct,time
ROOT=Path(__file__).resolve().parents[3];F=ROOT/'psionic/fixtures/robotics/locomotion'
STEP=struct.Struct('<3d2q');CASE=struct.Struct('<4q2d');GAE=struct.Struct('<3d2q');PPO=struct.Struct('<4dq')
U=Fraction(1,1<<53)
def require(ok,message):
    if not ok:raise RuntimeError(message)
def identity(path):p=Path(path);return dict(bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest())
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def bits(x):return struct.unpack('<Q',struct.pack('<d',x))[0]
def frac(x):return Fraction.from_float(x)
def negative(x):return bool(bits(x)>>63)
def rounded(q,zero_negative=False):
    x=float(q)
    return math.copysign(0.,-1 if zero_negative else 1)if x==0 else x
def add(a,b):
    q=frac(a)+frac(b);return rounded(q,negative(a)and negative(b))
def sub(a,b):return add(a,-b)
def mul(a,b):return rounded(frac(a)*frac(b),negative(a)!=negative(b))
def finite(values,n,label):require(len(values)==n and all(math.isfinite(x)for x in values),label+' ALLfinite/extent');return values
def axes_steps(W,H,fn):return[tuple(fn(w,t))for w in range(W)for t in range(H)]
def gae_inputs():
    cases=[]
    def take(name,W,H,g,l,rows,oracle='fraction'):
        require(len(rows)==W*H,'input fixture extent');cases.append(dict(name=name,worlds=W,horizon=H,gamma=g,lambda_=l,steps=rows,oracle=oracle))
    # ALL64 assignments of independent term/trunc flags on three transitions.
    for mask in range(64):
        rows=[(.125*(t+1),[.25,-.5,.75][t],[.5,-.25,1.][t],(mask>>(2*t))&1,(mask>>(2*t+1))&1)for t in range(3)]
        take('three-row-all-flags-%02d'%mask,1,3,.75,.5,rows)
    for g in[0.,.5,1.]:
        for l in[0.,.5,1.]:take('hyper-grid-%s-%s'%(g,l),2,4,g,l,axes_steps(2,4,lambda w,t:((w+1)*(t-1.5),.25*(t-w),.5*(t+1-w),int(t==1 and w==0),int(t==2 or(t==1 and w==1)))))
    for label,term,trunc in[('continuing',0,0),('terminated',1,0),('truncated',0,1),('both-termination-wins',1,1)]:take('single-'+label,1,1,.9,.95,[(.5,1.25,7.,term,trunc)],'decimal80')
    take('collector-final-nextvalue-not-reset',1,1,.75,.5,[(.5,1.25,7.,0,0)])
    take('collector-reset-value-alternative',1,1,.75,.5,[(.5,1.25,0.,0,0)])
    for value in[1e6,-1e6]:take('terminated-nextvalue-%s'%value,1,4,.75,.5,[(.5,.25,.75,0,0),(1.,.5,value,1,0),(1e6,-1e6,1e6,0,0),(-1e6,1e6,-1e6,0,0)])
    for value in[7.,0.]:take('truncation-nextvalue-%s'%value,1,4,.75,.5,[(.5,.25,.75,0,0),(1.,.5,value,0,1),(1000.,-500.,1000.,0,0),(-1000.,500.,-1000.,0,0)])
    take('signed-zero-cuts',2,4,-0.,1.,[(r,v,n,t,tr)for r,v,n,t,tr in[(-0.,0.,0.,0,0),(-0.,0.,-0.,1,0),(0.,-0.,0.,0,1),(-0.,-0.,0.,1,1),(0.,0.,-0.,0,0),(-0.,-0.,-0.,0,1),(-0.,0.,0.,1,0),(-0.,0.,0.,0,0)]])
    tiny=float.fromhex('0x0.0000000000001p-1022')
    take('subnormal-rewards-and-values',1,8,1.,1.,[(tiny*(1 if t%2==0 else-1),-tiny,tiny,int(t==3),int(t==5))for t in range(8)])
    take('subnormal-discount-product',1,8,tiny,.5,[(tiny,-tiny,1e6,0,0)for _ in range(8)])
    take('near-one-discount-cancellation',1,128,math.nextafter(1.,0.),math.nextafter(1.,0.),[(float((-1)**t)*1e6,float((-1)**(t+1))*999999.9,float((-1)**t)*999999.8,0,0)for t in range(128)],'decimal80')
    take('unit-discount-long-residuals',1,128,1.,1.,[(float((-1)**t)*1e6,-1e6,1e6,0,0)for t in range(128)])
    dense=axes_steps(16,128,lambda w,t:(((w+1)*(t%9-4))/16.,(t%11-w)/8.,(t%7+w)/4.,0,0))
    take('max16worlds128steps-continuing',16,128,.75,.875,dense)
    take('max16worlds128steps-moving-flags',16,128,.99,.95,axes_steps(16,128,lambda w,t:((w+1)*(t%9-4)/17.,(t%11-w)/19.,(t%7+w)/23.,int((t+w)%17==0),int((2*t+w)%13==0))),'decimal80')
    for label,scale in[('large',1e6),('tiny',tiny)]:take('world-isolation-'+label,2,8,.75,.5,axes_steps(2,8,lambda w,t:((t-3)*.25,.125*t,.25*(t+1),0,0)if w==0 else(scale*(-1)**t,scale/2,scale/4,int(t==2),int(t==4))))
    take('unit-discount-telescoping',3,7,1.,1.,axes_steps(3,7,lambda w,t:((w-t)/8.,(t+w)/4.,(t+1+w)/4.,int(t==4 and w==1),int(t==2 and w==2))))
    take('arbitrary64-dense',4,13,.99,.95,axes_steps(4,13,lambda w,t:(((w*97+t*31)%101-50)/17.,((w*13+t*19)%89-44)/23.,((w*11+t*41)%83-41)/29.,int((w+t)%11==0),int((w+2*t)%9==0))),'decimal80')
    take('singleworld-copy-of-maxbatch-world0',1,128,.75,.875,dense[:128])
    take('large-small-cancellation-order',1,128,1.,1.,[([1e6,1e-6,-1e6,-1e-6][t%4],0.,0.,0,0)for t in range(128)])
    require(len(cases)==96,'fixed64+9+23 GAEcase inventory');return cases
def ppo_inputs():
    result=[];rmin=2.**-20;tiny=float.fromhex('0x0.0000000000001p-1022')
    eps=[('zero',0.),('negative-zero',-0.),('subnormal',tiny),('dyadic',.125),('point2-below',math.nextafter(.2,0.)),('point2',.2),('point2-above',math.nextafter(.2,1.)),('half',.5),('one-below',math.nextafter(1.,0.)),('one',1.)]
    advantages=[('cap-negative',-1e9),('negative-one',-1.),('negative-eighth',-.125),('negative-subnormal',-tiny),('negative-zero',-0.),('zero',0.),('subnormal',tiny),('eighth',.125),('one',1.),('cap',1e9)]
    for en,e in eps:
        lo=sub(1.,e);hi=add(1.,e);rr=[('minimum',rmin),('minimum-next',math.nextafter(rmin,math.inf)),('quarter',.25),('half',.5),('one',1.),('two',2.),('maximum',16.)]
        for bn,b in[('lower',lo),('upper',hi)]:rr +=[(bn+'-below',math.nextafter(b,-math.inf)),(bn,b),(bn+'-above',math.nextafter(b,math.inf))]
        seen=set()
        for rn,r in rr:
            if not(rmin<=r<=16)or bits(r)in seen:continue
            seen.add(bits(r))
            for an,a in advantages:result.append(dict(name=en+'/'+rn+'/'+an,rho=r,advantage=a,epsilon=e))
    # Actual GAE composability scale, both signs, no normalization or clipping.
    for e in[0.,.2,1.]:
        for r in[rmin,1.,16.]:
            for a in[-384e6,384e6]:result.append(dict(name='GAE-composable-%s-%s-%s'%(e,r,a),rho=r,advantage=a,epsilon=e))
    return result
def freeze():
    require(not F.exists(),'preserve original input freeze');F.mkdir(parents=True);cases=gae_inputs();ppo=ppo_inputs();rows=[];descriptors=[];meta=[]
    for i,c in enumerate(cases):
        W,H=c['worlds'],c['horizon'];at=len(rows);finite([v for r in c['steps']for v in r[:3]],3*W*H,'ALL new GAEinputs');require(all(abs(v)<=1e6 for r in c['steps']for v in r[:3])and all(x in[0,1]for r in c['steps']for x in r[3:]),'all inputreward/value/flag domains');descriptors.append(CASE.pack(W,H,at,W*H,c['gamma'],c['lambda_']));rows.extend(c['steps']);meta.append(dict(case=i,name=c['name'],worlds=W,horizon=H,offset_steps=at,count=W*H,gamma=c['gamma'],lambda_=c['lambda_'],gamma_bits=hex(bits(c['gamma'])),lambda_bits=hex(bits(c['lambda_'])),oracle=c['oracle']))
    (F/'gae-cases.bin').write_bytes(b''.join(descriptors));(F/'gae-steps.bin').write_bytes(b''.join(STEP.pack(*r)for r in rows));(F/'ppo-inputs.f64').write_bytes(b''.join(struct.pack('<3d',c['rho'],c['advantage'],c['epsilon'])for c in ppo))
    e=dict(schema='psionic.locomotion-foundation.experiment.v1',frozen_before_product_comparison=True,scope='CPUF64 typed GAEreturn targets and clippedPPO scalar objective only; no policy/model/RNG/optimizer/environment/gait/physics/device/training claim.',ABI=dict(RgaStep40='<3d2q> reward/current_value/next_value,terminated/truncated',RgaOutput40='<3d2q> delta/advantage/lambda_return,bootstrap/trace',RcpOutput40='<4dq> surrogate/loss/dloss_dlogp,clipped_ratio,clipped_active',gae_cases48='<4q2d> worlds,horizon,offset_steps,count,gamma,lambda',ppo_input24='<3d> rho,advantage,epsilon'),domain=dict(worlds=[1,16],horizon=[1,128],world_major_total_max=2048,reward_value_nextvalue_abs_max=1e6,flags=[0,1],both_flags_true='permitted; terminationwins',gamma_lambda=[0,1],rho=[2.**-20,16],PPO_advantage_abs_max=1e9,epsilon=[0,1],all_numeric_inputs_finite_even_masked=True),gae_recipe=['q=RN(gamma*lambda)','bootstrap=!terminated','trace=!(terminated||truncated||local_collector_last_row)','next=bootstrap?RN(gamma*next_value):positivezero','delta=RN(RN(reward+next)-current_value)','advantage=trace?RN(delta+RN(q*next_advantage)):delta','lambda_return=RN(advantage+current_value)'],semantics=dict(next_value='Actual final observation value before reset; API cannot attest caller observation identity.',collector_tail='notenvironmenttermination; bootstrap survives unless terminated, recursive tailA0',truncation='external continuing-task cut; bootstrapfinalV and cutrecursion, do not passresetV',finite_horizon_terminal='a truefinite-horizon MDP end must be labelledterminated, not indiscriminatelybootstrapped',targets_detached=True,masks_not_one_done_bit=True),ppo_recipe=['lower=RN(1-epsilon);upper=RN(1+epsilon)','clipped_ratio=clamp(rho,lower,upper)','active=(A>0&&rho>upper)||(A<0&&rho<lower)','A==signedzero:surrogate/loss/gradient allpositivezero;clipped_ratio stillobserved','otherwise:unclipped=RN(rho*A);limited=RN(clipped_ratio*A);surrogate=min(unclipped,limited);loss=-surrogate','formal_dloss_dlogp=active?positivezero:-unclipped'],derivative='Formal piecewise objective wrt newlogprob with oldpolicy/advantagestopgradient; at EXACT rounded ratio knots chooseUNCLIPPEDderivative. Activeflag fromsign/ratiobounds,NEVERroundedproductties. NotgradientofquantizedroundedF64program.',numeric_profile='IEEE754binary64 RNties-even eachlistedop,gradualunderflow,noFMA/no-fastmath/reassociation; GAEsignedzerosfollowrecipe;PPOzeroAcanonicalpositivezeros',budget=dict(gae_unit_roundoff='u=2^-53',gae_effective_segment_L='countfromrowthroughfirsttracecut,atmostlocalhorizon128,NEVERtotalbatch2048',gae_local_delta_and_coefficient='exactFraction+RN atlistedboundaries;deltaall64bits exact',gae_advantage_absolute='2e-12 + gamma_(2L+4)*sum_abs(q^j*rounded_delta[t+j]);gamma_n=n*u/(1-n*u)',gae_return_absolute='advantageallowance + u*(abs(exact_future_sum)+abs(current_value)+advantageallowance) + 2^-1074',gae_budgets_rounded_outward=True,gae_primary='Explicitfuture residualsum/products againstreverseproductionrecurrence;Fraction orDecimal80,neverreverseoracle',gae_single_segment_L1='delta/advantage/return bits exact,including signedzero;longer mathematicalzero signnotassertedfromrealoracle',ppo='ALL4F64outputwords andactiveflag bitexact against exactFraction-RN operation/branchpolicy',all_masks_status_flags_exact=True,all3_native_context_bytes_exact=True,all_actual_expected_finite_beforemax=True),input_files={n:identity(F/n)for n in['gae-cases.bin','gae-steps.bin','ppo-inputs.f64']},gae_cases=meta,gae_steps=len(rows),PPO_cases=ppo,counts=dict(gae_cases=len(cases),gae_rows=len(rows),PPO_cases=len(ppo)),sources=[dict(title='High-Dimensional Continuous Control Using Generalized Advantage Estimation',revision='arXiv1506.02438v6',equation='16',url='https://arxiv.org/pdf/1506.02438v6'),dict(title='Proximal Policy Optimization Algorithms',revision='arXiv1707.06347v2',equation='7',url='https://arxiv.org/pdf/1707.06347v2'),dict(title='Time Limits in Reinforcement Learning',venue='PMLR80:4045-4054,2018',url='https://proceedings.mlr.press/v80/pardo18a.html')],paper_vs_owned_profile='Papers provide GAE/clippedsurrogate andtime-limitbootstrapping motivation. Bounds,finiteflags,maskoutput/layout,roundedknots/subgradient,atomicity andnumericprofileareowned explicitcontracts.',source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__)})
    write(F/'experiment.json',e);write(F/'input-freeze.json',dict(schema='psionic.locomotion-foundation.input-freeze.v1',passed=True,experiment=identity(F/'experiment.json'),input_files=e['input_files'],metadata_only=True,expected_generation_calls=0,product_or_model_or_environment_calls=0));print(json.dumps(dict(passed=True,experiment=identity(F/'experiment.json'),counts=e['counts'])))
def ppo_reference(row):
    rho,A,eps=row;lo=sub(1.,eps);hi=add(1.,eps);clipped=lo if rho<lo else hi if rho>hi else rho;active=int((A>0 and rho>hi)or(A<0 and rho<lo))
    if A==0:return(0.,0.,0.,clipped,0)
    raw=mul(rho,A);limited=mul(clipped,A);s=raw if raw<=limited else limited;return(s,-s,0. if active else-raw,clipped,active)
def local_residual(row,gamma):
    reward,V,nextV,term,trunc=row;b=int(not term);return sub(add(reward,mul(gamma,nextV)if b else 0.),V),b
def outward(value):
    x=float(value);return math.nextafter(x,math.inf)
def generate(out):
    require(not out.exists(),'preserve FIRST expected-data attempt');out.mkdir(parents=True);began=time.monotonic();r=dict(schema='psionic.locomotion-foundation.reference.v1',passed=False,product_calls=0,old_gate_calls=0,environment_or_learner_or_RNG_calls=0)
    try:
        e=json.loads((F/'experiment.json').read_text());seal=json.loads((F/'input-freeze.json').read_text());require(seal['experiment']==identity(F/'experiment.json')and e['source'][str(Path(__file__).resolve().relative_to(ROOT))]==identity(__file__),'immutable precomparison experiment/producer');require(all(identity(F/n)==v for n,v in e['input_files'].items()),'all frozen inputbytes');raw=(F/'gae-steps.bin').read_bytes();steps=[STEP.unpack_from(raw,i*40)for i in range(e['gae_steps'])];expected=[];bounds=[];exact=[];diagnostics=[]
        with localcontext()as ctx:
            ctx.prec=80
            for case in e['gae_cases']:
                W,H,at,n,g,l=case['worlds'],case['horizon'],case['offset_steps'],case['count'],case['gamma'],case['lambda_'];rows=steps[at:at+n];q=mul(g,l);delta=[];bs=[];cs=[]
                for i,row in enumerate(rows):d,b=local_residual(row,g);delta.append(d);bs.append(b);cs.append(int(not(row[3]or row[4]or i%H==H-1)))
                for t,row in enumerate(rows):
                    end=t
                    while cs[end]:end+=1
                    L=end-t+1
                    if case['oracle']=='fraction':
                        terms=[frac(delta[j])*(frac(q)**(j-t))for j in range(t,end+1)];A=sum(terms,Fraction());magnitude=sum(map(abs,terms),Fraction());av=float(A);target=float(A+frac(row[1]));absA=abs(A)
                    else:
                        terms=[Decimal.from_float(delta[j])*(Decimal.from_float(q)**(j-t))for j in range(t,end+1)];A=sum(terms,Decimal());magnitude=sum(map(abs,terms),Decimal());av=float(A);target=float(A+Decimal.from_float(row[1]));absA=abs(A)
                    mask=1
                    if L==1:av=delta[t];target=add(av,row[1]);mask=7
                    gn=Fraction(2*L+4)*U/(1-Fraction(2*L+4)*U);mag=Decimal(magnitude.numerator)/Decimal(magnitude.denominator)if isinstance(magnitude,Fraction)else magnitude;aa=Decimal(absA.numerator)/Decimal(absA.denominator)if isinstance(absA,Fraction)else absA;gnd=Decimal(gn.numerator)/Decimal(gn.denominator);ud=Decimal(U.numerator)/Decimal(U.denominator);allow=Decimal('2e-12')+gnd*mag;ret=allow+ud*(aa+abs(Decimal.from_float(row[1]))+allow)+Decimal.from_float(float.fromhex('0x0.0000000000001p-1022'));record=(delta[t],av,target,bs[t],cs[t]);finite(record[:3],3,'ALL independently summed expectedGAEwords');expected.append(record);bounds.append((0.,outward(allow),outward(ret)));exact.append(mask);diagnostics.append(dict(case=case['case'],row=t,segment_L=L,sum_abs_weighted_residuals=float(magnitude),advantage_allowance=outward(allow),return_allowance=outward(ret),exact_word_mask=mask))
        pporaw=(F/'ppo-inputs.f64').read_bytes();pporows=[struct.unpack_from('<3d',pporaw,i*24)for i in range(e['counts']['PPO_cases'])];ppowant=[ppo_reference(row)for row in pporows];finite([x for row in ppowant for x in row[:4]],4*len(ppowant),'ALL expectedPPOwords')
        (out/'gae-expected.bin').write_bytes(b''.join(GAE.pack(*x)for x in expected));(out/'gae-budgets.f64').write_bytes(b''.join(struct.pack('<3d',*x)for x in bounds));(out/'gae-exact.u8').write_bytes(bytes(exact));(out/'ppo-expected.bin').write_bytes(b''.join(PPO.pack(*x)for x in ppowant));write(out/'gae-rounding.json',diagnostics)
        require(len(expected)==e['gae_steps']and len(ppowant)==e['counts']['PPO_cases'],'complete frozen output inventories');r.update(passed=True,experiment=identity(F/'experiment.json'),source=e['source'],input_files=e['input_files'],counts=e['counts'],gae_finite_words=3*len(expected),ppo_finite_words=4*len(ppowant),GAE_primary_oracle='Explicitfuture sums/products andmask-cutsegments;Fraction orDecimal80;notreverse recurrence',GAE_local_delta_and_masks_exact=True,GAE_L1_all_word_bits_exact=sum(x==7 for x in exact),PPO_all_words_exact=True,outputs={n:identity(out/n)for n in['gae-expected.bin','gae-budgets.f64','gae-exact.u8','gae-rounding.json','ppo-expected.bin']})
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),counts=r['counts'])))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--freeze',action='store_true');p.add_argument('--output',type=Path);a=p.parse_args();require(a.freeze!=bool(a.output),'chooseinputfreeze OR once-onlyreference');freeze()if a.freeze else generate(a.output.resolve())
if __name__=='__main__':main()
