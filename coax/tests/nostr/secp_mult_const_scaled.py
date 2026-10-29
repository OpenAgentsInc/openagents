#!/usr/bin/env python3
"""Independent projective-scale peer probes for variable-base arithmetic."""
import random
import unittest
import secp_field as base
import secp_group as group
from scalar_candidate import scalar
from oracle import N
from secp_mult_const import VariableBase
class ScaledPeers(VariableBase):
 def test_scaled_random_peers(self):
  rng=random.Random(20261030)
  for label,lib in self.libs:
   for i in range(1000):
    q=rng.randrange(1,N);k=rng.getrandbits(256);z=rng.randrange(1,base.P)
    got=lib.smc_multiply(scalar(k),group.point(self.affine(q),z=z))
    self.assertTrue(got.ok,(label,i));self.check_point(got.value,self.affine(k*q))
if __name__=='__main__':unittest.main(defaultTest='ScaledPeers.test_scaled_random_peers')
