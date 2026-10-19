#!/usr/bin/env python3
"""Publish bounded operational measurements from the owned control node."""
import argparse
from datetime import datetime, timezone
import http.client
import json
import os
import socket
import subprocess
import time
import xml.etree.ElementTree as ET
from cluster_secrets import identity, secret
from database_check import credential
from storage_inventory import protocol, unfinished_uploads

PROJECT='openagentsgemini'
PREFIX='custom.googleapis.com/openagents/space/'
SERVICES={'space-api','space-admin-api','space-cluster-controller','space-credmgr-authority',
    'space-data-plane','space-internal-api','space-credential-manager','space-node-api',
    'space-node-ctl','space-node-manager','space-runtime-manager','space-sandbox-manager',
    'space-template-manager','space-volume-manager'}


def kube(*args):
    result=subprocess.run(['/usr/local/bin/k3s','kubectl',*args],capture_output=True,check=True,timeout=20)
    if len(result.stdout)>8388608: raise RuntimeError('The operational response exceeded its size limit.')
    return json.loads(result.stdout)


# Computers-layer states the native measurements can't see (#12147): a computer
# held in `turn` by a command outcome nobody could confirm, a computer in
# `unknown`, and a lifecycle request still `requested` with no provider answer.
COMPUTERS_SQL="""SELECT json_build_object(
 'computers_stranded',(SELECT count(*) FROM computers.computers c WHERE c.phase='turn' AND c.desired<>'deleted'
   AND EXISTS (SELECT 1 FROM computers.turns t WHERE t.computer=c.id AND t.error LIKE 'We couldn''t confirm%')),
 'oldest_stranded_turn_seconds',(SELECT COALESCE(max(extract(epoch FROM now()-t.finished_at)),0) FROM computers.computers c
   JOIN computers.turns t ON t.computer=c.id WHERE c.phase='turn' AND c.desired<>'deleted' AND t.error LIKE 'We couldn''t confirm%'),
 'computers_unknown',(SELECT count(*) FROM computers.computers WHERE phase='unknown' AND desired<>'deleted'),
 'oldest_requested_lifecycle_seconds',(SELECT COALESCE(max(extract(epoch FROM now()-o.created_at)),0) FROM computers.lifecycle_operations o
   WHERE o.disposition='requested' AND EXISTS (SELECT 1 FROM computers.computers c WHERE c.phase<>'deleted' AND strpos(c.record::text,o.resource)>0)));"""


