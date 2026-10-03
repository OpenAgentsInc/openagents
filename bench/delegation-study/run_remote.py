#!/usr/bin/env python3
"""Run one full-tool Claude attempt in a source-only, offline Linux namespace."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path,PurePosixPath
import signal
import shutil
import stat
import subprocess
import tarfile
import time
import uuid
from seed_manifest import cargo_features, feature_check_command, validate_seed
from candidate import write_manifest, MAX_PAYLOAD_BYTES
from capture_limits import Limits, Reader, Writer


def sha(path,limits=None):
    h=hashlib.sha256()
    with Path(path).open('rb') as source:
        reader=Reader(source,limits) if limits else source
        while chunk:=reader.read(1024*1024):h.update(chunk)
    return h.hexdigest()


def snapshot(root,limits=None):
    limits=limits or Limits()
    result={}
    for path in root.rglob('*'):
        limits.check()
        relative=path.relative_to(root).as_posix()
        if relative == '.git' or relative.startswith('.git/'):
            continue
        information=path.lstat()
        mode=information.st_mode
        limits.entry(information.st_size if stat.S_ISREG(mode) else 0)
        if stat.S_ISLNK(mode):
            result[relative]={'kind':'symlink','target':os.readlink(path),'mode':stat.S_IMODE(mode)}
        elif stat.S_ISREG(mode):
            result[relative]={'kind':'file','sha256':sha(path,limits),'bytes':path.stat().st_size,'mode':stat.S_IMODE(mode)}
        elif not stat.S_ISDIR(mode):
            raise ValueError('The candidate contains a special file')
    return result


def export(archive,root):
    total=0
    with tarfile.open(archive,'r:*') as source:
        members=source.getmembers()
        if len(members)>100_000:raise ValueError('The archive has too many entries')
        for item in members:
            path=PurePosixPath(item.name)
            if path.is_absolute() or '..' in path.parts or '.git' in path.parts or item.issym() or item.islnk() or not (item.isfile() or item.isdir()):
                raise ValueError('The source archive contains an unsupported entry')
            total+=item.size
            if total>2*1024*1024*1024:raise ValueError('The source archive is too large')
        source.extractall(root,members=members,filter='data')
    return {'archive_entries':len(members),'source_bytes':total}



def initialize_snapshot(workspace,home,source_commit):
    env=dict(os.environ,HOME=str(home),GIT_CONFIG_GLOBAL='/dev/null',GIT_CONFIG_NOSYSTEM='1',GIT_AUTHOR_NAME='Benchmark',GIT_AUTHOR_EMAIL='benchmark@example.invalid',GIT_COMMITTER_NAME='Benchmark',GIT_COMMITTER_EMAIL='benchmark@example.invalid',GIT_AUTHOR_DATE='2000-01-01T00:00:00Z',GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
    # Large snapshots must not leave background maintenance outside the namespace.
    git=['git','-c','gc.auto=0','-c','maintenance.auto=false']
    commands=[['init','-q','--initial-branch=main'],['config','--local','gc.auto','0'],['config','--local','maintenance.auto','false'],['add','-f','.'],['commit','-qm','Benchmark source snapshot '+source_commit]]
    for command in commands:
        subprocess.run(git+command,cwd=workspace,env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
    return subprocess.check_output(git+['rev-parse','HEAD'],cwd=workspace,env=env,text=True).strip()


def base_arguments(workspace,home,toolchain=None):
    args=['/usr/bin/bwrap','--die-with-parent','--new-session','--unshare-all','--cap-drop','ALL','--clearenv']
    for path in ['/usr','/bin','/lib','/lib64']:
        if Path(path).exists():args+=['--ro-bind',path,path]
    args+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--dir','/run','--dir','/home','--dir','/home/executor','--dir','/opt']
    for path in ['/etc/ssl','/etc/ld.so.cache','/etc/localtime','/etc/alternatives']:
        if Path(path).exists():args+=['--ro-bind',path,path]
    args+=['--bind',str(workspace),'/workspace','--bind',str(home),'/home/executor','--setenv','HOME','/home/executor','--setenv','PATH','/usr/local/bin:/usr/bin:/bin','--setenv','LANG','C.UTF-8','--setenv','GIT_CONFIG_NOSYSTEM','1','--setenv','GIT_CONFIG_GLOBAL','/dev/null','--setenv','GIT_TERMINAL_PROMPT','0']
    if toolchain:
        for source,destination in toolchain.get('read_only_mounts',[]):
            args+=['--ro-bind',source,destination]
        for key,value in toolchain.get('environment',{}).items():
            args+=['--setenv',key,value]
    return args


def arguments(workspace,home,tools,inputs,provider_socket,toolchain=None):
    return base_arguments(workspace,home,toolchain)+['--ro-bind',str(tools),'/opt/study','--ro-bind',str(inputs),'/opt/run','--ro-bind',str(provider_socket),'/run/provider.sock','--chdir','/workspace','--','/usr/bin/python3','/opt/study/run_inner.py']


def ledger_summary(path):
    admitted,finished,refused={},{},[]
    for line in path.read_text().splitlines():
        item=json.loads(line)
        phase=item.get('phase')
        if phase=='admitted':
            if item['call_id'] in admitted:raise ValueError('Duplicate provider admission')
            admitted[item['call_id']]=item
        elif phase=='finished':
            if item['call_id'] in finished:raise ValueError('Duplicate provider completion')
            finished[item['call_id']]=item
        elif phase=='refused':refused.append(item)
        else:raise ValueError('Invalid provider receipt')
    if finished.keys()-admitted.keys():raise ValueError('Provider completion has no admission')
    unknown=[key for key in admitted if key not in finished or finished[key].get('cost_usd') is None]
    known=sum(item['cost_usd'] for item in finished.values() if item.get('cost_usd') is not None)
    models={}
    for key,item in finished.items():
        model=admitted[key]['model']
        target=models.setdefault(model,{'requests':0,'reported_cost_usd':0.0,'usage':{}})
        target['requests']+=1
        target['reported_cost_usd']+=item.get('cost_usd') or 0
        for name,count in item.get('usage',{}).items():
            if type(count) is int:target['usage'][name]=target['usage'].get(name,0)+count
    return {'admitted_requests':len(admitted),'finished_requests':len(finished),'refused_requests':len(refused),'unknown_cost_requests':len(unknown),'unfinished_requests':len(admitted.keys()-finished.keys()),'reported_cost_usd':known,'cost_usd':None if unknown else known,'models':models,'refused_models':sorted(set(x['requested_model'] for x in refused if x.get('requested_model'))),'accounting_complete':not unknown}


def start_broker(config,output):
    meter=dict(config['provider_meter'])
    meter['run_id']=config['run_id']
    meter_path=output/'provider-config.json'
    meter_path.write_text(json.dumps(meter,sort_keys=True))
    socket=output/'provider.sock'
    records=output/'provider-calls.jsonl'
    log=(output/'provider-stderr.log').open('wb')
    process=subprocess.Popen(['/usr/bin/python3',str(Path(__file__).with_name('broker.py')),'--socket',str(socket),'--credential-file',config['credential_file'],'--records',str(records),'--config',str(meter_path)],stdout=subprocess.DEVNULL,stderr=log,start_new_session=True)
    log.close()
    try:
        for _ in range(250):
            if socket.exists():return process,socket,records
            if process.poll() is not None:raise ValueError('The provider broker did not start')
            time.sleep(0.02)
        raise ValueError('The provider broker did not become ready')
    except BaseException:
        if process.poll() is None:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        raise



def native_summary(path,exit_code,timed_out):
    results=[];errors=set()
    path=Path(path)
    try:
        if not path.exists():raise ValueError('missing_stream')
        if path.stat().st_size>128*1024*1024:raise ValueError('oversized_stream')
        with path.open() as handle:
            while line:=handle.readline(4*1024*1024+1):
                if len(line)>4*1024*1024:raise ValueError('oversized_event')
                try:item=json.loads(line)
                except ValueError:errors.add('invalid_json');continue
                if not isinstance(item,dict):errors.add('non_object_event');continue
                if item.get('type')=='result':results.append(item)
                if len(results)>10:raise ValueError('too_many_results')
    except (OSError,ValueError) as error:
        errors.add(str(error) if type(error) is ValueError else 'stream_read_error')
    result=results[-1] if results else {}
    cost=result.get('total_cost_usd')
    if type(cost) not in (int,float) or not math.isfinite(cost) or cost<0:
        cost=None;errors.add('invalid_or_missing_native_cost')
    usage=result.get('usage')
    if not isinstance(usage,dict):usage=None;errors.add('invalid_or_missing_native_usage')
    models=result.get('modelUsage')
    if not isinstance(models,dict) or any(not isinstance(k,str) or not isinstance(v,dict) for k,v in models.items()):
        models=None;errors.add('invalid_or_missing_model_usage')
    complete=bool(result) and result.get('is_error') is False and exit_code==0 and not timed_out and not errors
    return dict(model_completed=complete,cost_usd=cost,usage=usage,model_usage=models,served_models=sorted(models or {}),result_count=len(results),trace_parse_errors=sorted(errors))


def run(config,output):
    began=time.monotonic()
    os.umask(0o077)
    output.mkdir(parents=True,exist_ok=False)
    row={'schema':'openagents.delegation.native-attempt.v1','status':'incomplete','accepted':False,'source_commit':config['source_commit'],'phases':{}}
    process=None
    provider=None
    provider_records=None
    try:
        if str(uuid.UUID(config['run_id'])) != config['run_id']:
            raise ValueError('The run identity must be a canonical UUID')
        row['run_id']=config['run_id']
        features=cargo_features(config.get('cargo_features',[]))
        if features:row['cargo_features']=features
        binary=Path(config['binary']).resolve(strict=True)
        actual=sha(binary)
        if actual!=config['binary_sha256']:raise ValueError('The native CLI hash changed')
        version=subprocess.check_output([str(binary),'--version'],text=True,timeout=20).strip()
        if version!=config['binary_version']:raise ValueError('The native CLI version changed')
        row.update(cli_sha256=actual,cli_version=version)
        archive=Path(config['source_archive'])
        if sha(archive)!=config['source_archive_sha256']:raise ValueError('The source archive hash changed')
        workspace,home,inputs=output/'workspace',output/'home',output/'inputs'
        for path in (workspace,home,inputs):path.mkdir()
        start=time.monotonic();row.update(export(archive,workspace));row['source_archive_bytes']=archive.stat().st_size
        original=snapshot(workspace,Limits(config.get('capture_limits')))
        row['capture_limits']=dict(Limits.DEFAULTS,**config.get('capture_limits',{}))
        (output/'original.json').write_text(json.dumps(original,sort_keys=True))
        row['phases']['export_s']=time.monotonic()-start
        if config.get('initialize_git',True):
            start=time.monotonic()
            row['snapshot_commit']=initialize_snapshot(workspace,home,config['source_commit'])
            row['phases']['git_snapshot_s']=time.monotonic()-start
        if config.get('target_seed'):
            start=time.monotonic()
            seed=Path(config['target_seed'])
            validate_seed(seed,config['target_seed_manifest_sha256'],config['source_commit'],config['source_archive_sha256'],config.get('toolchain',{}).get('environment',{}),features=features)
            # The trusted builder has filtered this seed to the base-only Cargo graph.
            subprocess.run(['cp','-a','--reflink=auto',str(seed/'target'),str(home/'target')],check=True,stderr=subprocess.PIPE)
            row['phases']['target_seed_copy_s']=time.monotonic()-start
            row['target_seed_manifest_sha256']=config['target_seed_manifest_sha256']
        prompt=Path(config['prompt_file']).read_bytes()
        if hashlib.sha256(prompt).hexdigest()!=config['prompt_sha256']:raise ValueError('The prompt hash changed')
        if features and feature_check_command(features).encode() not in prompt:
            raise ValueError('The task prompt lacks its bound Cargo feature check command')
        prompt=('Benchmark run ID: '+config['run_id']+'\n\n').encode()+prompt
        (inputs/'prompt.txt').write_bytes(prompt)
        row['delivered_prompt_sha256']=hashlib.sha256(prompt).hexdigest()
        argv=[str(binary),'-p','--output-format','stream-json','--verbose','--dangerously-skip-permissions','--no-session-persistence','--model',config['model'],'--effort',config['effort'],'--max-budget-usd',str(config.get('cli_budget_usd',2))]
        if config.get('system_file'):
            text=Path(config['system_file']).read_bytes()
            if hashlib.sha256(text).hexdigest()!=config['system_sha256']:raise ValueError('The system prompt hash changed')
            (inputs/'system.txt').write_bytes(text)
            argv+=['--system-prompt-file','/opt/run/system.txt']
        if config.get('tools') is not None:argv+=['--tools',config['tools']]
        (inputs/'request.json').write_text(json.dumps({'argv':argv}))
        row.update(argv=argv,model=config['model'],effort=config['effort'],prompt_sha256=config['prompt_sha256'],source_archive_sha256=config['source_archive_sha256'])
        runtime=output/'runtime'
        runtime.mkdir()
        for name in ('bridge.py','run_inner.py'):
            shutil.copyfile(Path(__file__).with_name(name),runtime/name)
        provider,provider_socket,provider_records=start_broker(config,output)
        command=arguments(workspace,home,runtime,inputs,provider_socket,config.get('toolchain'))
        row['phases']['prepare_s']=time.monotonic()-began
        start=time.monotonic()
        with (output/'events.jsonl').open('wb') as stdout,(output/'stderr.log').open('wb') as stderr:
            process=subprocess.Popen(command,stdout=stdout,stderr=stderr,start_new_session=True)
            try:row['exit_code']=process.wait(timeout=config.get('timeout_s',900))
            except subprocess.TimeoutExpired:
                row['timed_out']=True
                os.killpg(process.pid,signal.SIGTERM)
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
                row['exit_code']=process.returncode
        row['phases']['executor_s']=time.monotonic()-start
        start=time.monotonic()
        capture_started=time.monotonic()
        final=snapshot(workspace,Limits(config.get('capture_limits')))
        changed={p:{'before':original.get(p),'after':final.get(p)} for p in sorted(original.keys()|final.keys()) if original.get(p)!=final.get(p)}
        (output/'changes.json').write_text(json.dumps(changed,sort_keys=True,indent=2)+'\n')
        limits=Limits(config.get('capture_limits'))
        limits.deadline=capture_started+limits.config['timeout_s']
        with (output/'candidate.tar.gz').open('wb') as raw, tarfile.open(fileobj=Writer(raw,limits.deadline,MAX_PAYLOAD_BYTES),mode='w:gz') as retained:
            for p in changed:
                if p not in final:continue
                path=workspace/p;member=retained.gettarinfo(path,arcname=p)
                limits.entry(member.size)
                if member.isfile():
                    with path.open('rb') as source:retained.addfile(member,Reader(source,limits))
                else:retained.addfile(member)
        row['candidate_manifest_sha256'],row['candidate_payload_sha256']=write_manifest(output,config['source_commit'],config['source_archive_sha256'],changed,limits.deadline,config.get('capture_limits'))
        row['changed_paths']=list(changed)
        row['phases']['capture_s']=time.monotonic()-start
        row['cli_hash_after']=sha(binary)
        if row['cli_hash_after']!=actual:raise ValueError('The native CLI changed during the attempt')
        row['status']='complete'
        # Independent acceptance is deliberately a separate trusted phase.
    except Exception as error:
        row.update(status='infrastructure_error',error_type=type(error).__name__,error=str(error)[:400])
    finally:
        if process is not None and process.poll() is None:
            os.killpg(process.pid,signal.SIGKILL);process.wait()
        if provider is not None:
            drain=time.monotonic()
            try:
                while row.get('error_type')!='InterruptedError' and provider.poll() is None and time.monotonic()-drain<320:
                    accounting=ledger_summary(provider_records)
                    if not accounting['unfinished_requests']:break
                    time.sleep(0.1)
            except Exception as error:
                row['provider_accounting_error']=type(error).__name__
            finally:
                provider.terminate()
                try:provider.wait(timeout=5)
                except subprocess.TimeoutExpired:provider.kill();provider.wait()
            row['phases']['provider_drain_s']=time.monotonic()-drain
            try:
                row['provider_accounting']=ledger_summary(provider_records)
                row['provider_cost_usd']=row['provider_accounting']['cost_usd']
                row['provider_accounting_complete']=row['provider_accounting']['accounting_complete']
            except Exception as error:
                row.update(provider_cost_usd=None,provider_accounting_complete=False,provider_accounting_error=type(error).__name__)
        try:row.update(native_summary(output/'events.jsonl',row.get('exit_code'),row.get('timed_out',False)))
        except Exception as error:row.update(model_completed=False,cost_usd=None,trace_parse_errors=['summary_'+type(error).__name__])
        row['execution_closed']=(process is None or process.poll() is not None) and (provider is None or provider.poll() is not None)
        row['total_retained_wall_s']=time.monotonic()-began
        (output/'result.json').write_text(json.dumps(row,indent=2)+'\n')
    return row


def main():
    def interrupted(_signal,_frame):raise InterruptedError('The executor wrapper was interrupted')
    signal.signal(signal.SIGTERM,interrupted)
    signal.signal(signal.SIGINT,interrupted)
    parser=argparse.ArgumentParser()
    parser.add_argument('config')
    parser.add_argument('output')
    args=parser.parse_args()
    row=run(json.loads(Path(args.config).read_text()),Path(args.output).resolve())
    print(json.dumps({k:row.get(k) for k in ['status','model_completed','exit_code','cost_usd','total_retained_wall_s','error_type','error']}))


if __name__=='__main__':main()
