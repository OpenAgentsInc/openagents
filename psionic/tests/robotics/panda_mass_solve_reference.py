#!/usr/bin/env python3
"""NEW mass solve: pivoted Gaussian reference and retained-component residuals.
No SDK, renderer, neural runtime, BLAS or old numerical gate executes here.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-mass-solve'
LIB=ROOT/'psionic/lib/robotics'

def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def doubles(path):
    data=Path(path).read_bytes();require(len(data)%8==0,'F64 byte extent');values=struct.unpack('<'+'d'*(len(data)//8),data);require(all(math.isfinite(x) for x in values),'finite decoded fixture');return values
def rows(values,width):
    require(len(values)%width==0,'row extent');return [list(values[i:i+width]) for i in range(0,len(values),width)]
def finite(values,count,label):require(len(values)==count and all(math.isfinite(v) for v in values),label+' complete finite values')
def matvec(matrix,vector):
    finite(matrix,81,'matrix');finite(vector,9,'vector');return [math.fsum(matrix[i*9+j]*vector[j] for j in range(9)) for i in range(9)]

def gaussian(matrix,force):
    """Independent row-pivoted elimination, not Cholesky/triangular product math."""
    finite(matrix,81,'Gaussian matrix');finite(force,9,'Gaussian force');a=[list(matrix[i*9:i*9+9])+[force[i]] for i in range(9)]
    for column in range(9):
        pivot=max(range(column,9),key=lambda row:abs(a[row][column]));require(abs(a[pivot][column])>1e-20,'Gaussian pivot');a[column],a[pivot]=a[pivot],a[column]
        for row in range(column+1,9):
            ratio=a[row][column]/a[column][column]
            for j in range(column+1,10):a[row][j]-=ratio*a[column][j]
            a[row][column]=0.
    answer=[0.]*9
    for row in reversed(range(9)):answer[row]=(a[row][9]-math.fsum(a[row][j]*answer[j] for j in range(row+1,9)))/a[row][row]
    finite(answer,9,'Gaussian answer');return answer

def compare(actual,expected,atol,rtol,label):
    require(len(actual)==len(expected)>0,label+' extent');finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' reference')
    errors=[abs(a-e) for a,e in zip(actual,expected)];fractions=[error/(atol+rtol*abs(e)) for error,e in zip(errors,expected)];worst=max(errors);fraction=max(fractions)
    require(fraction<=1,f'{label}: maxabs{worst:.17g}, budgetfraction{fraction:.17g}')
    return {'values':len(actual),'max_abs_error':worst,'max_budget_fraction':fraction,'atol':atol,'rtol':rtol,'passed':True}

def raw_profiles():
    def diagonal(values):return [values[i] if i==j else 0. for i in range(9) for j in range(9)]
    dense=[(2. if i==j else 0.)+(i+1)*(j+1)/64 for i in range(9) for j in range(9)]
    lower=[[0. if j>i else (1.+i/8 if i==j else ((i*3+j*5)%7-3)/16) for j in range(9)] for i in range(9)]
    gram=[math.fsum(lower[i][k]*lower[j][k] for k in range(9)) for i in range(9) for j in range(9)]
    known=[(i-4)/8 for i in range(9)];other=[(-1. if i%2 else 1.)*(i+1)/16 for i in range(9)]
    return [
      {'kind':'identity zero force','matrix':diagonal([1.]*9),'force':[0.]*9,'known':[0.]*9},
      {'kind':'independent diagonal','matrix':diagonal([1.+i/4 for i in range(9)]),'force':[(i-4)*.125 for i in range(9)]},
      {'kind':'inclusive .25 pivot and force cap','matrix':diagonal([.25]*9),'force':[1e4 if i%2==0 else -1e4 for i in range(9)]},
      {'kind':'dense rational rank-one SPD','matrix':dense,'force':matvec(dense,known),'known':known},
      {'kind':'dense independent Gram SPD','matrix':gram,'force':matvec(gram,other),'known':other},
      {'kind':'inclusive coefficient cap','matrix':diagonal([1e8]*9),'force':[1e4]*9,'known':[1e-4]*9},
      {'kind':'signedzero force','matrix':diagonal([2.**(i-1) for i in range(9)]),'force':[-0. if i%2 else 0. for i in range(9)],'known':[0.]*9}]

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();compiler=args.compiler.resolve();out=args.output.resolve()
    require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 little-endian scalar profile');require(not out.exists(),'new output directory required');out.mkdir(parents=True)
    experiment=json.loads((FIXTURE/'experiment.json').read_text());require(experiment['frozen_before_new_numerical_execution'] is True,'frozen experiment')
    require(experiment['admission']['normalized_force_abs_J']==1e4 and experiment['admission']['raw_normalized_matrix_entry_abs_kg_m2']==1e8 and experiment['admission']['raw_cholesky_pivot_min_kg_m2']==.25,'frozen normalized numerical bounds')
    acceleration=experiment['budgets']['acceleration'];residual=experiment['budgets']['residual'];require(acceleration=={'atol':2e-10,'rtol':2e-12} and residual=={'atol':2e-9,'rtol':2e-12},'unchanged preselected budgets')
    for name,digest in experiment['frozen_product_dependencies'].items():require(sha(ROOT/name)==digest,'frozen dependency '+name)
    component=experiment['cached_metric_component'];require(sha(ROOT/component['qualified_public_receipt'])==component['qualified_public_receipt_sha256'],'prior component qualification identity')
    case_path=FIXTURE/'cached-metric-cases.bin';metric_path=FIXTURE/'cached-metric-and-gravity.f64';require(case_path.stat().st_size==component['input_bytes'] and sha(case_path)==component['input_sha256'] and metric_path.stat().st_size==component['output_bytes'] and sha(metric_path)==component['output_sha256'],'old recorded component inputs/outputs exact')
    data=case_path.read_bytes();magic,n=struct.unpack('<II',data[:8]);require(magic==0x314d4950 and n==57 and len(data)==8+57*9*8,'57 recorded configurations');qrows=rows(struct.unpack('<513d',data[8:]),9);require(all(math.isfinite(v) for q in qrows for v in q),'finite recorded configurations');mrows=rows(doubles(metric_path),90);require(len(mrows)==57,'57 recorded complete metric/gravity rows')
    for name,spec in experiment['retained_SDK'].items():
        p=ROOT/'psionic/fixtures/robotics/panda-fingers'/name;require(p.stat().st_size==spec['bytes'] and sha(p)==spec['sha256'],'retained SDK identity '+name)
    sdk_q=rows(doubles(ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-cases.f64'),9);sdk_m=rows(doubles(ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-dynamics.f64'),90);require(len(sdk_q)==len(sdk_m)==5,'five retained SDK contexts');require(sdk_q==qrows[:5],'old component/SDK coordinate identity')
    contexts=[]
    for index,(q,m) in enumerate(zip(qrows,mrows)):
        for kind,force in [('alternatingforcecap',[1e4 if i%2==0 else -1e4 for i in range(9)]),('smallorderedforce',[(i-4)*.125 for i in range(9)])]:contexts.append({'kind':kind,'source':'previously qualified Coax M9 component','source_case':index,'q':q,'matrix':m[:81],'force':force})
    for index,(q,m) in enumerate(zip(sdk_q,sdk_m)):
        for known in experiment['corpus']['SDK_known_acceleration_patterns_per_s2']:
            finite(known,9,'preselected acceleration');force=matvec(m[:81],known);require(all(abs(x)<=1e4 for x in force),'known-acceleration force admission');contexts.append({'kind':'SDKMtimesknownacceleration','source':'retained original SDK mass matrix','source_case':index,'q':q,'matrix':m[:81],'force':force,'known':known})
    require(len(contexts)==129,'129 public solver contexts');raw=raw_profiles();require(len(raw)==7,'seven raw numerical profiles')
    for item in contexts+raw:item['reference']=gaussian(item['matrix'],item['force'])
    input_bytes=struct.pack('<III',0x31534d50,129,7)+b''.join(struct.pack('<18d',*c['q'],*c['force']) for c in contexts)+b''.join(struct.pack('<90d',*c['matrix'],*c['force']) for c in raw);(out/'cases.bin').write_bytes(input_bytes)
    oracle={'schema':'psionic.panda-mass-solve.reference-values.v1','public':contexts,'raw':raw};(out/'independent.json').write_text(json.dumps(oracle,indent=2)+'\n')
    sources=list(sorted(LIB.glob('panda-mass-solve*.coax')))+list(sorted((ROOT/'psionic/tests/robotics').glob('panda_mass_solve*')))+[FIXTURE/'experiment.json',FIXTURE/'known-solution-checks.json']
    frozen={str(p.relative_to(ROOT)):sha(p) for p in sources if p.is_file()};record={'schema':'psionic.panda-mass-solve.qualification.v1','passed':False,'compiler_sha256':sha(compiler),'platform':platform.platform(),'experiment_sha256':sha(FIXTURE/'experiment.json'),'source_sha256':frozen,'case_sha256':sha(out/'cases.bin'),'independent_reference_sha256':sha(out/'independent.json'),'public_cases':129,'raw_cases':7,'output_words_per_mode':1224,'old_component_gate_rerun':False,'SDK_calls':0,'scope':experiment['scope'],'basis':experiment['basis'],'numeric_profile':experiment['native_profile'],'modes':[],'source_refusals':[],'commands':[]}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        r=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'exit_status':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_bytes':len(r.stderr),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode==0 and not r.stderr,(name,r.stderr.decode(errors='replace')));return r.stdout
    began=time.monotonic();save()
    try:
        for name,code in [('angle','E0317'),('hinge_force','E0317'),('slide_force','E0317'),('acceleration','E0330')]:
            p=ROOT/('psionic/tests/robotics/panda_mass_solve_bad_'+name+'.coax');r=subprocess.run([str(compiler),'check',str(p),'--library'],cwd=ROOT,capture_output=True,timeout=180);(out/('refusal-'+name+'.stderr')).write_bytes(r.stderr);require(r.returncode!=0 and ('error['+code+']').encode() in r.stderr,'source unit refusal '+name);record['source_refusals'].append({'name':name,'diagnostic':code,'refused':True,'source_sha256':sha(p),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        blobs=[];cc=os.environ.get('CC','cc')
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();c=d/'panda-mass-solve.c';h=d/'panda-mass-solve.h';binary=d/'native';c.write_bytes(command('emit-'+mode,[compiler,'c','psionic/tests/robotics/panda_mass_solve_entry.coax','--library',*extra,'--header',h]));command('build-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I',d,c,ROOT/'psionic/tests/robotics/panda_mass_solve_native.c','-lm','-o',binary])
            native=json.loads(command('run-'+mode,[binary,out/'cases.bin',d/'acceleration.f64']));require(native['passed'] and native['public_cases']==129 and native['raw_cases']==7 and native['output_words']==1224,'native exact corpus inventory');blob=(d/'acceleration.f64').read_bytes();require(len(blob)==1224*8,'complete native output extent');values=rows(doubles(d/'acceleration.f64'),9);require(len(values)==136,'native exact row count');blobs.append(blob)
            got=[];expected=[];lhs=[];rhs=[];known_got=[];known_expected=[]
            for item,actual in zip(contexts+raw,values):
                finite(actual,9,'actual acceleration');finite(item['reference'],9,'reference acceleration');got.extend(actual);expected.extend(item['reference']);lhs.extend(matvec(item['matrix'],actual));rhs.extend(item['force'])
                if 'known' in item:known_got.extend(actual);known_expected.extend(item['known'])
            accuracy=compare(got,expected,**acceleration,label='independent pivoted Gaussian acceleration');equation=compare(lhs,rhs,**residual,label='retained-matrix equation residual');known=compare(known_got,known_expected,**acceleration,label='independently prescribed acceleration')
            record['modes'].append({'mode':mode,'native':native,'C_sha256':sha(c),'header_sha256':sha(h),'binary_sha256':sha(binary),'output_sha256':sha(d/'acceleration.f64'),'Gaussian':accuracy,'residual':equation,'known_acceleration':known});save()
        require(blobs[0]==blobs[1]==blobs[2],'explicit scalar-libm native mode byte identity');require(all(sha(ROOT/name)==digest for name,digest in frozen.items()) and sha(compiler)==record['compiler_sha256'],'source/compiler changed during qualification');record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'public_cases':129,'raw_cases':7,'record':str(out/'record.json'),'seconds':record['seconds']}))
if __name__=='__main__':main()
