#!/usr/bin/env python3
"""Prepare the isolated Computers candidate's numeric secret versions."""
import argparse
import hashlib
from pathlib import Path
import tempfile
import json
import re
import secrets
import subprocess
import time
from urllib.parse import quote
from broker import cloud, PROJECT, PREFIX, REGION, MARKER, LABELS, named_resource
from database import sql_api


def saved(name):
    data=subprocess.check_output(['gcloud','secrets','versions','access','1','--secret='+PREFIX+'-'+name,'--project='+PROJECT,'--quiet'],stderr=subprocess.DEVNULL)
    if len(data)>65536: raise RuntimeError('The saved credential exceeds its size limit.')
    return data


def install(name,data):
    resource=PREFIX+'-'+name
    previous=cloud('secrets','describe',resource,missing=True)
    if previous is None:
        cloud('secrets','create',resource,'--replication-policy=automatic','--labels='+LABELS,'--data-file=-',data=data)
    elif previous.get('labels',{}).get('openagents-issue')!='11814' or saved(name)!=data:
        raise RuntimeError('The saved candidate credential differs. Review its existing consumers before changing it.')


def configure(passwords):
    instance=cloud('sql','instances','describe',PREFIX)
    if instance['settings'].get('userLabels',{}).get('openagents-issue')!='11814': raise RuntimeError('The native database belongs to another deployment.')
    native=json.loads(saved('native-database'))
    existing=cloud('secrets','describe',PREFIX+'-computers-database',missing=True)
    if existing is None:
        password=secrets.token_urlsafe(48)
        query='sslmode=verify-ca&sslrootcert=/var/run/space-postgres-ca/ca.crt'
        value={'role':'space_computers','password':password,'database':'space_computers',
            'dsn':f'postgresql://space_computers:{quote(password,safe="")}@{native["private_ip"]}/space_computers?{query}'}
        install('computers-database',json.dumps(value).encode())
    else:
        if existing.get('labels',{}).get('openagents-issue')!='11814': raise RuntimeError('The candidate credential belongs to another deployment.')
        value=json.loads(saved('computers-database'))
    if value['role']!='space_computers' or value['database']!='space_computers': raise RuntimeError('The candidate database identity differs.')
    cloud('secrets','add-iam-policy-binding',PREFIX+'-computers-database','--member=serviceAccount:'+PREFIX+'-control@'+PROJECT+'.iam.gserviceaccount.com','--role=roles/secretmanager.secretAccessor')
    fixture=json.loads(saved('qualification'))
    install('computers-dsn',value['dsn'].encode())
    proxy_dsn=f'postgresql://space_computers:{quote(value["password"],safe="")}@127.0.0.1:5432/space_computers?sslmode=disable'
    install('computers-proxy-dsn',proxy_dsn.encode())
    install('computers-token',fixture['computers_token'].encode())
    install('computers-space-token',fixture['api_key'].encode())
    install('computers-postgres-ca',native['server_ca_pem'].encode())
    if passwords:
        users=sql_api('GET',f'instances/{PREFIX}/users')
        if not any(u['name']=='space_computers' for u in users.get('items',[])): raise RuntimeError('Create the limited candidate role first.')
        operation=sql_api('PUT',f'instances/{PREFIX}/users?name=space_computers',{'name':'space_computers','password':value['password'],'type':'BUILT_IN'})
        deadline=time.monotonic()+40
        while operation['status']!='DONE' and time.monotonic()<deadline:
            time.sleep(1); operation=sql_api('GET','operations/'+operation['name'])
        if operation['status']!='DONE' or operation.get('error'): raise RuntimeError('Inspect the candidate password operation before retrying.')
    print(json.dumps({'database':'space_computers','numeric_secret_version':1,'passwords_in_sql_or_argv':False,'password_update_requested':passwords}))


