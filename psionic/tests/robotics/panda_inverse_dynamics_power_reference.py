#!/usr/bin/env python3
"""NEW cache-only physical power identity; no native/SDK/coordinate-FD rerun."""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
import panda_inverse_dynamics_reference as reference
import panda_inverse_dynamics_matrix as matrix
# Freeze the NEW directional time difference before its first comparison.
H_TIME_SECONDS=1e-5
POWER_ATOL_WATTS=2e-6
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();reference.require(not out.exists(),'preserve power proof');out.mkdir(parents=True);prior=json.loads((a.native/'record.json').read_text());reference.require(prior['passed'] and len(prior['modes'])==3 and prior['native_modes_byte_identical'],'frozen native proof');experiment,cases,nodes,model,_,dependencies=reference.load_reference();reference.require(experiment['power_atol_W']==POWER_ATOL_WATTS and prior['dependencies']==dependencies and prior['matrix_oracle_sha256']==sha(Path(matrix.__file__)),'frozen source/experiment/math');
    for name,h in prior['source_sha256'].items():reference.require(sha(reference.ROOT/name)==h,'executed source unchanged '+name)
    record={'schema':'psionic.panda-inverse-dynamics.power-reference.v1','passed':False,'h_time_seconds':H_TIME_SECONDS,'power_atol_W':POWER_ATOL_WATTS,'rtol':0,'case_count':24,'frozen_native_record_sha256':sha(a.native/'record.json'),'source_sha256':{str(Path(__file__).relative_to(reference.ROOT)):sha(Path(__file__)),str(Path(matrix.__file__).relative_to(reference.ROOT)):sha(Path(matrix.__file__))},'new_reference_work':'72 independent matrix-metric evaluations:base/plus/minus for24cachedcases. No old coordinate-FD table regeneration,nativeexecution,SDKorNN.'}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    began=time.monotonic();save()
    try:
        path=a.native/'debug/forces.f64';reference.require(sha(path)==prior['modes'][0]['output_sha256'],'cached actualforce identity');actual=reference.finite_rows(reference.words(path),24,9,'power actual cachedforces');comparisons=[]
        for case,tau in zip(cases,actual):
            q,v,acc=case[:9],case[9:18],case[18:];m,g=matrix.metric_gravity(nodes,model,q);mp,_=matrix.metric_gravity(nodes,model,[x+H_TIME_SECONDS*y for x,y in zip(q,v)]);mm,_=matrix.metric_gravity(nodes,model,[x-H_TIME_SECONDS*y for x,y in zip(q,v)]);mdot=[(x-y)/(2*H_TIME_SECONDS) for x,y in zip(mp,mm)];reference.require(len(m)==len(mdot)==81 and len(g)==9 and all(math.isfinite(x) for x in m+g+mdot),'all power reference coefficients finite')
            c=[tau[i]-math.fsum(m[i*9+j]*acc[j] for j in range(9))-g[i] for i in range(9)];lhs=math.fsum(v[i]*c[i] for i in range(9));rhs=.5*math.fsum(v[i]*mdot[i*9+j]*v[j] for i in range(9) for j in range(9));reference.require(math.isfinite(lhs) and math.isfinite(rhs),'finite power beforemax');comparisons.append({'velocity_bias_power_W':lhs,'kinetic_metric_change_power_W':rhs,'abs_error_W':abs(lhs-rhs)})
        reference.require(len(comparisons)==24,'complete power case coverage');worst=max(r['abs_error_W'] for r in comparisons);record['comparisons']=comparisons;record['max_abs_error_W']=worst;record['max_budget_fraction']=worst/POWER_ATOL_WATTS;reference.require(worst<=POWER_ATOL_WATTS,'frozen power bound failed');record['passed']=True
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'case_count':24,'max_abs_error_W':record['max_abs_error_W'],'record':str(out/'record.json')}))
if __name__=='__main__':main()
