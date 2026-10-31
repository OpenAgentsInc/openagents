#!/usr/bin/env python3
"""Independent augmented matrix polynomial and explicitly rounded F32 oracle.

No Coax/model imports. Formal derivatives concern q,v,u, with fixed context;
they are not derivatives of literal float rounding or of physical refusals.
"""
import argparse,hashlib,json,math,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
r32=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
def mm(a,b):return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def mv(a,v):return [sum(row[k]*v[k] for k in range(3)) for row in a]
def phi(a,half,h,sixth):
    # This polynomial is derived algebraically, not an RK4 program unroll.
    result=[[float(i==j) for j in range(3)] for i in range(3)];power=a
    for coefficient in [6*sixth,sixth*(4*half+h),sixth*(2*half*half+h*half),sixth*h*half*half]:
        for i in range(3):
            for j in range(3):result[i][j]+=coefficient*power[i][j]
        power=mm(power,a)
    return result

def rounded_step(a,z,half,h,sixth):
    def product(v):
        out=[]
        for row in a:
            total=r32(0)
            for x,y in zip(row,v):total=r32(total+r32(x*y))
            out.append(total)
        return out
    def shift(k,scale):return [r32(x+r32(scale*y)) for x,y in zip(z,k)]
    k1=product(z);k2=product(shift(k1,half));k3=product(shift(k2,half));k4=product(shift(k3,h))
    return [r32(x+r32(sixth*r32(r32(r32(k+ r32(2*l))+r32(2*m))+n))) for x,k,l,m,n in zip(z,k1,k2,k3,k4)]

def objective(m,error,weights):
    g=[2*sum(m[k][i]*weights[k]*error[k] for k in range(3)) for i in range(3)]
    h=[[2*sum(m[k][i]*weights[k]*m[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
    return g,h

def main():
    parser=argparse.ArgumentParser();parser.add_argument('transcript');parser.add_argument('--output');args=parser.parse_args()
    experiment=ROOT/'psionic/fixtures/physics/oscillator-functions/experiment.json';cfg=json.loads(experiment.read_text());data=Path(args.transcript).read_bytes();records=[json.loads(line) for line in data.splitlines()];points=[r for r in records if r['kind']=='point'];batches=[r for r in records if r['kind']=='batch'];assert len(records)==63 and len(points)==45 and len(batches)==18
    assert [p['id'] for p in points]==list(range(45));assert [(b['group'],b['mode']) for b in batches]==[(g,m) for g in range(9) for m in (3,4)]
    maximum={'formal':0.,'physical_primal':0.,'physical_jacobian':0.,'physical_cost':0.,'physical_solver':0.,'physical_force_fd':0.};counts={'formal':0,'f32_primal_words':0,'batch_exact':0,'force_fd':0}
    def close(name,actual,expected,budget):
        assert len(actual)==len(expected)
        for a,b in zip(actual,expected):
            assert math.isfinite(a) and math.isfinite(b);error=abs(a-b);maximum[name]=max(maximum[name],error);assert error<=budget['atol']+budget['rtol']*abs(b),(name,a,b,error);counts['formal']+=name=='formal'
    flat=lambda x:[v for row in x for v in row]
    for index,p in enumerate(points):
        pair=cfg['spring_damping_pairs'][index//15];dt=cfg['dt'][(index//5)%3];z=cfg['position_velocity_force'][index%5];assert p['group']==index//5 and [p['spring'],p['damping']]==pair and p['dt']==dt and p['initial']==z
        k,c=pair;h=r32(dt);x=[*map(r32,z),0.,1.,0.,-r32(k),-r32(c),1.,0.,0.,0.,r32(h/2),h,r32(h/6),2.,-r32(cfg['target_position_velocity'][0]),-r32(cfg['target_position_velocity'][1]),0.,*map(r32,cfg['cost_weights'])];assert p['packed']==x
        a=[x[3:6],x[6:9],x[9:12]];m=phi(a,x[12],x[13],x[14]);primal=rounded_step(a,x[:3],x[12],x[13],x[14]);error=[r32(v+t) for v,t in zip(primal,x[16:19])];weights=x[19:22];cost=r32(0)
        for e,w in zip(error,weights):cost=r32(cost+r32(r32(e*e)*w))
        assert p['step']==primal and p['cost']==[cost],('f32 primal',index,p['step'],primal,p['cost'],cost);counts['f32_primal_words']+=4
        gradient,hessian=objective(m,error,weights);budget=cfg['budgets']['formal_partials_and_products'];direction=cfg['direction_position_velocity_force'];cot=cfg['step_cotangent']
        for key,expected in [('jacobian',flat(m)),('gradient',gradient),('hessian',flat(hessian)),('jvp',mv(m,direction)),('vjp',[sum(m[j][i]*cot[j] for j in range(3)) for i in range(3)]),('hvp_gradient',gradient),('hvp',mv(hessian,direction))]:close('formal',p[key],expected,budget)
        physical_a=[[0.,1.,0.],[-k,-c,1.],[0.,0.,0.]];physical_m=phi(physical_a,dt/2,dt,dt/6);physical_y=mv(physical_m,z);close('physical_solver',p['physical'],physical_y[:2],cfg['budgets']['physical_f64_solver_vs_matrix_polynomial']);close('physical_primal',p['step'][:2],p['physical'],cfg['budgets']['physical_f64_primal']);close('physical_jacobian',p['jacobian'],flat(physical_m),cfg['budgets']['physical_f64_state_action_jacobian'])
        target=cfg['target_position_velocity']+[0.];physical_gradient,physical_hessian=objective(physical_m,[v-t for v,t in zip(physical_y,target)],cfg['cost_weights']);close('physical_cost',p['gradient']+p['hessian'],physical_gradient+flat(physical_hessian),cfg['budgets']['physical_f64_cost_gradient_hessian'])
        for e in range(2):close('physical_force_fd',p['physical_force_fd'][2*e:2*e+2],[physical_m[0][2],physical_m[1][2]],cfg['budgets']['physical_f64_state_action_jacobian']);counts['force_fd']+=2
    for batch in batches:
        key='gradient' if batch['mode']==3 else 'hessian';expected=sum([p[key] for p in points[batch['group']*5:batch['group']*5+5]],[]);assert batch['values']==expected;counts['batch_exact']+=len(expected)
    receipt={'schema':'psionic-oscillator-functions-reference-v1','profile':cfg['profile'],'experiment_sha256':hashlib.sha256(experiment.read_bytes()).hexdigest(),'oracle_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'transcript_sha256':hashlib.sha256(data).hexdigest(),'points':45,'batches':18,'counts':counts,'max_absolute':maximum,'budgets':cfg['budgets'],'passed':True,'scope':cfg['limits']}
    print(json.dumps(receipt,indent=2));
    if args.output:Path(args.output).write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
