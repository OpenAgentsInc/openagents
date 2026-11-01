#!/usr/bin/env python3
"""NEW guard for the frozen named SO3 grid, using cached expected data only.

Independent Rodrigues(theta,axis) certifies intended world-axis/angle coverage.
No expected generation, Gaussian, Coax/product/SDK or old gate is executed.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
from panda_pose_ik_reference import quat_matrix
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-ik'
EXPERIMENT_SHA='4ff40ce11ebcbfa7be54f3a02aded7d35ac3b56101541717dd365b40d7c47d88'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(v)for v in values),label+' ALL finite and complete');return list(values)
def flat(r):return [x for row in r for x in row]
def rodrigues(axis,theta):
    n=math.hypot(*axis);a=[x/n for x in axis];c=math.cos(theta);s=math.sin(theta);skew=[[0.,-a[2],a[1]],[a[2],0.,-a[0]],[-a[1],a[0],0.]]
    return [c*float(i==j)+(1-c)*a[i]*a[j]+s*skew[i][j]for i in range(3)for j in range(3)]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--reference',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();out=args.output.resolve();require(not out.exists(),'preserve earlier guard');out.mkdir(parents=True);record=dict(schema='psionic.panda-pose-ik.rotation-grid-guard.v1',passed=False,SDK_calls=0,Coax_calls=0,expected_regenerated=False);began=time.monotonic()
    try:
        require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen named grid');e=json.loads((FIXTURE/'experiment.json').read_text());reference=args.reference.resolve();receipt=json.loads((reference/'reference.json').read_text());require(receipt['passed'] and receipt['experiment_sha256']==EXPERIMENT_SHA,'original independent reference');require(sha(ROOT/'psionic/tests/robotics/panda_pose_ik_reference.py')==receipt['source_sha256']['psionic/tests/robotics/panda_pose_ik_reference.py'],'exact executed pure definitions');raw=(FIXTURE/'rotation-cases.f64').read_bytes();require(sha(FIXTURE/'rotation-cases.f64')==e['rotation_cases_sha256'] and len(raw)==4200*8*8,'frozen4200 input bytes');inputs=finite(struct.unpack('<33600d',raw),33600,'all SO3 input coefficients');expected_path=reference/'rotation-expected.f64';require(sha(expected_path)==receipt['outputs']['rotation-expected.f64']['sha256'] and expected_path.stat().st_size==4200*13*8,'unchanged expected data');expected=finite(struct.unpack('<54600d',expected_path.read_bytes()),54600,'all cached expected fields');flags=(FIXTURE/'rotation-flags.u8').read_bytes();require(len(flags)==4200 and sha(FIXTURE/'rotation-flags.u8')==e['rotation_flags_sha256'],'original cut mask')
        g=e['rotation_grid'];require(len(g['axes'])==28 and len(g['angles_radians'])==25 and len(g['current_quaternions'])==3,'named grid extents');errors=dict(current=[],relative=[],angle=[],ordinary_vector=[]);pair_checks=0
        for index in range(4200):
            current=index//1400;axis=(index//50)%28;angle=(index//2)%25;theta=g['angles_radians'][angle];row=inputs[index*8:(index+1)*8];target=expected[index*13:(index+1)*13];desired_current=flat(quat_matrix(g['current_quaternions'][current]));actual_current=flat(quat_matrix(row[:4]));errors['current']+= [abs(x-y)for x,y in zip(actual_current,desired_current)];require((row[0]<0)==bool((axis+angle)%2),'current quaternion sign alternation');wanted=rodrigues(g['axes'][axis],theta);errors['relative']+=[abs(x-y)for x,y in zip(target[4:],wanted)];principal=math.remainder(theta,2*math.pi);errors['angle'].append(abs(target[3]-abs(principal)))
            if not flags[index]&1:
                length=math.hypot(*g['axes'][axis]);errors['ordinary_vector'] += [abs(target[j]-principal*g['axes'][axis][j]/length)for j in range(3)]
            if index%2==0:
                plus=struct.unpack_from('<8Q',raw,index*64);minus=struct.unpack_from('<8Q',raw,(index+1)*64);require(plus[:4]==minus[:4] and all((x^y)==1<<63 for x,y in zip(plus[4:],minus[4:])),'all2100 exact target q/−q input bit pairs');pair_checks+=1
        limits={'current':2e-12,'relative':e['budgets']['physical_rotation_matrix_max_entry_atol'],'angle':e['budgets']['angular_magnitude_radians_atol'],'ordinary_vector':e['budgets']['angular_vector_radians_atol']};metrics={}
        for name,values in errors.items():
            finite(values,len(values),name+' differences');require(values and max(values)<=limits[name],name+' frozen grid acceptance');metrics[name]=dict(values=len(values),max_abs_error=max(values),atol=limits[name],rtol=0)
        record.update(passed=True,cases=4200,exact_target_sign_pairs=pair_checks,metrics=metrics,reference_sha256=sha(reference/'reference.json'),expected_rotation_sha256=sha(expected_path),source_sha256={str(p.relative_to(ROOT)):sha(p)for p in [Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_pose_ik_reference.py',FIXTURE/'experiment.json',FIXTURE/'rotation-cases.f64',FIXTURE/'rotation-flags.u8']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),metrics=metrics)))
if __name__=='__main__':main()
