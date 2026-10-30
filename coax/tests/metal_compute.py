#!/usr/bin/env python3
"""coax metal-compute: the CUDA compute subset's admission spelled for Metal,
the Metal-only refusals (f64 is float-float there), the kernel entry layout,
and (on macOS with the Metal toolchain) that the generated Metal Shading
Language compiles with fast math off. No GPU is needed; the Apple GPU runs
are psionic/tests/run_metal_kernels.sh."""
import os, shutil, subprocess, sys, tempfile, unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from cuda import COMPILER, KERNEL, REFUSALS, WARP, WARP_REFUSALS

METAL_REFUSALS=[
    ('an f64 bit pattern','k :: (item:int,out:[]u64) #export_c {x:f64=1;out[item]=f64_bits(x);}'),
    ('this f64 operation','k :: (item:int,out:[]f64) #export_c {x:f64=1;out[item]=sin(x);}'),
    ('this f64 operation','k :: (item:int,out:[]int) #export_c {x:f64=1.5;out[item]=cast(int)x;}'),
    ('a view of a register array','f :: (v:[]f32)->f32 {return v[0];}\nk :: (item:int,out:[]f32) #export_c {a:[4]f32;out[item]=f(a[..]);}'),
]

def metal_toolchain():
    if sys.platform!='darwin' or not shutil.which('xcrun'):return False
    return subprocess.run(['xcrun','-sdk','macosx','-f','metal'],capture_output=True).returncode==0

class MetalCompute(unittest.TestCase):
    def run_metal(self,source,*flags):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'k.coax';p.write_text(source)
            return subprocess.run([str(COMPILER),'metal-compute',str(p),*flags],capture_output=True,text=True)
    def test_cuda_refusals_hold_for_metal(self):
        for reason,source in REFUSALS+WARP_REFUSALS:
            with self.subTest(reason=reason,source=source[:60]):
                r=self.run_metal(source)
                self.assertNotEqual(r.returncode,0)
                self.assertIn('error[E0702]',r.stderr)
                self.assertIn(reason+' is outside the Metal compute subset',r.stderr)
                self.assertEqual(r.stdout,'')
    def test_metal_only_refusals(self):
        for reason,source in METAL_REFUSALS:
            with self.subTest(reason=reason,source=source):
                r=self.run_metal(source)
                self.assertNotEqual(r.returncode,0)
                self.assertIn('error[E0702]',r.stderr)
                self.assertIn(reason+' is outside the Metal compute subset',r.stderr)
                with tempfile.TemporaryDirectory() as d:  # the same source is still a C library
                    p=Path(d)/'k.coax';p.write_text(source)
                    self.assertEqual(subprocess.run([str(COMPILER),'c',str(p),'--library'],capture_output=True,text=True).returncode,0)
    def test_a_module_needs_a_kernel(self):
        r=self.run_metal('f :: (x:f32)->f32 {return x;}')
        self.assertNotEqual(r.returncode,0);self.assertIn('error[E0703]',r.stderr)
    def test_kernel_entry_layout(self):
        for flags in ([],['--release']):
            r=self.run_metal(KERNEL,*flags);self.assertEqual(r.returncode,0,r.stderr)
            out=r.stdout
            self.assertIn('#pragma METAL fp math_mode(safe)',out)
            self.assertIn('struct coax_args_scale {\n  int64_t coax_items;',out)
            self.assertIn('kernel void scale(constant coax_args_scale &coax_a [[buffer(0)]], device atomic_uint *coax_errors [[buffer(1)]],',out)
            self.assertIn('if (coax_item >= coax_a.coax_items) return;',out)
            self.assertIn('uint coax_err[2] = {0, 0};',out)
            self.assertIn('static inline float coax_p_twice(float l_x, uint coax_lane, thread uint *coax_err)',out)
            self.assertNotIn('coax_p_label',out)  # reached only from a check
            self.assertNotIn('__trap',out)
    def test_warp_kernels_use_the_simdgroup(self):
        r=self.run_metal(WARP,'--release');self.assertEqual(r.returncode,0,r.stderr)
        out=r.stdout
        self.assertIn('int64_t coax_item = (int64_t)(coax_gid >> 5);',out)
        self.assertIn('coax_mma_bf16_16x8x16(l_fa, l_fb, l_fc, coax_lane, coax_mc);',out);self.assertIn('coax_mma_cache coax_mc = {};',out)
        self.assertIn('simd_shuffle',out)
        self.assertNotIn('coax_p_cu_mma',out)
    def test_f64_is_float_float(self):
        r=self.run_metal('k :: (item:int,x:[]f64,out:[]f64) #export_c {out[item]=exp(x[item])*2+log(x[item]);}')
        self.assertEqual(r.returncode,0,r.stderr)
        self.assertIn('coax_ff_exp(',r.stdout);self.assertIn('coax_ff_log(',r.stdout)
        self.assertIn('typedef struct { device coax_ff *data; int64_t count; } coax_v_f64;',r.stdout)
    def test_generated_metal_compiles(self):
        if not metal_toolchain():self.skipTest('no Metal toolchain')
        for source,flags in ((KERNEL,[]),(KERNEL,['--release']),(WARP,['--release'])):
            r=self.run_metal(source,*flags);self.assertEqual(r.returncode,0,r.stderr)
            with tempfile.TemporaryDirectory() as d:
                p=Path(d)/'k.metal';p.write_text(r.stdout)
                c=subprocess.run(['xcrun','-sdk','macosx','metal','-std=metal3.1','-fno-fast-math','-Werror','-c',str(p),'-o',str(Path(d)/'k.air')],capture_output=True,text=True)
                self.assertEqual(c.returncode,0,c.stderr)

if __name__=='__main__':unittest.main()
