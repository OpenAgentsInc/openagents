#!/usr/bin/env python3
"""coax cuda: compute-subset admission, kernel entry layout, and (when NVRTC
is installed) that the generated CUDA C compiles. No GPU is needed."""
import ctypes as C
import os, subprocess, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax'))

KERNEL='''
twice :: (x:f32)->f32 {return x+x;}
scale :: (item:int,factor:f32,x:[]f32,out:[]f32) #export_c {
    acc:[4]f32;
    for i:0..<4 {acc[i]=twice(x[item])*factor;}
    out[item]=acc[3];
}
bits :: (item:int,w:[]u64,out:[]u32) #export_c {
    out[item]=cast,trunc(u32)(w[item]>>16)|add_wrap(cast(u32)1,cast(u32)2);
    for value:w {if value==0 {break;}}
}
// Reached only from a check: stays on the CPU.
label :: (x:f32)->string {if x>0 {return "positive";}return "other";}
#test "kernels run on the CPU too" {
    x:=[2]f32.[1,2];out:[2]f32;
    for i:0..<2 {scale(i,3,x[..],out[..]);}
    #assert out[0]==6 && out[1]==12;
    #assert label(out[0])=="positive";
}
'''

REFUSALS=[
    ('a string value','k :: (item:int,out:[]f32) #export_c {s:="x";out[item]=1;}'),
    ('a struct value','P :: struct {a:f32;}\nk :: (item:int,out:[]f32) #export_c {p:P;out[item]=p.a;}'),
    ('an enum value','E :: enum u8 {a;b;}\nk :: (item:int,out:[]f32) #export_c {e:=E.a;out[item]=1;}'),
    ('an array that is not a fixed array of at most 4096 scalars','k :: (item:int,out:[]f32) #export_c {a:[4097]f32;out[item]=a[0];}'),
    ('a view parameter of a kernel that is not a view of numbers','k :: (item:int,flags:[]bool,out:[]f32) #export_c {out[item]=1;}'),
    ('a view of something other than numbers','f :: (v:[]bool)->bool {return v[0];}\nk :: (item:int,out:[]f32) #export_c {a:[2]bool;if f(a[..]) {out[item]=1;}}'),
    ('a while loop','k :: (item:int,out:[]f32) #export_c {i:=0;while i<3 {i+=1;}out[item]=1;}'),
    ('defer','k :: (item:int,out:[]f32) #export_c {defer {out[item]=2;}out[item]=1;}'),
    ('#assert','k :: (item:int,out:[]f32) #export_c {#assert item>=0;out[item]=1;}'),
    ('print','#import "Basic";\nk :: (item:int,out:[]f32) #export_c {print("hi\\n");out[item]=1;}'),
    ('a #foreign call','host :: (x:f32)->f32 #foreign;\nk :: (item:int,out:[]f32) #export_c {out[item]=host(1);}'),
    ('a recursive call','f :: (n:int)->int {if n<=0 {return 0;}return f(n-1);}\nk :: (item:int,out:[]f32) #export_c {out[item]=f32(f(item));}'),
    ('this storage intrinsic','k :: (item:int,out:[]u64) #export_c {out[item]=value_barrier(cast(u64)1);}'),
    ('a kernel that returns a value','k :: (item:int,out:[]f32)->int #export_c {out[item]=1;return 0;}'),
    ('a kernel whose first parameter is not its int work item','k :: (item:u32,out:[]f32) #export_c {out[0]=1;}'),
    ('a returned value that is not a scalar','f :: ()->[2]f32 {a:[2]f32;return a;}\nk :: (item:int,out:[]f32) #export_c {out[item]=f()[0];}'),
    ('a call chain deeper than 64',''.join('g%d :: ()->int {return g%d();}\n'%(i,i+1) for i in range(70))+'g70 :: ()->int {return 1;}\nk :: (item:int,out:[]f32) #export_c {out[item]=f32(g0());}'),
]

