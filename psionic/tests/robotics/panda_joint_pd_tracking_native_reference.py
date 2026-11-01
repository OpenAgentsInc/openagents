#!/usr/bin/env python3
"""SDK-free fresh-feedback and recorded-command-shadow replay of ONE new capture.

Consumes once-emitted Coax C/H and immutable SDK data. No SDK/capture/old gates
or independent gravity/target generation is invoked by this driver.
"""
from pathlib import Path
import argparse,hashlib,json,os,platform,time
from panda_joint_pd_tracking_sdk_reference import ROOT,FIXTURE,TESTS,Run,identity,frozen,decode_sdk,decode_own,finite,require,verify_sources,flags
from panda_joint_pd_tracking_audit import trajectory_metrics,goals
NATIVE=TESTS/'panda_joint_pd_tracking_native.c'
def compare(values,expected,atol,label):
    finite(values,len(expected),label+'actual');finite(expected,len(values),label+'reference');require(values,label+'nonempty');error=max(abs(a-b)for a,b in zip(values,expected));require(error<=atol,label+'fixedbudget');return dict(passed=True,values=len(values),max_abs_error=error,atol=atol,rtol=0)
def metrics(own,sdk,e):
    b=e['parity'];result={}
    for name,indices,shadow_indices,tolerance in [('hinge_q',range(101,108),range(130,137),b['hinge_position_atol_radians']),('slide_q',range(108,110),range(137,139),b['slide_position_atol_metres']),('hinge_v',range(110,117),range(139,146),b['hinge_rate_atol_radians_per_second']),('slide_v',range(117,119),range(146,148),b['slide_rate_atol_metres_per_second'])]:
        wanted=[row[i]for row in sdk for i in indices];result[name]=compare([row[i]for row in own for i in indices],wanted,tolerance,name);result['shadow_'+name]=compare([row[i]for row in own for i in shadow_indices],wanted,tolerance,'shadow'+name)
    result['all_controller_fields']=compare([x for row in own for x in row[39:74]],[x for row in sdk for x in row[39:74]],b['controller_and_actuator_force_atol'],'all35Pjpfields');result['actuator_force']=compare([x for row in own for x in row[92:101]],[x for row in sdk for x in row[92:101]],b['controller_and_actuator_force_atol'],'Pa_generalized_vs_unitmoment_SDK_scalar')
    for a,s in zip(own,sdk):
        require(a[120:130]==s[120:130],'exact allstage/code/reference/Pjp/Pa clipmetadata');require(a[150:152]==s[128:130],'recorded-command shadow clipcounts exact');require(a[18:39]==s[18:39]and a[74:83]==tuple(a[67:74])+tuple(s[81:83])and a[83:92]==s[83:92],'fixedreference/controltransport/external choices');require(abs(a[119]-s[119])<=b['clock_atol_seconds'],'logical clocks')
    return result

def reader_guards(sdkraw,ownraw,e):
    count=0
    for raw,decode,offsets in [(sdkraw,decode_sdk,[0,92*8,119*8]),(ownraw,decode_own,[0,92*8,119*8,1040+17*8])]:
        width=1040 if decode==decode_sdk else 1216;bad=[raw[:-1],raw+b'\0']
        import struct
        for value in[float('nan'),float('inf'),-float('inf')]:
            for offset in offsets:
                copy=bytearray(raw);struct.pack_into('<d',copy,16+2999*width+offset,value);bad.append(bytes(copy))
        for offset,value in [(960,2),(968,748),(976,1),(984,1),(992,8),(1000,1),(1008,-1),(1016,1),(1024,10),(1032,3)]:
            copy=bytearray(raw);struct.pack_into('<q',copy,16+2999*width+offset,value);bad.append(bytes(copy))
        for value in bad:
            try:decode(value,e)
            except RuntimeError:count+=1
            else:raise RuntimeError('new tracking reader admitted poison')
    require(count==45,'all45 actualSDK/own lateextent/IEEE/metadata poisons');return dict(passed=True,cases=count,SDK_calls=0)

