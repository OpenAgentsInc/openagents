#!/usr/bin/env python3
"""Exercise the isolated private Computers service with a synthetic owner."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import shlex
from cluster_secrets import identity,secret

ROOT=Path('/space/qualification-11814')
HOST='oa-space-service-computers-ezxz4mgdsq-uc.a.run.app'
OWNER='space-qualification-11814'
TURNS={'turn':'cwc-turn-start','memory':'cwc-turn-memory',
    'memory-replacement':'cwc-turn-memory-replacement','network':'cwc-turn-network'}


def network_command():
    script='''import http.client,json,socket
results=[]
for host,port in [("10.219.14.10",6443),("10.219.14.10",8443),("10.219.14.10",9810),("10.219.14.11",9800),("10.220.1.1",9800),("10.219.16.3",5432)]:
    try:
        with socket.create_connection((host,port),timeout=2): pass
        results.append({"host":host,"port":port,"denied":False})
    except OSError: results.append({"host":host,"port":port,"denied":True})
try:
    c=http.client.HTTPConnection("169.254.169.254",timeout=2)
    c.request("GET","/computeMetadata/v1/instance/id",headers={"Metadata-Flavor":"Google"})
    r=c.getresponse();results.append({"metadata_status":r.status,"denied":r.status in (401,403)})
    c.close()
except OSError: results.append({"metadata":"unreachable","denied":True})
print(json.dumps(results))
raise SystemExit(0 if all(r["denied"] for r in results) else 1)
'''
    return 'python3 -c '+shlex.quote(script)


def request(method,path,data=None,allow_refusal=False):
    value=secret('qualification',identity())
    connection=http.client.HTTPSConnection(HOST,timeout=30)
    try:
        connection.request(method,path,body=json.dumps(data).encode() if data is not None else None,
            headers={'Authorization':'Bearer '+value['computers_token'],'X-OpenAgents-Account':OWNER,'Content-Type':'application/json'})
        response=connection.getresponse(); raw=response.read(1048577)
        if len(raw)>1048576: raise RuntimeError('The Computers response exceeded its size limit.')
        try: value=json.loads(raw) if raw else None
        except json.JSONDecodeError: raise RuntimeError(f'Computers endpoint returned non-JSON HTTP {response.status}. Check its private route before retrying.')
        if not 200<=response.status<300:
            if allow_refusal and 400<=response.status<500: return {'http_status':response.status,'error':value}
            raise RuntimeError(f'Computers request returned HTTP {response.status}: {json.dumps(value)[:1000]}')
        return value
    finally: connection.close()


def save(name,value):
    path=ROOT/(name+'.json');path.write_text(json.dumps(value,indent=2)+'\n');path.chmod(0o600)


def main(action,run):
    global ROOT
    key='space-11814-runtime-small-1'
    if run!='legacy':
        ROOT=ROOT/run
        key='space-11814-'+run
    if os.getuid()!=0: raise RuntimeError('Run qualification through the owned control identity.')
    ROOT.mkdir(mode=0o700,exist_ok=True)
    if action=='probe':
        print(json.dumps({'capabilities':request('GET','/v1/capabilities'),'computers':request('GET','/v1/computers')}));return
    if action in ('create','recover-create'):
        path=ROOT/'cwc-seed.json'
        if action=='recover-create':
            pending=json.loads(path.read_text())
            if pending!={'state':'submission_pending','idempotency_key':key}: raise RuntimeError('Recovery requires the exact pending outer idempotency key.')
        else:
            if path.exists(): raise RuntimeError('Inspect the saved computer before another create.')
            save('cwc-seed',{'state':'submission_pending','idempotency_key':key})
        value=request('POST','/v1/computers',{'size':'small','idempotency_key':key},allow_refusal=True)
        save('cwc-seed',value);print(json.dumps({key:value.get(key) for key in ('id','state','desired','image','size','last_error','http_status','error') if key in value}));return
    computer=json.loads((ROOT/'cwc-seed.json').read_text())
    identifier=computer.get('id')
    if not isinstance(identifier,str) or not identifier.replace('-','').replace('_','').isalnum(): raise RuntimeError('Inspect the create receipt before continuing.')
    base='/v1/computers/'+identifier
    if action=='status': value=request('GET',base)
    elif action in ('stop','wake'): value=request('POST',base+'/'+action,{})
    elif action=='delete': value=request('DELETE',base)
    elif action.endswith('-status'):
        key=TURNS[action.removesuffix('-status')]
        turn=json.loads((ROOT/(key+'.json')).read_text())
        turn_id=turn.get('id')
        if not isinstance(turn_id,str) or not turn_id.replace('-','').replace('_','').isalnum(): raise RuntimeError('Inspect the turn receipt before continuing.')
        value=request('GET',base+'/turns/'+turn_id)
        save(key+'-status',value)
    else:
        key=TURNS[action]
        if (ROOT/(key+'.json')).exists(): raise RuntimeError('Inspect the saved turn before submitting another.')
        if action=='turn':
            canary=secrets.token_hex(32);save('memory-canary',{'sha256':hashlib.sha256(canary.encode()).hexdigest()})
            command="set -eu; env QA_MEMORY_11814="+canary+" setsid sh -c 'while :; do sleep 3600; done' </dev/null >/dev/null 2>&1 & pid=$!; echo $pid >/tmp/oa-11814-memory.pid; cat /proc/sys/kernel/random/boot_id; echo $pid; awk '{print $22}' /proc/$pid/stat; tr '\\0' '\\n' </proc/$pid/environ | sed -n 's/^QA_MEMORY_11814=//p' | tr -d '\\n' | sha256sum"
        elif action in ('memory','memory-replacement'):
            command="set -eu; pid=$(cat /tmp/oa-11814-memory.pid); cat /proc/sys/kernel/random/boot_id; echo $pid; awk '{print $22}' /proc/$pid/stat; tr '\\0' '\\n' </proc/$pid/environ | sed -n 's/^QA_MEMORY_11814=//p' | tr -d '\\n' | sha256sum"
        else: command=network_command()
        save(key,{'state':'submission_pending'})
        value=request('POST',base+'/turns',{'command':command,'timeout_seconds':45})
        save(key,value)
    print(json.dumps({key:value[key] for key in ('id','state','desired','resource','turns','attempts','last_error','exit_code','stdout','stderr','error') if key in value}))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',choices=['legacy','encrypted-1','capacity-2','capacity-3'],default='legacy')
    parser.add_argument('action',choices=['probe','create','recover-create','status','turn','turn-status','memory','memory-status','memory-replacement','memory-replacement-status','network','network-status','stop','wake','delete'])
    args=parser.parse_args()
    main(args.action,args.run)
