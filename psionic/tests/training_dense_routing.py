#!/usr/bin/env python3
"""New dense-unit dispatcher routes; spies never run Coax or model provisioning."""
from pathlib import Path
import os,shutil,subprocess,tempfile,unittest
ROOT=Path(__file__).resolve().parents[2]
class DenseRoutes(unittest.TestCase):
 def test_dense_aliases_delegate_before_provisioning(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);(root/'psionic/tests').mkdir(parents=True);shutil.copyfile(ROOT/'psionic/check.sh',root/'psionic/check.sh')
   child=root/'psionic/tests/check_training_dense_units.sh';child.write_text('#!/bin/bash\nprintf "%s\\n" "$COAX" "$COAX_CC" "$@" > "$ROUTE_SPY"\nexit 73\n');child.chmod(0o755)
   for unit in ('training-dense','device/training-dense','device/training-dense-kernels'):
    for alias in (unit,unit+'.coax','psionic/lib/'+unit+'.coax'):
     with self.subTest(alias=alias):
      spy=root/'spy';spy.unlink(missing_ok=True);env=dict(os.environ,COAX='/selected/compiler',COAX_CC='/selected/cc',ROUTE_SPY=str(spy))
      p=subprocess.run(['bash',str(root/'psionic/check.sh'),'--only',alias,'--jobs','3'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
      self.assertEqual(p.returncode,73,p.stderr.decode());self.assertEqual(spy.read_text().splitlines(),['/selected/compiler','/selected/cc','--mutate','--jobs','3'])
if __name__=='__main__':unittest.main()
