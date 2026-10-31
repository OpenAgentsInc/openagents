#!/usr/bin/env python3
"""Only new oracle admission guards; no model/native/SDK execution."""
import math,unittest
import panda_mass_solve_reference as ref
class Guards(unittest.TestCase):
    def test_finite_both_actual_and_reference(self):
        for value in (math.nan,math.inf,-math.inf):
            for actual,expected in (([0.,value],[0.,0.]),([0.,0.],[0.,value])):
                with self.subTest(value=value,actual=actual):
                    with self.assertRaisesRegex(RuntimeError,'finite'):ref.compare(actual,expected,2e-10,2e-12,'newguard')
    def test_complete_nonempty_extents(self):
        for actual,expected in (([],[]),([0.],[]),([0.],[0.,0.])):
            with self.subTest(actual=actual):
                with self.assertRaisesRegex(RuntimeError,'extent'):ref.compare(actual,expected,2e-10,2e-12,'newguard')
    def test_fixed_absolute_plus_relative_budget(self):
        self.assertTrue(ref.compare([2e-10],[0.],2e-10,2e-12,'newguard')['passed'])
        with self.assertRaisesRegex(RuntimeError,'budgetfraction'):ref.compare([math.nextafter(2e-10,math.inf)],[0.],2e-10,2e-12,'newguard')
        for expected in (-1000.,1000.):
            allowance=2e-10+2e-12*abs(expected);self.assertTrue(ref.compare([expected+.5*allowance],[expected],2e-10,2e-12,'newguard')['passed'])
            with self.assertRaisesRegex(RuntimeError,'budgetfraction'):ref.compare([expected+2*allowance],[expected],2e-10,2e-12,'newguard')
    def test_gaussian_independent_diagonal_and_invalid_inputs(self):
        matrix=[float(i+2) if i==j else 0. for i in range(9) for j in range(9)];force=[float((i+1)*(i+2)) for i in range(9)];self.assertEqual(ref.gaussian(matrix,force),[float(i+1) for i in range(9)])
        with self.assertRaisesRegex(RuntimeError,'complete finite'):ref.gaussian(matrix[:-1],force)
        with self.assertRaisesRegex(RuntimeError,'complete finite'):ref.gaussian(matrix,[math.nan]*9)
        with self.assertRaisesRegex(RuntimeError,'pivot'):ref.gaussian([0.]*81,force)
if __name__=='__main__':unittest.main()
