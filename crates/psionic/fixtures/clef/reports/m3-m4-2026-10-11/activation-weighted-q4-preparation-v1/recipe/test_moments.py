"""Tiny CPU-only lifecycle and independent arithmetic checks; no model imports."""
import unittest
import numpy as np
from moments import Moments


def bf16(rows):
    values = np.array(rows, dtype=np.float32)
    bits = values.view(np.uint32)
    assert np.all(bits & 0xffff == 0)
    return (bits >> 16).astype('<u2').tobytes()


class Tests(unittest.TestCase):
    def test_token_weighted_means_and_zero_columns(self):
        m=Moments({'x':3})
        m.begin('a'*64,1);m.observe_bf16('x',bf16([[1,0,-2]]),1,3);m.commit()
        m.begin('b'*64,2);m.observe_bf16('x',bf16([[3,0,4],[-1,0,0.5]]),2,3);m.commit()
        expected=np.array([11/3,0,20.25/3],dtype=np.float32)
        np.testing.assert_array_equal(m.exported()['x'],expected)
        self.assertEqual(m.counts,{'x':3})

    def test_partial_request_never_publishes(self):
        m=Moments({'x':1,'y':1});m.begin('c'*64,1)
        m.observe_bf16('x',bf16([[2]]),1,1)
        with self.assertRaises(ValueError):m.commit()
        self.assertEqual(m.counts,{'x':0,'y':0});m.abort()
        self.assertEqual(m.completed,[])

    def test_duplicate_wrong_shape_nonfinite_refused(self):
        m=Moments({'x':1});m.begin('d'*64,1)
        with self.assertRaises(ValueError):m.observe_bf16('x',bf16([[2]]),2,1)
        with self.assertRaises(ValueError):m.observe_bf16('x',np.array([0x7fc0],dtype='<u2').tobytes(),1,1)
        m.observe_bf16('x',bf16([[2]]),1,1)
        with self.assertRaises(ValueError):m.observe_bf16('x',bf16([[2]]),1,1)
        m.commit()
        with self.assertRaises(ValueError):m.begin('d'*64,1)

    def test_gate_up_identity_and_abort_preserve_prior(self):
        names={'blk.0.ffn_gate.weight':1,'blk.0.ffn_up.weight':1};m=Moments(names)
        m.begin('e'*64,1)
        for name in names:m.observe_bf16(name,bf16([[2]]),1,1)
        m.commit();m.begin('f'*64,1)
        for name,value in zip(names,[2,3]):m.observe_bf16(name,bf16([[value]]),1,1)
        with self.assertRaises(ValueError):m.commit()
        m.abort();self.assertEqual(m.completed,['e'*64])
        for value in m.exported().values():np.testing.assert_array_equal(value,[4])

    def test_reduction_crosses_64_row_boundary(self):
        rows=[[float((i%7)-3),float((i%3)*0.5)] for i in range(129)]
        # The live collector admits <=1024 rows; exercise a nonaligned reduction tail.
        m=Moments({'x':2});m.begin('0'*64,129);m.observe_bf16('x',bf16(rows),129,2);m.commit()
        expected=np.array([sum(r[j]*r[j] for r in rows)/129 for j in range(2)],dtype=np.float32)
        np.testing.assert_array_equal(m.exported()['x'],expected)


if __name__=='__main__':unittest.main()
