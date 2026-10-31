#!/usr/bin/env python3
# Firm reason: 2 (runs independent native and WebAssembly memory/layout/byte oracles).
"""Focused product-reader qualification; no full site or compiler build."""
from pathlib import Path
import json, os, re, subprocess, sys, tempfile, unittest

ROOT=Path(__file__).resolve().parents[3]
COAX=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
sys.path.insert(0,str(ROOT/'coax/tests'))
from quiet_crash import run as native_run

class ChangelogReader(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-changelog-reader-');cls.addClassCleanup(cls.temp.cleanup)
  cls.work=Path(cls.temp.name);cls.c=cls.work/'reader.c';cls.header=cls.work/'changelog.h'
  content=(ROOT/'apps/openagents-web/changelog-content.coax').read_text();kinds=re.search(r'^CC_KIND :: \[CC_BLOCKS\]int\.\[(.*?)\];$',content,re.M)
  assert kinds is not None;cls.first_release=[int(k)for k in kinds[1].split(',')].index(1)
  with cls.c.open('w') as output:
   subprocess.run([str(COAX),'c','apps/openagents-web/changelog.coax','--library','--header',str(cls.header)],cwd=ROOT,stdout=output,check=True,timeout=30)
 def test_authored_product_checks(self):
  result=subprocess.run([str(COAX),'test','apps/openagents-web/changelog.coax','--only','changelog'],cwd=ROOT,capture_output=True,text=True,timeout=60)
  self.assertEqual(result.returncode,0,result.stdout+result.stderr)
 def native(self,name,flags):
  binary=self.work/name
  subprocess.run([os.environ.get('CC','cc'),'-std=c11',*flags,'-ffp-contract=off','-I',str(self.work),str(self.c),str(ROOT/'coax/web/tests/changelog_reader_native.c'),'-lm','-o',str(binary)],cwd=ROOT,check=True,timeout=45)
  env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0' if sys.platform=='darwin' else 'detect_leaks=1','UBSAN_OPTIONS':'halt_on_error=1'}
  result=native_run([str(binary),str(ROOT/'coax/CHANGELOG.md'),str(self.first_release)],cwd=ROOT,capture_output=True,text=True,env=env,timeout=30)
  self.assertEqual(result.returncode,0,result.stdout+result.stderr)
  record=json.loads(result.stdout);self.assertEqual(record['frames'],16);self.assertTrue(record['guard_bytes']);self.assertTrue(record['finite_geometry']);self.assertEqual(record['copy_bytes'],(ROOT/'coax/CHANGELOG.md').stat().st_size)
  print(name+': '+result.stdout.strip(),flush=True)
 def test_native_debug_and_optimized(self):
  self.native('debug',['-O0']);self.native('optimized',['-O2'])
 def test_native_sanitizers(self):
  self.native('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])
 def test_actual_wasm_abi(self):
  wasm=Path(os.environ.get('COAX_CHANGELOG_WASM',ROOT/'apps/openagents-web/build/changelog.wasm')).resolve()
  if not wasm.is_file():
   subprocess.run(['coax/web/wasm.sh','apps/openagents-web/changelog.coax','apps/openagents-web/changelog-face.c',str(wasm)],cwd=ROOT,check=True,timeout=180)
  result=subprocess.run(['node','coax/web/tests/changelog_reader_wasm.cjs',str(wasm),'coax/CHANGELOG.md',str(self.first_release)],cwd=ROOT,capture_output=True,text=True,timeout=60)
  self.assertEqual(result.returncode,0,result.stdout+result.stderr);record=json.loads(result.stdout);self.assertEqual(record['frames'],16);self.assertTrue(record['guard_bytes']);self.assertTrue(record['finite_geometry'])
  print('wasm: '+result.stdout.strip(),flush=True)

if __name__=='__main__':unittest.main()
