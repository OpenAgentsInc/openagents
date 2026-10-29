#!/usr/bin/env python3
"""Public artifact evidence refuses stale and foreign build identities."""
import copy,unittest
from ecdh_owned_vectors import verify_required_inputs
class ManifestIdentity(unittest.TestCase):
 def test_required_identity(self):
  required={'/repo/coax/bin/coax':'a'*64,'/repo/coax/lib/crypto/ecdh.coax':'b'*64,'/repo/coax/lib/crypto/platform/ecdh_owned.c':'c'*64}
  manifest={'inputs':dict(required)};self.assertTrue(verify_required_inputs(manifest,required))
  for path in required:
   stale=copy.deepcopy(manifest);stale['inputs'][path]='0'*64;self.assertFalse(verify_required_inputs(stale,required))
   foreign=copy.deepcopy(manifest);foreign['inputs']['/other'+path]=foreign['inputs'].pop(path);self.assertFalse(verify_required_inputs(foreign,required))
  self.assertFalse(verify_required_inputs({},required));self.assertFalse(verify_required_inputs(manifest,{}))
if __name__=='__main__':unittest.main()
