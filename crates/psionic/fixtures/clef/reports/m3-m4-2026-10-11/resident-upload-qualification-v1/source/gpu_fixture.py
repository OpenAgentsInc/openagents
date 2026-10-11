"""Run only the small ignored positional-upload fixture after a separate grant."""
import os,signal,subprocess,time
import proof

def run():
    proof.require_leases();proof.clean_inherited_environment();proof.frozen();candidate=proof.candidate()
    binary=candidate['fixture_binary'];proof.require(proof.sha(binary['path'])==binary['sha256'],'fixture binary changed')
    out=proof.HERE/'gpu-fixture-1';out.mkdir(exist_ok=False)
    row={'status':'failed','cap_bytes':4*1024**3,'minimum_start_bytes':16*1024**3,'minimum_available_bytes':12*1024**3,'deadline_seconds':180,
         'fixture_binary':binary,'candidate_receipt_sha256':proof.sha(proof.HERE/'candidate.json'),'sources_rechecked':False}
    command=[binary['path'],'qwen35::clef_metal::tests::weight_upload::positional_weight_upload_preserves_resident_bytes_and_gemm_outputs','--exact','--ignored','--test-threads=1','--nocapture']
    row['command']=command;process=monitor=None;previous={};start=time.monotonic()
    def save():proof.write(out/'guard.json',row)
    def interrupted(signum,frame):raise KeyboardInterrupt(str(signum))
    save()
    try:
        proof.disk_check(start=True);row['preflight']=proof.bench.available_memory()
        proof.require(row['preflight']['available_memory_bytes']>=row['minimum_start_bytes'],'fixture memory preflight refused')
        for sig in (signal.SIGINT,signal.SIGTERM):previous[sig]=signal.signal(sig,interrupted)
        env={k:v for k,v in os.environ.items() if not k.startswith('PSIONIC_CLEF_')};env.update(RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
        with (out/'stdout.log').open('x') as log:
            process=subprocess.Popen(command,cwd=proof.ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            monitor=proof.bench.OwnedProcessGuard(process,row['cap_bytes'],out/'memory.jsonl',minimum_available_memory_bytes=row['minimum_available_bytes'])
            while process.poll() is None:
                monitor.guard();proof.disk_check()
                if time.monotonic()-start>180:raise TimeoutError('fixture reached 180-second deadline')
                time.sleep(.1)
            monitor.guard();row['exit']=process.returncode
            proof.require(process.returncode==0 and 'test result: ok. 1 passed; 0 failed; 0 ignored;' in (out/'stdout.log').read_text(),'fixture failed or did not execute exactly once')
            row.update(status='complete',tests_passed=1,tests_failed=0,tests_ignored=0)
    except BaseException as error:row.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        if monitor is not None:
            try:monitor.close()
            except BaseException as error:row.update(status='failed',cleanup_error=str(error))
            row['memory']=monitor.receipt()
            if monitor.failure or monitor.cleanup_error or not monitor.stopped:row.update(status='failed',close_time_guard_error=monitor.failure or monitor.cleanup_error or 'owned group remains')
        elif process is not None:
            try:proof.bench.stop_owned_group(process)
            except BaseException as error:row.update(status='failed',cleanup_error=str(error))
        try:proof.frozen();proof.candidate();proof.disk_check();row['sources_rechecked']=True
        except BaseException as error:row.update(status='failed',postcheck_error=str(error))
        for sig,handler in previous.items():signal.signal(sig,handler)
        row['seconds']=time.monotonic()-start;save()
    print(__import__('json').dumps(row,indent=2));return 0 if row['status']=='complete' else 1
if __name__=='__main__':raise SystemExit(run())