def computers_measurements():
    value=credential();saved=secret('computers-database',identity())
    environment={**os.environ,'PGHOST':value['private_ip'],'PGDATABASE':'space_computers','PGUSER':'space_computers',
        'PGPASSWORD':saved['password'],'PGSSLMODE':'verify-ca','PGSSLROOTCERT':'/space/config/postgres-ca.crt',
        'PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
    result=subprocess.run(['psql','-X','-A','-t','-w','--set=ON_ERROR_STOP=1','-c',COMPUTERS_SQL],env=environment,capture_output=True,check=True,timeout=15)
    return {name:float(number) for name,number in json.loads(result.stdout).items()}


def collect():
    measurements={};errors=[]
    try:
        nodes=kube('get','nodes','-o','json')['items']
        expected={'oa-space-service-control','oa-space-service-worker'}
        measurements['nodes_ready']=sum(any(c['type']=='Ready' and c['status']=='True' for c in n['status']['conditions']) for n in nodes if n['metadata']['name'] in expected)
        pods=kube('-n','space','get','pods','-l','openagents-native-service=true','-o','json')['items']
        pods=[p for p in pods if p['status']['phase']!='Succeeded' and not p['metadata'].get('deletionTimestamp')]
        ready={name for p in pods if any(c['type']=='Ready' and c['status']=='True' for c in p['status'].get('conditions',[]))
            for name in SERVICES if p['metadata']['name'].startswith(name+'-')}
        measurements['services_ready_ratio']=len(ready)/len(SERVICES)
        summary=kube('get','--raw','/api/v1/nodes/oa-space-service-worker/proxy/stats/summary')
        disk=summary['node']['runtime']['imageFs']
        if disk['capacityBytes']!=322122547200: raise RuntimeError('The measured worker filesystem differs from its data disk.')
        measurements['worker_disk_free_bytes']=disk['availableBytes']
    except (OSError,ValueError,KeyError,RuntimeError,subprocess.SubprocessError): errors.append('cluster')
    try:
        value=credential()
        environment={**os.environ,'PGHOST':value['private_ip'],'PGDATABASE':'space','PGUSER':'space_ro',
            'PGPASSWORD':value['role_passwords']['RO'],'PGSSLMODE':'verify-ca',
            'PGSSLROOTCERT':'/space/config/postgres-ca.crt','PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
        sql="""SELECT json_build_object(
 'oldest_operation_seconds',(SELECT COALESCE(max(extract(epoch FROM now()-created_at)),0) FROM public_operations WHERE state IN ('queued','running')),
 'oldest_transition_seconds',(SELECT COALESCE(max(extract(epoch FROM now()-lifecycle_changed_at)),0) FROM sandboxes WHERE lifecycle IN ('creating','starting','restoring','pausing','resuming','suspending','stopping','checkpointing')),
 'sandbox_count',(SELECT count(*) FROM sandboxes),
 'orphan_snapshot_locations',(SELECT count(*) FROM snapshot_storage_locations WHERE orphaned_at IS NOT NULL),
 'oldest_orphan_seconds',(SELECT COALESCE(max(extract(epoch FROM now()-orphaned_at)),0) FROM snapshot_storage_locations WHERE orphaned_at IS NOT NULL));"""
        result=subprocess.run(['psql','-X','-A','-t','-w','--set=ON_ERROR_STOP=1','-c',sql],env=environment,capture_output=True,check=True,timeout=15)
        measurements.update(json.loads(result.stdout))
    except (OSError,ValueError,KeyError,subprocess.SubprocessError): errors.append('database')
    try:
        measurements.update(computers_measurements())
    except (OSError,ValueError,KeyError,subprocess.SubprocessError): errors.append('computers')
    try:
        value=secret('object-credentials',identity())
        client=protocol()
        pending=[unfinished_uploads(client.Store(value,PROJECT+'-oa-space-service-'+purpose))
            for purpose in ('templates','snapshots')]
        measurements['unfinished_uploads']=sum(item['unfinished_uploads'] for item in pending)
        measurements['oldest_unfinished_upload_seconds']=max(item['oldest_unfinished_upload_seconds'] for item in pending)
    except (OSError,ValueError,KeyError,RuntimeError,ET.ParseError): errors.append('storage')
    measurements['collector_errors']=len(errors)
    return measurements,errors


# Email alerts to the owner through Resend (owner decision, 2026-10-18). The
# conditions match monitoring.py's policies; a condition opens after it holds
# for its duration and clears when it stops holding. Each change sends one
# email, at most MAIL_PER_HOUR per hour, deduplicated by a durable state file.
ALERTS=[('nodes_ready','lt',2,120),('services_ready_ratio','lt',1,120),
    ('collector_errors','gt',0,120),('oldest_operation_seconds','gt',600,120),
    ('oldest_transition_seconds','gt',900,120),('worker_disk_free_bytes','lt',60*1024**3,300),
    ('sandbox_count','gt',2,120),('oldest_orphan_seconds','gt',7200,120),
    ('oldest_unfinished_upload_seconds','gt',3600,120),
    ('computers_stranded','gt',0,300),('computers_unknown','gt',0,900),
    ('oldest_requested_lifecycle_seconds','gt',600,120)]
ALERT_STATE='/var/lib/oa-space-monitor/alert-state.json'  # the unit's StateDirectory
MAIL_PER_HOUR=6


def breached(kind,value,threshold):
    return value<threshold if kind=='lt' else value>threshold


def transitions(state,values,now):
    """Advance per-alert state; return the (metric, 'open'|'clear', value) changes."""
    changes=[]
    for metric,kind,threshold,duration in ALERTS:
        entry=state.setdefault(metric,{'since':None,'open':False})
        if metric not in values: continue
        if breached(kind,values[metric],threshold):
            entry['since']=entry['since'] or now
            if not entry['open'] and now-entry['since']>=duration:
                entry['open']=True;changes.append((metric,'open',values[metric]))
        else:
            if entry['open']: changes.append((metric,'clear',values[metric]))
            entry['since']=None;entry['open']=False
    return changes


def admit_mail(state,now,count=1):
    sent=[t for t in state.get('_sent',[]) if now-t<3600]
    allowed=max(0,MAIL_PER_HOUR-len(sent))
    state['_sent']=sent
    return min(allowed,count)


def send_mail(subject,text):
    value=secret('alert-mail',identity())
    connection=http.client.HTTPSConnection('api.resend.com',timeout=20)
    try:
        connection.request('POST','/emails',body=json.dumps({'from':value['from'],'to':[value['to']],'subject':subject,'text':text}),
            headers={'Authorization':'Bearer '+value['api_key'],'Content-Type':'application/json','User-Agent':'openagents-space-monitor'})
        response=connection.getresponse();raw=response.read(65536)
        if response.status!=200: raise RuntimeError(f'Resend returned HTTP {response.status}.')
        return json.loads(raw).get('id')
    finally: connection.close()


def alert(values,now=None):
    now=now or time.time()
    try: state=json.loads(open(ALERT_STATE).read())
    except (OSError,ValueError): state={}
    changes=transitions(state,values,now)
    allowed=admit_mail(state,now,len(changes))
    if changes and allowed:
        lines=[f"{'Opened' if change=='open' else 'Cleared'}: {metric} = {value:g}" for metric,change,value in changes]
        subject='SPACE private: '+', '.join(f"{m} {'opened' if c=='open' else 'cleared'}" for m,c,_ in changes)[:150]
        text='\n'.join(lines)+'\n\nRunbook: deploy/space-service/README.md (private SPACE service).\n'
        try:
            send_mail(subject,text);state['_sent'].append(now)
        except (OSError,ValueError,KeyError,RuntimeError) as error:
            # Keep the change pending so the next run retries the email.
            for metric,change,_ in changes: state[metric]['open']=(change!='open')
            print(json.dumps({'alert_mail_error':type(error).__name__}))
    try:
        tmp=ALERT_STATE+'.tmp'
        with open(tmp,'w') as handle: handle.write(json.dumps(state))
        os.chmod(tmp,0o600);os.replace(tmp,ALERT_STATE)
    except OSError:
        # Measurements still publish; a missing state file only repeats an email later.
        print(json.dumps({'alert_state_error':True}))
    return changes


def main(publish):
    if os.getuid()!=0 or socket.gethostname().split('.')[0]!='oa-space-service-control':
        raise RuntimeError('Run measurements on the owned control node.')
    values,errors=collect()
    timestamp=datetime.now(timezone.utc).isoformat().replace('+00:00','Z')
    if publish:
        series=[{'metric':{'type':PREFIX+name,'labels':{'deployment':'oa-space-service'}},
            'resource':{'type':'global','labels':{'project_id':PROJECT}},'metricKind':'GAUGE','valueType':'DOUBLE',
            'points':[{'interval':{'endTime':timestamp},'value':{'doubleValue':float(value)}}]} for name,value in values.items()]
        connection=http.client.HTTPSConnection('monitoring.googleapis.com',timeout=20)
        try:
            connection.request('POST','/v3/projects/'+PROJECT+'/timeSeries',body=json.dumps({'timeSeries':series}),
                headers={'Authorization':'Bearer '+identity(),'Content-Type':'application/json'})
            response=connection.getresponse();response.read(65536)
            if response.status!=200: raise RuntimeError(f'Metric publication returned HTTP {response.status}.')
        finally: connection.close()
    changes=alert(values) if publish else []
    print(json.dumps({'observed_at':timestamp,'measurements':values,'probe_errors':errors,'published':publish,'alert_changes':changes}))
    if errors: raise SystemExit(1)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publish',action='store_true')
    parser.add_argument('--test-mail',action='store_true',help='Send one test email and exit.')
    args=parser.parse_args()
    if args.test_mail:
        print(json.dumps({'resend_id':send_mail('SPACE monitoring test','A test email from the private SPACE monitor on oa-space-service-control.\nAlert emails go to this address when a condition opens and clears.\n')}))
    else: main(args.publish)
