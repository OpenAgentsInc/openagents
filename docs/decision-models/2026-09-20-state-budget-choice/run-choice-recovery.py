"""Measure unchanged state rungs on the admitted choice release after complete OOD jobs."""
import importlib.util
import email.utils
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time
import urllib.request

spec=importlib.util.spec_from_file_location('quiet_common',Path(__file__).with_name('run-quiet-kev.py'))
common=importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
ROOT,OUT=common.ROOT,common.OUT
SERVER=Path('/tmp/openagents-supervision/root-target/debug/lev-serve')
RUNNER=OUT/'sweep-choice-recovery-target/debug/lev-state-sweep-choice-recovery'
ROWS=OUT/'state-budget-lev-choice-recovery.jsonl'
ORIGINAL_ROWS=OUT/'state-budget-lev-choice.jsonl'
BOUND=10800
PREFIX=OUT/'state-sweep-choice-recovery'
path=lambda suffix:Path(str(PREFIX)+suffix)


def prerequisites(signature):
    report=json.loads((OUT/'ood-queue-result.json').read_text())
    if report.get('failure') or len(report.get('jobs',[]))!=3:
        raise RuntimeError('OOD queue did not complete all three jobs')
    for job in report['jobs']:
        measured=json.loads((OUT/job['result_file']).read_text())
        if job.get('controller_exit')!=0 or measured.get('exit_code')!=0 or measured.get('failure'):
            raise RuntimeError('OOD job has terminal failure: '+str(job))
    evidence=[]
    for case,count,door,adapter in [('band',130,'lev-band','fmadapter-levband-9799725'),('permutation',130,'lev-permutation','fmadapter-levperm-9799725'),('external',160,'lev-base','')]:
        filename=OUT/('ood-'+case+'.jsonl')
        rows=[json.loads(line) for line in filename.read_text().splitlines()]
        models=json.loads((OUT/('ood-'+case+'-models.json')).read_text())
        cards=models.get('models',[])
        expected_model='lev-base' if case=='external' else 'lev-adapted'
        # Base runtime publishes its compatibility prefix; attached adapters publish the full signature.
        # Verify the exact retained card and rows; never rewrite either identity.
        expected_signature='9799725' if case=='external' else signature
        if len(cards)!=1 or cards[0].get('name')!=expected_model or cards[0].get('adapter','')!=adapter or cards[0].get('base_model_signature')!=expected_signature:
            raise RuntimeError(case+' OOD retained card differs from expected runtime identity')
        keys={(r['split'],r['item_id']) for r in rows}
        if len(rows)!=count or len(keys)!=count:
            raise RuntimeError(case+' OOD rows incomplete or duplicated; no choice run started')
        suite_name='external-v1' if case=='external' else 'coder-turns-v1'
        suite=json.loads((ROOT/'crates/gym/suites'/ (suite_name+'.json')).read_text())
        partitions=('calibration','development') if case=='external' else ('development',)
        expected={(i['partition'],i['id']) for i in suite['items'] if i['partition'] in partitions}
        if keys!=expected:
            raise RuntimeError(case+' OOD item set differs from requested workload')
        for row in rows:
            identity=row['door_identity']
            if row['door']!=door or identity.get('adapter','')!=adapter or identity.get('base_model_signature')!=expected_signature or identity.get('model')!=expected_model or row['suite_digest']!=suite['digest']:
                raise RuntimeError(case+' OOD row identity differs from requested measurement')
            if row.get('samples')!=8 or row.get('seed_base')!=0 or row.get('permutation') is not None:
                raise RuntimeError(case+' OOD estimator differs from requested measurement')
        evidence.append({'case':case,'rows':len(rows),'sha256':common.digest(filename)})
    return evidence


