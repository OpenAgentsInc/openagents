#!/usr/bin/env python3
"""NEW held-state paired-controller gate; cached qualified samples, no SDK/plant."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-owned-controller'
EXPERIMENT_SHA='1e1b6c6b9a872d3bc096ae3260634305d1056c9318939b27b7258741f28594ea'
ROW=struct.Struct('<10q30d')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite_samples(raw):
    require(len(raw)==15*144*8,'exact15x144sample bytes');values=struct.unpack('<2160d',raw);require(all(math.isfinite(v) for v in values),'every sample word finite');return values
def expected_pairs():
    result=[]
    for sample in range(15):
        for pattern in range(3):result.append((0,sample,pattern,-1));result.extend((1,sample,pattern,tick) for tick in range(25))
    result.extend((10+fault,sample,-1,-1) for sample in range(15) for fault in range(5));return result
def decode(raw):
    require(len(raw)==2490*ROW.size,'complete2490-event transcript');records=[]
    for i in range(2490):
        row=ROW.unpack_from(raw,i*ROW.size);meta=row[:10];values=row[10:];require(all(math.isfinite(v) for v in values),'every decoded actual/expected field finite beforemax');require(meta[4] in (0,1) and 0<=meta[5]<=8 and meta[6] in (0,1) and 0<=meta[7]<=7 and 0<=meta[8]<=6 and 0<=meta[9]<=8,'logical status/ready/clip/prepare fields');records.append((meta,values))
    return records
def compare(raw,profile):
    records=decode(raw);keys=expected_pairs();require(len(keys)==1245,'closed paired event inventory');bounds=[profile['tolerances']['torque_atol']]*7+[profile['tolerances']['gripper_atol']]*2+[profile['tolerances']['state_f64_atol']]*21;maximum=[0.]*30;regular_status={};refused_status={};reset_status={};first=None
    for i,key in enumerate(keys):
        (left,x),(right,y)=records[2*i:2*i+2];require(tuple(left[:4])==tuple(right[:4])==key and left[4]==0 and right[4]==1,'exactsample/action/tick/event coverage/order');require(left[:4]+left[5:]==right[:4]+right[5:],'status/ready/clipping/prep decisions must agree exactly')
        group=reset_status if key[0]==0 else regular_status if key[0]==1 else refused_status;group[str(left[5])]=group.get(str(left[5]),0)+1
        if left[5]!=0:
            require(left[7]==left[8]==right[7]==right[8]==0 and all(v==0 for v in x[:9]+y[:9]),'refused outputs are canonicalzero torque/gripper/clipcounts')
        for j,(a,b) in enumerate(zip(x,y)):
            error=abs(a-b);maximum[j]=max(maximum[j],error)
            if error>bounds[j] and first is None:first={'pair':i,'event':key,'field':j,'SDK':a,'owned':b,'error':error,'atol':bounds[j]}
        if key[0]>=10:
            expected=5 if key[0] in (10,11) else 2 if key[0]==12 else 1 if key[0]==13 else 7
            if key[0]!=10 and left[9]!=0:expected=7 if key[0]!=13 else 1
            require(left[5]==expected,'declared refusal sibling status preserved')
    require(first is None,('frozen paired controller numerical bound failed',first))
    return {'passed':True,'paired_events':1245,'normal_held_pairs':1125,'normal_reset_pairs':45,'refusal_sibling_pairs':75,'torque_values':8715,'gripper_values':2490,'state_values':26145,'torque_max_abs_error':max(maximum[:7]),'gripper_max_abs_error':max(maximum[7:9]),'state_max_abs_error':max(maximum[9:]),'per_field_max_abs_error':maximum,'status_ready_clipcounts_exact':True,'regular_step_status_counts':regular_status,'reset_status_counts':reset_status,'declared_refusal_status_counts':refused_status,'all_regular_steps_successful':regular_status=={'0':1125}}
def guard_checks():
    raw=bytearray(2490*ROW.size);baseline=ROW.pack(*([0]*10),*([0.]*30))
    for i in range(2490):raw[i*ROW.size:(i+1)*ROW.size]=baseline
    require(len(decode(bytes(raw)))==2490,'finite guard control')
    refused=0
    for bad in (float('nan'),float('inf'),-float('inf')):
        copy=raw[:];struct.pack_into('<d',copy,(2490-1)*ROW.size+80+29*8,bad)
        try:decode(copy)
        except RuntimeError:refused+=1
        else:raise RuntimeError('late nonfinite field accepted')
    for bad in (raw[:-1],raw+bytes(ROW.size)):
        try:decode(bad)
        except RuntimeError:refused+=1
        else:raise RuntimeError('bad extent accepted')
    return {'passed':True,'finite_control':True,'refused_nonfinite_or_extent_cases':refused,'poison_field':'lastrecord,laststatefield','both_SDK_expected_and_owned_actual_share_guard':True}
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--qualified-samples',type=Path,default=Path('/tmp/panda-owned-sample-native-first'));a=p.parse_args();out=a.output.resolve();require(not out.exists(),'preserve earlier paired proof');out.mkdir(parents=True);require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 profile');require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'exact pre-execution experiment');profile=json.loads((FIXTURE/'experiment.json').read_text());require(profile['frozen_before_execution'] and profile['case_samples']==15 and profile['held_calls']==25 and profile['normal_pairs']==45 and profile['tolerances']=={'torque_atol':2e-8,'gripper_atol':2e-12,'state_f64_atol':1e-10,'rtol':0,'status_ready_clipping_counts_exact':True},'unchanged corpus/budgets')
    owned_record_path=a.qualified_samples/'record.json';owned_record=json.loads(owned_record_path.read_text());require(sha(owned_record_path)==profile['observed_inputs']['owned_previous_record_sha256'] and owned_record['passed'] and len(owned_record['modes'])==3,'qualified owned generator receipt')
    for name,h in profile['qualified_owned_sample_sources'].items():require(sha(ROOT/name)==h,'frozen qualified source/input '+name)
    sdk_path=ROOT/profile['observed_inputs']['SDK_sample_file'];owned_path=ROOT/profile['observed_inputs']['owned_sample_file'];require(sha(sdk_path)==profile['observed_inputs']['SDK_sha256'] and sha(owned_path)==profile['observed_inputs']['owned_sha256'],'paired input hashes');sdk=sdk_path.read_bytes();owned=owned_path.read_bytes();finite_samples(sdk);finite_samples(owned);require(sha(a.qualified_samples/'debug/samples.f64')==profile['observed_inputs']['owned_sha256'] and (a.qualified_samples/'debug/samples.f64').read_bytes()==owned,'owned cached output exact bundled alias')
    inputs=struct.pack('<4I',0x31434f50,15,3,25)+struct.pack('<7d',*profile['home_radians'])+b''.join(struct.pack('<7d',*row['action']) for row in profile['action_patterns'])+sdk+owned;(out/'input.bin').write_bytes(inputs);native_source=ROOT/'psionic/tests/robotics/panda_owned_controller_native.c';record={'schema':'psionic.panda-owned-controller.qualification.v1','passed':False,'profile_sha256':EXPERIMENT_SHA,'source_sha256':{str(native_source.relative_to(ROOT)):sha(native_source),str(Path(__file__).relative_to(ROOT)):sha(Path(__file__))},'controller_sources':profile['controller_sources'],'qualified_owned_generator_record_sha256':sha(owned_record_path),'SDK_sha256':sha(sdk_path),'owned_sha256':sha(owned_path),'input_sha256':sha(out/'input.bin'),'numeric_profile':profile['numeric_profile'],'scope':profile['outcome_scope'],'simulation_steps':0,'SDK_calls':0,'NN_calls':0,'commands':[],'modes':[],'decoder_guard_checks':guard_checks()}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        began=time.monotonic();r=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'argv':[str(x) for x in argv],'exit':r.returncode,'seconds':time.monotonic()-began,'stderr_bytes':len(r.stderr),'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode==0 and not r.stderr,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')));return r.stdout
    began=time.monotonic();save()
    try:
        blobs=[];cc=os.environ.get('CC','cc')
        for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();source=d/'panda-owned-sample.c';header=d/'panda-owned-sample.h';source.write_bytes((a.qualified_samples/source_mode/source.name).read_bytes());header.write_bytes((a.qualified_samples/source_mode/header.name).read_bytes());expected=profile['cached_C_header']['release_C_sha256' if source_mode=='release' else 'debug_C_sha256'];require(sha(source)==expected and sha(header)==profile['cached_C_header']['header_sha256'],'unchanged qualified Coax C/header identity');binary=d/'native';command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(d),source,native_source,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'paired.bin']));require(native['passed'] and native['events']==2490 and native['physics_steps']==0,'new native complete event ledger');blob=(d/'paired.bin').read_bytes();blobs.append(blob);metrics=compare(blob,profile);record['modes'].append({'mode':mode,'native':native,'cached_C_sha256':sha(source),'cached_header_sha256':sha(header),'binary_sha256':sha(binary),'transcript_sha256':sha(d/'paired.bin'),'metrics':metrics});save()
        require(blobs[0]==blobs[1]==blobs[2],'named-profile event transcripts byte identical');require({str(native_source.relative_to(ROOT)):sha(native_source),str(Path(__file__).relative_to(ROOT)):sha(Path(__file__))}==record['source_sha256'],'new harness sources changed');record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'paired_cases':45,'held_calls_each':25,'record':str(out/'record.json'),'seconds':record['seconds']}))
if __name__=='__main__':main()
