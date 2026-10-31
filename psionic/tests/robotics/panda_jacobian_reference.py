#!/usr/bin/env python3
"""NEW Jacobian reference: retained original SDK J plus independent matrix-FK FD.
No MuJoCo, downloaded code, model runtime or old gate is invoked.
"""
from pathlib import Path
import argparse,ast,hashlib,importlib.util,json,math,os,platform,random,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-jacobian'
FK=ROOT/'psionic/fixtures/robotics/panda-fk'
LIB=ROOT/'psionic/lib/robotics'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def words(path):
    raw=Path(path).read_bytes();require(len(raw)%8==0,'F64 shape');return struct.unpack('<'+'d'*(len(raw)//8),raw)
def chunk(values,n):require(len(values)%n==0,'row extent');return [values[i:i+n] for i in range(0,len(values),n)]
def require_finite_reference(values,label):
    require(all(math.isfinite(v) for v in values),label+' nonfinite expected entry')
def difference(actual,expected,budget,label):
    require(len(actual)==len(expected) and all(math.isfinite(v) for v in actual),label+' shape/numbers');require_finite_reference(expected,label);error=max((abs(a-b) for a,b in zip(actual,expected)),default=0.);require(error<=budget,f'{label} exceeds frozen {budget:g}: {error:.17g}')
    return {'values':len(actual),'max_abs_error':error,'atol':budget,'rtol':0,'max_budget_fraction':error/budget,'passed':True}
def world_log(rotation_plus,rotation_minus):
    # WORLD left increment. This is matrix algebra, not the product's quaternion or cross formula.
    d=[[math.fsum(rotation_plus[i][k]*rotation_minus[j][k] for k in range(3)) for j in range(3)] for i in range(3)]
    v=[.5*(d[2][1]-d[1][2]),.5*(d[0][2]-d[2][0]),.5*(d[1][0]-d[0][1])];s=math.sqrt(math.fsum(x*x for x in v));c=.5*(math.fsum(d[i][i] for i in range(3))-1.)
    if s==0:return [0.,0.,0.]
    factor=math.atan2(s,c)/s;return [x*factor for x in v]
def fd_reference(matrix_reference,nodes,q,h):
    linear=[0.]*21;angular=[0.]*21
    for col in range(7):
        plus=list(q);minus=list(q);plus[col]+=h;minus[col]-=h;p=matrix_reference(nodes,plus)[11];m=matrix_reference(nodes,minus)[11];omega=world_log(p,m)
        for row in range(3):linear[row*7+col]=(p[row][3]-m[row][3])/(2*h);angular[row*7+col]=omega[row]/(2*h)
    return linear,angular

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--retained',type=Path,default=FK/'retained-controller');parser.add_argument('--output',type=Path,required=True);parser.add_argument('--prepared',type=Path,help='reuse hash-bound prepared FD/input artifacts from an earlier attempt with no native modes');a=parser.parse_args();compiler=a.compiler.resolve();out=a.output.resolve()
    require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','frozen Darwin-arm64 scalar-libm profile');require(not out.exists(),'preserve existing/partial proof');out.mkdir(parents=True);(out/'oracle-source.py').write_bytes(Path(__file__).read_bytes())
    prior=None
    if a.prepared:
        prior=json.loads((a.prepared/'record.json').read_text());require(not prior['modes'],'prepared reuse only before any native mode');require(prior['compiler_sha256']==sha(compiler) and prior['platform']==platform.platform(),'prepared compiler/platform changed')
    experiment=json.loads((FIXTURE/'experiment.json').read_text());require(experiment['frozen_before_comparison'],'experiment not frozen');require(experiment['retained_atol']==2e-12 and experiment['FD_primary_h_radians']==1e-5,'frozen acceptance profile')
    for name,h in experiment['frozen_FK_sources'].items():require(sha(ROOT/name)==h,'frozen FK source changed: '+name)
    for name,h in experiment['frozen_FK_fixture_sources'].items():require(sha(FK/name)==h,'frozen FK fixture changed: '+name)
    profile=json.loads((FK/'source-profile.json').read_text());nodes=profile['nodes'];model=json.loads((FK/'sdk-model.json').read_text());require(len(nodes)==12 and len(model['arm_joints'])==7,'fixed source shape')
    require(all(j['type_name']=='hinge' and j['axis']==[0,0,1] and j['position']==[0,0,0] and j['qpos0_reference']==0 and j['qpos_adr']==i and j['dof_adr']==i for i,j in enumerate(model['arm_joints'])),'compiled fixed-Z zero-anchor/reference model')
    spec=importlib.util.spec_from_file_location('frozen_independent_fk_matrix',ROOT/'psionic/tests/robotics/panda_fk_reference.py');matrix=importlib.util.module_from_spec(spec);spec.loader.exec_module(matrix)
    retained=[];retained_ids=[]
    for info in experiment['retained_pc_samples']:
        path=a.retained/('row'+str(info['row'])+'.pc-sample.f64');require(path.stat().st_size==info['bytes']==1152 and sha(path)==info['sha256'],'retained sample identity');retained.append(words(path));retained_ids.append({'row':info['row'],'sha256':sha(path),'bytes':1152})
    require(len(retained)==7,'retained seven')
    sdk_q=chunk(words(FK/'sdk-cases.f64'),7);require(len(sdk_q)==7,'SDK q inventory; values only, no new SDK call')
    cases=[(0.,)*7]+[r[12:19] for r in retained]+list(sdk_q);rng=random.Random(experiment['case_seed'])
    for _ in range(32):
        q=[]
        for j in model['arm_joints']:
            lo,hi=j['range'];margin=.1*(hi-lo);q.append(rng.uniform(lo+margin,hi-margin))
        cases.append(tuple(q))
    require(len(cases)==47,'FD interior inventory')
    for col in range(7):
        for sign in (-1.,1.):q=[0.]*7;q[col]=sign*100.;cases.append(tuple(q))
    q=[0.]*7;q[3]=-0.;cases.append(tuple(q));require(len(cases)==62,'complete fixed inventory')
    raw=struct.pack('<II',0x314a4a50,len(cases))+b''.join(struct.pack('<7d',*q) for q in cases);(out/'cases.bin').write_bytes(raw)
    # Independently compute all step sizes ONCE; primary h is already frozen above.
    fd={}
    if prior:
        require(prior['experiment_sha256']==sha(FIXTURE/'experiment.json') and prior['case_sha256']==sha(out/'cases.bin'),'prepared experiment/input changed')
        require((a.prepared/'cases.bin').read_bytes()==raw and sha(a.prepared/'independent-fd.json')==prior['FD_table_sha256'],'prepared artifact identity')
        require(prior['independent_matrix_oracle_source_sha256']==sha(ROOT/'psionic/tests/robotics/panda_fk_reference.py'),'matrix reference changed')
        old_source=a.prepared/'oracle-source.py';require(sha(old_source)==prior['source_sha256'][str(Path(__file__).resolve().relative_to(ROOT))],'prepared oracle source identity')
        def recipes(path):
            tree=ast.parse(Path(path).read_text());return {n.name:ast.dump(n,include_attributes=False) for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ('world_log','fd_reference')}
        require(recipes(old_source)==recipes(__file__),'prepared FD recipe changed')
        table=json.loads((a.prepared/'independent-fd.json').read_text());require(table['case_count']==47 and table['primary_h_radians']==experiment['FD_primary_h_radians'],'prepared FD profile');fd=table['steps']
        require(set(fd)=={str(h) for h in experiment['FD_h_sweep_radians']} and all(len(rows)==47 and all(len(pair)==2 and all(len(side)==21 for side in pair) for pair in rows) for rows in fd.values()),'prepared FD inventory')
        (out/'independent-fd.json').write_bytes((a.prepared/'independent-fd.json').read_bytes())
    else:
        for h in experiment['FD_h_sweep_radians']:fd[str(h)]=[fd_reference(matrix.matrix_reference,nodes,q,h) for q in cases[:47]]
        (out/'independent-fd.json').write_text(json.dumps({'schema':'psionic.panda-jacobian.independent-fd.v1','primary_h_radians':experiment['FD_primary_h_radians'],'case_count':47,'steps':fd},indent=2)+'\n')
    require_finite_reference((v for rows in fd.values() for pair in rows for side in pair for v in side),'FD table')
    source_paths=list(sorted(LIB.glob('panda-jacobian*.coax')))+[Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_jacobian_native.c',FIXTURE/'experiment.json',ROOT/'psionic/tests/robotics/panda_jacobian_bad_angle.coax',ROOT/'psionic/tests/robotics/panda_jacobian_bad_linear_unit.coax',ROOT/'psionic/tests/robotics/panda_jacobian_bad_angular_unit.coax']
    source_hashes={str(p.relative_to(ROOT)):sha(p) for p in source_paths};record={'schema':'psionic.panda-jacobian.qualification.v1','passed':False,'compiler_sha256':sha(compiler),'platform':platform.platform(),'source_sha256':source_hashes,'independent_matrix_oracle_source_sha256':sha(ROOT/'psionic/tests/robotics/panda_fk_reference.py'),'experiment_sha256':sha(FIXTURE/'experiment.json'),'case_count':len(cases),'case_sha256':sha(out/'cases.bin'),'FD_case_count':47,'FD_table_sha256':sha(out/'independent-fd.json'),'retained_samples':retained_ids,'modes':[],'commands':[],'source_refusals':[],'scope':experiment['scope'],'numeric_profile':experiment['numeric_profile'],'SDK_work':'None; seven old original SDK J captures reused by hash. No SDK import/link/forward/render/NN/episode.','FD_scope':experiment['FD_policy']}
    if prior:record['prepared_reuse']={'prior_record':str(a.prepared/'record.json'),'prior_record_sha256':sha(a.prepared/'record.json'),'prior_passed':prior['passed'],'FD_recipe_AST_identity':True,'FD_input_and_table_identity':True,'prior_failed_diagnostic_expectation_preserved':True}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        start=time.monotonic();r=subprocess.run([str(v) for v in args],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'arguments':[str(v) for v in args],'exit':r.returncode,'duration_seconds':time.monotonic()-start,'stderr_bytes':len(r.stderr),'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode==0,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')));require(not r.stderr,(name,'stderr',r.stderr.decode(errors='replace')));return r
    begin=time.monotonic();cc=os.environ.get('CC','cc');outputs=[]
    try:
        for name,code in [('angle','E0317'),('linear_unit','E0330'),('angular_unit','E0330')]:
            path=ROOT/('psionic/tests/robotics/panda_jacobian_bad_'+name+'.coax')
            reused=next((entry for entry in prior['source_refusals'] if entry['name']==name),None) if prior else None
            if reused:
                require(reused['source_sha256']==sha(path) and reused['diagnostic']==code and reused['exit']!=0,'reused refusal identity')
                require(sha(a.prepared/('refusal-'+name+'.stderr'))==reused['stderr_sha256'],'reused refusal bytes')
                record['source_refusals'].append({**reused,'reused_not_repeated':True});continue
            r=subprocess.run([str(compiler),'check',str(path)],cwd=ROOT,capture_output=True,timeout=180);(out/('refusal-'+name+'.stderr')).write_bytes(r.stderr)
            record['source_refusals'].append({'name':name,'exit':r.returncode,'diagnostic':code,'source_sha256':sha(path),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save()
            require(r.returncode!=0 and ('error['+code+']').encode() in r.stderr,('source unit refusal',name,r.stderr.decode(errors='replace')))
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();header=d/'panda-jacobian.h';source=d/'panda-jacobian.c';binary=d/'native'
            source.write_bytes(command('emit-'+mode,[compiler,'c',LIB/'panda-jacobian.coax','--library',*extra,'--header',header]).stdout);require('pj_project_owned(' not in header.read_text(),'private projection helper exported unexpectedly')
            command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-builtin-sin','-fno-builtin-cos','-fno-fast-math','-ffp-contract=off','-I'+str(d),source,ROOT/'psionic/tests/robotics/panda_jacobian_native.c','-lm','-o',binary])
            native=json.loads(command('run-'+mode,[binary,out/'cases.bin',d/'jacobians.f64']).stdout);require(native['cases']==62 and native['output_words_per_case']==42,'native complete coverage');blob=(d/'jacobians.f64').read_bytes();require(len(blob)==62*42*8,'native exact extent');outputs.append(blob);rows=chunk(words(d/'jacobians.f64'),42);require(len(rows)==62 and all(math.isfinite(v) for r in rows for v in r),'complete finite J outputs')
            actual=[v for row in rows[1:8] for v in row];expected=[v for r in retained for v in r[75:117]];gold=difference(actual,expected,experiment['retained_atol'],'all294 retained SDK J values')
            sweep={}
            for h in experiment['FD_h_sweep_radians']:
                reference=fd[str(h)];linear=max(abs(rows[i][j]-reference[i][0][j]) for i in range(47) for j in range(21));angular=max(abs(rows[i][21+j]-reference[i][1][j]) for i in range(47) for j in range(21));sweep[str(h)]={'cases':47,'linear_values':987,'angular_values':987,'linear_max_abs_error':linear,'angular_max_abs_error':angular,'within_primary_linear_budget':linear<=experiment['FD_linear_primary_atol_metres_per_radian'],'within_primary_angular_budget':angular<=experiment['FD_angular_primary_atol']}
            primary=sweep[str(experiment['FD_primary_h_radians'])];require(primary['within_primary_linear_budget'] and primary['within_primary_angular_budget'],'fixed primary FD budget failed; do not select another h')
            velocity=[];velocity_gold=[]
            for row,sample in zip(rows[1:8],retained):
                qd=sample[19:26];velocity.extend(math.fsum(row[r*7+j]*qd[j] for j in range(7)) for r in range(6));velocity_gold.extend(sample[117:123])
            rate=difference(velocity,velocity_gold,experiment['velocity_diagnostic_atol'],'retained world site J@qd')
            require(abs(rows[0][0])<=2e-12 and abs(rows[0][7]-.088)<=2e-12 and abs(rows[0][14])<=2e-12,'analytic zero-angle first translation column')
            require(all(abs(row[r*7+6])<=2e-12 for row in rows for r in range(3)),'fixed distal-axis last translation column')
            record['modes'].append({'mode':mode,'native':native,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'jacobians.f64'),'retained_original_SDK_J':gold,'FD_sweep':sweep,'primary_FD_h_radians':experiment['FD_primary_h_radians'],'retained_site_velocity':rate,'private_helper_not_exported':True,'analytic_columns_passed':True});save()
        require(outputs[0]==outputs[1]==outputs[2],'native mode output bytes differ under frozen scalar-libm profile');require({str(p.relative_to(ROOT)):sha(p) for p in source_paths}==source_hashes,'new source changed');require(sha(compiler)==record['compiler_sha256'],'compiler changed');record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['duration_seconds']=time.monotonic()-begin;save()
    print(json.dumps({'passed':True,'cases':62,'J_values_per_mode':2604,'retained_reference_values':294,'record':str(out/'record.json'),'seconds':record['duration_seconds']}))
if __name__=='__main__':main()
