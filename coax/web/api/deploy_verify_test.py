#!/usr/bin/env python3
"""Offline verification: no gcloud, network, deployments or credentials."""
import copy
import json
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import deploy_verify as guard

IMAGE=guard.REPOSITORY+'@sha256:'+'a'*64
OTHER=guard.REPOSITORY+'@sha256:'+'b'*64
COMMIT='1'*40
RELEASE='123e4567-e89b-42d3-a456-426614174000'
SERVICE='oa-api-staging'
OLD=SERVICE+'-00001-old'
NEW=SERVICE+'-00002-new'
def service(generation, revision, image):
    return {'apiVersion':guard.API_VERSION,'kind':'Service','metadata':{'name':SERVICE,'generation':generation,'namespace':guard.NAMESPACE,'uid':'service-uid','labels':{'cloud.googleapis.com/location':guard.LOCATION}},
            'status':{'observedGeneration':generation,'conditions':[{'type':'Ready','status':'True'}],
                      'latestReadyRevisionName':revision,'latestCreatedRevisionName':revision,
                      'traffic':[{'revisionName':revision,'percent':100,'latestRevision':True}]},
            'spec':{'template':{'spec':{'containers':[{'image':image}]}}}}
def revision():
    return {'apiVersion':guard.API_VERSION,'kind':'Revision','metadata':{'name':NEW,'generation':1,'namespace':guard.NAMESPACE,'labels':{'serving.knative.dev/service':SERVICE,'cloud.googleapis.com/location':guard.LOCATION}},
            'status':{'observedGeneration':1,'conditions':[{'type':'Ready','status':'True'}],'imageDigest':IMAGE},
            'spec':{'containers':[{'image':IMAGE}]}}
def inputs():
    return {'pre':service(1,OLD,OTHER), 'before':service(2,NEW,IMAGE), 'revision_doc':revision(),
            'after':service(2,NEW,IMAGE), 'health':{'ok':True,'coaxNostrJson':True,'build':COMMIT,'release':RELEASE,'service':SERVICE,'revision':NEW},
            'service':SERVICE,'image':IMAGE,'commit':COMMIT,'release':RELEASE,'deploy_exit':0,'http_status':200}
def call(d):
    return guard.verify(d['pre'],d['before'],d['revision_doc'],d['after'],d['health'],service=d['service'],image=d['image'],
                        commit=d['commit'],release=d['release'],deploy_exit=d['deploy_exit'],http_status=d['http_status'])
