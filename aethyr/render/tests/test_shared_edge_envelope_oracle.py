#!/usr/bin/env python3
# Firm reason: 2 (independent rational segment-hull, exact float decoding and geometry-census oracle controls; no renderer or comparator implementation is loaded).
import itertools
import struct
import unittest
from fractions import Fraction as F
from shared_edge_envelope_oracle import reachable, nearest, decode_f32, derive

def lattice_band(a,b,p,r):
    if a==b:return False
    dx,dy=b[0]-a[0],b[1]-a[1]
    return (min(a[0],b[0])-r<=p[0]<=max(a[0],b[0])+r and
            min(a[1],b[1])-r<=p[1]<=max(a[1],b[1])+r and
            abs(dx*(p[1]-a[1])-dy*(p[0]-a[0]))<=r*(abs(dx)+abs(dy)))

def vertex(clip,world):return struct.pack('<14f',*clip,*world,0,0,1,1,1,1,0)
def pair(shift=0,degenerate=False,clip_change=False):
    a=vertex((-.5,-.5,.5,1),(0,0,0));b=vertex((.5,.5,.5,1),(1,0,0));c=vertex((-.5,.5,.5,1),(0,1,0))
    aa=vertex((-.5+shift,-.5,.5,1),(0,0,0));bb=vertex((.5,.5,.5+(0.125 if clip_change else 0),1),(1,0,0));d=vertex((.5,-.5,.5,1),(2 if degenerate else 0,0,0 if degenerate else 1))
    return a+b+c+aa+bb+d

class GeometryEnvelope(unittest.TestCase):
    def test_rational_segment_intersection_equals_independently_expanded_hull(self):
        vertices=list(itertools.product((-2,0,2),repeat=2))
        for a,b,p,r in itertools.product(vertices,vertices,itertools.product(range(-3,4),repeat=2),(0,1)):
            self.assertEqual(reachable(a,b,p,r) is not None,lattice_band(a,b,p,r),(a,b,p,r))
    def test_exact_known_points_and_finite_segment_controls(self):
        self.assertIsNotNone(reachable((22176,45526),(21556,36052),(21760,39168),1))
        self.assertIsNone(reachable((22176,45526),(21556,36052),(21760,39168),0))
        self.assertIsNotNone(reachable((134724,45340),(134136,55518),(134400,50944),1))
        self.assertIsNone(reachable((134724,45340),(134136,55518),(134400,50944),0))
        self.assertEqual(reachable((0,0),(4,4),(-1,-1),1),(F(0),F(0)))
        self.assertIsNone(reachable((0,0),(4,4),(-2,-2),1))
    def test_exact_float_decoding_and_both_profile_ties(self):
        self.assertEqual(decode_f32(0x3f800000),1);self.assertEqual(decode_f32(0x80000000),0)
        self.assertEqual(decode_f32(1),F(1,2**149));self.assertEqual(decode_f32(0x7f7fffff),(2**24-1)*2**104)
        for n,even,positive in [(F(1,2),0,1),(F(3,2),2,2),(F(-1,2),0,0),(F(-3,2),-2,-1)]:
            self.assertEqual(nearest(n,'nearest-even'),even);self.assertEqual(nearest(n,'nearest-positive'),positive)
        for bad in (0x7f800000,0xff800000,0x7fc00001):
            with self.assertRaises(ValueError):decode_f32(bad)
    def test_global_geometry_matches_full_endpoint_keys_and_excludes_degenerates(self):
        result=derive(pair(),4,4,'nearest-even')
        self.assertEqual(result['census']['triangles'],2);self.assertEqual(result['census']['edge_occurrences'],6)
        self.assertEqual(result['census']['eligible_shared_fold_edges'],1);self.assertEqual(result['mask'],bytes([0,0,0,0,0,0,1,0,0,1,0,0,0,0,0,0]))
        self.assertEqual(derive(pair(shift=F(1,16)),4,4,'nearest-even')['census']['exact_shared_edges'],0)
        self.assertEqual(derive(pair(clip_change=True),4,4,'nearest-even')['census']['exact_shared_edges'],0)
        self.assertEqual(derive(pair(degenerate=True),4,4,'nearest-even')['census']['eligible_shared_fold_edges'],0)
    def test_noncoplanar_fold_is_not_selected_by_projected_coincidence(self):
        data=bytearray(pair());struct.pack_into('<3f',data,5*56+16,0,2,0)
        result=derive(bytes(data),4,4,'nearest-even');self.assertEqual(result['census']['eligible_shared_coplanar_edges'],1);self.assertEqual(sum(result['mask']),0)
    def test_invalid_packet_and_nonfinite_any_input_word_are_fatal(self):
        for data in (b'',pair()[:-1]):
            with self.assertRaises(ValueError):derive(data,4,4,'nearest-even')
        for offset in (0,4*4,13*4):
            data=bytearray(pair());struct.pack_into('<I',data,offset,0x7fc00001)
            with self.assertRaises(ValueError):derive(bytes(data),4,4,'nearest-even')
        with self.assertRaises(ValueError):derive(pair(),4,4,'analytic')

if __name__=='__main__':unittest.main()
