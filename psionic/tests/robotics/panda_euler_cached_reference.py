"""NEW algebraic Euler targets from retained SDK operands; no SDK/product call."""
from pathlib import Path
import argparse, hashlib, json, math, struct
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-euler'
def require(value,message):
    if not value:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite_words(path,count):
    raw=Path(path).read_bytes();require(len(raw)==count*8,'complete F64 byte extent')
    result=struct.unpack('<'+str(count)+'d',raw);require(all(math.isfinite(v)for v in result),'all operands finite');return result
def finite_target(values,count):
    require(len(values)==count and all(math.isfinite(v)for v in values),'complete finite target before residual/output');return list(values)
def gaussian(matrix,rhs):
    rows=[list(matrix[9*i:9*i+9])+[rhs[i]]for i in range(9)]
    for col in range(9):
        pivot=max(range(col,9),key=lambda row:abs(rows[row][col]));require(abs(rows[pivot][col])>1e-12,'independent nonsingular pivot');rows[col],rows[pivot]=rows[pivot],rows[col]
        divisor=rows[col][col]
        for j in range(col,10):rows[col][j]/=divisor
        for row in range(9):
            if row==col:continue
            factor=rows[row][col]
            for j in range(col,10):rows[row][j]-=factor*rows[col][j]
    return [row[9]for row in rows]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();out=args.output.resolve();require(not out.exists(),'preserve earlier record');out.mkdir(parents=True)
    exp=FIXTURE/'experiment.json';require(sha(exp)=='9f4eb555739fdc6ac2d61f02d68a1d432599078ce77eb792eba2ee68b1572262','frozen experiment');profile=json.loads(exp.read_text());cases=profile['cases'];require(len(cases)==6 and profile['step_seconds']==0.002,'frozen six-row profile')
    pid=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics';pa=ROOT/'psionic/fixtures/robotics/panda-actuation';old_cases=finite_words(pid/'cases.f64',24*27);require(all(x==0 for x in old_cases[:18]),'PID row0 exact zero q/v');mass_bias=finite_words(pid/'sdk-mass-bias.f64',15*90);mass=list(mass_bias[:81]);bias=list(mass_bias[81:90]);observations=finite_words(pa/'sdk-values.f64',18*162);pa_cases=json.loads((pa/'experiment.json').read_text())['cases'];damping=profile['damping'];require(damping==[.1,.1,.1,.1,.1,.01,.01,100,100],'source joint damping')
    h=profile['step_seconds'];modified=mass[:]
    for i in range(9):modified[9*i+i]+=h*damping[i]
    targets=[];details=[]
    for index in range(3):
        case=cases[index];original=pa_cases[12+index];require(case['q']==original['q'] and case['v']==original['v'] and case['ctrl']==original['ctrl'] and all(x==0 for x in case['q']+case['v']+case['external']),'exact cached operand match');row=observations[(12+index)*162:(13+index)*162];force=[row[9+i]+row[54+i]-bias[i]for i in range(9)];acceleration=gaussian(modified,force);velocity=[case['v'][i]+h*acceleration[i]for i in range(9)];position=[case['q'][i]+h*velocity[i]for i in range(9)];admitted=finite_target(position+velocity+acceleration,27);residuals=finite_target([sum(modified[9*i+j]*acceleration[j]for j in range(9))-force[i]for i in range(9)],9);residual=max(abs(v)for v in residuals);require(residual<1e-10,'independent Gaussian residual');targets.extend(admitted);details.append({'case':index,'source_Pa_row':12+index,'source_PID_mass_bias_row':0,'q':position,'v':velocity,'effective_acceleration':acceleration,'normalized_force_residual':residual})
    raw=struct.pack('<81d',*targets);(out/'targets.f64').write_bytes(raw);record={'schema':'psionic.panda-euler.cached-algebraic-reference.v1','passed':True,'reference_is_observed_SDK_step':False,'source':'Retained SDK M/bias row0 and Pa actuator/passive rows12/13/14; new independent Gaussian(M+hB) algebra, no Coax output or SDK calls.','cases':details,'matrix':mass,'modified_matrix':modified,'bias':bias,'targets_sha256':sha(out/'targets.f64'),'dependencies':{str(p.relative_to(ROOT)):sha(p)for p in [exp,FIXTURE/'cases.f64',pid/'cases.f64',pid/'sdk-mass-bias.f64',pa/'experiment.json',pa/'sdk-values.f64',Path(__file__)]},'SDK_calls':0,'Coax_calls':0};(out/'record.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'passed':True,'record':str(out/'record.json'),'rows':3,'effective_acceleration_row0':details[0]['effective_acceleration']}))
if __name__=='__main__':main()
