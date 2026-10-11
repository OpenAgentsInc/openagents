"""Bound the single build group and fail on source or close-time guard errors."""
import os,signal,subprocess,sys,time,shutil
import proof
from build_checks import frozen

def run():
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    proof.require({'build','memory','disk'}<=leases and not {'gpu','quiet'}&leases,'build/memory/disk only required')
    proof.require(os.environ.get('OPENAGENTS_BUILD_DISK_GB')=='2','explicit 2 GiB build disk declaration required')
    out=proof.HERE/'build-1';out.mkdir(exist_ok=False)
    record={'status':'failed','cap_bytes':24*1024**3,'minimum_start_bytes':36*1024**3,'minimum_available_bytes':12*1024**3,
            'deadline_seconds':1800,'minimum_start_disk_bytes':31*1024**3,'minimum_disk_bytes':25*1024**3,
            'declared_build_disk_gib':2,'explicit_disk_lease_gib':4,'output_limit_bytes':2*1024**3,'cargo_jobs':2,'rust_toolchain':'1.97.1','target':'@@CLEF_TARGET@@',
            'source_manifest_sha256':proof.sha(proof.HERE/'source-manifest.json'),'sources_rechecked':False}
    def save():proof.write(out/'guard.json',record)
    save();process=monitor=None;previous={};start=time.monotonic()
    def interrupt(signum,frame):raise KeyboardInterrupt(str(signum))
    try:
        frozen();record['harness_manifest_sha256']=proof.sha(proof.HERE/'qualification-source.json')
        record['preflight']=proof.bench.available_memory()
        proof.require(record['preflight']['available_memory_bytes']>=record['minimum_start_bytes'],'build available-memory preflight refused')
        record['disk_preflight']=proof.disk_check(start=True,build=True)
        for sig in (signal.SIGINT,signal.SIGTERM):previous[sig]=signal.signal(sig,interrupt)
        with (out/'stdout.log').open('x') as log:
            process=subprocess.Popen([sys.executable,str(proof.HERE/'build_checks.py')],cwd=proof.ROOT,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            monitor=proof.bench.OwnedProcessGuard(process,record['cap_bytes'],out/'memory.jsonl',minimum_available_memory_bytes=record['minimum_available_bytes'])
            record.update(status='running',pid=process.pid);save()
            while process.poll() is None:
                monitor.guard();proof.disk_check()
                if time.monotonic()-start>1800:raise TimeoutError('build reached 1800-second deadline')
                time.sleep(.5)
            monitor.guard();proof.require(process.returncode==0,'build child failed');record.update(status='complete',exit=process.returncode)
    except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        if monitor is not None:
            try:monitor.close()
            except BaseException as error:record.update(status='failed',cleanup_error=str(error))
            record['memory']=monitor.receipt()
            if monitor.failure or monitor.cleanup_error or not monitor.stopped:record.update(status='failed',close_time_guard_error=monitor.failure or monitor.cleanup_error or 'owned group remains')
        elif process is not None:
            try:proof.bench.stop_owned_group(process)
            except BaseException as error:record.update(status='failed',cleanup_error=str(error))
        try:
            frozen();record['disk_after_cleanup']=proof.disk_check();record['sources_rechecked']=True
        except BaseException as error:record.update(status='failed',source_error=str(error))
        for sig,handler in previous.items():signal.signal(sig,handler)
        record['seconds']=time.monotonic()-start;save()
    if record['status']=='complete':
        binaries=proof.load(out/'binaries.json');server=binaries['artifacts']['server']
        proof.write(proof.HERE/'candidate.json',{'status':'complete','source_manifest_unchanged':True,
            'binary':server['path'],'binary_sha256':server['sha256'],'build_guard':str(out/'guard.json'),
            'build_guard_sha256':proof.sha(out/'guard.json'),'source_manifest':str(proof.HERE/'source-manifest.json'),
            'source_manifest_sha256':record['source_manifest_sha256'],'fixture_binary':binaries['artifacts']['psionic-serve']})
    print(__import__('json').dumps(record,indent=2));return 0 if record['status']=='complete' else 1
if __name__=='__main__':raise SystemExit(run())
