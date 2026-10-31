#!/usr/bin/env python3
"""NEW smooth composition proof against cached SDK facts; no SDK acquisition.
Imports only the previously qualified Pa scalar definition for clip counts.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,re,struct,subprocess,sys,time
from panda_actuation_reference import scalar as actuation_scalar
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-smooth'
SOURCE=ROOT/'psionic/lib/robotics/panda-smooth.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_smooth_native.c'
EXPERIMENT_SHA='a560a91a9b6cd5a443e3995b0037d62cc4553db5640baf5b6257e6878871a6e7'
ROW=struct.Struct('<q18d2q')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(x) for x in values),label+' ALL finite and complete');return list(values)
def words(raw,count,label):
    require(len(raw)==count*8,label+' exact bytes');return finite(struct.unpack('<'+str(count)+'d',raw),count,label)
def graph(path,seen=None):
    if seen is None:seen={}
    path=path.resolve()
    if path in seen:return seen
    seen[path]=sha(path)
    for name in re.findall(r'^\s*#load\s*"([^"]+)"',path.read_text(),re.M):graph(path.parent/name,seen)
    return seen
def compare(actual,expected,atol,label):
    finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty');error=max(abs(x-y) for x,y in zip(actual,expected));require(error<=atol,f'{label} {error:.17g} exceeds frozen {atol:g}');return {'passed':True,'values':len(actual),'max_abs_error':error,'atol':atol,'rtol':0}
def decode(raw):
    require(len(raw)==20*ROW.size,'twenty exact168-byte native records');rows=[]
    for i in range(20):
        row=ROW.unpack_from(raw,i*ROW.size);require(row[0]==0,'every frozen context must admit');finite(row[1:19],18,'all actual applied/acceleration fields');require(0<=row[19]<=9 and 0<=row[20]<=2,'actual clip-count domain');rows.append(row)
    return rows
def decoder_guards():
    good=ROW.pack(0,*([0.]*18),0,0)*20;decode(good);bad=[good[:-1],good+ROW.pack(0,*([0.]*18),0,0)]
    for x in (float('nan'),float('inf'),-float('inf')):
        b=bytearray(good);struct.pack_into('<d',b,19*168+8+17*8,x);bad.append(bytes(b))
    b=bytearray(good);struct.pack_into('<q',b,19*168,3);bad.append(bytes(b));b=bytearray(good);struct.pack_into('<q',b,len(b)-8,3);bad.append(bytes(b))
    for value in bad:
        try:decode(value)
        except RuntimeError:pass
        else:raise RuntimeError('decoder accepted late poison or malformed extent')
    return {'passed':True,'refused_cases':len(bad),'poisons':'last acceleration word/status/clipping count; wrong extents'}
def load_reference():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen smooth experiment');e=json.loads((FIXTURE/'experiment.json').read_text());require(e['frozen_before_implementation_or_comparison'] and e['case_count']==20 and e['direct_SDK_contexts']==18 and e['derived_paired_force_targets']==2,'frozen target inventory');require(e['budgets']=={'applied_force':{'atol':5e-10,'rtol':0},'smooth_acceleration':{'atol':5e-9,'rtol':0}},'unchanged frozen scalar budgets')
    require(all(sha(ROOT/name)==digest for name,digest in e['dependencies'].items()),'cached independent SDK source bindings');require(sha(FIXTURE/'cases.f64')==e['cases_sha256'] and sha(FIXTURE/'targets.f64')==e['targets_sha256'],'case/target bytes');cases=words((FIXTURE/'cases.f64').read_bytes(),720,'inputs');target=words((FIXTURE/'targets.f64').read_bytes(),360,'targets');require((FIXTURE/'cases.f64').read_bytes()==b''.join(struct.pack('<36d',*c['q'],*c['v'],*c['ctrl'],*c['external']) for c in e['cases']),'every explicit input bit')
    pa=ROOT/'psionic/fixtures/robotics/panda-actuation';sdk=words((pa/'sdk-values.f64').read_bytes(),2916,'all cached SDK fields');old=json.loads((pa/'experiment.json').read_text())['cases'];expected=[]
    for i,c in enumerate(e['cases']):
        finite(c['q']+c['v']+c['ctrl']+c['external'],36,'case fields');src=c['SDK_force_target_row'];accsrc=c['SDK_smooth_target_row'];require(src==accsrc and 0<=src<18,'target source row inventory')
        if i<18:
            require(src==i and c['external']==[0.]*9 and c['q']==old[i]['q'] and c['v']==old[i]['v'] and c['ctrl']==old[i]['ctrl'],'eighteen direct retained contexts')
        else:
            base=8 if i==18 else 9;other=9 if i==18 else 8;require(src==other and c['q']==old[base]['q']==old[other]['q'] and c['v']==old[base]['v']==old[other]['v'] and c['ctrl']==old[base]['ctrl'],'two exact same-state derived contexts');require(c['external']==[sdk[other*162+9+j]-sdk[base*162+9+j] for j in range(9)],'external is original actuator-force difference');require(sdk[base*162+54:base*162+63]==sdk[other*162+54:other*162+63],'same-state SDK passive equality')
        applied=[sdk[src*162+9+j]+sdk[src*162+54+j] for j in range(9)];acceleration=sdk[accsrc*162+99:accsrc*162+108];require(struct.pack('<18d',*applied,*acceleration)==(FIXTURE/'targets.f64').read_bytes()[i*144:(i+1)*144],'target exactly rebuilds cached SDK sum/smooth acceleration');scalar=actuation_scalar(c);expected.append({'applied':applied,'acceleration':acceleration,'control_clipped':scalar[2],'force_clipped':scalar[3],'target_kind':'direct original SDK' if i<18 else 'derived equal-total-force; no new SDK query'})
    return e,cases,expected
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--source-refusals',type=Path);a=parser.parse_args();out=a.output.resolve();require(not out.exists(),'preserve earlier/failed native proof');out.mkdir(parents=True);compiler=a.compiler.resolve();record={'schema':'psionic.panda-smooth.native-qualification.v1','passed':False,'SDK_calls':0,'NN_calls':0,'simulation_steps':0,'render_calls':0,'commands':[],'modes':[]};began=time.monotonic()
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        start=time.monotonic();result=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=240);(out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr);record['commands'].append({'name':name,'arguments':[str(x) for x in args],'exit':result.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest(),'stderr_bytes':len(result.stderr)});save();require(result.returncode==0 and not result.stderr,(name,result.stdout.decode(errors='replace'),result.stderr.decode(errors='replace')));return result.stdout
    save()
    try:
        require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile');e,case_values,expected=load_reference();record.update(experiment_sha256=EXPERIMENT_SHA,compiler_sha256=sha(compiler),direct_SDK_contexts=18,derived_equal_total_force_contexts=2,expected_status_counts={'ok':20},numeric_profile='Darwin arm64 F64 scalar libm, no fast math/contraction; no cross-target claim',scope=e['scope'],decoder_guards=decoder_guards());dependencies=graph(SOURCE);dependencies.update({Path(__file__).resolve():sha(__file__),NATIVE:sha(NATIVE),Path(__file__).with_name('panda_actuation_reference.py'):sha(Path(__file__).with_name('panda_actuation_reference.py'))})
        for name in ['experiment.json','cases.f64','targets.f64']:dependencies[FIXTURE/name]=sha(FIXTURE/name)
        for name in e['dependencies']:dependencies[ROOT/name]=sha(ROOT/name)
        for name in ['external','target','rate','output']:
            path=ROOT/('psionic/tests/robotics/panda_smooth_bad_'+name+'.coax');dependencies[path]=sha(path)
        record['source_sha256']={str(p.relative_to(ROOT)):s for p,s in sorted(dependencies.items())};(out/'input.bin').write_bytes(struct.pack('<II',0x31465350,20)+struct.pack('<720d',*case_values));(out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n');record['expected_sha256']=sha(out/'expected.json')
        if a.source_refusals:
            proof=json.loads(a.source_refusals.read_text());require(proof['compiler_sha256']==record['compiler_sha256'] and len(proof['checks'])==4 and {x['name'] for x in proof['checks']}=={'external','target','rate','output'},'exact reused source-refusal inventory/compiler')
            for item in proof['checks']:
                path=ROOT/('psionic/tests/robotics/panda_smooth_bad_'+item['name']+'.coax');require(item['source_sha256']==sha(path) and item['diagnostic']=='E0317' and (item.get('refused') is True or item.get('exit',0)!=0),'reused source-refusal identity/result')
            record['source_refusals']=proof['checks'];record['source_refusals_reused']={'receipt_sha256':sha(a.source_refusals),'no_refusal_calls_repeated':True}
        else:
            for name in ['external','target','rate','output']:
                path=ROOT/('psionic/tests/robotics/panda_smooth_bad_'+name+'.coax');result=subprocess.run([str(compiler),'check',str(path.relative_to(ROOT))],cwd=ROOT,capture_output=True,timeout=120);(out/('refusal-'+name+'.stderr')).write_bytes(result.stderr);require(result.returncode!=0 and b'error[E0317]' in result.stderr,'precise unit/result refusal '+name);record.setdefault('source_refusals',[]).append({'name':name,'exit':result.returncode,'diagnostic':'E0317','source_sha256':sha(path),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
        emitted={}
        for mode,flags in [('debug',[]),('release',['--release'])]:
            directory=out/mode;directory.mkdir();header=directory/'panda-smooth.h';source=directory/'panda-smooth.c';source.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*flags,'--header',header]));emitted[mode]=(source,header)
        payloads=[]
        for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            directory=out/mode;directory.mkdir(exist_ok=True);source,header=emitted[source_mode];binary=directory/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(header.parent),source,NATIVE,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',directory/'outputs.bin']));require(native['passed'] and native['cases']==20 and native['record_bytes']==168 and native['coordinate_rate_endpoints']==36 and native['control_endpoints']==18 and native['external_endpoints']==18 and native['alias_cases']==4,'complete new native coverage');payload=(directory/'outputs.bin').read_bytes();actual=decode(payload);payloads.append(payload);applied=[];acceleration=[];want_applied=[];want_acceleration=[]
            for row,want in zip(actual,expected):require(row[19:]==(want['control_clipped'],want['force_clipped']),'independent clipping counts on actual controls');applied+=row[1:10];acceleration+=row[10:19];want_applied+=want['applied'];want_acceleration+=want['acceleration']
            metrics={'applied':compare(applied,want_applied,5e-10,'smooth applied force'),'acceleration':compare(acceleration,want_acceleration,5e-9,'smooth acceleration')};separate={}
            for field,values,wanted,budget in [('applied',applied,want_applied,5e-10),('acceleration',acceleration,want_acceleration,5e-9)]:
                for label,indices in [('hinge',range(7)),('slide',range(7,9))]:separate[field+'.'+label]=compare([values[i*9+j] for i in range(20) for j in indices],[wanted[i*9+j] for i in range(20) for j in indices],budget,field+' '+label)
            record['modes'].append({'mode':mode,'native':native,'metrics':metrics,'coordinate_metrics':separate,'clip_counts_exact':True,'all360_actual_floating_fields_finite':True,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(directory/'outputs.bin')});save()
        require(payloads[0]==payloads[1]==payloads[2],'complete owned-mode output/status/count byte identity');require(all(sha(path)==digest for path,digest in dependencies.items()) and sha(compiler)==record['compiler_sha256'],'frozen source/compiler/reference at completion');record.update(passed=True,native_modes_byte_identical=True,cases=20,output_shape='20×(int64 status,18F64,two int64 counts)',old_component_gates_repeated=False)
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'modes':[x['native'] for x in record['modes']]}))
if __name__=='__main__':main()
