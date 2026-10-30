#!/usr/bin/env python3
"""Offline control-flow checks; fake gcloud/curl never access the network."""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import deploy_verify as guard

IMAGE=guard.REPOSITORY+'@sha256:'+'a'*64
COMMIT='1'*40
RELEASE='123e4567-e89b-42d3-a456-426614174000'
OLD='oa-api-staging-00001-old'
NEW='oa-api-staging-00002-new'
SERVICE='oa-api-staging'

def service(generation,revision,image):
 return {'apiVersion':guard.API_VERSION,'kind':'Service','metadata':{'name':SERVICE,'namespace':guard.NAMESPACE,'uid':'service-uid','generation':generation,'labels':{'cloud.googleapis.com/location':guard.LOCATION}},'status':{'observedGeneration':generation,'conditions':[{'type':'Ready','status':'True'}],'latestCreatedRevisionName':revision,'latestReadyRevisionName':revision,'traffic':[{'revisionName':revision,'percent':100}]},'spec':{'template':{'spec':{'containers':[{'image':image}]}}}}
def revision():
 return {'apiVersion':guard.API_VERSION,'kind':'Revision','metadata':{'name':NEW,'generation':1,'namespace':guard.NAMESPACE,'labels':{'serving.knative.dev/service':SERVICE,'cloud.googleapis.com/location':guard.LOCATION}},'status':{'observedGeneration':1,'conditions':[{'type':'Ready','status':'True'}],'imageDigest':IMAGE},'spec':{'containers':[{'image':IMAGE}]}}
def health(): return {'ok':True,'build':COMMIT,'release':RELEASE,'service':SERVICE,'revision':NEW}

GCLOUD='''#!/usr/bin/env python3
import json,os,pathlib,sys
p=pathlib.Path(os.environ['STUB_DIR']); args=sys.argv[1:]
with (p/'calls').open('a') as f:f.write(' '.join(args)+'\\n')
if args[:3]==['run','services','describe']:
 counter=p/'count';i=int(counter.read_text()) if counter.exists() else 0;counter.write_text(str(i+1)); print((p/(['pre.json','before.json','after.json'][min(i,2)])).read_text())
elif args[:3]==['run','revisions','describe']:print((p/'revision.json').read_text())
elif args[:2]==['run','deploy']:
 if os.environ.get('STUB_FAIL_DEPLOY')=='1':print('deployment failed',file=sys.stderr);sys.exit(2)
 print('revision deployed')
else:sys.exit(3)
'''
CURL='''#!/usr/bin/env python3
import os,pathlib,sys
p=pathlib.Path(os.environ['STUB_DIR']);args=sys.argv[1:]
with (p/'calls').open('a') as f:f.write('curl '+ ' '.join(args)+'\\n')
pathlib.Path(args[args.index('-o')+1]).write_bytes((p/'health.json').read_bytes())
print(os.environ.get('STUB_HTTP','200'),end='')
'''
class Rollout(unittest.TestCase):
 def run_case(self, transform=None, *, fail=False, http='200'):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);bin=root/'bin';bin.mkdir();work=root/'work';work.mkdir();here=Path(__file__).resolve().parent
   for filename,script in [('gcloud',GCLOUD),('curl',CURL),('sleep','#!/bin/sh\nexit 0\n')]:
    p=bin/filename;p.write_text(script);p.chmod(0o755)
   docs={'pre':service(1,OLD,guard.REPOSITORY+'@sha256:'+'b'*64),'before':service(2,NEW,IMAGE),'after':service(2,NEW,IMAGE),'revision':revision(),'health':health()}
   if transform:transform(docs)
   for name,value in docs.items():(root/(name+'.json')).write_text(json.dumps(value))
   env={**os.environ,'STUB_DIR':str(root),'STUB_FAIL_DEPLOY':'1' if fail else '0','STUB_HTTP':http,'PATH':str(bin)+':'+os.environ['PATH']}
   command='set -euo pipefail; here="$1"; project=openagentsgemini; region=us-central1; source "$here/deploy-command.sh"; source "$here/deploy-rollout.sh"; api_verified_rollout "$2" "$3" "$4" "$5" "$6" "$7" --set-env-vars X=test; echo STAGING_VERIFIED'
   args=['bash','-c',command,'test',str(here),SERVICE,'staging.openagents.com',IMAGE,COMMIT,RELEASE,str(work)]
   result=subprocess.run(args,env=env,capture_output=True,text=True)
   calls=(root/'calls').read_text() if (root/'calls').exists() else ''
   return result,calls
 def test_good_rollout_pins_digest_and_verifies(self):
  result,calls=self.run_case();self.assertEqual(result.returncode,0,result.stderr);self.assertIn('STAGING_VERIFIED',result.stdout);self.assertIn('--image '+IMAGE,calls);self.assertIn('run revisions describe '+NEW,calls);self.assertIn('curl ',calls)
 def test_failed_deploy_never_checks_old_healthy_service(self):
  result,calls=self.run_case(fail=True);self.assertNotEqual(result.returncode,0);self.assertNotIn('curl ',calls);self.assertNotIn('STAGING_VERIFIED',result.stdout)
 def test_stale_or_wrong_routed_health_refused(self):
  for change in (lambda x:x['health'].update(revision=OLD),lambda x:x['health'].update(service='oa-api'),lambda x:x['health'].update(release='123e4567-e89b-42d3-a456-426614174001')):
   result,calls=self.run_case(change);self.assertNotEqual(result.returncode,0);self.assertNotIn('STAGING_VERIFIED',result.stdout);self.assertIn('curl ',calls)
 def test_bad_snapshot_or_http_refused(self):
  for change in (lambda x:(x['before']['metadata'].update(namespace='other'),x['after']['metadata'].update(namespace='other')),lambda x:x['revision']['status'].update(imageDigest=guard.REPOSITORY+'@sha256:'+'b'*64),lambda x:x['after']['status']['traffic'][0].update(revisionName=OLD),lambda x:x['after']['metadata'].update(uid='recreated')):
   result,_=self.run_case(change);self.assertNotEqual(result.returncode,0);self.assertNotIn('STAGING_VERIFIED',result.stdout)
  result,_=self.run_case(http='302');self.assertNotEqual(result.returncode,0);self.assertNotIn('STAGING_VERIFIED',result.stdout)

if __name__=='__main__':unittest.main()