MMA=str((ROOT/'lib/compute/mma.coax').resolve())
WARP='#load "'+MMA+'";\n'+'''
tile :: (item:int,a:[]u32,b:[]u32,c:[]f32,d:[]f32) #export_c {
    fa:[128]u32;fb:[64]u32;fc:[128]f32;
    for lane:0..<32 {
        for j:0..<4 {fa[lane*4+j]=a[item*128+lane*4+j];fc[lane*4+j]=c[item*128+lane*4+j];}
        fb[lane*2]=b[item*64+lane*2];fb[2*lane+1]=b[item*64+lane*2+1];
        own:=lane*2;own+=1;
    }
    cu_mma_bf16_16x8x16(fa[..],fb[..],fc[..]);
    for lane:0..<32 {for j:0..<4 {d[item*128+lane*4+j]=fc[lane*4+j];}}
}
'''
def warp(body,arrays='fa:[128]u32;fb:[64]u32;fc:[128]f32;'):
    return '#load "'+MMA+'";\nwarp_kernel :: (item:int,out:[]f32) #export_c {'+arrays+body+'}'
WARP_REFUSALS=[
    ('a lane loop that is not for lane: 0..<32 in a kernel',warp('for lane:0..<16 {fc[lane*4]=1;}')),
    ('a lane loop that is not for lane: 0..<32 in a kernel','f :: () {for lane:0..<32 {}}\nk :: (item:int,out:[]f32) #export_c {f();out[item]=1;}'),
    ('a per-lane index that is not lane*k+c into a [32*k] array',warp('for lane:0..<32 {fc[lane*4+4]=1;}')),
    ('a per-lane index that is not lane*k+c into a [32*k] array',warp('for lane:0..<32 {fc[lane*2]=1;}')),
    ('a per-lane index that is not lane*k+c into a [32*k] array',warp('for lane:0..<32 {fc[lane+lane]=1;}')),
    ('a per-lane array outside lane loops and device primitives',warp('for lane:0..<32 {fc[lane*4]=1;}out[item]=fc[0];')),
    ('a lane loop assigning a value the warp shares',warp('total:f32=0;for lane:0..<32 {fc[lane*4]=1;total=fc[lane*4];}out[item]=total;')),
    ('break or continue directly in a lane loop',warp('for lane:0..<32 {fc[lane*4]=1;if lane==3 {break;}}')),
    ('a device primitive with the wrong arguments',warp('for lane:0..<32 {fc[lane*4]=1;}cu_mma_bf16_16x8x16(fa[..],fa[0..<64],fc[..]);')),
    ('a device primitive with the wrong arguments',warp('for lane:0..<32 {fd[lane*3]=1;}cu_mma_bf16_16x8x16(fa[..],fb[..],fd[..]);','fa:[128]u32;fb:[64]u32;fd:[96]f32;')),
    ('a device primitive inside a lane loop',warp('for lane:0..<32 {fc[lane*4]=1;cu_mma_bf16_16x8x16(fa[..],fb[..],fc[..]);}')),
]

def nvrtc():
    candidates=[os.environ.get('PSIONIC_NVRTC',''),os.path.join(os.environ.get('CUDA_HOME',''),'lib/libnvrtc.so.13'),
                os.path.expanduser('~/cuda-13.2/lib/libnvrtc.so.13'),'libnvrtc.so.13']
    for c in candidates:
        if not c:continue
        try:return C.CDLL(c)
        except OSError:pass
    return None

