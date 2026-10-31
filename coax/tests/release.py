#!/usr/bin/env python3
"""Focused release/status generation and publication drift regressions."""
from pathlib import Path
import importlib.util,json,os,shutil,subprocess,tempfile,unittest
from unittest import mock
ROOT=Path(__file__).resolve().parents[2]
COAX=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
OUTPUTS=['coax/src/release.h','coax/release-status.json','coax/STATUS.md','coax/book/appendix-a-keywords.md','coax/generated/current.md','coax/generated/version-example.md']
MANAGED=['coax/README.md','coax/spec.md','coax/ROADMAP.md','coax/book/ch01-getting-started.md','coax/book/ch06-memory.md','coax/book/ch20-inside-coax0.md','coax/book/ch21-road-ahead.md','coax/book/ch22-organizing-programs.md','coax/book/appendix-b-error-codes.md']
class Release(unittest.TestCase):
 def test_compute_catalogue_records_shipped_group_kernels(self):
  data=json.loads((ROOT/'coax/release-status.json').read_text())
  features={x['id']:x for x in data['features']}
  self.assertIn('group kernels',features['target.cuda']['meaning'])
  self.assertIn('barriers',features['target.cuda']['meaning'])
  self.assertNotIn('no shared memory',features['target.cuda']['meaning'])
  self.assertNotIn('no shared memory',features['target.cuda-warp']['meaning'])
  self.assertEqual(features['target.cuda']['status'],'partial')
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory(prefix='coax-release-test-');self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
  for name in OUTPUTS+MANAGED+['scripts/coax-release.sh','coax/release.coax','coax/release-model.coax']:
   p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/name,p)
  (self.root/'scripts/coax-release.sh').chmod(0o755)
 def tool(self,mode='--check',*args):
  return subprocess.run([str(self.root/'scripts/coax-release.sh'),mode,*map(str,args)],capture_output=True,text=True,env={**os.environ,'COAX_RELEASE_COMPILER':str(COAX)},timeout=30)
 def test_cli_and_snapshots_have_one_identity(self):
  r=subprocess.run([str(COAX),'status','--json'],capture_output=True,text=True);self.assertEqual(r.returncode,0,r.stderr)
  data=json.loads(r.stdout);self.assertEqual(data,json.loads((ROOT/'coax/release-status.json').read_text()))
  self.assertEqual(subprocess.check_output([str(COAX),'version'],text=True).strip(),'coax0 '+data['implementation_version'])
  self.assertEqual(subprocess.check_output([str(COAX),'status'],text=True),(ROOT/'coax/STATUS.md').read_text())
  self.assertEqual(data['schema_version'],1);self.assertEqual(next(x for x in data['features'] if x['id']=='source.units')['introduced'],'0.11.0')
 def test_every_snapshot_drift_is_detected_and_repaired(self):
  for name in OUTPUTS:
   p=self.root/name;original=p.read_bytes();p.write_bytes(original+b'changed\n')
   r=self.tool();self.assertNotEqual(r.returncode,0,name);self.assertIn('drift',r.stderr)
   fixed=self.tool('--write');self.assertEqual(fixed.returncode,0,fixed.stderr);self.assertEqual(p.read_bytes(),original)
 def test_version_bump_preserves_schema_introductions_and_unmanaged_prose(self):
  for _ in range(2):
   old=json.loads((self.root/'coax/release-status.json').read_text())
   major,minor,patch=map(int,old['implementation_version'].split('.'));patch+=1;minor+=patch//1000;patch%=1000;major+=minor//1000;minor%=1000
   next_version=f'{major}.{minor}.{patch}'
   p=self.root/'coax/release.coax';p.write_text(p.read_text().replace('IMPLEMENTATION_VERSION :: "'+old['implementation_version']+'"','IMPLEMENTATION_VERSION :: "'+next_version+'"'))
   prose=self.root/'coax/README.md';prose.write_text(prose.read_text()+'\nHistorical introduction: 0.13.0.\n')
   self.assertNotEqual(self.tool().returncode,0)
   r=self.tool('--write');self.assertEqual(r.returncode,0,r.stderr)
   new=json.loads((self.root/'coax/release-status.json').read_text());self.assertEqual(new['implementation_version'],next_version);self.assertEqual(new['schema_version'],old['schema_version'])
   self.assertEqual(new['features'],old['features']);self.assertIn('Historical introduction: 0.13.0.',prose.read_text());self.assertIn('coax0 '+next_version,prose.read_text())
   self.assertEqual(self.tool().returncode,0)
 def test_invalid_metadata_never_overwrites_outputs(self):
  before={p:(self.root/p).read_bytes() for p in OUTPUTS};src=self.root/'coax/release.coax';authored=src.read_text()
  for old,new in [('status="works"','status="unknown"'),('section="Keywords"','section="Hidden"')]:
   src.write_text(authored.replace(old,new,1));r=self.tool('--write');self.assertNotEqual(r.returncode,0)
   self.assertEqual(before,{p:(self.root/p).read_bytes() for p in OUTPUTS})
 def test_missing_or_duplicate_managed_regions_fail(self):
  p=self.root/'coax/README.md';original=p.read_text();p.write_text(original.replace('<!-- coax-release:current:start -->','<!-- removed -->'))
  self.assertNotEqual(self.tool().returncode,0)
  p.write_text(original+'\n<!-- coax-release:current:start -->\nextra\n<!-- coax-release:current:end -->\n');self.assertNotEqual(self.tool().returncode,0)
 def test_malformed_region_cannot_partially_write_a_release(self):
  p=self.root/'coax/README.md';p.write_text(p.read_text().replace('<!-- coax-release:current:start -->','<!-- removed -->'))
  snapshot=self.root/'coax/STATUS.md';snapshot.write_text('pending local change\n')
  before={p:(self.root/p).read_bytes() for p in OUTPUTS+MANAGED}
  self.assertNotEqual(self.tool('--write').returncode,0)
  self.assertEqual(before,{p:(self.root/p).read_bytes() for p in OUTPUTS+MANAGED})
 @unittest.skipUnless(os.uname().sysname=='Darwin','Xcode SDK regression is macOS-only')
 def test_xcode_sdk_still_executes_catalogue_on_host(self):
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  with mock.patch.dict(os.environ,{'SDKROOT':sdk,'IPHONEOS_DEPLOYMENT_TARGET':'16.0'}):
   r=self.tool();self.assertEqual(r.returncode,0,r.stderr)
 def test_stale_binary_metadata_fails_even_with_matching_version(self):
  data=json.loads((self.root/'coax/release-status.json').read_text());version='coax0 '+data['implementation_version'];data['features'][0]['status']='designed'
  (self.root/'stale.json').write_text(json.dumps(data)+'\n');(self.root/'version').write_text(version+'\n')
  fake=self.root/'binary';fake.write_text('#!/bin/sh\ncd "'+str(self.root)+'"\nif [ "$1" = version ]; then cat version; elif [ "$2" = --json ]; then cat stale.json; else cat coax/STATUS.md; fi\n');fake.chmod(0o755)
  r=self.tool('--check','--binary',fake);self.assertNotEqual(r.returncode,0);self.assertIn('binary metadata',r.stderr)
 def test_representative_acceptance_claims_use_the_real_frontend(self):
  cases=[('values','main :: () {x:uint=3;data:[4]u8=---;data[0]=cast(u8)x;}',True),('barrier','main :: () {a:u32=3;b:u64=7;#assert value_barrier(a)==a && value_barrier(b)==b;}',True),('layout','Pair :: struct {x:int;}\nmain :: () {n:=type_info(Pair).size;}',True),('reserved','Bad :: union {x:int;}\nmain :: () {}',False),('borrow','bad :: ()->[]int {a:[2]int;return a[..];}\nmain :: () {}',False)]
  for name,text,success in cases:
   p=self.root/(name+'.coax');p.write_text(text);r=subprocess.run([str(COAX),'check',str(p)],capture_output=True,text=True)
   self.assertEqual(r.returncode==0,success,r.stderr)
 def test_publisher_rejects_drift_before_credentials_or_network(self):
  spec=importlib.util.spec_from_file_location('coax_publisher',ROOT/'scripts/publish-coax-book.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
  with mock.patch.object(mod.subprocess,'run',side_effect=subprocess.CalledProcessError(1,['drift'])) as run, mock.patch.object(mod,'token_from_file') as token, mock.patch.object(mod.urllib.request,'build_opener') as network, mock.patch('sys.argv',['publisher','--token-file','secret']):
   with self.assertRaises(subprocess.CalledProcessError):mod.main()
   run.assert_called_once();token.assert_not_called();network.assert_not_called()
 def test_publisher_rejects_book_claim_drift_before_credentials_or_network(self):
  spec=importlib.util.spec_from_file_location('coax_publisher_claims',ROOT/'scripts/publish-coax-book.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
  failure=subprocess.CalledProcessError(1,['book-claims'])
  with mock.patch.object(mod.subprocess,'run',side_effect=[subprocess.CompletedProcess(['release'],0),failure]) as run, mock.patch.object(mod,'token_from_file') as token, mock.patch.object(mod.urllib.request,'build_opener') as network, mock.patch('sys.argv',['publisher','--token-file','secret']):
   with self.assertRaises(subprocess.CalledProcessError):mod.main()
   self.assertEqual(run.call_count,2);token.assert_not_called();network.assert_not_called()
if __name__=='__main__':unittest.main()
