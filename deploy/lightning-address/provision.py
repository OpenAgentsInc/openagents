#!/usr/bin/env python3
"""Provision the production database, secrets, ingress rules, and webhook DNS.

Run from the repository root with a Cloud SQL Auth Proxy on localhost:15434.
The automation account needs its existing SQL, secrets, compute, and DNS access.
Project IAM changes are not needed; the existing runtime identity is used.
"""
import os,subprocess,json,secrets,re,sys,urllib.request
from pathlib import Path
from urllib.parse import urlsplit,parse_qs,unquote
os.environ['CLOUDSDK_CONFIG']=str(Path.home()/'work/.secrets/gcloud-sa-config')
sys.path.insert(0,'deploy/labs')
from database import database_url
project='openagentsgemini';sa='oa-vertex-inference@'+project+'.iam.gserviceaccount.com'
def g(*args,data=None,missing=False):
 r=subprocess.run(['gcloud',*args,'--project='+project,'--quiet'],input=data,capture_output=True)
 if r.returncode:
  if missing and (b'NOT_FOUND' in r.stderr or b'not found' in r.stderr): return None
  raise RuntimeError('gcloud operation failed: '+ ' '.join(args[:3])+' '+r.stderr.decode()[:300])
 return r.stdout
if g('iam','service-accounts','describe',sa,missing=True) is None:
 g('iam','service-accounts','create','oa-lnurl','--display-name=Lightning address backend')
g('artifacts','repositories','add-iam-policy-binding','openagents','--location=us-central1','--member=serviceAccount:'+sa,'--role=roles/artifactregistry.reader','--condition=None')
runtime=g('secrets','describe','openagents-lnurl-runtime',missing=True)
if runtime is None:
 raw=g('secrets','versions','access','latest','--secret=openagents-production-pg-dsn').decode().strip()
 u=urlsplit(database_url(raw));q=parse_qs(u.query)
 admin=unquote(u.username or q.get('user',[''])[0]);password=unquote(u.password or q.get('password',[''])[0])
 env=dict(os.environ,PGHOST='127.0.0.1',PGPORT='15434',PGDATABASE='openagents',PGUSER=admin,PGPASSWORD=password,PGSSLMODE='disable')
 lnurl_password=secrets.token_hex(32)
 sql=f'''CREATE ROLE lnurl LOGIN PASSWORD '{lnurl_password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
GRANT lnurl TO {admin};
CREATE DATABASE lnurl OWNER lnurl;
REVOKE lnurl FROM {admin};
'''
 r=subprocess.run(['/opt/homebrew/opt/postgresql@17/bin/psql','-X','-v','ON_ERROR_STOP=1'],input=sql,text=True,env=env,capture_output=True)
 if r.returncode: raise RuntimeError('Database provisioning failed: '+r.stderr.replace(lnurl_password,'[redacted]').replace(password,'[redacted]'))
 key=re.search(r'pub const BREEZ_API_KEY: &str = "([^"]+)"',Path('crates/spark-wallet/src/config.rs').read_text()).group(1)
 values={
 'BREEZ_LNURL_ADDRESS':'127.0.0.1:8181',
 'BREEZ_LNURL_DB_URL':f'postgres://lnurl:{lnurl_password}@127.0.0.1:5432/lnurl',
 'BREEZ_LNURL_DOMAINS':'openagents.com',
 'BREEZ_LNURL_NETWORK':'mainnet',
 'BREEZ_LNURL_SCHEME':'https',
 'BREEZ_LNURL_AUTO_MIGRATE':'true',
 'BREEZ_LNURL_DEFAULT_API_KEY':key,
 'BREEZ_LNURL_SSP_AUTH_SEED':secrets.token_hex(32),
 'BREEZ_LNURL_WEBHOOK_DOMAIN':'lnurl-hooks.openagents.com',
 'BREEZ_LNURL_LOG_LEVEL':'warn',
 'BREEZ_LNURL_MIN_SENDABLE':'1000',
 'BREEZ_LNURL_MAX_SENDABLE':'100000000',
 'BREEZ_LNURL_MAX_REGISTRATIONS_PER_DAY':'5'}
 g('secrets','create','openagents-lnurl-runtime','--replication-policy=automatic')
 g('secrets','versions','add','openagents-lnurl-runtime','--data-file=-',data=''.join(k+'='+v+'\n' for k,v in values.items()).encode())
 g('secrets','create','openagents-lnurl-breez-api-key','--replication-policy=automatic')
 g('secrets','versions','add','openagents-lnurl-breez-api-key','--data-file=-',data=key.encode())
g('secrets','add-iam-policy-binding','openagents-lnurl-runtime','--member=serviceAccount:'+sa,'--role=roles/secretmanager.secretAccessor','--condition=None')
for member in ['157437760789-compute@developer.gserviceaccount.com','oa-vertex-inference@'+project+'.iam.gserviceaccount.com']:
 g('secrets','add-iam-policy-binding','openagents-lnurl-breez-api-key','--member=serviceAccount:'+member,'--role=roles/secretmanager.secretAccessor','--condition=None')
name='oa-lnurl-1'
for suffix,sources,ports,priority in [('web','10.42.27.0/26,10.42.26.0/26','tcp:8180','900'),('hooks','0.0.0.0/0','tcp:80,tcp:443','900'),('iap','35.235.240.0/20','tcp:22','900')]:
 if g('compute','firewall-rules','describe',name+'-'+suffix,missing=True) is None:
  g('compute','firewall-rules','create',name+'-'+suffix,'--network=default','--direction=INGRESS','--priority='+priority,'--source-ranges='+sources,'--target-tags='+name,'--allow='+ports)
if g('compute','firewall-rules','describe',name+'-deny-other',missing=True) is None:
 g('compute','firewall-rules','create',name+'-deny-other','--network=default','--direction=INGRESS','--priority=910','--source-ranges=0.0.0.0/0','--target-tags='+name,'--action=DENY','--rules=all')
if g('compute','addresses','describe',name,'--region=us-central1',missing=True) is None:
 g('compute','addresses','create',name,'--region=us-central1')
ip=g('compute','addresses','describe',name,'--region=us-central1','--format=value(address)').decode().strip()
# Only the dedicated webhook name is created; existing records must match.
text=(Path.home()/'work/.secrets/cloudflare-openagents.env').read_text()
cf=re.search(r'^\s*(?:export\s+)?CLOUDFLARE_API_TOKEN=[\'"]?([^\n\'"]+)',text,re.M).group(1).strip()
url='https://api.cloudflare.com/client/v4/zones/bd33d951ee951a7c18fa4ab2ddcbe3a7/dns_records'
headers={'Authorization':'Bearer '+cf,'Content-Type':'application/json'}
d=json.load(urllib.request.urlopen(urllib.request.Request(url+'?name=lnurl-hooks.openagents.com',headers=headers)))
assert d['success']
if d['result']:
 assert len(d['result'])==1 and d['result'][0]['content']==ip,'Existing webhook DNS differs; inspect it before changing.'
else:
 body=json.dumps({'type':'A','name':'lnurl-hooks.openagents.com','content':ip,'ttl':300,'proxied':False,'comment':'Self-hosted Lightning payment webhooks (#12307)'}).encode()
 d=json.load(urllib.request.urlopen(urllib.request.Request(url,data=body,headers=headers,method='POST')))
 assert d['success']
print('Provisioned the dedicated database, secrets, firewall, and webhook DNS:',ip)