def deploy(commit,digest,template,vcpu,memory,disable_creates,rollout=None,keep_project_iam=False,poll_ms=None):
    rollout=rollout or ['space-qualification-11814']
    # Explicit test owners use the web's digest form; at most two for dedicated tests.
    if len(rollout)>2 or len(set(rollout))!=len(rollout) or any(not (o=='space-qualification-11814' or re.fullmatch(r'account:[0-9a-f]{64}',o)) for o in rollout): raise RuntimeError('Name one or two exact test owners.')
    if not re.fullmatch(r'[0-9a-f]{40}',commit) or not re.fullmatch(r'sha256:[0-9a-f]{64}',digest): raise RuntimeError('Provide full source and image identities.')
    if not re.fullmatch(r'(seed|code-runtime)@sha256:[0-9a-f]{64}',template): raise RuntimeError('Pin the published system template digest.')
    if (vcpu,memory)!=(None,None) and (vcpu!=2 or memory not in (1024,2048,4096)): raise RuntimeError('Use the measured small template shape within the qualification capacity.')
    subprocess.run(['git','fetch','-q','origin','main'],check=True)
    subprocess.run(['git','merge-base','--is-ancestor',commit,'origin/main'],check=True)
    current_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    subprocess.run(['git','merge-base','--is-ancestor',current_commit,'origin/main'],check=True)
    image=f'us-central1-docker.pkg.dev/{PROJECT}/{PREFIX}/computers'
    published=cloud('artifacts','docker','images','describe',image+':'+commit)
    if published['image_summary']['digest']!=digest: raise RuntimeError('The candidate image digest differs.')
    name=PREFIX+'-computers'
    identity=f'{name}@{PROJECT}.iam.gserviceaccount.com'
    named_resource(['iam','service-accounts','describe',identity],['iam','service-accounts','create',name,'--display-name='+MARKER,'--description='+MARKER])
    config={'default':'space-private-11814','policy':'space-private-11814-v1','deployments':[{
        'id':'space-private-11814','kind':'space','api_url':'https://oa-space-service-api-ezxz4mgdsq-uc.a.run.app',
        'data_plane_url':'https://oa-space-service-data-ezxz4mgdsq-uc.a.run.app','credential_env':'SPACE_API_TOKEN',
        'template':template,'scope':'00000000-0000-4000-8000-000000011814','capacity':2,'admit_new':not disable_creates,
        'rollout_accounts':rollout,'shapes':({'small':{'vcpus':vcpu,'memory_mib':memory,'min_disk_gib':None}} if vcpu is not None else {})}]}
    registry=json.dumps(config,separators=(',',':')).encode()
    registry_key='registry-'+hashlib.sha256(registry).hexdigest()[:16]
    install('computers-'+registry_key,registry)
    keys=('proxy-dsn','token','space-token',registry_key)
    for key in keys:
        cloud('secrets','add-iam-policy-binding',PREFIX+'-computers-'+key,'--member=serviceAccount:'+identity,'--role=roles/secretmanager.secretAccessor')
    condition=f'''expression=resource.name == "projects/{PROJECT}/instances/{PREFIX}" && resource.type == "sqladmin.googleapis.com/Instance",title=space-native-instance'''
    # A redeploy by an operator without project IAM rights keeps the existing
    # instance-scoped binding; the new revision's proxy startup then proves it.
    if not keep_project_iam:
        cloud('projects','add-iam-policy-binding',PROJECT,'--member=serviceAccount:'+identity,'--role=roles/cloudsql.client','--condition='+condition)
    previous=cloud('run','services','describe',name,'--region='+REGION,missing=True)
    if previous:
        if previous['metadata'].get('labels',{}).get('openagents-issue')!='11814': raise RuntimeError('The candidate service belongs to another deployment.')
        old=previous['metadata']['labels'].get('openagents-commit')
        if old: subprocess.run(['git','merge-base','--is-ancestor',old,current_commit],check=True)
    labels=dict(pair.split('=',1) for pair in LABELS.split(','))
    labels.update({'openagents-commit':current_commit,'openagents-image-source':commit})
    env=[{'name':'OA_COMPUTERS_REGISTRY_FILE','value':'/var/run/space-registry/registry.json'},
         {'name':'OA_COMPUTERS_KEEP_STOPPED_DAYS','value':'1'}]
    if poll_ms is not None:
        # Service and driver polling bound readiness; the defaults are 1 s and 3 s.
        if not 50<=poll_ms<=3000: raise RuntimeError('Choose a poll interval from 50 to 3000 ms.')
        env+=[{'name':'OA_COMPUTERS_POLL_MS','value':str(poll_ms)},{'name':'OA_COMPUTERS_DRIVER_POLL_MS','value':str(poll_ms)}]
    for variable,key in (('OA_COMPUTERS_DATABASE_URL','proxy-dsn'),('OA_COMPUTERS_TOKEN','token'),('SPACE_API_TOKEN','space-token')):
        env.append({'name':variable,'valueFrom':{'secretKeyRef':{'name':PREFIX+'-computers-'+key,'key':'1'}}})
    spec={'apiVersion':'serving.knative.dev/v1','kind':'Service','metadata':{'name':name,'labels':labels,'annotations':{'run.googleapis.com/ingress':'internal'}},
      'spec':{'template':{'metadata':{'labels':labels,'annotations':{
        'run.googleapis.com/execution-environment':'gen2','run.googleapis.com/cpu-throttling':'false',
        'autoscaling.knative.dev/minScale':'1','autoscaling.knative.dev/maxScale':'1',
        'run.googleapis.com/vpc-access-egress':'all-traffic',
        'run.googleapis.com/network-interfaces':json.dumps([{'network':PREFIX,'subnetwork':PREFIX+'-run'}]),
        'run.googleapis.com/container-dependencies':json.dumps({'computers':['db-proxy']})}},
       'spec':{'serviceAccountName':identity,'containerConcurrency':16,'timeoutSeconds':300,'containers':[
        {'name':'computers','image':image+'@'+digest,'ports':[{'containerPort':8080,'name':'http1'}],
         'resources':{'limits':{'cpu':'1','memory':'512Mi'}},'env':env,
         'volumeMounts':[{'name':'registry','mountPath':'/var/run/space-registry'}],
         'startupProbe':{'tcpSocket':{'port':8080},'periodSeconds':2,'timeoutSeconds':1,'failureThreshold':30}},
        {'name':'db-proxy','image':'gcr.io/cloud-sql-connectors/cloud-sql-proxy@sha256:86e4f3cc266020e7ee07740df78d7511e2c340c274b56f84d80b7251913af9ad',
         'resources':{'limits':{'cpu':'1','memory':'128Mi'}},
         'args':['--private-ip','--structured-logs','--health-check','--http-address=0.0.0.0','--http-port=9090','--address=127.0.0.1','--port=5432','--max-connections=32',PROJECT+':'+REGION+':'+PREFIX],
         'startupProbe':{'httpGet':{'path':'/startup','port':9090},'periodSeconds':2,'timeoutSeconds':1,'failureThreshold':30}}],
        'volumes':[{'name':'registry','secret':{'secretName':PREFIX+'-computers-'+registry_key,'items':[{'key':'1','path':'registry.json'}]}}]}},
       'traffic':[{'latestRevision':True,'percent':100}]}}
    scratch=Path(subprocess.check_output(['openagents','scratch'],text=True).strip())
    with tempfile.TemporaryDirectory(prefix='space-computers-',dir=scratch) as directory:
        config_path=Path(directory)/'service.json';config_path.write_text(json.dumps(spec));config_path.chmod(0o600)
        result=cloud('run','services','replace',str(config_path),'--region='+REGION)
    cloud('run','services','add-iam-policy-binding',name,'--region='+REGION,'--member=allUsers','--role=roles/run.invoker')
    print(json.dumps({'service':name,'url':result['status']['url'],'image':digest,'image_source':commit,'deployment_source':current_commit,'template':template,'admit_new':not disable_creates,'rollout_accounts':rollout,'poll_ms':poll_ms,'registry_secret':PREFIX+'-computers-'+registry_key,'qualification_only':True}))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--set-password',action='store_true')
    parser.add_argument('--deploy-commit')
    parser.add_argument('--deploy-digest')
    parser.add_argument('--template')
    parser.add_argument('--small-vcpus',type=int)
    parser.add_argument('--small-memory-mib',type=int)
    parser.add_argument('--disable-creates',action='store_true')
    parser.add_argument('--poll-ms',type=int,help='Service and driver poll interval in milliseconds.')
    parser.add_argument('--keep-project-iam',action='store_true',help='Redeploy without changing the existing instance-scoped Cloud SQL binding.')
    parser.add_argument('--rollout-account',action='append',help='An exact test owner (account:HEX); repeat for a second.')
    args=parser.parse_args()
    if args.deploy_commit:
        deploy(args.deploy_commit,args.deploy_digest,args.template,args.small_vcpus,args.small_memory_mib,args.disable_creates,args.rollout_account,args.keep_project_iam,args.poll_ms)
    else: configure(args.set_password)