def replay(args,e,input_bytes):
    reference=args.reference.resolve();capture_path=reference/'record.json';capture=json.loads(capture_path.read_text());trace=reference/'sdk-trace.bin';require(capture['passed']and capture['experiment_sha256']==e.get('experiment_sha256','cfe805220004838440a4ea92c258880597ca5de42b7360ca7102281e242b3612')and capture['trace']==identity(trace),'one completed unchanged NEW SDK acquisition');sdkraw=trace.read_bytes();sdk=decode_sdk(sdkraw,e);verify_sources(capture['source']);require(capture['new_ABI_guards']['passed']and capture['new_ABI_guards']['SDK_calls']==0,'new ABI guardphase alreadyqualifiedonce');require(identity(NATIVE)==capture['new_ABI_guard_source']and identity(reference/'guard-native')==capture['guard_binary'],'reuseexactnewnative source/guardbinary');out=args.output.resolve();require(not out.exists(),'preserve any replayattempt');out.mkdir(parents=True);record=dict(schema='psionic.panda-joint-pd-tracking.native.v1',passed=False,SDK_calls=0,NN_calls=0,old_gates_repeated=False,commands=[],capture_record=identity(capture_path),capture_trace=identity(trace),experiment_sha256=capture['experiment_sha256'],input=identity(reference/'input.bin'),source={str(NATIVE.relative_to(ROOT)):identity(NATIVE),str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__),str((TESTS/'panda_joint_pd_tracking_audit.py').relative_to(ROOT)):identity(TESTS/'panda_joint_pd_tracking_audit.py')},emissions=capture['emissions'],emissions_reused_without_repeating_codegen=True,ABI_guards_carried_without_repeat=capture['new_ABI_guards'],budgets=e['parity']);run=Run(out,record);run.save();outputs={}
    try:
        require((reference/'input.bin').read_bytes()==input_bytes,'exact captured initialtransport');require(all(v==dict(source=identity(reference/m/'panda-joint-pd-tracking.c'),header=identity(reference/m/'panda-joint-pd-tracking.h'))for m,v in capture['emissions'].items()),'both onceemittedCoax contexts unchanged')
        for mode in['debug','release','sanitized']:
            d=out/('native-'+mode);d.mkdir();c_mode='release'if mode=='release'else'debug';c=reference/c_mode/'panda-joint-pd-tracking.c';h=reference/c_mode/'panda-joint-pd-tracking.h';binary=d/'native'
            if mode=='sanitized':
                # Exactly the same sanitizer-mode binary already passed guards;
                # run only its NEW full corpus entry, never the guards again.
                binary=reference/'guard-native';record['sanitized_binary_reused_without_recompilation']=True
            else:
                numeric=['-O2']if mode=='release'else['-O0','-g'];run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*flags(),'-Wno-unused-parameter',*numeric,'-c',c,'-o',d/'coax.o']);run.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*flags(),*numeric,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',binary])
            env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');report=json.loads(run.command('run-own-shadow-'+mode,[binary,reference/'input.bin',trace,d/'own-trace.bin'],env));require(report['passed']and report['SDK_calls']==0 and report['cases']==4 and report['steps_per_branch']==3000 and report['own_branches']==2 and report['record_bytes']==1216 and report['fpcr']==0,'exactnewown/shadowwork inventory');raw=(d/'own-trace.bin').read_bytes();own=decode_own(raw,e);record.setdefault('modes',{})[mode]=dict(native=report,binary=identity(binary),output=identity(d/'own-trace.bin'),metrics=metrics(own,sdk,e),all_actual_finite_words=414000,all_SDK_reference_finite_words=360000);outputs[mode]=raw;run.save()
        require(outputs['debug']==outputs['release']==outputs['sanitized'],'allcompleteown/shadow files byteexact');record['new_reader_guards']=reader_guards(sdkraw,outputs['debug'],e);references=finite(__import__('struct').unpack('<63084d',(FIXTURE/'reference-states.f64').read_bytes()),63084,'all frozen targets beforequality');debug=decode_own(outputs['debug'],e);quality=[trajectory_metrics(debug[c*750:(c+1)*750],references[c*751*21:(c+1)*751*21],e['cases'][c])for c in range(4)];record['quality_metrics']=quality;record['quality_hypotheses']=goals(quality);record['quality_failure_does_not_change_parity_budget']=True;verify_sources(capture['source']);verify_sources(record['source']);require(all(v==dict(source=identity(reference/m/'panda-joint-pd-tracking.c'),header=identity(reference/m/'panda-joint-pd-tracking.h'))for m,v in capture['emissions'].items())and identity(reference/'guard-native')==capture['guard_binary'],'final reusedC/H andguardbinary identities');require(identity(capture_path)==record['capture_record']and identity(trace)==record['capture_trace'],'unchanged completed reference');record.update(passed=True,modes_byte_identical=True,output_sha256=hashlib.sha256(outputs['debug']).hexdigest(),own_steps_per_profile=6000,total_new_own_steps=18000)
    except BaseException as error:record['failure']=repr(error);raise
    finally:
        record['partial_or_complete_outputs']={str(p.relative_to(out)):dict(identity=identity(p),complete_records=max(0,p.stat().st_size-16)//1216,trailing_bytes=max(0,p.stat().st_size-16)%1216)for p in out.glob('native-*/own-trace.bin')};run.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),quality=record['quality_hypotheses'],assertions={m:v['native']['assertions']for m,v in record['modes'].items()})))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args();require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualifiedDarwinarm64');e,initial=frozen();replay(args,e,initial)
if __name__=='__main__':main()
