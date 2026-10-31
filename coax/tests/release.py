#!/usr/bin/env python3
"""Focused release/status generation and publication drift regressions."""
from pathlib import Path
import importlib.util,json,os,re,shlex,shutil,subprocess,tempfile,unittest
from unittest import mock
ROOT=Path(__file__).resolve().parents[2]
COAX=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
OUTPUTS=['coax/src/release.h','coax/release-status.json','coax/STATUS.md','coax/book/appendix-a-keywords.md','coax/generated/current.md','coax/generated/version-example.md']
CHANGELOG='coax/CHANGELOG.md'
CHANGELOG_TOOLS=['coax/changelog-model.coax','coax/changelog.coax','coax/changelog-bridge.c']
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
  for name in [CHANGELOG]+CHANGELOG_TOOLS:
   p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/name,p)
  (self.root/'scripts/coax-release.sh').chmod(0o755)
 def tool(self,mode='--check',*args):
  return subprocess.run([str(self.root/'scripts/coax-release.sh'),mode,*map(str,args)],capture_output=True,text=True,env={**os.environ,'COAX_RELEASE_COMPILER':str(COAX)},timeout=30)
 def snapshot(self):
  return {name:(self.root/name).read_bytes() if (self.root/name).exists() else None for name in OUTPUTS+MANAGED+[CHANGELOG]}
 def next_version(self):
  old=json.loads((self.root/'coax/release-status.json').read_text())['implementation_version']
  major,minor,patch=map(int,old.split('.'));patch+=1;minor+=patch//1000;patch%=1000;major+=minor//1000;minor%=1000
  return f'{major}.{minor}.{patch}'
 def author_bump(self,version,commit='pending'):
  src=self.root/'coax/release.coax';src.write_text(re.sub(r'IMPLEMENTATION_VERSION :: "[^\"]+"',f'IMPLEMENTATION_VERSION :: "{version}"',src.read_text(),count=1))
  path=self.root/CHANGELOG;text=path.read_text();at=text.index('## ')
  path.write_text(text[:at]+f'## {version} — 2026-10-31\n\nCommit: {commit}\n\n- Fix the bounded release regression fixture.\n\n'+text[at:])
 def git(self,*args):
  r=subprocess.run(['git','-C',str(self.root),*args],capture_output=True,text=True,env={**os.environ,'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull},timeout=10)
  self.assertEqual(r.returncode,0,r.stderr);return r.stdout.strip()
 def test_changelog_is_required_and_validated_before_any_write(self):
  path=self.root/CHANGELOG;original=path.read_text();heading=re.search(r'^## ([0-9.]+) — ([0-9-]+)$',original,re.M)
  self.assertIsNotNone(heading)
  first_end=original.find('\n## ',heading.end());first_end=len(original) if first_end<0 else first_end
  first=original[heading.start():first_end]
  commit=re.search(r'^Commit: .+$',original,re.M).group()
  bad_cases={
   'missing':None,
   'stale-current-version':original.replace(heading.group(),heading.group().replace(heading.group(1),'99.99.99'),1),
   'duplicate':original+'\n'+first,
   'bad-date':original.replace(heading.group(2),'2026-02-30',1),
   'empty-summary':original[:heading.start()]+original[heading.start():first_end].split('\n- ',1)[0]+'\n'+original[first_end:],
   'todo-summary':original[:heading.start()]+first.split('\n- ',1)[0]+'\n- TODO\n'+original[first_end:],
   'bad-commit':original.replace(commit,'Commit: this-is-not-a-commit',1),
  }
  for name,bad in bad_cases.items():
   for mode in ['--check','--write']:
    with self.subTest(case=name,mode=mode):
     if bad is None:path.unlink(missing_ok=True)
     else:path.write_text(bad)
     before=self.snapshot();r=self.tool(mode)
     self.assertEqual((r.returncode==0,self.snapshot()==before),(False,True),r.stdout+r.stderr)
  path.write_text(original)
 def test_version_bump_without_new_changelog_entry_refuses(self):
  src=self.root/'coax/release.coax';src.write_text(re.sub(r'IMPLEMENTATION_VERSION :: "[^\"]+"',f'IMPLEMENTATION_VERSION :: "{self.next_version()}"',src.read_text(),count=1))
  for mode in ['--check','--write']:
   with self.subTest(mode=mode):
    before=self.snapshot();r=self.tool(mode)
    self.assertEqual((r.returncode==0,self.snapshot()==before),(False,True),r.stdout+r.stderr)
 def test_archive_accepts_concrete_history_but_check_rejects_pending(self):
  self.assertEqual(self.tool().returncode,0)
  path=self.root/CHANGELOG;path.write_text(re.sub(r'^Commit: .+$','Commit: pending',path.read_text(),count=1,flags=re.M))
  before=self.snapshot();r=self.tool();self.assertNotEqual(r.returncode,0,r.stdout+r.stderr);self.assertEqual(self.snapshot(),before)
 def test_git_bump_resolves_pending_and_repairs_rebased_hash(self):
  self.git('init','-q');self.git('config','user.name','Coax release test');self.git('config','user.email','coax-release-test@example.invalid');self.git('config','core.hooksPath',os.devnull);self.git('config','commit.gpgsign','false')
  self.git('add','.');self.git('commit','-qm','baseline release fixture')
  baseline=self.git('rev-parse','HEAD')
  fixed=self.tool('--write');self.assertEqual(fixed.returncode,0,fixed.stdout+fixed.stderr)
  self.assertIn(baseline,(self.root/CHANGELOG).read_text());self.assertEqual(self.tool().returncode,0)
  self.git('add','.');self.git('commit','-qm','record baseline identity')
  history=(self.root/CHANGELOG).read_text();old_entries=history[history.index('## '):]
  version=self.next_version();self.author_bump(version)
  path=self.root/CHANGELOG;pending_text=path.read_text();forged='2'*40
  path.write_text(pending_text.replace('Commit: pending',f'Commit: [`{forged}`](https://github.com/OpenAgentsInc/openagents/commit/{forged})',1))
  for mode in ['--check','--write']:
   before=self.snapshot();r=self.tool(mode);self.assertEqual((r.returncode==0,self.snapshot()==before),(False,True),r.stdout+r.stderr)
  path.write_text(pending_text)
  prepared=self.tool('--write');self.assertEqual(prepared.returncode,0,prepared.stdout+prepared.stderr)
  path=self.root/CHANGELOG;self.assertIn('Commit: pending',path.read_text());self.assertTrue(path.read_text().endswith(old_entries))
  self.assertNotEqual(self.tool().returncode,0)
  self.git('add','.');self.git('commit','-qm','introduce fixture version')
  introduced=self.git('rev-parse','HEAD');resolved=self.tool('--write');self.assertEqual(resolved.returncode,0,resolved.stdout+resolved.stderr)
  self.assertIn(introduced,path.read_text());self.assertNotIn('Commit: pending',path.read_text());self.assertTrue(path.read_text().endswith(old_entries));self.assertEqual(self.tool().returncode,0)
  self.git('add','.');self.git('commit','--amend','-qm','introduce fixture version after rebase')
  rebased=self.git('rev-parse','HEAD');self.assertNotEqual(introduced,rebased)
  self.assertNotEqual(self.tool().returncode,0)
  repaired=self.tool('--write');self.assertEqual(repaired.returncode,0,repaired.stdout+repaired.stderr)
  self.assertIn(rebased,path.read_text());self.assertNotIn(introduced,path.read_text());self.assertTrue(path.read_text().endswith(old_entries));self.assertEqual(self.tool().returncode,0)
 def test_cached_build_still_checks_changelog_before_install(self):
  build=self.root/'coax/build.sh';shutil.copyfile(ROOT/'coax/build.sh',build);build.chmod(0o755)
  cache=self.root/'cache/fixture';cache.mkdir(parents=True)
  compiler=cache/'coax';compiler.write_text('#!/bin/sh\nexec '+shlex.quote(str(COAX))+' "$@"\n');compiler.chmod(0o755)
  for name in ['lexer.c','lexer.h','parser.c','parser.h','checker.c','checker.h','sources.c','sources.h','proto_runtime.inc','proto_importer.c','proto_importer.h']:(cache/name).write_text('fixture artifact\n')
  provenance=self.root/'coax/provenance.sh';provenance.write_text('#!/bin/sh\ncase "$1" in\nfingerprint) echo fixture;;\ninstall) cp "$3/coax" "$2/coax/bin/coax"; chmod +x "$2/coax/bin/coax"; touch "$2/cache-installed";;\nverify) exit 0;;\n*) exit 2;;\nesac\n');provenance.chmod(0o755)
  env={**os.environ,'COAX_RELEASE_COMPILER':str(COAX),'COAX_RELEASE_CACHE':str(self.root/'cache'),'COAX_BUILD_FRESH':''}
  good=subprocess.run([str(build)],capture_output=True,text=True,env=env,timeout=30)
  self.assertEqual(good.returncode,0,good.stdout+good.stderr);marker=self.root/'cache-installed';self.assertTrue(marker.exists());marker.unlink()
  path=self.root/CHANGELOG;path.write_text(re.sub(r'^## [0-9.]+ —','## 99.99.99 —',path.read_text(),count=1,flags=re.M))
  bad=subprocess.run([str(build)],capture_output=True,text=True,env=env,timeout=30)
  self.assertNotEqual(bad.returncode,0,bad.stdout+bad.stderr);self.assertIn('changelog',bad.stderr.lower());self.assertFalse(marker.exists())
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
   old=json.loads((self.root/'coax/release-status.json').read_text());next_version=self.next_version()
   path=self.root/CHANGELOG;old_entries=path.read_text()[path.read_text().index('## '):]
   self.author_bump(next_version)
   prose=self.root/'coax/README.md';prose.write_text(prose.read_text()+'\nHistorical introduction: 0.13.0.\n')
   self.assertNotEqual(self.tool().returncode,0)
   r=self.tool('--write');self.assertEqual(r.returncode,0,r.stderr)
   new=json.loads((self.root/'coax/release-status.json').read_text());self.assertEqual(new['implementation_version'],next_version);self.assertEqual(new['schema_version'],old['schema_version'])
   self.assertEqual(new['features'],old['features']);self.assertIn('Historical introduction: 0.13.0.',prose.read_text());self.assertIn('coax0 '+next_version,prose.read_text())
   self.assertTrue(path.read_text().endswith(old_entries));self.assertIn('Commit: pending',path.read_text());self.assertNotEqual(self.tool().returncode,0)
   commit='1'*40;line=f'Commit: [`{commit}`](https://github.com/OpenAgentsInc/openagents/commit/{commit})'
   path.write_text(re.sub(r'^Commit: pending$',line,path.read_text(),count=1,flags=re.M))
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
