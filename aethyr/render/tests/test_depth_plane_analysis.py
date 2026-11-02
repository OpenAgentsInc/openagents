#!/usr/bin/env python3
# Firm reason: 2 (pins the independent Fraction area/barycentric/depth oracle with separately expanded rational answers and quantization tie cases; it runs no Coax product logic).
import unittest
from fractions import Fraction as F
from analyze_depth_plane import prediction,f32
from analyze_fragment_planes import plane
class ExactPlanes(unittest.TestCase):
 def test_authored_triangle_has_independently_expanded_integer_area_weights(self):
  # In units of1/8 pixel the signed area is174353, with sample weights
  #75617,48972,49764. Expand the two affine numerators independently.
  self.assertEqual(prediction(0,0,'unsnapped'),[f32(F(567713,1394824)),f32(F(322061,697412)),1,1])
  self.assertEqual(prediction(0,1,'unsnapped'),[f32(F(567713,1394824)),f32(F(348131,798524)),f32(F(199631,174353)),1])
 def test_inside_bin_and_halfway_tie_are_distinct_from_unsnapped_interpolation(self):
  base=prediction(0,0,'unsnapped');self.assertEqual(prediction(F(1,1024),0,'positive8'),base);self.assertNotEqual(prediction(F(1,1024),0,'unsnapped'),base)
  self.assertEqual(prediction(F(2,1024),0,'even8'),base);self.assertEqual(prediction(F(2,1024),0,'positive8'),prediction(F(3,1024),0,'positive8'))
 def test_fragment_plane_reconstruction_retains_original_clip_coordinates(self):
  r={'x':0,'y':0,'vertices':[{'clip':[-1,1,.1,1]},{'clip':[1,1,.5,1]},{'clip':[-1,-1,.9,1]}]}
  q=plane(r,2,2,'even8');self.assertEqual(q['weights'],[.5,.25,.25]);self.assertEqual(q['depth'],f32(.4))
if __name__=='__main__':unittest.main()
