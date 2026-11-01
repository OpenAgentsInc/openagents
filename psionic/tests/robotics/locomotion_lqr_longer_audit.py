#!/usr/bin/env python3
"""Offline audit of NEW continuation chunks; carries the original numeric kernel.

All coefficient observations are source-bound recomputations, not invented
internal instrumentation.  This tool imports frozen independent math kernels
as definitions and never calls their input/expected-generation entry points.
"""
from __future__ import annotations
import argparse
from fractions import Fraction
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import time

ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
X=ROOT/'psionic/fixtures/robotics/locomotion-lqr-longer'
SOURCE=Path(__file__).resolve()
STATE=struct.Struct('<3d2q'+('dQQ'*16)+'qQ')
EPOCH=struct.Struct('<3d2q3d4q')
ROW=struct.Struct('<8d2q3d2q')
COEFF=struct.Struct('<10dq')
ITER_BYTES=62696


def need(ok,message):
    if not ok:raise ValueError(message)


def identity(path):
    b=Path(path).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}


def load(path):return json.loads(Path(path).read_text())
def write(path,value):Path(path).write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')
def bits(value):return struct.pack('<d',value)
def finite(values,n,role):need(len(values)==n and all(math.isfinite(x) for x in values),role+' ALL finite/extent')


def math_kernel():
    path=ROOT/'psionic/tests/robotics/locomotion_lqr_reference.py'
    spec=importlib.util.spec_from_file_location('frozen_lqr_math',path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m


def header(raw,magic,width,cap,context,role):
    need(len(raw)>=48,role+' header extent')
    need(struct.unpack_from('<4I',raw)==(magic,1,width,cap),role+' version/profile/header')
    need(raw[16:48]==context,role+' exact context')
    return raw[48:]


def decode_state(raw):
    need(len(raw)==440,'state440');r=STATE.unpack(raw)
    values=list(r[:3])+[r[5+3*w] for w in range(16)];finite(values,19,'state numeric fields')
    return {'raw':raw,'K':r[0],'m':r[1],'v':r[2],'step':r[3],'skipped':r[4],
            'worlds':[r[5+3*w:8+3*w] for w in range(16)],'batch':r[-2],'seed':r[-1]}


def admit_state(state,seed=None,batch=None):
    need(state['seed'] in (0,1,7),'prescribed seed')
    if seed is not None:need(state['seed']==seed,'seed identity')
    if batch is not None:need(state['batch']==batch,'batch identity')
    b=state['batch'];need(128<=b<=512 and state['step']==4*b and state['skipped']==0,'state index/Adam lineage')
    need(.25<=state['K']<=1 and state['v']>=0,'state actor/moment domains')
    for w,(x,s,c) in enumerate(state['worlds']):
        need(abs(x)<=16 and s==state['seed'] and c==131072+w*24576+(b-128)*64,'state world/counter lineage')


def snapshot(path,context):
    body=header(Path(path).read_bytes(),0x3153584c,440,0,context,'LXS1')
    return decode_state(body)


def admit_import_words(old_raw,new_raw,seed):
    """Only the sixteen RNG-counter words may change at authenticated import."""
    old=decode_state(old_raw);new=decode_state(new_raw)
    need(old['seed']==seed and seed in (0,1,7),'original prescribed seed')
    need(old['batch']==128 and old['step']==512 and old['skipped']==0,'original completed prefix')
    need(.25<=old['K']<=1 and old['v']>=0,'original state domains')
    for w,(x,s,c) in enumerate(old['worlds']):
        need(abs(x)<=16 and s==seed and c==w*8192+128*64,'original world/counter metadata')
    expected=bytearray(old_raw)
    for w in range(16):struct.pack_into('<Q',expected,40+w*24+16,131072+w*24576)
    need(new_raw==bytes(expected),'EXACT non-counter preservation and remapped counter words')
    admit_state(new,seed,128)


def audit_import(args,context):
    need(args.seed in (0,1,7),'import prescribed seed')
    e=load(X/'experiment.json');item=e['resume_sources'][str(args.seed)]
    need(identity(args.before)==item['snapshot'],'EXACT original final snapshot')
    old_context=bytes.fromhex(identity(F/'learning-execution/descriptor.json')['sha256'])
    old_raw=header(args.before.read_bytes(),0x3153514c,440,0,old_context,'original LQS1')
    original=load(ROOT/item['audit_path'])
    need(identity(ROOT/item['audit_path'])==item['audit'],'original independent endpoint receipt')
    need(original['schema']=='psionic.locomotion-lqr.learning-audit.v1' and original['passed'] is True,'original PASSED audit')
    need(original['auditor']==e['original_auditor'],'original whole auditor identity')
    need(original['inputs']['descriptor']==e['original_descriptor'],'original audit context')
    need(original['inputs']['after']==item['snapshot'],'original audited endpoint identity')
    need(original['result']['seed']==args.seed and original['result']['first_batch']==112 and original['result']['batches']==16,'original completed batch128')
    imported=snapshot(args.after,context);admit_import_words(old_raw,imported['raw'],args.seed)
    need({'bytes':440,'sha256':hashlib.sha256(imported['raw']).hexdigest()}==item['remapped_raw'],'sealed expected counter-only import')
    return {'passed':True,'seed':args.seed,'first_batch':128,'batches':0,'import_only':True,
            'plant_steps':0,'AdamW_updates':0,'coefficient_observations':0,'changed_counter_words':16,
            'noncounter_bytes_exact':True,'original_endpoint_audit':item['audit'],
            'scope':'New context import only; no original prefix, optimizer, plant or RNG execution.'}


def admit_previous(previous,seed,batch,descriptor_identity,before_identity):
    need(previous['schema']=='psionic.locomotion-lqr-longer.learning-audit.v1' and previous['passed'] is True,'prior PASSED audit')
    need(previous['auditor']==identity(SOURCE),'prior same-whole-source audit')
    need(previous['inputs']['descriptor']==descriptor_identity,'prior same-context audit')
    result=previous['result'];need(result['seed']==seed and result['first_batch']+result['batches']==batch,'prior contiguous seed/batch audit')
    need(previous['inputs']['after']==before_identity,'before equals prior independently audited endpoint')


def decode_iterations(raw,context,count):
    body=header(raw,0x3149584c,62696,16,context,'LXI1');need(len(body)==count*ITER_BYTES,'complete iteration extent')
    result=[];values=[]
    for i in range(count):
        chunk=body[i*ITER_BYTES:(i+1)*ITER_BYTES];h=struct.unpack_from('<3q3d',chunk)
        need(h[:3]==(16,32,512),'named16x32 batch')
        rows=[ROW.unpack_from(chunk,48+j*120) for j in range(512)]
        worlds=[struct.unpack_from('<d2Q',chunk,61488+j*24) for j in range(16)]
        epochs=[EPOCH.unpack_from(chunk,61872+j*96) for j in range(4)]
        next_state=decode_state(chunk[62256:62696]);values.extend(h[3:])
        for r in rows:values.extend(r[:8]);values.extend(r[10:13])
        values.extend(w[0] for w in worlds)
        for e in epochs:values.extend(e[:3]);values.extend(e[5:8])
        result.append({'raw':chunk,'header':h,'rows':rows,'worlds':worlds,'epochs':epochs,'next':next_state})
    finite(values,count*(3+512*11+16+4*6),'ALL complete iteration numeric fields')
    return result


def decode_coefficients(raw,context,count):
    body=header(raw,0x3143584c,88,32768,context,'LXC1');need(len(body)==count*2048*88,'complete coefficient extent')
    rows=list(COEFF.iter_unpack(body));finite([x for r in rows for x in r[:10]],count*2048*10,'ALL coefficient observations')
    need(all(r[-1] in (0,1) for r in rows),'coefficient active flags')
    return rows


def bindings(args):
    descriptor=load(args.descriptor);need(descriptor['schema']=='psionic.locomotion-lqr-longer.learning-descriptor.v1','continuation descriptor version')
    e=load(X/'experiment.json');original=load(F/'experiment.json');ref=load(F/'reference.json');seal=load(F/'input-freeze.json')
    need(descriptor['experiment']==identity(X/'experiment.json'),'descriptor continuation experiment')
    need(descriptor['original_experiment']==identity(F/'experiment.json') and descriptor['independent_reference']==identity(F/'reference.json'),'descriptor carried math identities')
    need(descriptor['live_auditor']==identity(SOURCE),'WHOLE continuation auditor sealed before effects')
    need(ref['passed'] is True and ref['producer']==seal['producer'] and ref['experiment']==seal['experiment'],'carried independent math proof')
    need(identity(ROOT/'psionic/tests/robotics/locomotion_lqr_reference.py')==seal['producer'],'carried whole c105 math kernel')
    for name,h in e['metadata_inputs'].items():need(identity(ROOT/name)==h,'sealed carried metadata '+name)
    qualification=load(args.math_record);need(qualification['passed'] is True and not qualification['plan_only'],'original FIRST passed math contexts')
    need(descriptor['math_qualification']==identity(args.math_record),'descriptor original math receipt')
    need(qualification['experiment']==identity(F/'experiment.json'),'original math context')
    extension=load(args.extension_record);need(extension['passed'] is True and not extension.get('plan_only',False),'passed extension guard/native binding')
    need(descriptor['extension_qualification']==identity(args.extension_record),'descriptor extension qualification')
    need(descriptor['native_binary']==extension['modes']['release']['binary'],'qualified extension release binary lineage')
    inverse=struct.pack('<d',4.)
    need(qualification['modes']['release']['outputs']['inverse-sigma.f64']=={'bytes':8,'sha256':hashlib.sha256(inverse).hexdigest()},'carried realized fixed inverse')
    for name,h in qualification['source'].items():need(identity(ROOT/name)==h,'unchanged original qualified source '+name)
    for name,h in extension['source'].items():need(identity(ROOT/name)==h,'qualified extension source '+name)
    for name,h in original['carried'].items():need(identity(ROOT/name)==h,'unchanged carried helper '+name)
    return e,descriptor,bytes.fromhex(identity(args.descriptor)['sha256']),4.


def close(actual,expected,absolute=2e-12,relative=2e-12,role='ordinary'):
    finite([actual,expected],2,role);error=abs(actual-expected)
    need(error<=absolute+relative*abs(expected),role+' frozen budget');return error


def audit_chunk(args,e,context,inverse,m):
    need(args.seed in (0,1,7) and 1<=args.count<=16 and 128<=args.first_batch<=512-args.count,'declared chunk labels')
    before=snapshot(args.before,context);after=snapshot(args.after,context)
    iterations=decode_iterations(args.iterations.read_bytes(),context,args.count)
    coefficients=decode_coefficients(args.coefficients.read_bytes(),context,args.count)
    admit_state(before,args.seed,args.first_batch);admit_state(after,args.seed,args.first_batch+args.count)
    need(args.previous_audit is not None,'prior PASSED import/chunk audit required')
    admit_previous(load(args.previous_audit),args.seed,args.first_batch,identity(args.descriptor),identity(args.before))
    # EVERY coefficient in the chunk is independently admitted before any VJP
    # or optimizer comparison.  These are recomputed source-bound observations.
    coefficient_reports=[]
    for b,it in enumerate(iterations):
        current=before['K'] if b==0 else iterations[b-1]['next']['K']
        rlp_rows=[(r[0],r[1],r[3],r[11]) for r in it['rows']]
        for epoch in range(4):
            at=(b*4+epoch)*512;probes=coefficients[at:at+512]
            coefficient_reports.append(m.admit_coefficients(current,rlp_rows,probes,inverse))
            current=it['epochs'][epoch][0]
    errors={'sampler_action':0.,'sampler_z':0.,'GAE_advantage':0.,'GAE_return':0.,'loss':0.,'gradient':0.,'candidate':0.}
    total_clipped=projections=0;minimum_state_margin=16.;maximum_action=0.
    state=before
    for b,it in enumerate(iterations):
        index=args.first_batch+b;admit_state(state,args.seed,index)
        K=state['K'];h=it['header'];need(bits(h[3])==bits(K),'rollout-start K word')
        P,C,_=m.critic_policy(K);need(bits(h[4])==bits(P) and bits(h[5])==bits(C),'fixed old-policy critic words')
        rows=it['rows'];steps=[]
        for w in range(16):
            x,seed,counter=state['worlds'][w]
            for t in range(32):
                i=w*32+t;r=rows[i];need(bits(r[0])==bits(x),'signed x lineage')
                need(abs(r[0])<=16 and abs(r[1])<=18.25 and abs(r[4])<=9,'actual state/action/z domains')
                mean=-m.mul(K,x);u=m.gaussian().splitmix_top53(seed,counter);v=m.gaussian().splitmix_top53(seed,counter+1)
                expected_action,expected_z=m.gaussian().uniform_reference(mean,m.LOG_STD,u,v)
                errors['sampler_action']=max(errors['sampler_action'],close(r[1],expected_action,8e-14,8e-15,'new-context sampler action'))
                errors['sampler_z']=max(errors['sampler_z'],close(r[4],expected_z,8e-14,8e-15,'new-context sampler z'))
                if u==0:need(bits(r[1])==bits(mean) and bits(r[4])==bytes(8),'explicit radial-zero copy')
                next_x=m.add(x,r[1]);reward=-m.add(m.mul(x,x),m.mul(r[1],r[1]));V=m.critic_policy(K,x)[2];nextV=m.critic_policy(K,next_x)[2]
                need(bits(r[2])==bits(next_x) and abs(next_x)<=16,'exact source plant transition')
                need(all(bits(a)==bits(c) for a,c in zip(r[5:8],(reward,V,nextV))),'exact reward/fixed critic words')
                need(r[8:10]==(0,0) and r[13:15]==(1,int(t!=31)),'continuing-task/local-tail masks')
                need(abs(reward)<=1e6 and abs(V)<=1e6 and abs(nextV)<=1e6,'GAE input caps')
                delta=m.sub(m.add(reward,m.mul(m.GAMMA,nextV)),V);need(bits(r[10])==bits(delta),'exact local residual')
                # Epoch0 queries the unchanged rollout-start actor: this binds
                # every persisted old logp without calling the product again.
                need(bits(r[3])==bits(coefficients[b*2048+i][0]),'old-logp to epoch0 density bytes')
                minimum_state_margin=min(minimum_state_margin,16-abs(next_x));maximum_action=max(maximum_action,abs(r[1]))
                x=next_x;counter+=2;steps.append(r)
            need(bits(it['worlds'][w][0])==bits(x) and it['worlds'][w][1:]==(seed,counter),'next world and RNG lineage')
        expected_targets=m.target_rows(steps,32);q=m.mul(m.GAMMA,m.LAMBDA)
        for i,(r,target) in enumerate(zip(rows,expected_targets)):
            L=32-i%32;terms=[Fraction(rows[j][10])*Fraction(q)**(j-i) for j in range(i,(i//32+1)*32)]
            exactA=sum(terms,Fraction());mag=sum(map(abs,terms),Fraction());n=2*L+4;gn=Fraction(n)*m.U/(1-Fraction(n)*m.U)
            allowance=Fraction('2e-12')+gn*mag;return_allowance=allowance+m.U*(abs(exactA)+abs(Fraction(r[6]))+allowance)+Fraction(float.fromhex('0x0.0000000000001p-1022'))
            errors['GAE_advantage']=max(errors['GAE_advantage'],close(r[11],target[1],math.nextafter(float(allowance),math.inf),0.,'explicit future-sum GAE'))
            errors['GAE_return']=max(errors['GAE_return'],close(r[12],target[2],math.nextafter(float(return_allowance),math.inf),0.,'explicit future-sum return'))
            if L==1:need(bits(r[11])==bits(r[10]) and bits(r[12])==bits(m.add(r[11],r[6])),'tail exact target words')
        optimizer=(state['K'],state['m'],state['v'],state['step'],state['skipped'])
        rlp_rows=[(r[0],r[1],r[3],r[11]) for r in rows]
        for epoch,a in enumerate(it['epochs']):
            current=optimizer[0];need(max(.25,m.sub(K,m.TRUST))<=current<=min(1.,m.add(K,m.TRUST)),'epoch anchored trust interval')
            probes=coefficients[(b*4+epoch)*512:(b*4+epoch+1)*512];losses=[];gradients=[];mags=[0.,0.];clip=0
            for j in range(0,512,64):
                values,conditioners,clipped=m.conditional_block(current,rlp_rows[j:j+64],probes[j:j+64],inverse)
                losses.append(values[0]);gradients.append(values[1]);mags=[x+y for x,y in zip(mags,conditioners[:2])];clip+=clipped
            loss=math.fsum(losses)/512;gradient=math.fsum(gradients)/512
            errors['loss']=max(errors['loss'],close(a[5],loss,8e-12+4e-13*mags[0]/512,0.,'live full-batch mean loss'))
            errors['gradient']=max(errors['gradient'],close(a[6],gradient,8e-12+4e-13*mags[1]/512,0.,'live analytic mean gradient'))
            candidate,m2,v2,step,skip=m.adam_policy(optimizer,a[6])
            need(bits(a[1])==bits(m2) and bits(a[2])==bits(v2) and a[3:5]==(step,skip),'exact conditional moments/Adam state')
            errors['candidate']=max(errors['candidate'],close(a[7],candidate,role='live independently bounded candidate'))
            projected=m.project(K,a[7]);need(bits(a[0])==bits(projected),'exact observed-candidate projection')
            need(a[8:]==(int(projected!=a[7]),512,8,clip),'epoch counts/projection/clipped flags')
            optimizer=a[:5];total_clipped+=clip;projections+=a[8]
        expected_state=struct.pack('<3d2q',*optimizer)+b''.join(struct.pack('<d2Q',*w) for w in it['worlds'])+struct.pack('<qQ',index+1,args.seed)
        need(it['next']['raw']==expected_state,'one atomic next state from epochs/worlds')
        admit_state(it['next'],args.seed,index+1);state=it['next']
    need(after['raw']==state['raw'],'checkpoint equals completed chunk endpoint')
    need(args.progress is not None,'chunk progress ledger required')
    ledger=[json.loads(x) for x in args.progress.read_text().splitlines()]
    expected=[event for b in range(args.first_batch,args.first_batch+args.count) for event in [{'event':'before','batch':b},{'event':'committed','batch':b+1}]]
    need(ledger==expected,'exact complete progress ledger')
    return {'passed':True,'seed':args.seed,'first_batch':args.first_batch,'batches':args.count,'plant_steps':args.count*512,
            'AdamW_updates':args.count*4,'coefficient_observations':args.count*2048,'maximum_absolute_errors':errors,
            'ppo_clipped_samples':total_clipped,'projected_updates':projections,
            'minimum_state_box_margin':minimum_state_margin,'peak_action_magnitude':maximum_action,
            'coefficient_density_max':[max(x['density_absolute'][j] for x in coefficient_reports) for j in range(4)],
            'coefficient_rho_relative_max':max(x['rho_relative'] for x in coefficient_reports),
            'counter_status_masks_projection_lineage_exact':True,'all_coefficients_admitted_before_VJP':True,
            'scope':'New captured canonical or declared replay chunk only; no product/old-gate/optimizer/environment execution by auditor.'}


def audit_evaluation(args,context,m):
    state=snapshot(args.after,context);admit_state(state,args.seed)
    need(not args.baseline,'original baseline evaluation is carried, never repeated')
    need(state['batch']==512 and state['step']==2048,'final continuation actor identity')
    need(args.previous_audit is not None,'final actor prior audit required')
    admit_previous(load(args.previous_audit),args.seed,512,identity(args.descriptor),identity(args.after))
    raw=header(args.evaluations.read_bytes(),0x3145584c,72,6,context,'LXE1');need(len(raw)==432,'six deterministic evaluation rows')
    rows=list(struct.iter_unpack('<q7dq',raw));finite([x for r in rows for x in r[1:8]],42,'ALL evaluation fields')
    qraw=header(args.quality.read_bytes(),0x3147584c,200,1,context,'LXG1');need(len(qraw)==200,'quality extent');q=struct.unpack('<23d2q',qraw);finite(list(q[:23]),23,'ALL quality fields')
    maximum=[0.]*7
    for i,r in enumerate(rows):
        need(r[0]==i and r[-1]==128 and bits(r[1])==bits(state['K']),'evaluation labels/count/K')
        expected=m.eval_reference(state['K'],i,128);finite(list(expected[:7]),7,'ALL direct-power expected fields')
        for j in range(7):maximum[j]=max(maximum[j],close(r[j+1],expected[j],role='direct-power deterministic evaluation'))
    realP,_=m.real_benchmark();close(q[1],float(realP),role='independent real optimum')
    expected=m.quality_policy(state['K'],q[1]);need(struct.pack('<23d2q',*q)==struct.pack('<23d2q',*expected),'exact conditional source quality policy')
    real=m.quality_real(state['K']);gain_margin=float(real['gain_margin']);cost_margin=float(real['cost_margin'])
    real_gaps=list(map(float,real['relative_gaps']));finite(real_gaps,6,'ALL real cost-gap diagnostics')
    finite([gain_margin,cost_margin],2,'real goal margins')
    # Fixed conservative diagnostic uncertainty, inherited ordinary2e-12+2e-12
    # field scale; ambiguity is explicit and cannot silently promote a seed.
    gain_uncertainty=2e-12+2e-12*float(real['gain_error']);cost_uncertainty=2e-12+2e-12*max(real_gaps)
    real_supported=gain_margin>gain_uncertainty and cost_margin>cost_uncertainty
    return {'passed':True,'seed':args.seed,'baseline':args.baseline,'K':state['K'],'steps':768,'evaluation_max_absolute':maximum,
            'source_gain_ok':q[-2],'source_gap_ok':q[-1],'real':real,'real_gain_uncertainty':gain_uncertainty,'real_cost_uncertainty':cost_uncertainty,
            'real_goals_supported':real_supported,'promoted':bool(not args.baseline and q[-2] and q[-1] and real_supported),
            'scope':'Executed deterministic mean-policy checks and separate nominal continuous-Gaussian analytic score; not stochastic held-out episodes.'}


def audit_replay(args,context):
    need(args.first_batch==256 and args.count==1,'predeclared replay batch256')
    canonical=load(args.canonical_audit)
    need(canonical['passed'] is True and canonical['auditor']==identity(SOURCE),'previously independently audited canonical target')
    need(canonical['result']['seed']==args.seed and canonical['result']['first_batch']==256,'canonical chunk begins at256')
    need(canonical['inputs']['descriptor']==identity(args.descriptor),'canonical replay same context')
    need(canonical['inputs']['before']==identity(args.before),'replay starts from identical canonical checkpoint')
    need(canonical['inputs']['iterations']==identity(args.canonical_iterations) and canonical['inputs']['coefficients']==identity(args.canonical_coefficients),'canonical immutable target artifacts')
    before=snapshot(args.before,context);after=snapshot(args.after,context);admit_state(before,args.seed,256);admit_state(after,args.seed,257)
    replay=decode_iterations(args.iterations.read_bytes(),context,1);decode_coefficients(args.coefficients.read_bytes(),context,1)
    old_iterations=header(args.canonical_iterations.read_bytes(),0x3149584c,62696,16,context,'canonical LXI1')
    old_coefficients=header(args.canonical_coefficients.read_bytes(),0x3143584c,88,32768,context,'canonical LXC1')
    need(replay[0]['raw']==old_iterations[:ITER_BYTES],'checkpoint replay iteration EXACT')
    new_coefficients=header(args.coefficients.read_bytes(),0x3143584c,88,32768,context,'replay LXC1')
    need(new_coefficients==old_coefficients[:2048*88],'checkpoint replay coefficient observations EXACT')
    need(after['raw']==replay[0]['next']['raw'],'replay checkpoint endpoint')
    need(args.progress is not None,'replay ledger required')
    ledger=[json.loads(x) for x in args.progress.read_text().splitlines()]
    need(ledger==[{'event':'before','batch':256},{'event':'committed','batch':257}],'replay ledger')
    return {'passed':True,'seed':args.seed,'plant_steps':512,'AdamW_updates':4,'coefficient_observations':2048,
            'iteration_and_coefficients_byte_exact':True,'prior_numeric_proof_carried':identity(args.canonical_audit),
            'scope':'NEW declared resume replay versus already audited canonical batch257; no repeated independent numeric generation.'}


def continuation_guards(output):
    """NEW metadata/import/counter/chain guards only; old readers are carried."""
    output.mkdir(parents=True,exist_ok=False);positives=poisons=0
    def reject(fn):
        nonlocal poisons
        try:fn()
        except (ValueError,KeyError):poisons+=1
        else:raise ValueError('new continuation guard poison admitted')
    e=load(X/'experiment.json')
    for seed in (0,1,7):
        original=(ROOT/e['resume_sources'][str(seed)]['snapshot_path']).read_bytes()[48:]
        imported=bytearray(original)
        for w in range(16):struct.pack_into('<Q',imported,40+w*24+16,131072+w*24576)
        admit_import_words(original,bytes(imported),seed);positives+=1
        for offset in [0,8,16,40,40+15*24,24,32,424,432,40+8,40+15*24+8]:
            bad=bytearray(imported);bad[offset]^=1
            reject(lambda b=bytes(bad):admit_import_words(original,b,seed))
        for w in range(16):
            bad=bytearray(imported);at=40+w*24+16;c=struct.unpack_from('<Q',bad,at)[0];struct.pack_into('<Q',bad,at,c+1)
            reject(lambda b=bytes(bad):admit_import_words(original,b,seed))
        for batch in (128,129,255,256,511,512):
            state=bytearray(imported);struct.pack_into('<q',state,24,4*batch);struct.pack_into('<q',state,424,batch)
            for w in range(16):struct.pack_into('<Q',state,40+w*24+16,131072+w*24576+(batch-128)*64)
            admit_state(decode_state(state),seed,batch);positives+=1
        for batch in (127,513):
            state=bytearray(imported);struct.pack_into('<q',state,24,4*batch);struct.pack_into('<q',state,424,batch)
            reject(lambda b=bytes(state):admit_state(decode_state(b),seed))
        state=bytearray(imported)
        for w in range(16):struct.pack_into('<Q',state,40+w*24+16,w*8192+128*64)
        reject(lambda b=bytes(state):admit_state(decode_state(b),seed,128))
    desc={'bytes':1,'sha256':'11'*32};endpoint={'bytes':488,'sha256':'22'*32}
    for first,count in ((128,0),(128,16),(496,16)):
        previous={'schema':'psionic.locomotion-lqr-longer.learning-audit.v1','passed':True,'auditor':identity(SOURCE),'inputs':{'descriptor':desc,'after':endpoint},'result':{'seed':0,'first_batch':first,'batches':count}}
        admit_previous(previous,0,first+count,desc,endpoint);positives+=1
    for path,value in [(('schema',),'psionic.locomotion-lqr.learning-audit.v1'),(('passed',),False),(('auditor',),e['original_auditor']),(('inputs','descriptor'),{'bytes':1,'sha256':'33'*32}),(('inputs','after'),{'bytes':488,'sha256':'33'*32}),(('result','seed'),1),(('result','batches'),15)]:
        bad=json.loads(json.dumps(previous));target=bad
        for key in path[:-1]:target=target[key]
        target[path[-1]]=value;reject(lambda p=bad:admit_previous(p,0,512,desc,endpoint))
    intervals=[(131072+w*24576,131072+(w+1)*24576) for w in range(16)]
    need(all(a>=131072 and b-a==24576 for a,b in intervals) and all(intervals[w][1]==intervals[w+1][0] for w in range(15)) and intervals[-1][1]==524288,'exact disjoint slot reservation');positives+=1
    record={'schema':'psionic.locomotion-lqr-longer.audit-guards.v1','passed':True,'producer':identity(SOURCE),'experiment':identity(X/'experiment.json'),'new_positive_cases':positives,'new_poisons_refused':poisons,'old_reader_numeric_or_product_calls':0}
    write(output/'record.json',record);print(json.dumps(record))


def main():
    p=argparse.ArgumentParser();p.add_argument('--continuation-guards',action='store_true');p.add_argument('--plan',action='store_true');p.add_argument('--import-state',action='store_true')
    p.add_argument('--descriptor',type=Path);p.add_argument('--math-record',type=Path);p.add_argument('--extension-record',type=Path)
    p.add_argument('--before',type=Path);p.add_argument('--after',type=Path);p.add_argument('--iterations',type=Path);p.add_argument('--coefficients',type=Path);p.add_argument('--progress',type=Path)
    p.add_argument('--seed',type=int);p.add_argument('--first-batch',type=int);p.add_argument('--count',type=int);p.add_argument('--evaluations',type=Path);p.add_argument('--quality',type=Path);p.add_argument('--baseline',action='store_true');p.add_argument('--output',required=True,type=Path)
    p.add_argument('--canonical-audit',type=Path);p.add_argument('--canonical-iterations',type=Path);p.add_argument('--canonical-coefficients',type=Path);p.add_argument('--previous-audit',type=Path)
    args=p.parse_args()
    if args.continuation_guards:continuation_guards(args.output);return
    need(not args.output.exists(),'preserve FIRST continuation audit context');args.output.mkdir(parents=True);began=time.monotonic()
    r={'schema':'psionic.locomotion-lqr-longer.learning-audit.v1','passed':False,'auditor':identity(SOURCE),'product_learner_model_SDK_calls':0}
    write(args.output/'record.json',r)
    try:
        e,descriptor,context,inverse=bindings(args)
        if args.plan:result={'passed':True,'plan_only':True,'profile_binding_only':True}
        elif args.import_state:result=audit_import(args,context)
        elif args.canonical_audit:result=audit_replay(args,context)
        else:
            m=math_kernel()
            result=audit_evaluation(args,context,m) if args.evaluations else audit_chunk(args,e,context,inverse,m)
        paths={k:identity(v) for k,v in vars(args).items() if isinstance(v,Path) and k!='output'}
        r.update(passed=True,inputs=paths,result=result)
    except BaseException as error:r['error']=type(error).__name__+': '+str(error);raise
    finally:r['seconds']=time.monotonic()-began;write(args.output/'record.json',r)
    print(json.dumps({'passed':True,'record':identity(args.output/'record.json')}))


if __name__=='__main__':main()
