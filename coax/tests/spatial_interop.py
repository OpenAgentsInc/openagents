#!/usr/bin/env python3
"""New spatial/unit/module/generic seams, independent of old release gates."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax")).resolve()
FRAME = "struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f64) {}"
PRELUDE = f'#load "{ROOT / "coax/lib/quantity.coax"}" as u;\nWorld :: {FRAME}\n'


class SpatialInterop(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="coax-spatial-interop-")
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)

    def invoke(self, source, *, library=True):
        path = self.work / "case.coax"
        path.write_text(PRELUDE + source + "\n")
        return subprocess.run(
            [str(COMPILER), "c", str(path), *(["--library"] if library else [])],
            cwd=ROOT, text=True, capture_output=True, timeout=120,
        )

    def test_unit_form_cancellation_and_velocity_vectors(self):
        result = self.invoke('''
same :: (p:Point3(World,UnitQuotient(UnitProduct(u.Metre,u.Second),u.Second),f64))->Point3(World,u.Metre,f64) {return p;}
delta :: (p:Point3(World,u.Metre,f64),q:Point3(World,u.Metre,f64))->Vector3(World,UnitQuotient(UnitProduct(u.Second,u.Metre),u.Second),f64) {return p-q;}
twice :: (v:Vector3(World,UnitQuotient(u.Metre,u.Second),f64))->Vector3(World,UnitQuotient(u.Metre,u.Second),f64) {return 2*v;}
calc :: ()->f64 {p:Point3(World,u.Metre,f64);p.x=6;q:Point3(World,u.Metre,f64);q.x=2;v:Vector3(World,UnitQuotient(u.Metre,u.Second),f64);v.x=3;return delta(same(p),q).x+twice(v).x;}
V :: #run calc();#assert V==10;
''')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_physically_length_compound_remains_nominal(self):
        admitted = self.invoke('''
f :: (p:Point3(World,UnitQuotient(UnitProduct(u.Metre,u.Metre),u.HalfMetre),f64))->f64 {return p.x;}
''')
        self.assertEqual(admitted.returncode, 0, admitted.stderr)
        refused = self.invoke('''
f :: (p:Point3(World,UnitQuotient(UnitProduct(u.Metre,u.Metre),u.HalfMetre),f64),v:Vector3(World,u.Metre,f64)) {x:=p+v;}
''')
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("E0834", refused.stderr)
        self.assertEqual(refused.stdout, "")

    def test_aliases_share_one_frame_but_equal_module_tags_do_not(self):
        for name in ("a", "b"):
            (self.work / f"{name}.coax").write_text(
                f'#module "spatial.interop.{name}";\n#export World;\nWorld :: {FRAME}\n'
            )
        loads = '#load "a.coax" as a;\n#load "a.coax" as again;\n#load "b.coax" as b;\n'
        admitted = self.invoke(loads + '''
same :: (p:Point3(a.World,u.Metre,f64))->Point3(again.World,u.Metre,f64) {return p;}
delta :: (p:Point3(a.World,u.Metre,f64),q:Point3(again.World,u.Metre,f64))->Vector3(a.World,u.Metre,f64) {return p-q;}
calc :: ()->f64 {p:Point3(a.World,u.Metre,f64);p.x=6;q:Point3(again.World,u.Metre,f64);q.x=2;return delta(same(p),q).x;}
V :: #run calc();#assert V==4;
main :: () {}
''', library=False)
        self.assertEqual(admitted.returncode, 0, admitted.stderr)
        refused = self.invoke(loads + '''
wrong :: (p:Point3(a.World,u.Metre,f64))->Point3(b.World,u.Metre,f64) {return p;}
main :: () {}
''', library=False)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("Point3", refused.stderr)
        self.assertEqual(refused.stdout, "")

    def test_copyable_identity_preserves_concrete_spatial_kinds(self):
        result = self.invoke('''
identity :: (x:$T:copyable)->$T {return x;}
calc :: ()->f64 {p:Point3(World,u.Metre,f64);p.x=6;v:Vector3(World,u.Metre,f64);v.x=2;a:=identity(p+v);b:=identity(v+v);return a.x+b.x;}
V :: #run calc();#assert V==12;
''')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_native_verification_preserves_generated_spatial_helpers(self):
        fixture = ROOT / "coax/tests/spatial/native-verification-integration.coax"
        result = subprocess.run(
            ["python3", str(ROOT / "coax/tools/native_check_execution.py"), str(fixture),
             "--compiler", str(COMPILER), "--keep", str(self.work / "native-artifacts"),
             "--out", str(self.work / "native-record.json")],
            cwd=ROOT, text=True, capture_output=True, timeout=120,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
