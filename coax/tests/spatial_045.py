#!/usr/bin/env python3
"""New 0.45 integration seam: named capacities with concrete spatial elements."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'coax/bin/coax')).resolve()
PRELUDE = ('#load "' + str(ROOT / 'coax/lib/quantity.coax') + '" as u;\n'
           '#load "' + str(ROOT / 'coax/lib/bounded.coax') + '" as buf;\n' + '''
World :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {}
Other :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {}
CAP :: 2;
identity :: (x:$T:copyable)->$T {return x;}
''')

class Spatial045(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='coax-spatial-045-')
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)

    def compile(self, source):
        path = self.work / 'case.coax'
        path.write_text(PRELUDE + source + '\n')
        return subprocess.run([str(COMPILER), 'c', str(path), '--library'],
                              cwd=ROOT, capture_output=True, text=True, timeout=120)

    def test_named_capacity_preserves_spatial_push_get_set_and_live_view(self):
        for representation, size in [('f32', 32), ('f64', 56)]:
            with self.subTest(representation=representation):
                source = '''
calc :: ()->R {
    points:[1]buf.Bounded(Point3(World,u.Metre,R),CAP);
    vectors:[1]buf.Bounded(Vector3(World,u.Metre,R),CAP);
    p:Point3(World,u.Metre,R);p.x=6;p.y=2;p.z=1;
    q:Point3(World,u.Metre,R);q.x=2;q.y=1;
    if !buf.bounded_push(points[..],p) || !buf.bounded_push(points[..],q) {return -1;}
    if buf.bounded_push(points[..],p) || points[0].count!=CAP || !buf.bounded_full(points[..]) {return -2;}
    got:=identity(buf.bounded_get(points[..],0));got.x=10;
    if buf.bounded_get(points[..],0).x!=6 {return -3;}
    if !buf.bounded_set(points[..],1,got) || buf.bounded_set(points[..],CAP,q) {return -4;}
    pv:=buf.bounded_view(points[..]);pv[0].y=3;
    if pv.count!=CAP || buf.bounded_get(points[..],0).y!=3 {return -5;}
    delta:=buf.bounded_get(points[..],1)-buf.bounded_get(points[..],0);
    if delta.x!=4 || delta.y!= -1 || delta.z!=0 {return -6;}
    if !buf.bounded_push(vectors[..],identity(delta)) || !buf.bounded_push(vectors[..],2*delta) {return -7;}
    if buf.bounded_push(vectors[..],delta) || !buf.bounded_set(vectors[..],0,-delta) {return -8;}
    vv:=buf.bounded_view(vectors[..]);vv[1]=vv[0]+vv[1];
    moved:=identity(buf.bounded_get(vectors[..],1)+buf.bounded_get(points[..],0));
    if moved.x!=10 || moved.y!=2 || moved.z!=1 {return -9;}
    return buf.bounded_get(points[..],0).y+buf.bounded_get(vectors[..],0).x+moved.x;
}
VALUE :: #run calc();#assert VALUE==9;
SIZE :: #run size_of(buf.Bounded(Point3(World,u.Metre,R),CAP));#assert SIZE==EXPECTED_SIZE;
'''.replace('EXPECTED_SIZE', str(size)).replace('R', representation)
                result = self.compile(source)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stderr, '')

    def test_named_capacity_does_not_erase_nominal_frame_on_inferred_push(self):
        result = self.compile('''
wrong :: (store:[]buf.Bounded(Point3(World,u.Metre,f32),CAP),p:Point3(Other,u.Metre,f32)) {
    accepted:=buf.bounded_push(store,p);
}
''')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')
        self.assertIn('E0348', result.stderr)
        self.assertIn('Point3(World,Metre,f32)', result.stderr)
        self.assertIn('Point3(Other,Metre,f32)', result.stderr)

if __name__ == '__main__':
    unittest.main()
