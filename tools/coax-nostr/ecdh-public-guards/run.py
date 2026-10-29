#!/usr/bin/env python3
"""Reviewed outer trust anchor for one conditional source-only decision."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[3]
INNER=ROOT/'tools/coax-nostr/ecdh-public-guards/check.py'
POLICY=ROOT/'tools/coax-nostr/ecdh-public-guards/policy.coax'
INNER_SHA='be8b8cc19a476a8d3f2e68ff2df24ce2fbac8b16553608b34cb63a3ab45500a4'
POLICY_SHA='f6d655997411d9f04dfb69a20d4cda8e5a7f2ef07a8a724f3830363ee2b67c3a'
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def isolated():
 prefix=sys.pycache_prefix
 if not sys.flags.isolated or not sys.flags.no_site or not sys.dont_write_bytecode or not prefix or Path(prefix).exists():raise ValueError('requires isolated Python without site initialization and an unused bytecode cache prefix')
 for key in os.environ:
  if key.startswith(('DYLD_','LD_','PYTHON')) or key in ('DEVELOPER_DIR','SDKROOT','MACOSX_DEPLOYMENT_TARGET','CPATH','C_INCLUDE_PATH','CPLUS_INCLUDE_PATH','LIBRARY_PATH','Z3_LIBRARY_PATH','CCC_OVERRIDE_OPTIONS'):
   raise ValueError('unreviewed loader or compiler environment')
 os.environ.clear();os.environ.update({'PATH':'/usr/bin:/bin','LC_ALL':'C'})
def unique(pairs):
 out={}
 for key,value in pairs:
  if key in out:raise ValueError('duplicate result key')
  out[key]=value
 return out

def validate(result):
 if not isinstance(result,dict) or set(result)!={'raw_audit','registry_match'}:raise ValueError('invalid provisional result')
 raw=result['raw_audit'];match=result['registry_match']
 if not isinstance(raw,dict) or not isinstance(match,dict):raise ValueError('invalid provisional record')
 if raw.get('accepted') is not False or raw.get('error')!=14 or raw.get('error_name')!='guard-obligations' or raw.get('source_status')!='analyzed-with-guard-obligations':raise ValueError('raw audit is not the reviewed refusal')
 if not isinstance(raw.get('authored_guard_obligations'),list) or len(raw['authored_guard_obligations'])!=2:raise ValueError('raw guard set missing')
 if match.get('schema')!='coax.reviewed-guard-registry-match.v1' or match.get('provisional') is not True or match.get('matched') is not True or match.get('verifier_sha256')!=INNER_SHA:raise ValueError('registry did not match')
 if not isinstance(match.get('policy_build'),dict) or match['policy_build'].get('policy_source_sha256')!=POLICY_SHA:raise ValueError('unreviewed registry')
 if match.get('target_timing_approved') is not False or match.get('erasure_approved') is not False or match.get('production_crypto_approved') is not False:raise ValueError('invalid approval flags')
 if not isinstance(match.get('trust_premises'),list) or not match['trust_premises']:raise ValueError('missing trust premises')

def verify(context=None):
 isolated();launcher_sha=sha(__file__)
 if sha(INNER)!=INNER_SHA or sha(POLICY)!=POLICY_SHA:raise ValueError('reviewed inner tool or registry changed')
 with tempfile.TemporaryDirectory(prefix='coax-guard-launch-') as td:
  command=[sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(Path(td)/'unused-cache'),str(INNER),'--reviewed-ecdh-entry']
  child=subprocess.run(command,capture_output=True,text=True,cwd=td)
  try:result=json.loads(child.stdout,object_pairs_hook=unique)
  except (ValueError,TypeError):raise ValueError('inner verifier returned invalid output')
  if isinstance(result,dict) and 'raw_audit' in result and context is not None:context['raw_audit']=result['raw_audit']
  if child.returncode!=0:raise ValueError('inner registry refused')
  if sha(INNER)!=INNER_SHA or sha(POLICY)!=POLICY_SHA or sha(__file__)!=launcher_sha:raise ValueError('reviewed tool changed during verification')
  validate(result);match=result['registry_match']
  decision={'schema':'coax.reviewed-guard-decision.v1','source_scope_accepted':True,'conditional':True,
   'trust_premises':['the reviewed outer launcher itself is a trusted code anchor; its hash is recorded but cannot self-authenticate']+match['trust_premises'],
   'launcher_sha256':launcher_sha,'inner_sha256':INNER_SHA,'registry_sha256':POLICY_SHA,
   'target_timing_approved':False,'erasure_approved':False,'production_crypto_approved':False}
  return {'raw_audit':result['raw_audit'],'registry_match':match,'source_scope_decision':decision}

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--reviewed-ecdh-entry',action='store_true');parser.add_argument('--report',type=Path);args=parser.parse_args()
 if not args.reviewed_ecdh_entry:parser.error('explicit --reviewed-ecdh-entry is required')
 context={}
 try:output=verify(context)
 except (ValueError,KeyError,TypeError,OSError,json.JSONDecodeError) as error:
  output={**context,'source_scope_decision':{'source_scope_accepted':False,'conditional':True,'reason':str(error),'target_timing_approved':False,'erasure_approved':False,'production_crypto_approved':False}}
 text=json.dumps(output,indent=2)+'\n'
 if args.report:
  with args.report.open('x') as stream:stream.write(text)
 else:print(text,end='')
 return 0 if output['source_scope_decision']['source_scope_accepted'] else 1
if __name__=='__main__':raise SystemExit(main())