class Cuda(unittest.TestCase):
    def run_cuda(self,source,*flags):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'k.coax';p.write_text(source)
            return subprocess.run([str(COMPILER),'cuda',str(p),*flags],capture_output=True,text=True)
    def test_refusals_name_what_is_outside_the_subset(self):
        for reason,source in REFUSALS:
            with self.subTest(reason=reason):
                r=self.run_cuda(source)
                self.assertNotEqual(r.returncode,0)
                self.assertIn('error[E0700]',r.stderr)
                self.assertIn(reason+' is outside the CUDA compute subset',r.stderr)
                self.assertEqual(r.stdout,'')
    def test_warp_kernel_refusals(self):
        for reason,source in WARP_REFUSALS:
            with self.subTest(reason=reason,source=source):
                r=self.run_cuda(source)
                self.assertNotEqual(r.returncode,0)
                self.assertIn(reason+' is outside the CUDA compute subset',r.stderr)
    def test_warp_kernels_keep_per_lane_values_and_emit_one_mma(self):
        r=self.run_cuda(WARP,'--release');self.assertEqual(r.returncode,0,r.stderr)
        out=r.stdout
        self.assertIn('uint32_t l_fa[4] = {};',out);self.assertIn('uint32_t l_fb[2] = {};',out);self.assertIn('float l_fc[4] = {};',out)
        self.assertIn('const int64_t l_lane = (int64_t)(threadIdx.x & 31u);',out)
        self.assertIn('l_fa[(',out);self.assertIn(') - l_lane * 4] =',out);self.assertIn(') - l_lane * 2] =',out)
        self.assertEqual(out.count('mma.sync.aligned.m16n8k16.row.col.f32.bf16.bf16.f32'),1)
        self.assertIn('int64_t coax_item = ((int64_t)blockIdx.x * (int64_t)blockDim.x + (int64_t)threadIdx.x) >> 5;',out)
        self.assertNotIn('coax_p_cu_mma',out)  # the primitive's body is its CPU meaning only
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'w.coax';p.write_text(WARP)
            self.assertEqual(subprocess.run([str(COMPILER),'c',str(p),'--library'],capture_output=True,text=True).returncode,0)
    def test_a_module_needs_a_kernel(self):
        r=self.run_cuda('f :: (x:f32)->f32 {return x;}')
        self.assertNotEqual(r.returncode,0);self.assertIn('error[E0701]',r.stderr)
    def test_the_same_source_is_still_a_c_library(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'k.coax';p.write_text(KERNEL)
            self.assertEqual(subprocess.run([str(COMPILER),'test',str(p)],capture_output=True,text=True).returncode,0)
            self.assertEqual(subprocess.run([str(COMPILER),'c',str(p),'--library'],capture_output=True,text=True).returncode,0)
    def test_kernels_get_an_item_count_and_a_work_item(self):
        for flags in ([],['--release']):
            r=self.run_cuda(KERNEL,*flags)
            self.assertEqual(r.returncode,0,r.stderr)
            out=r.stdout
            self.assertIn('extern "C" __global__ void scale(int64_t coax_items, float l_factor, coax_v_f32 l_x, coax_v_f32 l_out)',out)
            self.assertIn('int64_t coax_item = (int64_t)blockIdx.x * (int64_t)blockDim.x + (int64_t)threadIdx.x;',out)
            self.assertIn('if (coax_item >= coax_items) return;',out)
            self.assertIn('coax_k_scale(coax_item, l_factor, l_x, l_out);',out)
            self.assertIn('static __device__ float coax_p_twice(float l_x)',out)
            self.assertIn('#pragma unroll',out)
            self.assertIn('__trap()',out)
            self.assertNotIn('coax_p_label',out)  # reached only from a check
            self.assertNotIn('printf',out)
            self.assertNotIn('#include',out)  # NVRTC has no libc headers
            self.assertIn(('coax_wadd_s64' if flags else 'coax_add_s64')+'(',out)
    def test_generated_cuda_compiles_with_nvrtc(self):
        lib=nvrtc()
        if lib is None:self.skipTest('NVRTC is not installed')
        for source,flags in ((KERNEL,[]),(KERNEL,['--release']),(WARP,['--release'])):
            r=self.run_cuda(source,*flags);self.assertEqual(r.returncode,0,r.stderr)
            prog=C.c_void_p()
            self.assertEqual(lib.nvrtcCreateProgram(C.byref(prog),r.stdout.encode(),b'k.cu',0,None,None),0)
            opts=(C.c_char_p*3)(b'--gpu-architecture=compute_89',b'--fmad=false',b'--std=c++17')
            rc=lib.nvrtcCompileProgram(prog,3,opts)
            size=C.c_size_t();lib.nvrtcGetProgramLogSize(prog,C.byref(size));log=C.create_string_buffer(size.value)
            lib.nvrtcGetProgramLog(prog,log)
            self.assertEqual(rc,0,log.value.decode())
            self.assertNotIn(b'error',log.value)
            lib.nvrtcDestroyProgram(C.byref(prog))

if __name__=='__main__':unittest.main()
