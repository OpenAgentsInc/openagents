#!/usr/bin/env python3
"""The optional oracle context contains only its pinned, reviewed source closure."""
import hashlib,json,pathlib,subprocess,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
class Context(unittest.TestCase):
 def test_pinned_minimal_source_and_runtime_component(self):
  with tempfile.TemporaryDirectory(prefix='att-context-') as d:
   out=pathlib.Path(d)/'new context';r=subprocess.run(['bash',str(ROOT/'scripts/prepare-confidential-oracle.sh'),str(out)],capture_output=True,text=True,timeout=15)
   self.assertEqual(r.returncode,0,r.stdout+r.stderr);m=json.loads((out/'source.json').read_text());self.assertEqual(m['source_commit'],'853417f290e7cae1df18c081b89b0818432e60af')
   self.assertEqual(m['cargo_lock_sha256'],hashlib.sha256((ROOT/'docs/verification/data/oct30-confidential/oracle-Cargo.lock').read_bytes()).hexdigest())
   self.assertEqual((out/'oracle/Cargo.lock').read_bytes(),(ROOT/'docs/verification/data/oct30-confidential/oracle-Cargo.lock').read_bytes())
   self.assertEqual(sorted(p.name for p in (out/'oracle/crates').iterdir()),['agent-contract','nostr','nostr-transport','oa-att','psionic','receipts'])
   self.assertEqual((out/'probe-confidential-pylon.sh').stat().st_mode&0o755,0o755);self.assertTrue((out/'confidential-pylon-summary.jq').stat().st_mode&0o004);self.assertFalse((out/'oracle/apps').exists());self.assertFalse((out/'oracle/crates/pylon').exists());self.assertIn('1.97.1-alpine@sha256:',(out/'Dockerfile').read_text());self.assertIn('FROM scratch',(out/'Dockerfile').read_text())
   expected=subprocess.check_output(['git','show',m['source_commit']+':crates/oa-att/src/bin/oa-att.rs'],cwd=ROOT)
   self.assertEqual((out/'oracle/crates/oa-att/src/bin/oa-att.rs').read_bytes(),expected)
   (out/'keep').write_text('keep');r=subprocess.run(['bash',str(ROOT/'scripts/prepare-confidential-oracle.sh'),str(out)],capture_output=True,text=True,timeout=15);self.assertNotEqual(r.returncode,0);self.assertEqual((out/'keep').read_text(),'keep')
if __name__=='__main__':unittest.main()
