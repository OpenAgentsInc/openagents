#!/usr/bin/env python3
"""Independent bounded LQR math fixtures, not an executing learner.

Input construction, one-shot expected generation, and later pure conditional
comparison helpers are deliberately separate. Existing Gaussian/counter/GAE
tooling is loaded as definitions only; no old main or corpus is executed.
"""
from __future__ import annotations
import argparse
from decimal import Decimal, localcontext
from fractions import Fraction
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import time

ROOT = Path(__file__).resolve().parents[3]
F = ROOT / "psionic/fixtures/robotics/locomotion-lqr"
SOURCE = Path(__file__).resolve()
GAMMA, LAMBDA, EPSILON = .95, .95, .2
LOG_STD, SIGMA2, TRUST = -1.3862943611198906, .0625, 1/256
STARTS = [-4., -2., -.75, .75, 2., 4.]
INITIAL = [-1.]*4 + [-.25]*4 + [.25]*4 + [1.]*4
U = Fraction(1, 1 << 53)
ROW = struct.Struct("<8d2q3d2q")
BLOCK = struct.Struct("<q3d3q")
EPOCH = struct.Struct("<q3d2q3d4q")
_gaussian = None


def need(ok, message):
    if not ok:
        raise ValueError(message)


def identity(path):
    b = Path(path).read_bytes()
    return {"bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}


def write(path, obj):
    Path(path).write_text(json.dumps(obj, indent=2, allow_nan=False)+"\n")


def gaussian():
    global _gaussian
    if _gaussian is None:
        path = ROOT/"psionic/tests/robotics/locomotion_gaussian_reference.py"
        spec = importlib.util.spec_from_file_location("carried_gaussian_definitions", path)
        _gaussian = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(_gaussian)
    return _gaussian


def bits(x): return struct.unpack("<Q",struct.pack("<d",x))[0]
def finite(xs, count, role): need(len(xs)==count and all(math.isfinite(x) for x in xs),role+" extent/ALL finite")
def add(a,b): return gaussian().add(a,b)
def sub(a,b): return gaussian().sub(a,b)
def mul(a,b): return gaussian().mul(a,b)
def div(a,b): return gaussian().rn(Fraction(a)/Fraction(b))
def dec(x): return Decimal.from_float(x)


def sqrt_rn(x):
    with localcontext() as c:
        c.prec=100
        value=float(dec(x).sqrt())
    # Exact midpoint certificate for this NEW positive operand, not a libm call.
    if x>0:
        lo=Fraction(math.nextafter(value,-math.inf)); hi=Fraction(math.nextafter(value,math.inf)); v=Fraction(value)
        need(((lo+v)/2)**2 <= Fraction(x) <= ((hi+v)/2)**2,"certified RN square-root midpoint")
    return value


def critic_policy(K,x=0.):
    b=sub(1.,K); D=sub(1.,mul(GAMMA,mul(b,b)))
    P=div(add(1.,mul(K,K)),D)
    C=div(mul(add(1.,mul(GAMMA,P)),SIGMA2),sub(1.,GAMMA))
    V=-add(mul(P,mul(x,x)),C)
    return P,C,V


def real_benchmark():
    with localcontext() as c:
        c.prec=80
        g=dec(GAMMA); a=1-2*g
        P=(-a+(a*a+4*g).sqrt())/(2*g)
        K=g*P/(1+g*P)
        return +P,+K


def benchmark_policy():
    a=sub(1.,mul(2.,GAMMA)); disc=add(mul(a,a),mul(4.,GAMMA))
    P=div(add(-a,sqrt_rn(disc)),mul(2.,GAMMA)); gp=mul(GAMMA,P)
    return P,div(gp,add(1.,gp))


def quality_policy(K, observed_Pstar=None):
    """Exact local-RN field/flag policy after independent Pstar accuracy."""
    Pstar,_=benchmark_policy() if observed_Pstar is None else (observed_Pstar,0.)
    gp=mul(GAMMA,Pstar); Kstar=div(gp,add(1.,gp))
    error=abs(sub(K,Kstar)); D=sub(1.,mul(GAMMA,mul(sub(1.,K),sub(1.,K))))
    deltaP=div(mul(add(1.,gp),mul(sub(K,Kstar),sub(K,Kstar))),D)
    Cstar=div(mul(add(1.,gp),SIGMA2),sub(1.,GAMMA))
    P,C,_=critic_policy(K)
    ratios=[]; nominal=[]; optimal=[]
    for x in STARTS:
        xx=mul(x,x); factor=add(xx,div(mul(GAMMA,SIGMA2),sub(1.,GAMMA)))
        optimum=add(mul(Pstar,xx),Cstar)
        ratios.append(div(mul(deltaP,factor),optimum))
        nominal.append(add(mul(P,xx),C)); optimal.append(optimum)
    finite(ratios,6,"quality ratios before maximum")
    return (K,Pstar,Kstar,error,max(ratios),*ratios,*nominal,*optimal,int(error<=.05),int(max(ratios)<=.01))


def quality_real(K):
    with localcontext() as c:
        c.prec=80
        g=dec(GAMMA); k=dec(K); Pstar,Kstar=real_benchmark()
        D=1-g*(1-k)**2; P=(1+k*k)/D; C=(1+g*P)*dec(SIGMA2)/(1-g)
        deltaP=(1+g*Pstar)*(k-Kstar)**2/D
        Cstar=(1+g*Pstar)*dec(SIGMA2)/(1-g)
        ratios=[deltaP*(dec(x)**2+g*dec(SIGMA2)/(1-g))/(Pstar*dec(x)**2+Cstar) for x in STARTS]
        return {"Pstar":str(Pstar),"Kstar":str(Kstar),"P":str(P),"C":str(C),"gain_error":str(abs(k-Kstar)),
                "gain_margin":str(Decimal('.05')-abs(k-Kstar)),"relative_gaps":[str(x) for x in ratios],
                "cost_margin":str(Decimal('.01')-max(ratios)),"nominal_costs":[str(P*dec(x)**2+C) for x in STARTS]}


def project(Kold,candidate):
    return min(max(candidate,max(.25,sub(Kold,TRUST))),min(1.,add(Kold,TRUST)))


def density_logp_policy(K,x,action, inverse=4.):
    mean=-mul(K,x); d=0. if action==mean else sub(action,mean); z=mul(d,inverse)
    return sub(sub(mul(-.5,mul(z,z)),LOG_STD),gaussian().C)


def input_row(Kold,index,zero_advantage=False,fd=None):
    if fd is None:
        x=[-16.,-2.,-1.,-0.,0.,1.,2.,16.][index%8]
        z=[-3.,-1.,-0.,0.,1.,3.][(index//2)%6]
        A=[-1e5,-1.,-float.fromhex('0x0.0000000000001p-1022'),-0.,0.,float.fromhex('0x0.0000000000001p-1022'),1.,1e5][(index*3+1)%8]
    else:
        x,z,A=fd
    action=add(-mul(Kold,x),mul(.25,z))
    return x,action,density_logp_policy(Kold,x,action),0. if zero_advantage else A


def build_inputs():
    below=math.nextafter(.5,0.);above=math.nextafter(.5,1.)
    old_values=[.25,below,.5,above,.75,1.]
    blocks=[]; rows=[]
    for old in old_values:
        for offset in [-TRUST,0.,TRUST/2,TRUST]:
            K=project(old,add(old,offset))
            for n in [1,2,16,64]:
                blocks.append((K,old,len(rows),n)); rows += [input_row(old,i) for i in range(n)]
    fd_indices=[]
    for x in [-2.,-1.,1.,2.]:
        for z in [-2.,2.]:
            for A in [-1.,1.]:
                fd_indices.append(len(blocks));blocks.append((.5,.5,len(rows),1));rows.append(input_row(.5,0,fd=(x,z,A)))
    ks=[.25,math.nextafter(.25,1.),.375,below,.5,above,.75,math.nextafter(1.,0.),1.]
    transitions=[]
    xs=[-16.,-4.,-1.,-0.,0.,float.fromhex('0x0.0000000000001p-1022'),1.,4.,16.]
    for k in ks:
        for i,x in enumerate(xs):
            z=[-3.,-1.,-0.,0.,1.,3.][i%6]; action=add(-mul(k,x),mul(.25,z))
            transitions.append((k,x,action,z))
    projections=[]
    for old in ks:
        lo=max(.25,sub(old,TRUST));hi=min(1.,add(old,TRUST))
        projections += [(old,x) for x in [math.nextafter(lo,-math.inf),lo,math.nextafter(lo,math.inf),math.nextafter(hi,-math.inf),hi,math.nextafter(hi,math.inf)]]
    epochs=[]; erows=[]
    histories=[(.5,0.,0.,0),(add(.5,TRUST),-1.,.25,3),(sub(.5,TRUST),1.,.25,3),(.5,.01,.25,127)]
    for history,(k,m,v,t) in enumerate(histories):
        for n in [1,2,16,64,65,512]:
            epochs.append((.5,k,m,v,t,0,len(erows),n));erows += [input_row(.5,i,zero_advantage=history in (1,2)) for i in range(n)]
    evaluations=[(k,j,[1,2,3,127][(i+j)%4]) for i,k in enumerate([.25,.5,.75,1.]) for j in range(6)]
    scores=[.25,math.nextafter(.25,1.),.375,below,.5,above,.55,.6,.625,.65,.7,.75,.9,math.nextafter(1.,0.),1.]
    Pstar,Kstar=benchmark_policy()
    for root in [sub(Kstar,.05),add(Kstar,.05)]:scores += [math.nextafter(root,0.),root,math.nextafter(root,1.)]
    with localcontext() as c:
        c.prec=80
        p,k=real_benchmark();g=dec(GAMMA);noise=g*dec(SIGMA2)/(1-g)
        jstar=p*16+(1+g*p)*dec(SIGMA2)/(1-g)
        pt=p+Decimal('.01')*jstar/(16+noise)
        a=1+g*pt;b=-2*g*pt;d=1-(1-g)*pt
        roots=[float((-b-sign*(b*b-4*a*d).sqrt())/(2*a)) for sign in [1,-1]]
    for root in roots:scores += [math.nextafter(root,0.),root,math.nextafter(root,1.)]
    configs=[(1,1),(1,2),(1,32),(2,1),(2,16),(4,4),(16,32)]
    rollouts=[]; worlds=[]
    for case,((W,H),k) in enumerate(zip(configs,[.25,.5,.75,.625,1.,.375,.5])):
        rollouts.append((W,H,k,len(worlds),W))
        for w in range(W):worlds.append((INITIAL[w],11+17*case,w*8192+case*128))
    need((len(blocks),len(rows),len(ks),len(transitions),len(projections),len(epochs),len(erows),len(evaluations),len(scores),len(rollouts),len(worlds))==(112,2008,9,81,54,24,2640,24,27,7,27),"sealed fixture counts")
    files={
      "actor-cases.bin":b''.join(struct.pack('<2d2q',*r) for r in blocks),"actor-rows.bin":b''.join(struct.pack('<4d',*r) for r in rows),
      "critic-inputs.f64":struct.pack('<9d',*ks),"transition-inputs.f64":b''.join(struct.pack('<4d',*r) for r in transitions),
      "projection-inputs.f64":b''.join(struct.pack('<2d',*r) for r in projections),"epoch-cases.bin":b''.join(struct.pack('<4d4q',*r) for r in epochs),
      "epoch-rows.bin":b''.join(struct.pack('<4d',*r) for r in erows),"eval-inputs.bin":b''.join(struct.pack('<d2q',*r) for r in evaluations),
      "score-inputs.f64":struct.pack('<27d',*scores),"rollout-cases.bin":b''.join(struct.pack('<2qd2q',*r) for r in rollouts),
      "rollout-worlds.bin":b''.join(struct.pack('<d2Q',*r) for r in worlds)}
    return files,{"FD_case_indices":fd_indices,"FD_h":[1e-3,1e-4,1e-5,1e-6,1e-7],"FD_primary_h":1e-5,"input_threshold_selection_benchmark":{"Pstar":Pstar,"Kstar":Kstar,"real_cost_roots":roots},"rollout_steps":sum(W*H for W,H in configs),"eval_steps":sum(r[2] for r in evaluations)}


def freeze(contract):
    need(not (F/'experiment.json').exists(),"immutable input freeze already exists")
    F.mkdir(parents=True,exist_ok=True);data,metadata=build_inputs()
    for name,raw in data.items():(F/name).write_bytes(raw)
    (F/'contract.md').write_bytes(contract.read_bytes())
    carried=["psionic/tests/robotics/locomotion_gaussian_reference.py","psionic/tests/robotics/locomotion_reference.py","psionic/lib/tape.coax","psionic/lib/optim/adamw.coax","psionic/lib/counter-rng.coax","psionic/fixtures/robotics/locomotion-gaussian/native-qualification.json","psionic/fixtures/robotics/locomotion/native-qualification.json"]
    e={"schema":"psionic.locomotion-lqr.experiment.v1","frozen_before_expected_product_learning":True,"contract":identity(F/'contract.md'),
       "counts":{"actor_blocks":112,"actor_rows":2008,"critic":9,"transition":81,"projection":54,"epochs":24,"epoch_rows":2640,"eval":24,"score":27,"rollouts":7,"worlds":27,"rollout_steps":597,"eval_steps":798},
       "metadata":metadata,"input_files":{n:identity(F/n) for n in data},"carried":{n:identity(ROOT/n) for n in carried},
       "new_source_qualification":identity(F/'source-qualification.json'),"new_unit_refusals":identity(F/'unit-refusals.json'),
       "input_layouts":{"actor-cases.bin":"<2d2q K,Kold,offset,N","actor-rows.bin":"<4d x,action,old_logp,A","critic-inputs.f64":"<d K","transition-inputs.f64":"<4d K,x,action,z","projection-inputs.f64":"<2d Kold,candidate","epoch-cases.bin":"<4d4q Kold,K,m,v,step,skipped,offset,N","epoch-rows.bin":"<4d x,action,old_logp,A","eval-inputs.bin":"<d2q K,startid,N","score-inputs.f64":"<d K","rollout-cases.bin":"<2qd2q W,H,K,worldOffset,worldCount","rollout-worlds.bin":"<d2Q x,seed,counter"},
       "output_layouts":{"actor-expected.bin":"<q3d3q status,loss,gradient,proxy,N,nodes,clipped","actor-coefficients.bin":"<10dq Density4+Rlr6+active (88B)","critic-expected.bin":"<q2d status,P,C","transition-expected.bin":"statusI64+RleRow120","projection-expected.bin":"<qd status,projectedK","epoch-expected.bin":"statusI64+RltEpoch96","epoch-coefficients.bin":"<10dq Density4+Rlr6+active (88B)","eval-expected.bin":"<q7dq status,Evaluation64","score-expected.bin":"<q23d2q status,Quality200","rollout-expected.bin":"statusI64+Batch61872; unused rows/worlds canonicalzero"},
       "domains":{"K":[.25,1.],"x":[-16.,16.],"action":[-18.25,18.25],"z":[-9.,9.],"old_logp":[-1e6,1e6],"advantage":[-1e9,1e9],"actor_rows":[1,64],"epoch_rows":[1,512]},
       "budgets":{"ordinary":{"absolute":2e-12,"relative":2e-12},"block":{"absolute":8e-12,"sum_absolute_contribution_factor":4e-13},"rollout_target":{"absolute":2e-11,"relative":2e-12},"density":{"absolute":4e-12,"relative":8e-15},"rho_inverse":{"absolute":0.,"relative":4e-15},"FD_primary_absolute":3e-8},
       "coefficient_proof_order":["ALL numeric finite","independent Decimal density/inverse accuracy","exact observed-logp dlog","independent Decimal exp accuracy","exact Fraction-RN Rcp","analytic stored-primal VJP using bounded weights, never observed score/gradient"],
       "epoch_proof_order":["independent actor gradient/loss bounds","conditional exact-RN moments from observed gradient","independent Decimal pow/sqrt candidate accuracy","exact projection/flag from observed candidate","retained moments/step"],
       "score_scope":"Exact source F64 fields/flags conditioned on independently accurate exposed Pstar; independent Decimal real-optimum and signed goal-margin diagnostics; ambiguous real thresholds never promote learning.",
       "unavailable":"Non-ok transport is canonical zero ONLY AFTER full native physical refusal atomicity assertions; not a published result.","node_formula":"7+13*N",
       "training":{"seeds":[0,1,7],"initial":INITIAL,"worlds":16,"horizon":32,"batches":128,"epochs":4,"block":64,"sigma":.25,"log_std":LOG_STD,"gamma":GAMMA,"lambda":LAMBDA,"epsilon":EPSILON,"initial_K":.25,"initial_m_v_step_skipped":[0,0,0,0],"lr":.001,"beta1":.9,"beta2":.999,"adam_eps":1e-8,"decay":0.,"trust":TRUST,"projection":"rounded rollout-start K_old endpoints, retain Adam moments/step","counter":"world*8192+batch*64","collector":"carry states, term/trunc0, local tail tracecut only","chunks":16,"command_timeout":180,"canonical_steps":196608,"canonical_updates":1536,"replay_steps":1536,"replay_updates":12,"deterministic_eval_steps":3072,"replay_after_completed_batches":64,"replay_zero_based_batch":64,"heldout_initials":STARTS,"all_seed_goals":{"gain_error_max":.05,"nominal_relative_cost_gap_max":.01,"complete_batches":128,"skips_refusals":0},"no_best_seed_checkpoint_or_tuning":True},
       "learning_executions":0,"model_SDK_old_gate_calls":0}
    write(F/'experiment.json',e);write(F/'input-freeze.json',{"schema":"psionic.locomotion-lqr.input-freeze.v1","experiment":identity(F/'experiment.json'),"producer":identity(SOURCE),"expected_generation_calls":0,"input_benchmark_selection_only":True})
    print(json.dumps({"experiment":identity(F/'experiment.json'),"producer":identity(SOURCE),"counts":e['counts']}))


def coefficients(K,row,inverse=4.):
    x,action,old,A=row; mean=-mul(K,x)
    density=gaussian().density_reference(mean,LOG_STD,action)
    # Ideal local-RN Gaussian point anchors conditional coefficient references.
    logp=density_logp_policy(K,x,action,inverse);density=(logp,*density[1:]);dlog=sub(logp,old)
    rho=gaussian().decimal_exp(dlog);objective=gaussian().ppo_from_ratio(rho,A,EPSILON)
    return (*density,dlog,rho,*objective)


def admit_coefficients(K,rows,probes,inverse):
    """Independently validate ALL realized coefficients before conditional VJP.

    This function calls no product and never accepts an observed score/gradient
    as the derivative oracle.  The actor's analytic derivative is computed
    separately from inputs and its stored-primal operation recipe.
    """
    need(len(rows)==len(probes),"complete coefficient rows")
    finite([x for p in probes for x in p[:10]],10*len(probes),"ALL coefficient fields before errors")
    finite([inverse],1,"fixed inverse sigma")
    reference_inverse=gaussian().decimal_exp(-LOG_STD)
    finite([reference_inverse],1,"expected fixed inverse sigma")
    need(abs(inverse-reference_inverse)<=4e-15*abs(reference_inverse),"independent fixed inverse accuracy")
    expected_density=[gaussian().density_reference(-mul(K,row[0]),LOG_STD,row[1]) for row in rows]
    finite([x for p in expected_density for x in p],4*len(rows),"ALL expected density coefficients")
    # First validate the complete density/dlog set before requesting any exp.
    for row,p,expected in zip(rows,probes,expected_density):
        x,action,old,A=row; mean=-mul(K,x)
        for j,(actual,want) in enumerate(zip(p[:4],expected)):
            error=abs(actual-want);need(error<=4e-12+8e-15*abs(want),"independent density coefficient accuracy")
        need(bits(p[3])==bits(expected[3]),"coefficient entropy exact")
        if action==mean:need(all(bits(a)==bits(b) for a,b in zip(p[:4],expected)),"density-at-mean exact coefficients")
        dlog=sub(p[0],old);need(bits(p[4])==bits(dlog),"exact realized-logp to dlog")
        lower,upper=gaussian().bounds();need(lower<=dlog<=upper,"safe coefficient exp interval")
        need(2**-20<=p[5]<=16,"realized coefficient ratio domain")
    expected_rho=[gaussian().decimal_exp(p[4]) for p in probes]
    finite(expected_rho,len(probes),"ALL expected coefficient exp values")
    for p,rho in zip(probes,expected_rho):
        error=abs(p[5]-rho)
        need(error<=4e-15*abs(rho),"independent coefficient exp accuracy")
    # Rcp is exact on each separately accuracy-qualified realized ratio.
    expected_objectives=[gaussian().ppo_from_ratio(p[5],row[3],EPSILON) for row,p in zip(rows,probes)]
    finite([x for p in expected_objectives for x in p[:4]],4*len(rows),"ALL expected conditional Rcp coefficients")
    for p,expected in zip(probes,expected_objectives):
        need(struct.pack('<4dq',*p[6:])==struct.pack('<4dq',*expected),"exact conditional Rcp coefficient words/flag")
    errors=[max((abs(p[j]-want[j]) for p,want in zip(probes,expected_density)),default=0.) for j in range(4)]
    rho_error=max((abs(p[5]-want)/abs(want) for p,want in zip(probes,expected_rho)),default=0.)
    return {"density_absolute":errors,"rho_relative":rho_error,"inverse":inverse,"rows":len(rows)}


def conditional_block(K,rows,probes,inverse):
    """Coefficients/inverse MUST be independently admitted before this call."""
    need(len(rows)==len(probes),"block coefficient extent")
    gradients=[]; losses=[]; proxies=[];clipped=0
    for (x,action,old,A),probe in zip(rows,probes):
        mean=-mul(K,x);d=0. if action==mean else sub(action,mean);z=mul(d,inverse)
        weight=probe[8]  # Density4, dlog,rho,S,loss,GRAD,clippedrho,active.
        gradients.append(Fraction(weight)*Fraction(-x)*Fraction(z)*Fraction(inverse))
        losses.append(Fraction(probe[7]));proxies.append(Fraction(weight)*Fraction(probe[0]));clipped+=probe[10]
    groups=[losses,gradients,proxies]
    values=[float(sum(v,Fraction())) for v in groups]
    magnitudes=[float(sum(map(abs,v),Fraction())) for v in groups]
    finite(values,3,"independent block values");finite(magnitudes,3,"block conditioners")
    return values,magnitudes,clipped


def adam_policy(optimizer,gradient):
    K,m,v,step,skipped=optimizer;t=step+1
    m2=add(mul(.9,m),mul(sub(1.,.9),gradient))
    v2=add(mul(.999,v),mul(mul(sub(1.,.999),gradient),gradient))
    with localcontext() as c:
        c.prec=100
        beta1=float(dec(.9)**t);beta2=float(dec(.999)**t)
    c1=sub(1.,beta1);c2=sub(1.,beta2)
    denominator=add(div(sqrt_rn(v2),sqrt_rn(c2)),1e-8)
    candidate=sub(K,mul(div(.001,c1),div(m2,denominator)))
    return candidate,m2,v2,t,skipped


def eval_reference(K,start,N):
    with localcontext() as c:
        c.prec=80
        k=dec(K);x=dec(STARTS[start]);g=dec(GAMMA);a=1-k;r=g*a*a
        P=(1+k*k)/(1-r); cost=(1+k*k)*x*x*(1-r**N)/(1-r)
        final=a**N*x; discount=g**N;tail=discount*P*final*final
        return K,float(cost),float(tail),float(cost+tail),float(P*x*x),float(final),float(discount),N


def transition_reference(K,x,action,z):
    nextx=add(x,action);P,C,V=critic_policy(K,x);nextV=critic_policy(K,nextx)[2]
    reward=-add(mul(x,x),mul(action,action))
    return (x,action,nextx,density_logp_policy(K,x,action),z,reward,V,nextV,0,0,0.,0.,0.,0,0)


def target_rows(steps,H):
    q=mul(GAMMA,LAMBDA);deltas=[sub(add(r[5],mul(GAMMA,r[7])),r[6]) for r in steps]
    out=[]
    for i,row in enumerate(steps):
        end=(i//H+1)*H;terms=[Fraction(deltas[j])*Fraction(q)**(j-i) for j in range(i,end)]
        A=sum(terms,Fraction());adv=float(A);target=float(A+Fraction(row[6]))
        if i==end-1:adv=deltas[i];target=add(adv,row[6])
        out.append((deltas[i],adv,target,1,int(i!=end-1)))
    return out


def rollout_reference(W,H,K,worlds):
    rows=[]; next_worlds=[];P,C,_=critic_policy(K)
    for x,seed,counter in worlds:
        for _ in range(H):
            mean=-mul(K,x);u=gaussian().splitmix_top53(seed,counter);v=gaussian().splitmix_top53(seed,counter+1)
            action,z=gaussian().uniform_reference(mean,LOG_STD,u,v)
            row=transition_reference(K,x,action,z);rows.append(row);x=row[2];counter+=2
        next_worlds.append((x,seed,counter))
    targets=target_rows(rows,H)
    rows=[(*r[:10],*target) for r,target in zip(rows,targets)]
    raw=struct.pack('<3q3d',W,H,W*H,K,P,C)+b''.join(ROW.pack(*r) for r in rows)+bytes((512-len(rows))*120)
    raw+=b''.join(struct.pack('<d2Q',*w) for w in next_worlds)+bytes((16-W)*24)
    need(len(raw)==61872,"fixed rollout ABI")
    return raw


def fd_loss(K,row):
    x,action,old,A=map(dec,row);k=K if isinstance(K,Decimal) else dec(K)
    with localcontext() as c:
        c.prec=80
        logp=-(action+k*x)**2*8-dec(LOG_STD)-dec(gaussian().C)
        rho=(logp-old).exp();lo=dec(sub(1.,EPSILON));hi=dec(add(1.,EPSILON))
        return -min(rho*A,min(max(rho,lo),hi)*A)


def read_records(path,fmt):
    f=struct.Struct(fmt);b=Path(path).read_bytes();need(len(b)%f.size==0,"input records");return list(f.iter_unpack(b))


def finite_expected_outputs(outputs):
    layouts={'actor-expected.bin':('<q3d3q',[1,2,3]),'critic-expected.bin':('<q2d',[1,2]),
      'transition-expected.bin':('<q8d2q3d2q',list(range(1,9))+[11,12,13]),
      'projection-expected.bin':('<qd',[1]),'epoch-expected.bin':('<q3d2q3d4q',[1,2,3,6,7,8]),
      'eval-expected.bin':('<q7dq',list(range(1,8))),'score-expected.bin':('<q23d2q',list(range(1,24))),
      'actor-coefficients.bin':('<10dq',list(range(10))),'epoch-coefficients.bin':('<10dq',list(range(10)))}
    for name,(layout,indices) in layouts.items():
        f=struct.Struct(layout);raw=outputs[name];need(len(raw)%f.size==0,name+' record extent')
        values=[row[i] for row in f.iter_unpack(raw) for i in indices]
        finite(values,len(raw)//f.size*len(indices),name+' expected payload')
    for name in ['actor-conditions.f64','epoch-conditions.f64']:
        raw=outputs[name];finite(list(struct.unpack('<'+str(len(raw)//8)+'d',raw)),len(raw)//8,name)
    raw=outputs['rollout-expected.bin'];need(len(raw)==7*61880,'ALL rollout expected extents')
    for case in range(7):
        body=raw[case*61880+8:(case+1)*61880]
        header=struct.unpack_from('<3q3d',body);finite(list(header[3:]),3,'rollout header')
        values=[]
        for i in range(512):
            row=ROW.unpack_from(body,48+i*120);values.extend(row[:8]);values.extend(row[10:13])
        values.extend(struct.unpack_from('<d2Q',body,48+512*120+i*24)[0] for i in range(16))
        finite(values,512*11+16,'ALL rollout active/unused payload')


def generate(output):
    need(not output.exists(),"preserve FIRST reference attempt");seal=json.loads((F/'input-freeze.json').read_text());e=json.loads((F/'experiment.json').read_text())
    need(seal['producer']==identity(SOURCE) and seal['experiment']==identity(F/'experiment.json'),"whole producer/input frozen")
    for n,h in e['input_files'].items():need(identity(F/n)==h,"immutable input "+n)
    for n,h in e['carried'].items():need(identity(ROOT/n)==h,"carried source/proof "+n)
    output.mkdir(parents=True);began=time.monotonic();record={"schema":"psionic.locomotion-lqr.reference.v1","passed":False,"producer":identity(SOURCE),"experiment":identity(F/'experiment.json'),"counts":e['counts'],"input_files":e['input_files'],"product_learner_model_SDK_calls":0}
    write(output/'record.json',record)
    actor_cases=read_records(F/'actor-cases.bin','<2d2q'); actor_rows=read_records(F/'actor-rows.bin','<4d')
    epoch_cases=read_records(F/'epoch-cases.bin','<4d4q');epoch_rows=read_records(F/'epoch-rows.bin','<4d')
    outputs={};conditioners={};all_coeff=[]
    actor_raw=bytearray();actor_conditions=[]
    for K,old,at,n in actor_cases:
        rows=actor_rows[at:at+n];probes=[coefficients(K,r) for r in rows];values,mags,clipped=conditional_block(K,rows,probes,4.)
        actor_raw+=BLOCK.pack(0,*values,n,7+13*n,clipped);actor_conditions.extend(mags);all_coeff+=probes
    outputs['actor-expected.bin']=actor_raw;outputs['actor-coefficients.bin']=b''.join(struct.pack('<10dq',*p) for p in all_coeff)
    outputs['actor-conditions.f64']=struct.pack('<'+str(len(actor_conditions))+'d',*actor_conditions)
    outputs['critic-expected.bin']=b''.join(struct.pack('<q2d',0,*critic_policy(k)[:2]) for (k,) in read_records(F/'critic-inputs.f64','<d'))
    outputs['transition-expected.bin']=b''.join(struct.pack('<q',0)+ROW.pack(*transition_reference(*r)) for r in read_records(F/'transition-inputs.f64','<4d'))
    outputs['projection-expected.bin']=b''.join(struct.pack('<qd',0,project(*r)) for r in read_records(F/'projection-inputs.f64','<2d'))
    epoch_raw=bytearray();epoch_conditions=[];epoch_coeff=[]
    for old,K,m,v,t,skip,at,n in epoch_cases:
        rows=epoch_rows[at:at+n];probes=[coefficients(K,r) for r in rows]
        losses=[];gradients=[];proxy=[];mags=[0.,0.,0.];clip=0
        for start in range(0,n,64):
            vals,ms,clipped=conditional_block(K,rows[start:start+64],probes[start:start+64],4.)
            losses.append(vals[0]);gradients.append(vals[1]);clip+=clipped
            mags=[add(a,b) for a,b in zip(mags,ms)]
        loss=div(math.fsum(losses),float(n));gradient=div(math.fsum(gradients),float(n));candidate,m2,v2,t2,s2=adam_policy((K,m,v,t,skip),gradient)
        projected=project(old,candidate);epoch_raw+=EPOCH.pack(0,projected,m2,v2,t2,s2,loss,gradient,candidate,int(projected!=candidate),n,(n+63)//64,clip)
        epoch_conditions.extend([div(mags[0],float(n)),div(mags[1],float(n))]);epoch_coeff+=probes
    outputs['epoch-expected.bin']=epoch_raw;outputs['epoch-conditions.f64']=struct.pack('<'+str(len(epoch_conditions))+'d',*epoch_conditions)
    outputs['epoch-coefficients.bin']=b''.join(struct.pack('<10dq',*p) for p in epoch_coeff)
    outputs['eval-expected.bin']=b''.join(struct.pack('<q7dq',0,*eval_reference(*r)) for r in read_records(F/'eval-inputs.bin','<d2q'))
    scores=read_records(F/'score-inputs.f64','<d');outputs['score-expected.bin']=b''.join(struct.pack('<q23d2q',0,*quality_policy(k)) for (k,) in scores)
    world_inputs=read_records(F/'rollout-worlds.bin','<d2Q')
    outputs['rollout-expected.bin']=b''.join(struct.pack('<q',0)+rollout_reference(W,H,K,world_inputs[at:at+n]) for W,H,K,at,n in read_records(F/'rollout-cases.bin','<2qd2q'))
    fd=[]
    with localcontext() as c:
        c.prec=80
        for case in e['metadata']['FD_case_indices']:
            K,old,at,n=actor_cases[case];row=actor_rows[at];d=dec(row[1])+dec(K)*dec(row[0]);rho=(-d*d*8-dec(LOG_STD)-dec(gaussian().C)-dec(row[2])).exp();analytic=rho*dec(row[3])*16*dec(row[0])*d
            sweep=[]
            for h in e['metadata']['FD_h']:
                hd=dec(h);value=(fd_loss(dec(K)+hd,row)-fd_loss(dec(K)-hd,row))/(2*hd)
                rhos=[]
                for probeK in [dec(K)+hd,dec(K)-hd]:
                    rhos.append((-(dec(row[1])+probeK*dec(row[0]))**2*8-dec(LOG_STD)-dec(gaussian().C)-dec(row[2])).exp())
                margin=min(min(r-dec(sub(1.,EPSILON)),dec(add(1.,EPSILON))-r) for r in rhos)
                need(margin>Decimal('.1'),'predeclared FD branch margin')
                sweep.append({'h':h,'value':str(value),'error':str(abs(value-analytic)),'clip_margin':str(margin)})
            primary=next(x for x in sweep if x['h']==e['metadata']['FD_primary_h']);need(Decimal(primary['error'])<=Decimal('3e-8'),'predeclared FD primary budget')
            fd.append({'case':case,'analytic':str(analytic),'sweep':sweep})
    finite_expected_outputs(outputs)
    for name,raw in outputs.items():(output/name).write_bytes(raw)
    write(output/'fd-diagnostics.json',fd);write(output/'score-real.json',[quality_real(k) for (k,) in scores])
    record.update(passed=True,seconds=time.monotonic()-began,outputs={n:identity(output/n) for n in [*outputs,'fd-diagnostics.json','score-real.json']},
                  conditional_scope="Native coefficients/inverse are independently admitted FIRST; then conditional VJP, observed-gradient AdamW, observed-candidate projection and observed-Pstar scorer. Ideal tables are not substitutes for those staged checks.")
    write(output/'record.json',record);print(json.dumps({'passed':True,'record':identity(output/'record.json')}))


def main():
    p=argparse.ArgumentParser();p.add_argument('--freeze',action='store_true');p.add_argument('--contract',type=Path,default=Path('/tmp/psionic171-lqr-contract-proposal.md'));p.add_argument('--output',type=Path);a=p.parse_args()
    if a.freeze:freeze(a.contract)
    else:
        need(a.output is not None,'--output required')
        try:generate(a.output)
        except BaseException as error:
            if (a.output/'record.json').exists():
                record=json.loads((a.output/'record.json').read_text());record.update(error=type(error).__name__+': '+str(error));write(a.output/'record.json',record)
            raise


if __name__=='__main__':main()
