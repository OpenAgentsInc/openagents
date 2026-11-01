#!/usr/bin/env python3
"""Reproducible OFFLINE tracking gate against the committed one-shot capture.

This additive adapter emits current Coax, invokes only the SDK-free own/shadow
binary and records a NEW compiler context. It never loads/links the SDK.
Original acquisition/native qualification receipts remain unchanged.
"""
from pathlib import Path
import argparse,hashlib,json,os,platform,struct
from panda_joint_pd_tracking_sdk_reference import ROOT,FIXTURE,TESTS,SOURCE,Run,identity,frozen,decode_sdk,decode_own,require,verify_sources,flags
from panda_joint_pd_tracking_native_reference import metrics,reader_guards
from panda_joint_pd_tracking_audit import trajectory_metrics,goals
from panda_pose_ik_native_reference import graph
NATIVE=TESTS/'panda_joint_pd_tracking_native.c'
def plan(compiler,out):
    rows=[]
    for mode,extra in[('debug',[]),('release',['--release'])]:rows.append(dict(name='emit-'+mode,argv=[str(compiler),'c',str(SOURCE.relative_to(ROOT)),'--library',*extra,'--header',str(out/mode/'panda-joint-pd-tracking.h')]))
    rows.append(dict(name='guard-once',argv=[str(out/'sanitized/native'),'--guards-only']))
    for mode in['debug','release','sanitized']:rows.append(dict(name='own-shadow-'+mode,argv=[str(out/mode/'native'),str(out/'input.bin'),str(FIXTURE/'sdk-trace.bin'),str(out/mode/'own-trace.bin')]))
    return rows

def offline(args):
    require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualifiedDarwinarm64');e,initial=frozen();reference=args.reference.resolve();capture_path=reference/'sdk-reference.json';capture=json.loads(capture_path.read_text());trace=reference/'sdk-trace.bin';require(capture['passed']and capture['trace']==identity(trace)and capture['experiment_sha256']=='cfe805220004838440a4ea92c258880597ca5de42b7360ca7102281e242b3612','original committed capture');sdk=decode_sdk(trace.read_bytes(),e);verify_sources(capture['source']);require(identity(NATIVE)==capture['new_ABI_guard_source'],'exactqualifiednative effects source');out=args.output.resolve();require(not out.exists(),'never overwrite oldattempt');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-joint-pd-tracking.offline-native.v1',passed=False,SDK_calls=0,SDK_linked=False,capture_repeated=False,commands=[],compiler=identity(compiler),capture_record=identity(capture_path),capture_trace=identity(trace),source={str(p.relative_to(ROOT)):identity(p)for p in set(graph(SOURCE))|{NATIVE,Path(__file__).resolve(),TESTS/'panda_joint_pd_tracking_native_reference.py',TESTS/'panda_joint_pd_tracking_sdk_reference.py',TESTS/'panda_joint_pd_tracking_audit.py'}},budgets=e['parity']);run=Run(out,record);run.save();outputs={}
    try:
        (out/'input.bin').write_bytes(initial);require(identity(out/'input.bin')==capture['input'],'samefrozeninitialinputs');emissions={}
        for mode,extra in[('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'panda-joint-pd-tracking.c';h=d/'panda-joint-pd-tracking.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={m:dict(source=identity(c),header=identity(h))for m,(c,h)in emissions.items()};run.save()
        # Build allnewcontextbinaries. Onlythenrunoneguardentry followedbythe
        # threefreshcorpusentries; guardentryneverloadsSDKorsimulatesepisodes.
        binaries={}
        for mode,cmode,numeric in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir(exist_ok=True);c,h=emissions[cmode];binary=d/'native';run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*flags(),'-Wno-unused-parameter',*numeric,'-c',c,'-o',d/'coax.o']);run.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*flags(),*numeric,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',binary]);binaries[mode]=binary
        env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');record['new_ABI_guards']=json.loads(run.command('guard-once',[binaries['sanitized'],'--guards-only'],env));require(record['new_ABI_guards']['passed']and record['new_ABI_guards']['guard_refusals']==89 and record['new_ABI_guards']['alias_cases']==4,'guardinventory')
        for mode,binary in binaries.items():
            output=out/mode/'own-trace.bin';report=json.loads(run.command('own-shadow-'+mode,[binary,out/'input.bin',trace,output],env));require(report['passed']and report['SDK_calls']==0 and report['cases']==4 and report['steps_per_branch']==3000 and report['own_branches']==2 and report['record_bytes']==1216 and report['fpcr']==0,'new replayinventory');raw=output.read_bytes();own=decode_own(raw,e);record.setdefault('modes',{})[mode]=dict(native=report,output=identity(output),binary=identity(binary),metrics=metrics(own,sdk,e));outputs[mode]=raw;run.save()
        require(outputs['debug']==outputs['release']==outputs['sanitized'],'completefreshmodebytesexact');record['reader_guards']=reader_guards(trace.read_bytes(),outputs['debug'],e);refs=struct.unpack('<63084d',(FIXTURE/'reference-states.f64').read_bytes());own=decode_own(outputs['debug'],e);quality=[trajectory_metrics(own[c*750:(c+1)*750],refs[c*751*21:(c+1)*751*21],e['cases'][c])for c in range(4)];record['quality_metrics']=quality;record['quality_hypotheses']=goals(quality);verify_sources(record['source']);verify_sources(capture['source']);require(identity(capture_path)==record['capture_record']and identity(trace)==record['capture_trace']and identity(compiler)==record['compiler'],'all reference/compileridentitiesunchanged');record.update(passed=True,modes_byte_identical=True,output_sha256=hashlib.sha256(outputs['debug']).hexdigest(),quality_failures_separate=True)
    except BaseException as error:record['failure']=repr(error);raise
    finally:run.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),quality=record['quality_hypotheses'])))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,default=ROOT/'coax/bin/coax');p.add_argument('--reference',type=Path,default=FIXTURE);p.add_argument('--output',type=Path,required=True);p.add_argument('--plan-only',action='store_true');args=p.parse_args()
    if args.plan_only:print(json.dumps(dict(SDK_calls=0,SDK_linked=False,commands=plan(args.compiler,args.output))));return
    offline(args)
if __name__=='__main__':main()