def main():
    with path('-claim.json').open('x') as claim:
        json.dump({'controller_pid':os.getpid(),'started_utc':common.utc()},claim)
    result={'started_utc':common.utc(),'overall_deadline_seconds':BOUND,'per_request_timeout_seconds':120,
            'retained_rows_before':384,'reason':'Two-attempt recovery: retain the complete first six rungs unchanged, including failures, and rerun every request in the five-rung suffix after fresh server startup.'}
    server=run=None
    started=time.monotonic()
    last_sample=0.0

    def interrupted(signum,frame):
        raise InterruptedError('controller received signal '+str(signum))

    def sample(stage):
        nonlocal last_sample
        now=time.monotonic()
        if now-last_sample>=5:
            with path('-load.jsonl').open('a') as log:
                log.write(json.dumps({'utc':common.utc(),'stage':stage,'load':os.getloadavg()})+'\n')
            last_sample=now
        if now-started>=BOUND:
            raise TimeoutError('three-hour recovery deadline exceeded; partial checkpoint retained')

    signal.signal(signal.SIGTERM,interrupted)
    signal.signal(signal.SIGINT,interrupted)
    try:
        if ROWS.exists() or any(path(s).exists() for s in ('-server.log','-run.log','-result.json','-admission.jsonl')):
            raise RuntimeError('earlier choice evidence exists; no overwrite or automatic resume')
        manifest_path=ROOT/'crates/lev/manifests/lev-adapted-v1.json'
        manifest=json.loads(manifest_path.read_text())
        signature=manifest['base']['signature']
        original_bytes=ORIGINAL_ROWS.read_bytes()
        lines=original_bytes.splitlines(keepends=True)
        original=[json.loads(line) for line in lines]
        if len(original)!=704:
            raise RuntimeError('original failed attempt does not have all 704 retained rows')
        first=original[:384]
        expected_rungs=['unbudgeted','output 512','output 256','commands 3','turns 8','turns 6']
        if [first[i]['rung'] for i in range(0,384,64)]!=expected_rungs:
            raise RuntimeError('first six complete rungs differ from planned recovery boundary')
        for i,rung in enumerate(expected_rungs):
            if any(row['rung']!=rung for row in first[i*64:(i+1)*64]):
                raise RuntimeError('retained rung is incomplete or mixed')
        result['composite_provenance']={'original_rows':str(ORIGINAL_ROWS),
            'original_rows_sha256':common.digest(ORIGINAL_ROWS),'original_row_count':704,
            'original_controller_exit':'unknown; terminal evidence was suppressed by cleanup failure',
            'original_retained_line_range':[1,384],'retained_rungs':expected_rungs,
            'retained_timeout_and_413':True,'rerun_request_count':80,
            'rerun_rungs':['turns 4','turns 6, message 1024','production: turns 6, message 768','turns 6, message 512','turns 4, message 512'],
            'selection_rule':'Rerun the entire suffix from the first failing rung, including every original answer; no selection by correctness.',
            'original_failed_rows_preserved':True}
        result['original_rows_sha256']=result['composite_provenance']['original_rows_sha256']
        result['recovery_rungs']=result['composite_provenance']['rerun_rungs']
        policy_ref=manifest['policySnapshot']
        policy_source=Path(policy_ref['source']).expanduser()
        if not policy_source.is_absolute():policy_source=manifest_path.parent/policy_source
        policy_bytes=policy_source.read_bytes()
        snapshot=json.loads(policy_bytes)
        policy_hash=common.digest(policy_source)
        issued=email.utils.parsedate_to_datetime(snapshot['issued']).timestamp()
        window=min(snapshot['freshnessWindowSeconds'],policy_ref['freshnessWindowSeconds'])
        remaining=issued+window-time.time()
        required=80*120+900+20
        result['policy_preflight']={'source':str(policy_source),'sha256':policy_hash,
            'remaining_seconds':remaining,'required_remaining_seconds':required,
            'budget_components_seconds':{'requests':80*120,'startup':900,'cleanup':20}}
        path('-policy-source-before.json').write_bytes(policy_bytes)
        common.write(path('-progress.json'),result)
        if remaining<required:
            raise RuntimeError('policy freshness is insufficient for all 80 requests plus startup; renew through documented publisher before a fresh recovery attempt')
        result['ood_prerequisites']=prerequisites(signature)
        result.update(source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                      git_status=subprocess.check_output(['git','status','--short'],cwd=ROOT,text=True),
                      chip=subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip(),
                      macos=subprocess.check_output(['sw_vers'],text=True).strip(),load_before=os.getloadavg())
        common.snapshot(path('-processes-before.txt'))
        helper=ROOT/'swift/lev-bridge/.build/release/lev-bridge'
        env=dict(os.environ,LEV_OS_BUILD=subprocess.check_output(['sw_vers','-buildVersion'],text=True).strip(),STATE_SWEEP_ROWS=str(ROWS))
        for key in ('LEV_BRIDGE_BIN','LEV_BRIDGE_ALLOW_UNSIGNED','LEV_BRIDGE_DEADLINE_MS'):
            env.pop(key,None)
        env['LEV_BRIDGE_BIN']=str(helper)
        result['environment_overrides']={k:env[k] for k in ('LEV_OS_BUILD','LEV_BRIDGE_BIN','STATE_SWEEP_ROWS')}
        result['environment_removed']=['LEV_BRIDGE_ALLOW_UNSIGNED','LEV_BRIDGE_DEADLINE_MS']
        check=subprocess.run(['codesign','--verify','--strict',str(helper)],capture_output=True,text=True)
        result['bridge_codesign']={'command':check.args,'exit_code':check.returncode,'stdout':check.stdout,'stderr':check.stderr}
        if check.returncode:
            raise RuntimeError('real bridge code signature failed')
        baseline_path=ROOT/'docs/decision-models/2026-09-20-state-budget-lev.jsonl'
        retained=[json.loads(line) for line in baseline_path.read_text().splitlines()]
        key=lambda r:(r['rung'],r['state'],r['family'])
        baseline={key(r):r for r in retained}
        if len(retained)!=704 or len(baseline)!=704:
            raise RuntimeError('base comparison does not contain 704 unique rows')
        sources=[SERVER,RUNNER,helper,manifest_path,baseline_path,OUT/'state-sweep-choice-recovery/src/main.rs',OUT/'state-sweep-choice-recovery/Cargo.toml',OUT/'state-sweep-choice-recovery/Cargo.lock',OUT/'choice-recovery-response-body.patch']
        result['hashes']={str(p):common.digest(p) for p in sources}
        result['runner_change']='Recovery-only stderr API response-body logging; original runner unchanged. Questions, caps, timeout, retry policy and row serialization unchanged.'
        result['manifest']=manifest
        artifact=manifest['artifact'];package=Path(artifact['path']).expanduser()
        for filename,field in [('adapter_weights.bin','sha256'),('metadata.json','metadataSha256')]:
            actual=common.digest(package/filename);result['hashes'][str(package/filename)]=actual
            if actual!=artifact[field]:
                raise RuntimeError('choice artifact differs from manifest: '+filename)
        with socket.socket() as check:
            check.bind(('127.0.0.1',11456))
        server_command=[str(SERVER),'--manifest',str(manifest_path),'--port','11456','--policy-refresh','off','--admission-record',str(path('-admission.jsonl'))]
        run_command=[str(RUNNER)]
        result.update(server_command=server_command,run_command=run_command)
        common.write(path('-progress.json'),result)
        with path('-server.log').open('x') as log:
            server=subprocess.Popen(server_command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        common.write(path('-pid.json'),{'controller_pid':os.getpid(),'server_pid':server.pid})
        startup_end=time.monotonic()+900
        while True:
            sample('startup')
            if server.poll() is not None:
                raise RuntimeError('server exited during startup: '+str(server.returncode))
            if time.monotonic()>startup_end:
                raise TimeoutError('15-minute startup deadline exceeded')
            try:
                with urllib.request.urlopen('http://127.0.0.1:11456/v1/models',timeout=1) as response:
                    models=json.load(response)
            except OSError:
                time.sleep(.5);continue
            common.write(path('-models.json'),models)
            cards=models.get('models',[])
            if len(cards)!=1:
                raise RuntimeError('unexpected model inventory')
            card=cards[0]
            if card.get('manifest',{}).get('release')!='lev-adapted@1' or card.get('adapter')!='lev-adapted@1' or card.get('base_model_signature')!=signature or card['manifest'].get('artifact_sha256')!=artifact['sha256']:
                raise RuntimeError('model card does not name the requested choice artifact')
            policy=card.get('policy') or {}
            if policy.get('state')!='current' or policy.get('snapshot_sha256')!=policy_hash or policy.get('expires_in_seconds',0)<80*120+20:
                raise RuntimeError('startup policy does not match retained current policy with sufficient remaining lifetime')
            if card.get('samples')!=8 or card.get('seed_base')!=0 or card.get('pool_width')!=4:
                raise RuntimeError('model card estimator differs from base sweep')
            break
        # Seed only after all preflight checks. Keep exact original bytes, not rewritten JSON.
        with ROWS.open('xb') as seed:seed.write(b''.join(lines[:384]))
        result['seeded_prefix_sha256']=common.digest(ROWS)
        common.write(path('-progress.json'),result)
        with path('-run.log').open('x') as log:
            run=subprocess.Popen(run_command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        common.write(path('-pid.json'),{'controller_pid':os.getpid(),'server_pid':server.pid,'run_pid':run.pid})
        while run.poll() is None:
            sample('sweep')
            if server.poll() is not None:
                raise RuntimeError('server exited during sweep')
            time.sleep(1)
        result['exit_code']=run.returncode
        rows=[json.loads(line) for line in ROWS.read_text().splitlines()]
        keys={key(r) for r in rows}
        result.update(rows=len(rows),rows_sha256=common.digest(ROWS))
        if len(rows)!=704 or len(keys)!=704 or keys!=set(baseline):
            raise RuntimeError('choice sweep does not cover all 704 unique base keys')
        for row in rows:
            before=baseline[key(row)]
            for field in ('caps','truth','state_bytes','suite','partition'):
                if row[field]!=before[field]:
                    raise RuntimeError('paired state differs in '+field+' for '+str(key(row)))
            if row['door']!='lev-adapted@1':
                raise RuntimeError('incorrect choice door label')
            ms=row.get('latency_ms')
            if not isinstance(ms,(int,float)) or not math.isfinite(ms) or ms<0:
                raise RuntimeError('invalid choice timing')
        if run.returncode:
            raise RuntimeError('sweep returned nonzero status')
        result['validated_complete_paired_sweep']=True
        # Runner checkpoint serialization may round floats. Restore the retained prefix byte-for-byte.
        recovered_lines=ROWS.read_bytes().splitlines(keepends=True)
        if len(recovered_lines)!=704:
            raise RuntimeError('recovery line count differs after validation')
        for index,line in enumerate(recovered_lines[:384]):
            row=json.loads(line)
            before=original[index]
            for field in before:
                if field!='latency_ms' and row[field]!=before[field]:
                    raise RuntimeError('runner changed retained prefix field '+field)
        ROWS.write_bytes(b''.join(lines[:384]+recovered_lines[384:]))
        result['rows_sha256']=common.digest(ROWS)
        result['measurement_completed_utc']=common.utc()
        common.write(path('-measurement-result.json'),result)

    except Exception as error:
        result['failure']=repr(error)
    finally:
        signal.signal(signal.SIGTERM,signal.SIG_IGN)
        signal.signal(signal.SIGINT,signal.SIG_IGN)
        result['measurement_phase_finished_utc']=common.utc()
        if run is not None:result['observed_runner_returncode_before_cleanup']=run.poll()
        common.write(path('-measurement-result.json'),result)
        common.stop(run);common.stop(server)
        result['cleanup_errors'] = list(common.CLEANUP_ERRORS)
        if common.CLEANUP_ERRORS:
            result.setdefault('failure','owned process cleanup was incomplete')
        result.update(seconds=time.monotonic()-started,load_after=os.getloadavg(),finished_utc=common.utc())
        common.snapshot(path('-processes-after.txt'))
        common.write(path('-result.json'),result)
        print(json.dumps(result),flush=True)
    return 1 if result.get('failure') else 0


if __name__=='__main__':
    sys.exit(main())
