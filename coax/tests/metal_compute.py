#!/usr/bin/env python3
"""coax metal-compute: the CUDA compute subset's admission spelled for Metal,
the Metal-only refusals (f64 is float-float there), the kernel entry layout,
and (on macOS with the Metal toolchain) that the generated Metal Shading
Language compiles with fast math off. No GPU is needed; the Apple GPU runs
are psionic/tests/run_metal_kernels.sh."""
import os, shutil, subprocess, sys, tempfile, unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from cuda import APPLE, COMPILER, GROUP, GROUP_REFUSALS, KERNEL, REFUSALS, WARP, WARP_REFUSALS

# The Apple matrix primitives in a group kernel of 128 threads: simdgroup
# tiles through threadgroup memory, and a Metal 4 tensor-op tile.
SIMDGROUP='#load "'+APPLE+'''";
sg :: (item:int,x:[]f32,out:[]f32) #export_c {
    a:[2048]f32;b:[1024]f32;acc:[2048]f32;
    for thread:0..<128 {
        for i:0..<16 {a[thread+128*i]=x[thread+128*i];}
        for i:0..<8 {b[thread+128*i]=x[thread+128*i];}
    }
    am_sg_mma_64x32x32(acc[..],a[..],b[..]);
    am_sg_spill_64x32(acc[..],a[..]);
    for thread:0..<128 {for i:0..<16 {out[item*2048+thread+128*i]=a[thread+128*i];}}
}
tensor :: (item:int,rows:int,cols:int,tokens:int,w:[]u32,x:[]f32,out:[]f32) #export_c {
    am_matmul_bf16(128,16,64,rows,cols,tokens,item*64,0,w,x,out);
}
'''
def apple(body,arrays='a:[2048]f32;b:[1024]f32;acc:[2048]f32;'):
    return '#load "'+APPLE+'";\nk :: (item:int,w:[]u32,x:[]f32,out:[]f32) #export_c {'+arrays+body+'}'
APPLE_REFUSALS=[
    ('an Apple matrix primitive with the wrong arguments or outside a group kernel\'s shared code',apple('for thread:0..<128 {a[thread]=1;b[thread]=1;}am_sg_mma_64x32x32(acc[..],b[..],b[..]);','a:[2048]f32;b:[2048]f32;acc:[2048]f32;')),
    ('a group kernel whose thread loops and group primitives disagree on its size, or that has lane loops or the MMA primitive',apple('for thread:0..<64 {a[thread]=1;b[thread]=1;}am_sg_mma_64x32x32(acc[..],a[..],b[..]);')),
    ('an Apple matrix primitive with the wrong arguments or outside a group kernel\'s shared code',apple('for thread:0..<128 {a[thread]=1;am_matmul_bf16(128,16,64,64,32,16,0,0,w,x,out);}','a:[2048]f32;')),
    ('an Apple matrix primitive with the wrong arguments or outside a group kernel\'s shared code',apple('am_matmul_bf16(128,24,64,64,32,16,0,0,w,x,out);','')),
    ('an Apple matrix primitive with the wrong arguments or outside a group kernel\'s shared code',apple('t:=16;am_matmul_bf16(128,t,64,64,32,16,0,0,w,x,out);','')),
    ('a matrix accumulator used other than by Apple matrix primitives',apple('for thread:0..<128 {a[thread]=1;b[thread]=1;}am_sg_mma_64x32x32(acc[..],a[..],b[..]);for thread:0..<128 {out[thread]=acc[thread];}')),
    ('a group kernel whose thread loops and group primitives disagree on its size, or that has lane loops or the MMA primitive',apple('for thread:0..<64 {out[thread]=1;}am_matmul_bf16(128,16,64,64,32,16,0,0,w,x,out);','')),
]
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
        for reason,source in REFUSALS+WARP_REFUSALS+GROUP_REFUSALS:
            with self.subTest(reason=reason,source=source[:60]):
                r=self.run_metal(source)
                self.assertNotEqual(r.returncode,0)
                self.assertIn('error[E0702]',r.stderr)
                self.assertIn(reason+' is outside the Metal compute subset',r.stderr)
                self.assertEqual(r.stdout,'')
    def test_metal_only_refusals(self):
        for reason,source in METAL_REFUSALS+APPLE_REFUSALS:
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
    def test_group_kernels_use_threadgroup_memory_and_barriers(self):
        r=self.run_metal(GROUP,'--release');self.assertEqual(r.returncode,0,r.stderr)
        out=r.stdout
        self.assertIn('threadgroup coax_a256_f32 coax_tg_l_shared;',out);self.assertIn('threadgroup coax_a256_f32 &l_shared = *coax_tg_l_shared;',out)
        self.assertIn('float l_own[2] = {};',out);self.assertNotIn('coax_tg_l_own',out)
        self.assertIn('uint coax_tid [[thread_index_in_threadgroup]], uint coax_tpg [[threads_per_threadgroup]]',out)
        self.assertIn('if (coax_tpg != 128u)',out);self.assertIn('int64_t coax_item = (int64_t)coax_tgid;',out)
        self.assertGreaterEqual(out.count('threadgroup_barrier(mem_flags::mem_threadgroup | mem_flags::mem_device);'),5)
    def test_apple_primitives_lower_to_matrix_units(self):
        r=self.run_metal(SIMDGROUP,'--release');self.assertEqual(r.returncode,0,r.stderr)
        out=r.stdout
        self.assertIn('simdgroup_float8x8 l_acc[8];',out);self.assertIn('coax_am_sg_mma(l_acc, l_a.e, l_b.e, coax_lane)',out)
        self.assertIn('coax_am_sg_spill(l_acc, l_a.e, coax_lane)',out);self.assertIn('simdgroup_multiply_accumulate',out)
        self.assertIn('coax_am_matmul<16, 64, 4>(',out);self.assertIn('matmul2d_descriptor(TM, TN',out)
        self.assertNotIn('coax_p_am_',out)  # the bodies are the CPU meaning only
    def test_f64_is_float_float(self):
        r=self.run_metal('k :: (item:int,x:[]f64,out:[]f64) #export_c {out[item]=exp(x[item])*2+log(x[item]);}')
        self.assertEqual(r.returncode,0,r.stderr)
        self.assertIn('coax_ff_exp(',r.stdout);self.assertIn('coax_ff_log(',r.stdout)
        self.assertIn('typedef struct { device coax_ff *data; int64_t count; } coax_v_f64;',r.stdout)
    def test_generated_metal_compiles(self):
        if not metal_toolchain():self.skipTest('no Metal toolchain')
        for source,flags,std in ((KERNEL,[],'metal3.1'),(KERNEL,['--release'],'metal3.1'),(WARP,['--release'],'metal3.1'),(GROUP,[],'metal3.1'),
                                 (GROUP,['--release'],'metal3.1'),(SIMDGROUP,[],'metal4.0'),(SIMDGROUP,['--release'],'metal4.0')):
            r=self.run_metal(source,*flags);self.assertEqual(r.returncode,0,r.stderr)
            with tempfile.TemporaryDirectory() as d:
                p=Path(d)/'k.metal';p.write_text(r.stdout)
                c=subprocess.run(['xcrun','-sdk','macosx','metal','-std='+std,'-fno-fast-math','-Werror','-c',str(p),'-o',str(Path(d)/'k.air')],capture_output=True,text=True)
                self.assertEqual(c.returncode,0,c.stderr)

if __name__=='__main__':unittest.main()
