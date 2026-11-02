# Firm reason: 2 (pins the independent real-number filter/quantization oracle's endpoints, tie rounding, axis ordering and mip expressions; it runs no Coax product logic).
import unittest
from analyze_filter_precision import decode,mix,predict,snap

class FilterModels(unittest.TestCase):
 def test_rounds_and_endpoints_are_independently_pinned(self):
  self.assertEqual(snap(1/512,256),1/256)
  self.assertEqual(snap(1/512,256,'even'),0)
  self.assertEqual(snap(3/512,256,'even'),2/256)
  self.assertEqual(snap(0,4080),0);self.assertEqual(snap(1,4080),1)
  self.assertEqual(snap(0.5,65535),32768/65535)
  self.assertEqual(snap(26/256,4080),414/4080)
  self.assertEqual(snap(.1,64,'floor'),6/64)
  self.assertEqual(snap(.2,64,'floor'),12/64)
 def test_varied_rgba_and_axis_rounding_are_distinct(self):
  t={'format':'rgba8unorm','bytes':[13,191,57,239,241,37,199,83,103,227,5,149,67,17,211,31,173,73,131,197]}
  texels=decode(t);self.assertEqual(texels[0],[13/255,191/255,57/255,239/255])
  self.assertEqual(mix([0,1,2,3],[4,5,6,7],0.25),[1,2,3,4])
  c=dict(x=.1,y=.2,lod=0)
  self.assertNotEqual(predict(t,c,256,4080,'m',256),predict(t,c,256,4080,'xym',256))
 def test_nonbinary_mip_endpoint_is_not_synthesized(self):
  t={'format':'rgba8unorm','bytes':[0,0,0,0]*4+[173,73,131,197]}
  c=dict(x=.5,y=.5,lod=1)
  from analyze_filter_precision import f32
  self.assertEqual(predict(t,c,0,0,'',0),[f32(x/255)for x in [173,73,131,197]])
if __name__=='__main__':unittest.main()
