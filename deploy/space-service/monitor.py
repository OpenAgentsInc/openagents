#!/usr/bin/env python3
"""Publish bounded operational measurements from the owned control node."""
import argparse
from datetime import datetime, timezone
import http.client
import json
import os
import socket
import subprocess
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
        value=secret('object-credentials',identity())
        client=protocol()
        pending=[unfinished_uploads(client.Store(value,PROJECT+'-oa-space-service-'+purpose))
            for purpose in ('templates','snapshots')]
        measurements['unfinished_uploads']=sum(item['unfinished_uploads'] for item in pending)
        measurements['oldest_unfinished_upload_seconds']=max(item['oldest_unfinished_upload_seconds'] for item in pending)
    except (OSError,ValueError,KeyError,RuntimeError,ET.ParseError): errors.append('storage')
    measurements['collector_errors']=len(errors)
    return measurements,errors


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
    print(json.dumps({'observed_at':timestamp,'measurements':values,'probe_errors':errors,'published':publish}))
    if errors: raise SystemExit(1)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publish',action='store_true')
    main(parser.parse_args().publish)
