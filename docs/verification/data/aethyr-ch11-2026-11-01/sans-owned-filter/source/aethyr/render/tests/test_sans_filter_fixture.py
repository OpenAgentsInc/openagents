import unittest
from make_sans_filter_fixture import byte,expected,f,sample_level
class SansOracle(unittest.TestCase):
 def test_authored_pattern_retains_axes_mips_and_every_channel(self):
  self.assertEqual(byte(0,0,0),[0,0,0,0]);self.assertEqual(byte(1,1,4),[136,188,230,52]);self.assertEqual(byte(95,41,4),[62,138,8,22])
 def test_origin_and_outer_clamps_read_hand_pinned_texels(self):
  self.assertEqual(sample_level(-1,-1,4),[f(x/255)for x in [116,164,212,44]])
  self.assertEqual(sample_level(1,1,4),[f(x/255)for x in [62,138,8,22]])
  self.assertEqual(sample_level(.5/1536,.5/672,0),[0,0,0,0])
 def test_midpoints_and_mip_fraction_follow_independent_closed_forms(self):
  for x,y in zip(sample_level(1/1536,1/672,0),[10/255,12/255,9/255,4/255]):self.assertLessEqual(abs(x-y),1e-8)
  self.assertEqual(expected(dict(x=0,y=0,lod=.5)),[f(f(x/255)*.5)for x in [29,41,53,11]])
if __name__=='__main__':unittest.main()
