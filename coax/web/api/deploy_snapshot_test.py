#!/usr/bin/env python3
"""Exercise deploy.sh's source snapshot with a real temporary git repository."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE=Path(__file__).resolve().parent
FAKE_GCLOUD='''#!/usr/bin/env python3
import json,os,pathlib,re,sys
args=sys.argv[1:];root=pathlib.Path(os.environ['FIXTURE_ROOT']);mode=os.environ.get('FIXTURE_MODE','fail')
repo='us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-api';image=repo+'@sha256:'+'a'*64
with (root/'calls').open('a') as f:f.write(' '.join(args)+'\\n')
def service(name):
 state=root/(name+'.json');done=state.exists();rev=name+('-00002-new' if done else '-00001-old');gen=2 if done else 1
 return {'apiVersion':'serving.knative.dev/v1','kind':'Service','metadata':{'name':name,'namespace':'157437760789','uid':name+'-uid','generation':gen,'labels':{'cloud.googleapis.com/location':'us-central1'}},'status':{'observedGeneration':gen,'conditions':[{'type':'Ready','status':'True'}],'latestCreatedRevisionName':rev,'latestReadyRevisionName':rev,'traffic':[{'revisionName':rev,'percent':100}]},'spec':{'template':{'spec':{'containers':[{'image':image}]}}}}
if args[:2]==['builds','submit']:
 stage=pathlib.Path(args[2]);assert (stage/'src'/'fixture.c').read_text()=='committed source\\n';assert (stage/'src'/'fixture.h').read_text()=='committed header\\n';assert (stage/'Dockerfile').read_text()=='committed Dockerfile\\n';assert (stage/'migrations'/'fixture.sql').read_text()=='committed migration\\n';assert not (stage/'coax').exists()
 tag=args[args.index('--tag')+1];print(json.dumps({'status':'SUCCESS','results':{'images':[{'name':tag,'digest':'sha256:'+'a'*64}]}}))
elif args[:3]==['run','services','describe']:
 if args[3] in ('oa-api-staging','oa-api'):print(json.dumps(service(args[3])))
 else:sys.exit(1)
elif args[:3]==['run','revisions','describe']:
 rev=args[3];name=rev.rsplit('-00002-new',1)[0];print(json.dumps({'apiVersion':'serving.knative.dev/v1','kind':'Revision','metadata':{'name':rev,'generation':1,'namespace':'157437760789','labels':{'serving.knative.dev/service':name,'cloud.googleapis.com/location':'us-central1'}},'status':{'observedGeneration':1,'conditions':[{'type':'Ready','status':'True'}],'imageDigest':image},'spec':{'containers':[{'image':image}]}}))
elif args[:2]==['secrets','describe']:sys.exit(1)
elif args[:2]==['run','deploy']:
 name=args[2];assert args[args.index('--image')+1]==image
 if mode=='fail' and name=='oa-api-staging':print('intentional staging refusal',file=sys.stderr);sys.exit(42)
 raw=args[args.index('--set-env-vars')+1];commit=re.search(r'~OA_BUILD_SHA=([0-9a-f]{40})(?:~|$)',raw).group(1);release=re.search(r'~OA_RELEASE_ID=([0-9a-f-]{36})(?:~|$)',raw).group(1)
 (root/(name+'.json')).write_text(json.dumps({'build':commit,'release':release,'service':name,'revision':name+'-00002-new'}));print('revision ready')
else:print('unexpected gcloud',file=sys.stderr);sys.exit(90)
'''
FAKE_CURL='''#!/usr/bin/env python3
import json,os,pathlib,sys
root=pathlib.Path(os.environ['FIXTURE_ROOT']);args=sys.argv[1:];url=args[-1];name='oa-api-staging' if 'staging.openagents.com' in url else 'oa-api'
(root/'curl').touch();data=json.loads((root/(name+'.json')).read_text());data['ok']=True
if os.environ.get('FIXTURE_MODE')=='stale':data['revision']=name+'-00001-old'
pathlib.Path(args[args.index('-o')+1]).write_text(json.dumps(data));print('200',end='')
'''

class Snapshot(unittest.TestCase):
 def make_repo(self,root):
  repo=root/'repo';api=repo/'coax/web/api';api.mkdir(parents=True);(api/'migrations').mkdir()
  for f in ('deploy.sh','deploy-command.sh','deploy-rollout.sh','deploy_verify.py'):shutil.copy2(HERE/f,api/f)
  for f,body in {'fixture.c':'committed source\n','fixture.h':'committed header\n','Dockerfile':'committed Dockerfile\n','admins.conf':'','migrations/fixture.sql':'committed migration\n','test.sh':'#!/bin/sh\nset -eu\ntest "$(git -C "$(dirname "$0")" rev-parse HEAD)" = "$FIXTURE_COMMIT"\nprintf tested > "$FIXTURE_ROOT/tested"\n'}.items():
   p=api/f;p.write_text(body)
  (api/'test.sh').chmod(0o755)
  for cmd in (['git','init','-q'],['git','config','user.email','test@example.test'],['git','config','user.name','Test'],['git','add','.'],['git','commit','-q','-m','fixture']):subprocess.run(cmd,cwd=repo,check=True,capture_output=True)
  commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip();return repo,api,commit
 def run_deploy(self,root,repo,commit,mode='fail'):
  bin=root/'bin';bin.mkdir();(bin/'gcloud').write_text(FAKE_GCLOUD);(bin/'gcloud').chmod(0o755)
  (bin/'curl').write_text(FAKE_CURL);(bin/'curl').chmod(0o755)
  env={**os.environ,'FIXTURE_ROOT':str(root),'FIXTURE_COMMIT':commit,'FIXTURE_MODE':mode,'PATH':str(bin)+':'+os.environ['PATH']}
  return subprocess.run(['bash',str(repo/'coax/web/api/deploy.sh')],env=env,capture_output=True,text=True)
 def test_exact_commit_snapshot_and_failed_staging_stops(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);repo,api,commit=self.make_repo(root);r=self.run_deploy(root,repo,commit);self.assertNotEqual(r.returncode,0);self.assertIn('intentional staging refusal',r.stderr);self.assertTrue((root/'tested').exists());calls=(root/'calls').read_text();self.assertIn('run deploy oa-api-staging ',calls);self.assertNotIn('run deploy oa-api ',calls);self.assertFalse((root/'curl').exists())
 def test_success_promotes_exact_same_digest(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);repo,api,commit=self.make_repo(root);r=self.run_deploy(root,repo,commit,'success');self.assertEqual(r.returncode,0,r.stderr);calls=(root/'calls').read_text().splitlines();deployed=[line for line in calls if line.startswith('run deploy ')];self.assertEqual(len(deployed),2);self.assertTrue(deployed[0].startswith('run deploy oa-api-staging '));self.assertTrue(deployed[1].startswith('run deploy oa-api '));self.assertEqual(deployed[0].split('--image ')[1].split()[0],deployed[1].split('--image ')[1].split()[0]);self.assertIn('@sha256:',deployed[0]);self.assertTrue((root/'curl').exists())
 def test_stale_routed_health_blocks_production(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);repo,api,commit=self.make_repo(root);r=self.run_deploy(root,repo,commit,'stale');self.assertNotEqual(r.returncode,0);calls=(root/'calls').read_text();self.assertIn('run deploy oa-api-staging ',calls);self.assertNotIn('run deploy oa-api ',calls);self.assertIn('health service or revision',r.stderr)
 def test_dirty_api_is_not_deployed(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);repo,api,commit=self.make_repo(root);(api/'fixture.c').write_text('dirty\n');r=self.run_deploy(root,repo,commit);self.assertNotEqual(r.returncode,0);self.assertIn('commit API changes',r.stderr);self.assertFalse((root/'calls').exists());self.assertFalse((root/'tested').exists())

if __name__=='__main__':unittest.main()
