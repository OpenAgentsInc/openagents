#!/usr/bin/env python3
"""Focused source spatial-token mutation CLI/worker/selection qualification."""
from pathlib import Path
import os
import re
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
ARTIFACTS=os.environ.get('COAX_TEST_ARTIFACTS')

class SpatialOperators(unittest.TestCase):
    def setUp(self):
        if ARTIFACTS:
            self.work=Path(ARTIFACTS)/self._testMethodName
            self.work.mkdir(parents=True,exist_ok=False)
        else:
            temp=tempfile.TemporaryDirectory(prefix='coax-spatial-operators-')
            self.addCleanup(temp.cleanup);self.work=Path(temp.name)

    def invoke(self,name,label,*flags):
        r=subprocess.run([str(COMPILER),'mutate',str(ROOT/'coax/tests/spatial'/name),'--library','--root-only',*flags],cwd=ROOT,text=True,capture_output=True,timeout=120)
        (self.work/(label+'.stdout')).write_text(r.stdout)
        (self.work/(label+'.stderr')).write_text(r.stderr)
        return r

    def passed(self,r,sites):
        self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.assertIn(f'{sites} mutant'+('s' if sites!=1 else '')+f', {sites} killed, 0 survived',r.stdout)
        self.assertNotIn('marked equivalent',r.stdout)

    def trace(self,r):
        return re.findall(r'^mutation-outcome (\d+) (-?\d+)$',r.stderr,re.M)

    def test_exact_twelve_sites_workers_and_shard_merge(self):
        serial=self.invoke('operators.coax','serial','--jobs','1','--mutation-trace')
        self.passed(serial,12)
        self.assertIn('12 eligible token mutations, 6 no admitted alternate, 2 unsupported unary operations',serial.stderr)
        self.assertEqual(self.trace(serial),[(str(i),'0' if i<6 else '1') for i in range(12)])
        parallel=self.invoke('operators.coax','parallel','--jobs','4','--mutation-trace')
        self.passed(parallel,12);self.assertEqual(serial.stdout,parallel.stdout);self.assertEqual(self.trace(serial),self.trace(parallel))
        records=[]
        for shard in [1,2]:
            r=self.invoke('operators.coax',f'shard-{shard}','--jobs','2','--shard',f'{shard}/2','--source-order')
            self.assertEqual(r.returncode,0,r.stdout+r.stderr)
            self.assertTrue(r.stdout.startswith('coax-mutation-shard v1\n'),r.stdout)
            path=self.work/f'{shard}.rec';path.write_text(r.stdout);records.append(path)
        merged=self.invoke('operators.coax','merged','--merge',str(records[0]),'--merge',str(records[1]),'--mutation-trace')
        self.passed(merged,12);self.assertEqual(serial.stdout,merged.stdout);self.assertEqual(self.trace(serial),self.trace(merged))

    def test_authored_owner_drives_selective_and_reference_checks(self):
        selected=self.invoke('operators-selective.coax','selective','--jobs','1','--mutation-stats','--mutation-trace')
        reference=self.invoke('operators-selective.coax','reference','--jobs','1','--mutation-stats','--mutation-trace','--mutation-reference')
        self.passed(selected,1);self.passed(reference,1)
        self.assertEqual(self.trace(selected),[('0','1')]);self.assertEqual(self.trace(selected),self.trace(reference))
        self.assertIn('mutation-scheduling selective checks=1 skipped=1',selected.stderr)
        self.assertIn('mutation-scheduling full checks=2 skipped=0',reference.stderr)

    def test_a_weak_check_leaves_a_real_token_mutant_alive(self):
        r=self.invoke('operators-weak.coax','weak','--jobs','1')
        self.assertNotEqual(r.returncode,0)
        self.assertIn('1 mutant, 0 killed, 1 survived',r.stdout)
        self.assertIn("spatial source '*' became '/'",r.stdout)
        self.assertNotIn('marked equivalent',r.stdout)

    def test_non_alternates_and_unary_are_not_false_successes(self):
        for source,inventory in [('operators-no-alternate.coax','0 eligible token mutations, 1 no admitted alternate, 0 unsupported unary operations'),('operators-unary.coax','0 eligible token mutations, 0 no admitted alternate, 1 unsupported unary operations')]:
            r=self.invoke(source,source,'--jobs','1')
            self.assertNotEqual(r.returncode,0);self.assertIn('0 mutants, 0 killed, 0 survived',r.stdout);self.assertIn('no reachable mutation sites',r.stdout);self.assertIn(inventory,r.stderr)

    def test_operator_and_operand_indices_coexist(self):
        r=self.invoke('operators-index.coax','indices','--jobs','1','--mutation-trace')
        self.passed(r,3);self.assertIn('1 eligible token mutations',r.stderr)
        self.assertEqual(self.trace(r),[('0','0'),('1','0'),('2','0')])

    def test_shared_normal_helper_stays_local_under_full_reference_checks(self):
        r=self.invoke('operators-shared.coax','shared','--jobs','1','--mutation-reference','--mutation-trace')
        self.passed(r,2);self.assertIn('2 eligible token mutations',r.stderr)
        self.assertEqual(self.trace(r),[('0','0'),('1','1')])

    def test_normal_program_at_instance_limit_refuses_incomplete_mutation_preparation(self):
        prelude='#load "'+str(ROOT/'coax/tests/spatial/operator-types.coax')+'";\n'
        # The instance limit is sized from the program (#12653): 256 below 2056
        # syntax nodes, then one per 8 more. So the 254 instances are struct
        # parameters, 3 syntax nodes each (as in tests/generics.py), to keep the
        # program small and its limit at the 256 floor.
        params=', '.join('a'+str(i)+':CapacityTag('+str(i)+')' for i in range(1,255))
        source=prelude+"""
CapacityTag :: struct($N) {values:[$N]f64;}
capacity_work :: (PARAMS) {}
scale_at_limit :: (v:Vector3(World,u.Metre,f64),s:f64)->Vector3(World,u.Metre,f64) {return v*s;}
#test "normal instance ceiling stays admissible until mutation preparation" {
 v:Vector3(World,u.Metre,f64);v.x=8;r:=scale_at_limit(v,2);#assert r.x==16;
}
""".replace('PARAMS',params)
        path=self.work/'capacity.coax';path.write_text(source)
        baseline=subprocess.run([str(COMPILER),'c',str(path),'--library'],cwd=ROOT,text=True,capture_output=True,timeout=120)
        (self.work/'normal.c').write_text(baseline.stdout);(self.work/'normal.stderr').write_text(baseline.stderr)
        self.assertEqual(baseline.returncode,0,baseline.stderr)
        changed=subprocess.run([str(COMPILER),'mutate',str(path),'--library','--root-only','--jobs','1'],cwd=ROOT,text=True,capture_output=True,timeout=120)
        (self.work/'prepare.stdout').write_text(changed.stdout);(self.work/'prepare.stderr').write_text(changed.stderr)
        self.assertNotEqual(changed.returncode,0);self.assertIn('E0339',changed.stderr)
        self.assertNotIn(' killed, ',changed.stdout);self.assertNotIn('no reachable mutation sites',changed.stdout)

    def test_side_effectful_operands_keep_order_and_once_contract(self):
        r=self.invoke('operators-order.coax','order','--jobs','1')
        self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.assertIn('1 eligible token mutations',r.stderr)
        self.assertRegex(r.stdout,r'\d+ mutants, \d+ killed, 0 survived')
        self.assertNotIn('marked equivalent',r.stdout)

if __name__=='__main__':unittest.main()