class Verification(unittest.TestCase):
    def test_build_digest_must_come_from_the_successful_requested_build(self):
        tag=guard.REPOSITORY+':abc123'
        build={'status':'SUCCESS','results':{'images':[{'name':tag,'digest':'sha256:'+'a'*64}]}}
        self.assertEqual(guard.build_image(build,tag), IMAGE)
        for edit in (lambda b:b.update(status='FAILURE'), lambda b:b['results']['images'][0].update(name=guard.REPOSITORY+':other'), lambda b:b['results']['images'][0].update(digest='sha256:'+'x'*64), lambda b:b['results']['images'].append(b['results']['images'][0])):
            b=copy.deepcopy(build);edit(b)
            with self.assertRaises(guard.Invalid):guard.build_image(b,tag)

    def test_happy_and_zero_traffic_tags(self):
        d=inputs();self.assertEqual(call(d),NEW)
        d['before']['status']['traffic'].append({'revisionName':OLD,'percent':0})
        d['after']['status']['traffic'].append({'revisionName':OLD,'percent':0})
        self.assertEqual(call(d),NEW)
    def test_rejects_failures(self):
        mutations={
          'wrong project namespace':lambda d:d['before']['metadata'].update(namespace='other'),
          'wrong region':lambda d:d['before']['metadata']['labels'].update({'cloud.googleapis.com/location':'elsewhere'}),
          'changed UID':lambda d:d['after']['metadata'].update(uid='replacement'),
          'wrong kind':lambda d:d['before'].update(kind='Revision'),
          'wrong revision namespace':lambda d:d['revision_doc']['metadata'].update(namespace='other'),
          'wrong revision region':lambda d:d['revision_doc']['metadata']['labels'].update({'cloud.googleapis.com/location':'elsewhere'}),
          'malformed condition':lambda d:d['before']['status']['conditions'].append('bad'),
          'missing traffic percent':lambda d:d['before']['status']['traffic'].append({'revisionName':OLD}),
          'missing traffic revision':lambda d:d['before']['status']['traffic'].append({'percent':0}),
          'health wrong service':lambda d:d['health'].update(service='oa-api'),
          'health wrong revision':lambda d:d['health'].update(revision=OLD),
          'failed deploy':lambda d:d.update(deploy_exit=1),
          'bad HTTP':lambda d:d.update(http_status=503),
          'tag in input':lambda d:d.update(image=guard.REPOSITORY+':latest'),
          'wrong repository':lambda d:d.update(image='attacker.example/image@sha256:'+'a'*64),
          'upper digest':lambda d:d.update(image=guard.REPOSITORY+'@sha256:'+'A'*64),
          'invalid digest suffix':lambda d:d.update(image=IMAGE+'-suffix'),
          'service tag substituted':lambda d:d['before']['spec']['template']['spec']['containers'][0].update(image=guard.REPOSITORY+':latest'),
          'service digest substituted':lambda d:d['before']['spec']['template']['spec']['containers'][0].update(image=OTHER),
          'revision digest substituted':lambda d:d['revision_doc']['status'].update(imageDigest=OTHER),
          'revision spec tag':lambda d:d['revision_doc']['spec']['containers'][0].update(image=guard.REPOSITORY+':latest'),
          'revision spec different digest':lambda d:d['revision_doc']['spec']['containers'][0].update(image=OTHER),
          'previous stale generation':lambda d:d['pre']['status'].update(observedGeneration=0),
          'previous not ready':lambda d:d['pre']['status']['conditions'][0].update(status='False'),
          'previous revision stale':lambda d:d['pre']['status'].update(latestReadyRevisionName=NEW,latestCreatedRevisionName=NEW),
          'same generation':lambda d:(d['pre']['metadata'].update(generation=2),d['pre']['status'].update(observedGeneration=2)),
          'service stale generation':lambda d:d['before']['status'].update(observedGeneration=1),
          'service not ready':lambda d:d['before']['status']['conditions'][0].update(status='False'),
          'service duplicate readiness':lambda d:d['before']['status']['conditions'].append({'type':'Ready','status':'True'}),
          'wrong created revision':lambda d:d['before']['status'].update(latestCreatedRevisionName=OLD),
          'traffic old':lambda d:d['before']['status']['traffic'][0].update(revisionName=OLD),
          'traffic split':lambda d:d['before']['status'].update(traffic=[{'revisionName':NEW,'percent':90},{'revisionName':OLD,'percent':10}]),
          'traffic missing':lambda d:d['before']['status'].update(traffic=[]),
          'traffic boolean':lambda d:d['before']['status']['traffic'][0].update(percent=True),
          'changed after health':lambda d:(d['after']['metadata'].update(generation=3),d['after']['status'].update(observedGeneration=3)),
          'changed revision after health':lambda d:d['after']['status'].update(latestReadyRevisionName=OLD,latestCreatedRevisionName=OLD),
          'wrong revision name':lambda d:d['revision_doc']['metadata'].update(name=OLD),
          'revision wrong owner':lambda d:d['revision_doc']['metadata']['labels'].update({'serving.knative.dev/service':'oa-api'}),
          'revision stale generation':lambda d:d['revision_doc']['status'].update(observedGeneration=0),
          'revision not ready':lambda d:d['revision_doc']['status']['conditions'][0].update(status='False'),
          'health unhealthy':lambda d:d['health'].update(ok=False),
          'old healthy build':lambda d:d['health'].update(build='0'*40),
          'same commit old release':lambda d:d['health'].update(release='123e4567-e89b-42d3-a456-426614174001'),
          'short commit':lambda d:d.update(commit='abcdef0'),
          'bad release':lambda d:d.update(release='same-release'),
          'unapproved service':lambda d:d.update(service='attacker-service'),
        }
        for label,change in mutations.items():
            with self.subTest(label=label):
                d=inputs();change(d)
                with self.assertRaises(guard.Invalid):call(d)
    def test_bounded_input_and_malformed_shapes(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'sparse.json'
            with p.open('wb') as f: f.truncate(32 * 1024 * 1024)
            real_open = Path.open
            requests = []
            class BoundedFile:
                def __init__(self, file): self.file = file
                def __enter__(self): return self
                def __exit__(self, *args): return self.file.__exit__(*args)
                def read(self, n): requests.append(n); return self.file.read(n)
            def tracked_open(path, *args, **kwargs): return BoundedFile(real_open(path, *args, **kwargs))
            with mock.patch.object(Path, 'open', tracked_open):
                with self.assertRaises(guard.Invalid): guard.read_json(p)
            self.assertEqual(requests, [guard.MAX_JSON + 1])
            p.write_bytes(b' ' * (guard.MAX_JSON - 2) + b'{}')
            self.assertEqual(guard.read_json(p), {})
            p.write_bytes(b' ' * (guard.MAX_JSON - 1) + b'{}')
            with self.assertRaises(guard.Invalid): guard.read_json(p)
            p.write_bytes(b'{"nested":{"x":1,"x":2}}')
            with self.assertRaises(guard.Invalid): guard.read_json(p)
            p.write_bytes(b'{"bad":"\xff"}')
            with self.assertRaises(UnicodeError): guard.read_json(p)

    def test_cli_and_duplicate_or_invalid_json(self):
        d=inputs()
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); files={}
            for field in ('pre','before','revision_doc','after','health'):
                p=root/(field+'.json');p.write_text(json.dumps(d[field]));files[field]=p
            cmd=[sys.executable,str(Path(guard.__file__)),'--pre',str(files['pre']),'--before',str(files['before']),
                 '--revision',str(files['revision_doc']),'--after',str(files['after']),'--health',str(files['health']),
                 '--service',SERVICE,'--image',IMAGE,'--commit',COMMIT,'--release',RELEASE,'--deploy-exit','0','--http-status','200']
            self.assertEqual(subprocess.run(cmd,capture_output=True).returncode,0)
            files['health'].write_text('{"ok":true,"ok":true}')
            self.assertNotEqual(subprocess.run(cmd,capture_output=True).returncode,0)
            files['health'].write_text('{"ok":NaN}')
            self.assertNotEqual(subprocess.run(cmd,capture_output=True).returncode,0)
            files['health'].write_bytes(b' '* (guard.MAX_JSON+1))
            self.assertNotEqual(subprocess.run(cmd,capture_output=True).returncode,0)
            # Failure must be handled before attempting to read absent snapshots.
            failed=cmd.copy();failed[failed.index('--deploy-exit')+1]='1';files['pre'].unlink()
            p=subprocess.run(failed,capture_output=True,text=True)
            self.assertNotEqual(p.returncode,0);self.assertIn('deploy command failed',p.stderr)

if __name__=='__main__':unittest.main()
