#!/usr/bin/env python3
"""Configure owned Cloud Monitoring alerts without changing unrelated policies."""
import http.client
import json
import subprocess
from broker import cloud,PROJECT,PREFIX

BASE='/v3/projects/'+PROJECT
METRICS='custom.googleapis.com/openagents/space/'
LABELS={'openagents_issue':'11814','openagents_service':'space'}


def api(method,path,body=None):
    token=subprocess.check_output(['gcloud','auth','print-access-token'],stderr=subprocess.DEVNULL).decode().strip()
    connection=http.client.HTTPSConnection('monitoring.googleapis.com',timeout=20)
    try:
        connection.request(method,path,body=json.dumps(body) if body is not None else None,
            headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
        response=connection.getresponse();raw=response.read(1048577)
        if len(raw)>1048576 or not 200<=response.status<300:
            raise RuntimeError(f'Monitoring configuration returned HTTP {response.status}.')
        return json.loads(raw) if raw else {}
    finally: connection.close()


def configure():
    cloud('projects','add-iam-policy-binding',PROJECT,'--member=serviceAccount:'+PREFIX+'-control@'+PROJECT+'.iam.gserviceaccount.com',
        '--role=roles/monitoring.metricWriter','--condition=None')
    existing=[];path=BASE+'/alertPolicies?pageSize=100'
    for _ in range(20):
        page=api('GET',path);existing.extend(page.get('alertPolicies',[]))
        token=page.get('nextPageToken')
        if not token: break
        from urllib.parse import quote
        path=BASE+'/alertPolicies?pageSize=100&pageToken='+quote(token,safe='')
    else: raise RuntimeError('The alert inventory exceeded its page bound.')
    specs=[('nodes_ready','COMPARISON_LT',2,120),('services_ready_ratio','COMPARISON_LT',1,120),
        ('collector_errors','COMPARISON_GT',0,120),('oldest_operation_seconds','COMPARISON_GT',600,120),
        ('oldest_transition_seconds','COMPARISON_GT',900,120),('worker_disk_free_bytes','COMPARISON_LT',60*1024**3,300)]
    policies=[]
    for metric,comparison,threshold,duration in specs:
        name='SPACE private: '+metric
        matches=[p for p in existing if p.get('displayName')==name]
        if len(matches)>1 or any(p.get('userLabels')!=LABELS for p in matches):
            raise RuntimeError('An alert with this name has different ownership.')
        condition={'filter':f'metric.type="{METRICS+metric}" AND resource.type="global" AND metric.label.deployment="oa-space-service"',
            'comparison':comparison,'thresholdValue':threshold,'duration':str(duration)+'s',
            'aggregations':[{'alignmentPeriod':'60s','perSeriesAligner':'ALIGN_MEAN'}],
            'trigger':{'count':1},'evaluationMissingData':'EVALUATION_MISSING_DATA_ACTIVE'}
        policy={'displayName':name,'userLabels':LABELS,'enabled':True,'combiner':'OR',
            'conditions':[{'displayName':metric,'conditionThreshold':condition}],
            'documentation':{'mimeType':'text/markdown','content':'Inspect the owned control and worker, current operations, and disk capacity. Disable new placements before draining a busy worker. See deploy/space-service/README.md.'},
            'notificationChannels':matches[0].get('notificationChannels',[]) if matches else []}
        if matches:
            policy['name']=matches[0]['name'];result=api('PATCH','/v3/'+policy['name'],policy)
        else: result=api('POST',BASE+'/alertPolicies',policy)
        policies.append({'name':result['name'],'metric':metric,'enabled':result['enabled'],
            'notification_channels':len(result.get('notificationChannels',[]))})
    print(json.dumps({'policies':policies,'notifications':'Cloud Monitoring incidents; existing channel assignments are preserved'}))


if __name__=='__main__': configure()
