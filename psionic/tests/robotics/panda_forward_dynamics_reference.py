#!/usr/bin/env python3
"""NEW applied-minus-bias query proof using retained SDK equations, no SDK calls."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import os
import platform
import re
import struct
import subprocess
import sys
import time
from panda_mass_solve_reference import gaussian,matvec,compare

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-forward-dynamics'
REFERENCE=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics'
SOURCE=ROOT/'psionic/lib/robotics/panda-forward-dynamics.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_forward_dynamics_native.c'
ORACLE=ROOT/'psionic/tests/robotics/panda_mass_solve_reference.py'
EXPERIMENT_SHA='7d868d26f4ff25ef32de04b7b1ff1d96c40b090a401eb4526b83238dbbc24392'
ROW=struct.Struct('<q9d')

def require(ok,message):
    if not ok:raise AssertionError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(x) for x in values),label+' complete finite words before comparison')
    return list(values)
def words(raw,count,label):
    require(len(raw)==count*8,label+' exact byte extent')
    return finite(struct.unpack('<'+str(count)+'d',raw),count,label)
def sources(path,seen=None):
    if seen is None:seen={}
    path=path.resolve()
    if path in seen:return seen
    seen[path]=sha(path)
    for name in re.findall(r'^\s*#load\s+"([^"]+)"',path.read_text(),re.M):sources(path.parent/name,seen)
    return seen

def decode(raw):
    require(len(raw)==36*ROW.size,'exact 36-row native status/acceleration extent')
    result=[]
    for i in range(36):
        row=ROW.unpack_from(raw,i*ROW.size);status=row[0]
        require(0<=status<=4,'declared PmsStatus domain')
        result.append((status,finite(row[1:],9,'native row '+str(i))))
    return result

def guard_checks():
    good=ROW.pack(0,*([0.]*9))*36;require(len(decode(good))==36,'finite decoder control')
    bad=[good[:-1],good+ROW.pack(0,*([0.]*9))]
    for value in (float('nan'),float('inf'),-float('inf')):bad.append(good[:-8]+struct.pack('<d',value))
    bad.append(good[:-80]+ROW.pack(255,*([0.]*9)))
    for index,blob in enumerate(bad):
        try:decode(blob)
        except AssertionError:pass
        else:raise AssertionError('decoder admitted poison '+str(index))
    return {'passed':True,'refused_cases':len(bad),'poison':'last acceleration word or last status; wrong extents','expected_finite_guard':True}

def load_references():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'exact pre-execution experiment')
    experiment=json.loads((FIXTURE/'experiment.json').read_text())
    require(experiment['frozen_before_new_product_execution'] and experiment['case_count']==36 and experiment['known_acceleration_contexts']==15 and experiment['zero_applied_gravity_contexts']==3 and experiment['signed_cap_contexts']==18,'frozen case scope')
    require(experiment['budgets']['acceleration']=={'atol':2e-10,'rtol':2e-12} and experiment['budgets']['residual']=={'atol':2e-9,'rtol':2e-12},'unchanged PMS scalar budgets')
    for name,h in experiment['reference_dependencies'].items():require(sha(ROOT/name)==h,'frozen reference dependency '+name)
    raw=(FIXTURE/'cases.f64').read_bytes();require(sha(FIXTURE/'cases.f64')==experiment['case_sha256'],'exact frozen q/v/APPLIED corpus');values=words(raw,36*27,'input corpus')
    require(len(experiment['cases'])==36 and len({c['name'] for c in experiment['cases']})==36,'all named contexts present')
    require(raw==b''.join(struct.pack('<27d',*c['q'],*c['v'],*c['applied']) for c in experiment['cases']),'named cases bind every input bit')
    sdk=json.loads((REFERENCE/'sdk-reference.json').read_text());require(sdk['passed'] and sdk['case_count']==15 and sdk['sdk_version']=='3.3.1','retained SDK reference scope')
    item=sdk['artifacts']['sdk-mass-bias.f64'];matrix_path=REFERENCE/'sdk-mass-bias.f64';require(sha(matrix_path)==item['sha256'] and matrix_path.stat().st_size==item['bytes'],'original SDK mass/bias artifact')
    all_metric=words(matrix_path.read_bytes(),15*90,'every original SDK M/bias coefficient');references=[]
    for index,case in enumerate(experiment['cases']):
        i=case['source_case'];require(0<=i<15,'declared source context');row=all_metric[i*90:(i+1)*90];metric=row[:81];bias=row[81:]
        applied=finite(case['applied'],9,'applied '+str(index));net=finite([a-b for a,b in zip(applied,bias)],9,'independent net '+str(index))
        require(all(abs(x)<=10000 for x in applied),'every corpus APPLIED force admits')
        expected_status=3 if any(abs(x)>10000 for x in net) else 0
        expected=gaussian(metric,net) if expected_status==0 else None
        if case['kind']=='known':
            require(expected_status==0,'known acceleration context admits');finite(case['known_acceleration'],9,'original prescribed acceleration')
            compare(expected,case['known_acceleration'],**experiment['budgets']['acceleration'],label='SDK equation/known acceleration consistency')
        if case['kind']=='gravity':require(expected_status==0 and all(x==0 for x in case['v']+case['applied']),'zero applied/zero rate gravity context')
        references.append({'name':case['name'],'kind':case['kind'],'expected_status':expected_status,'matrix':metric,'bias':bias,'net':net,'expected_acceleration':expected,'known_acceleration':case.get('known_acceleration')})
    require(any(r['expected_status']==3 for r in references[18:]) and any(r['expected_status']==0 for r in references[18:]),'signed caps observe separate admitted APPLIED and refused/admitted NET outcomes')
    return experiment,raw,references

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();compiler=a.compiler.resolve()
    require(not out.exists(),'preserve prior proof; choose a new output directory');out.mkdir(parents=True)
    record={'schema':'psionic.panda-forward-dynamics.qualification.v1','passed':False,'SDK_calls':0,'simulation_steps':0,'old_numerical_gates_rerun':False,'commands':[],'modes':[]}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        began=time.monotonic();r=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,timeout=180)
        (out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr)
        record['commands'].append({'name':name,'argv':[str(x) for x in argv],'exit':r.returncode,'seconds':time.monotonic()-began,'stderr_bytes':len(r.stderr),'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save()
        require(r.returncode==0 and not r.stderr,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')));return r.stdout
    began=time.monotonic();save()
    try:
        require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 target')
        experiment,raw,references=load_references();record.update({'experiment_sha256':EXPERIMENT_SHA,'case_sha256':sha(FIXTURE/'cases.f64'),'scope':experiment['scope'],'basis':experiment['basis'],'numeric_profile':experiment['numeric_profile'],'mutation_scope_limit':experiment['mutation_scope_limit'],'case_count':36,'known_contexts':15,'gravity_contexts':3,'cap_contexts':18,'expected_status_counts':{str(status):sum(r['expected_status']==status for r in references) for status in (0,3)},'decoder_guard_checks':guard_checks()})
        (out/'independent.json').write_text(json.dumps(references,indent=2)+'\n');record['independent_reference_sha256']=sha(out/'independent.json');(out/'input.bin').write_bytes(struct.pack('<II',0x31444650,36)+raw)
        dependencies=sources(SOURCE);dependencies.update({Path(__file__).resolve():sha(Path(__file__)),NATIVE:sha(NATIVE),ORACLE:sha(ORACLE),FIXTURE/'experiment.json':EXPERIMENT_SHA,FIXTURE/'cases.f64':sha(FIXTURE/'cases.f64')})
        dependencies.update({ROOT/name:h for name,h in experiment['reference_dependencies'].items()})
        record['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(dependencies.items())};record['compiler_sha256']=sha(compiler);record['input_sha256']=sha(out/'input.bin')
        (out/'executed-driver.py').write_bytes(Path(__file__).read_bytes());(out/'executed-native.c').write_bytes(NATIVE.read_bytes())
        for name in ('rate','hinge_force','slide_position','target'):
            path=ROOT/('psionic/tests/robotics/panda_forward_dynamics_bad_'+name+'.coax');dependencies[path]=sha(path)
            result=subprocess.run([str(compiler),'check',str(path.relative_to(ROOT))],cwd=ROOT,capture_output=True,timeout=120)
            (out/('refusal-'+name+'.stdout')).write_bytes(result.stdout);(out/('refusal-'+name+'.stderr')).write_bytes(result.stderr)
            record.setdefault('source_refusals',[]).append({'name':name,'source_sha256':sha(path),'exit':result.returncode,'diagnostic':'E0317','stderr_sha256':hashlib.sha256(result.stderr).hexdigest()});save()
            require(result.returncode!=0 and b'error[E0317]' in result.stderr,('source unit refusal',name,result.stderr.decode(errors='replace')))
        record['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(dependencies.items())}
        emitted={}
        for mode,extra in [('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'panda-forward-dynamics.c';h=d/'panda-forward-dynamics.h';c.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emitted[mode]=(c,h)
        blobs=[];cc=os.environ.get('CC','cc')
        for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir(exist_ok=True);source,header=emitted[source_mode];binary=d/'native'
            command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(header.parent),source,NATIVE,'-lm','-o',binary])
            native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'accelerations.bin']));require(native['passed'] and native['cases']==36 and native['words_per_case']==9 and native['record_bytes']==80 and native['position_rate_boundaries']==36 and native['applied_cap_observations']==18 and native['alias_cases']==4,'complete new native corpus and admission/alias inventory')
            blob=(d/'accelerations.bin').read_bytes();actual=decode(blob);blobs.append(blob);got=[];expected=[];known=[];known_expected=[];lhs=[];rhs=[];statuses={}
            for index,((status,acceleration),reference) in enumerate(zip(actual,references)):
                require(status==reference['expected_status'],('exact independent admission status',index,status,reference['expected_status']));statuses[str(status)]=statuses.get(str(status),0)+1
                if status!=0:continue
                finite(reference['expected_acceleration'],9,'independent expected acceleration');got.extend(acceleration);expected.extend(reference['expected_acceleration']);lhs.extend(matvec(reference['matrix'],acceleration));rhs.extend(reference['net'])
                if reference['known_acceleration'] is not None:known.extend(acceleration);known_expected.extend(reference['known_acceleration'])
            accuracy=compare(got,expected,**experiment['budgets']['acceleration'],label='independent pivoted Gaussian acceleration')
            residual=compare(lhs,rhs,**experiment['budgets']['residual'],label='original SDK M times actual acceleration versus applied minus original bias')
            known_accuracy=compare(known,known_expected,**experiment['budgets']['acceleration'],label='15 original prescribed accelerations')
            record['modes'].append({'mode':mode,'native':native,'status_counts':statuses,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'accelerations.bin'),'all_actual_words_finite':36*9,'acceleration':accuracy,'residual':residual,'known_acceleration':known_accuracy});save()
        require(blobs[0]==blobs[1]==blobs[2],'all status/acceleration streams byte identical under named numerical profile')
        require(all(sha(k)==v for k,v in dependencies.items()) and sha(compiler)==record['compiler_sha256'],'sources/compiler/reference changed during qualification')
        record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':record['seconds']}))

if __name__=='__main__':main()
