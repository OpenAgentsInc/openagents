//! Metal Q8_0 matvec on the shared mmap buffer.
//!
//! Frameworks stay `dlopen`-only so lib tests do not link Metal.

use std::cell::Cell;
use std::ffi::{c_void, CStr};
use std::ptr;
use std::sync::OnceLock;

use crate::generate::Q8_BLOCK;
use crate::generate::Q8_K;
use crate::metal_wrap::MetalShared;
use crate::mmap::MappedWeights;

thread_local! {
    static ACTIVE: Cell<*const MetalShared> = const { Cell::new(ptr::null()) };
}

pub struct BindGuard {
    prev: *const MetalShared,
}

impl Drop for BindGuard {
    fn drop(&mut self) {
        ACTIVE.with(|slot| slot.set(self.prev));
    }
}

pub fn bind(metal: Option<&MetalShared>) -> BindGuard {
    ACTIVE.with(|slot| {
        let prev = slot.get();
        slot.set(
            metal
                .map(|m| m as *const MetalShared)
                .unwrap_or(ptr::null()),
        );
        BindGuard { prev }
    })
}

pub fn try_q8_matvec(mapped: &MappedWeights, name: &str, x: &[f32]) -> Option<Vec<f32>> {
    let metal = ACTIVE.with(|slot| {
        let ptr = slot.get();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        }
    })?;
    let view = mapped.tensors.get(name)?;
    if view.info.ggml_type != 8 {
        return None;
    }
    let width = x.len();
    if width == 0 {
        return None;
    }
    let blocks = width.div_ceil(Q8_K);
    let row_bytes = blocks * Q8_BLOCK;
    if row_bytes == 0 || view.len < row_bytes {
        return None;
    }
    let rows = view.len / row_bytes;
    let base = mapped.mmap.as_ptr() as usize;
    let offset = (view.data as usize).checked_sub(base)?;
    if offset.saturating_add(view.len) > metal.length as usize {
        return None;
    }
    q8_matvec(metal, offset, rows, width, x)
}

#[cfg(target_os = "macos")]
fn q8_matvec(
    metal: &MetalShared,
    offset: usize,
    rows: usize,
    width: usize,
    x: &[f32],
) -> Option<Vec<f32>> {
    macos::q8_matvec(metal, offset, rows, width, x)
}

#[cfg(not(target_os = "macos"))]
fn q8_matvec(
    _metal: &MetalShared,
    _offset: usize,
    _rows: usize,
    _width: usize,
    _x: &[f32],
) -> Option<Vec<f32>> {
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    const STORAGE_SHARED: usize = 0;
    const THREADS: usize = 32;

    const SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Params {
    uint rows;
    uint width;
    uint off_lo;
    uint off_hi;
};

kernel void q8_matvec(
    device const uchar *weights [[buffer(0)]],
    device const float *x [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant Params &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    if (gid >= p.rows) {
        return;
    }
    const uint qk = 32u;
    const uint qb = 34u;
    uint blocks = (p.width + qk - 1u) / qk;
    ulong off = ulong(p.off_lo) | (ulong(p.off_hi) << 32u);
    device const uchar *row = weights + off + gid * (blocks * qb);
    threadgroup float partial[32];
    float acc = 0.0f;
    for (uint b = tid; b < blocks; b += 32u) {
        device const uchar *blk = row + b * qb;
        uint h = uint(blk[0]) | (uint(blk[1]) << 8u);
        float d = float(as_type<half>(ushort(h)));
        uint base = b * qk;
        for (uint j = 0u; j < qk; j++) {
            uint i = base + j;
            if (i >= p.width) {
                break;
            }
            acc += d * float(char(blk[2u + j])) * x[i];
        }
    }
    partial[tid] = acc;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 16u; s > 0u; s >>= 1u) {
        if (tid < s) {
            partial[tid] += partial[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0u) {
        y[gid] = partial[0];
    }
}
"#;

    #[repr(C)]
    struct Params {
        rows: u32,
        width: u32,
        off_lo: u32,
        off_hi: u32,
    }

    type Msg0 = unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void;
    type Msg1 = unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> *mut c_void;
    type MsgBuf = unsafe extern "C" fn(*mut c_void, *const c_void, usize, usize) -> *mut c_void;
    type MsgLib = unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void;
    type MsgPipe = unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void;
    type MsgSetBuf = unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void, usize, usize);
    type MsgSetBytes =
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, usize, usize);
    extern "C" {
        fn oa_metal_dispatch(
            encoder: *mut c_void,
            sel: *const c_void,
            groups: usize,
            threads: usize,
        );
    }
    type MsgVoid = unsafe extern "C" fn(*mut c_void, *const c_void);

    struct ComputeFns {
        get_class: unsafe extern "C" fn(*const i8) -> *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
        msg0: Msg0,
        msg1: Msg1,
        msg_buf: MsgBuf,
        msg_lib: MsgLib,
        msg_pipe: MsgPipe,
        msg_set_buf: MsgSetBuf,
        msg_set_bytes: MsgSetBytes,
        msg_void: MsgVoid,
        sel_new_queue: *const c_void,
        sel_new_lib: *const c_void,
        sel_new_fn: *const c_void,
        sel_new_pipe: *const c_void,
        sel_new_buf: *const c_void,
        sel_contents: *const c_void,
        sel_cmd: *const c_void,
        sel_enc: *const c_void,
        sel_set_pipe: *const c_void,
        sel_set_buf: *const c_void,
        sel_set_bytes: *const c_void,
        sel_dispatch: *const c_void,
        sel_end: *const c_void,
        sel_commit: *const c_void,
        sel_wait: *const c_void,
        sel_utf8: *const c_void,
    }

    unsafe impl Send for ComputeFns {}
    unsafe impl Sync for ComputeFns {}

    static FNS: OnceLock<Option<ComputeFns>> = OnceLock::new();

    fn fns() -> Option<&'static ComputeFns> {
        FNS.get_or_init(|| unsafe { load() }).as_ref()
    }

    unsafe fn load() -> Option<ComputeFns> {
        extern "C" {
            fn dlopen(filename: *const i8, flags: i32) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
        }
        const RTLD_LAZY: i32 = 1;
        let objc = dlopen(c_path(b"/usr/lib/libobjc.A.dylib\0")?, RTLD_LAZY);
        let _foundation = dlopen(
            c_path(b"/System/Library/Frameworks/Foundation.framework/Foundation\0")?,
            RTLD_LAZY,
        );
        if objc.is_null() {
            return None;
        }
        let sel: unsafe extern "C" fn(*const i8) -> *const c_void =
            transmute_sym(dlsym(objc, c_path(b"sel_registerName\0")?))?;
        let get_class: unsafe extern "C" fn(*const i8) -> *mut c_void =
            transmute_sym(dlsym(objc, c_path(b"objc_getClass\0")?))?;
        let release: unsafe extern "C" fn(*mut c_void) =
            transmute_sym(dlsym(objc, c_path(b"objc_release\0")?))?;
        let msg = dlsym(objc, c_path(b"objc_msgSend\0")?);
        if msg.is_null() {
            return None;
        }
        let s = |name: &[u8]| -> Option<*const c_void> {
            let sel = sel(c_path(name)?);
            if sel.is_null() {
                None
            } else {
                Some(sel)
            }
        };
        Some(ComputeFns {
            get_class,
            release,
            msg0: transmute_sym(msg)?,
            msg1: transmute_sym(msg)?,
            msg_buf: transmute_sym(msg)?,
            msg_lib: transmute_sym(msg)?,
            msg_pipe: transmute_sym(msg)?,
            msg_set_buf: transmute_sym(msg)?,
            msg_set_bytes: transmute_sym(msg)?,
            msg_void: transmute_sym(msg)?,
            sel_new_queue: s(b"newCommandQueue\0")?,
            sel_new_lib: s(b"newLibraryWithSource:options:error:\0")?,
            sel_new_fn: s(b"newFunctionWithName:\0")?,
            sel_new_pipe: s(b"newComputePipelineStateWithFunction:error:\0")?,
            sel_new_buf: s(b"newBufferWithLength:options:\0")?,
            sel_contents: s(b"contents\0")?,
            sel_cmd: s(b"commandBuffer\0")?,
            sel_enc: s(b"computeCommandEncoder\0")?,
            sel_set_pipe: s(b"setComputePipelineState:\0")?,
            sel_set_buf: s(b"setBuffer:offset:atIndex:\0")?,
            sel_set_bytes: s(b"setBytes:length:atIndex:\0")?,
            sel_dispatch: s(b"dispatchThreadgroups:threadsPerThreadgroup:\0")?,
            sel_end: s(b"endEncoding\0")?,
            sel_commit: s(b"commit\0")?,
            sel_wait: s(b"waitUntilCompleted\0")?,
            sel_utf8: s(b"stringWithUTF8String:\0")?,
        })
    }

    fn c_path(bytes: &[u8]) -> Option<*const i8> {
        CStr::from_bytes_with_nul(bytes).ok().map(|s| s.as_ptr())
    }

    unsafe fn transmute_sym<T>(ptr: *mut c_void) -> Option<T> {
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&ptr))
        }
    }

    struct Pipeline {
        queue: *mut c_void,
        pipeline: *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
    }

    unsafe impl Send for Pipeline {}
    unsafe impl Sync for Pipeline {}

    impl Drop for Pipeline {
        fn drop(&mut self) {
            unsafe {
                if !self.pipeline.is_null() {
                    (self.release)(self.pipeline);
                }
                if !self.queue.is_null() {
                    (self.release)(self.queue);
                }
            }
        }
    }

    fn pipeline_for(device: *mut c_void) -> Option<&'static Pipeline> {
        static PIPE: OnceLock<Option<Pipeline>> = OnceLock::new();
        PIPE.get_or_init(|| unsafe { compile(device) }).as_ref()
    }

    struct Scratch {
        x: *mut c_void,
        y: *mut c_void,
        x_cap: usize,
        y_cap: usize,
    }

    unsafe impl Send for Scratch {}
    unsafe impl Sync for Scratch {}

    fn scratch_pair(
        device: *mut c_void,
        x_bytes: usize,
        y_bytes: usize,
    ) -> Option<(*mut c_void, *mut c_void)> {
        use std::sync::Mutex;
        static SCRATCH: OnceLock<Mutex<Scratch>> = OnceLock::new();
        let fns = fns()?;
        let mut slot = SCRATCH
            .get_or_init(|| {
                Mutex::new(Scratch {
                    x: ptr::null_mut(),
                    y: ptr::null_mut(),
                    x_cap: 0,
                    y_cap: 0,
                })
            })
            .lock()
            .ok()?;
        unsafe {
            if slot.x_cap < x_bytes {
                if !slot.x.is_null() {
                    (fns.release)(slot.x);
                }
                slot.x = (fns.msg_buf)(device, fns.sel_new_buf, x_bytes, STORAGE_SHARED);
                slot.x_cap = if slot.x.is_null() { 0 } else { x_bytes };
            }
            if slot.y_cap < y_bytes {
                if !slot.y.is_null() {
                    (fns.release)(slot.y);
                }
                slot.y = (fns.msg_buf)(device, fns.sel_new_buf, y_bytes, STORAGE_SHARED);
                slot.y_cap = if slot.y.is_null() { 0 } else { y_bytes };
            }
            if slot.x.is_null() || slot.y.is_null() {
                return None;
            }
            Some((slot.x, slot.y))
        }
    }

    unsafe fn compile(device: *mut c_void) -> Option<Pipeline> {
        let fns = fns()?;
        let class = (fns.get_class)(c_path(b"NSString\0")?);
        if class.is_null() {
            return None;
        }
        let source = std::ffi::CString::new(SHADER).ok()?;
        let ns = (fns.msg1)(class, fns.sel_utf8, source.as_ptr() as *mut c_void);
        if ns.is_null() {
            return None;
        }
        let mut err = ptr::null_mut();
        let lib = (fns.msg_lib)(device, fns.sel_new_lib, ns, ptr::null_mut(), &mut err);
        if lib.is_null() {
            return None;
        }
        let name = std::ffi::CString::new("q8_matvec").ok()?;
        let fname = (fns.msg1)(class, fns.sel_utf8, name.as_ptr() as *mut c_void);
        let func = (fns.msg1)(lib, fns.sel_new_fn, fname);
        (fns.release)(lib);
        if func.is_null() {
            return None;
        }
        err = ptr::null_mut();
        let pipeline = (fns.msg_pipe)(device, fns.sel_new_pipe, func, &mut err);
        (fns.release)(func);
        if pipeline.is_null() {
            return None;
        }
        let queue = (fns.msg0)(device, fns.sel_new_queue);
        if queue.is_null() {
            (fns.release)(pipeline);
            return None;
        }
        Some(Pipeline {
            queue,
            pipeline,
            release: fns.release,
        })
    }

    pub(super) fn q8_matvec(
        metal: &MetalShared,
        offset: usize,
        rows: usize,
        width: usize,
        x: &[f32],
    ) -> Option<Vec<f32>> {
        if rows == 0 || width != x.len() {
            return None;
        }
        let fns = fns()?;
        let pipe = pipeline_for(metal.device)?;
        unsafe {
            let x_bytes = x.len() * 4;
            let y_bytes = rows * 4;
            let (x_buf, y_buf) = scratch_pair(metal.device, x_bytes, y_bytes)?;
            let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
            if x_ptr.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(x.as_ptr(), x_ptr, x.len());
            let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
            let enc = (fns.msg0)(cmd, fns.sel_enc);
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.pipeline);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, 0, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, x_buf, 0, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, 0, 2);
            let params = Params {
                rows: rows as u32,
                width: width as u32,
                off_lo: offset as u32,
                off_hi: (offset >> 32) as u32,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&params as *const Params).cast(),
                std::mem::size_of::<Params>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, rows, THREADS);
            (fns.msg_void)(enc, fns.sel_end);
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
            let y_ptr = (fns.msg0)(y_buf, fns.sel_contents) as *const f32;
            let mut out = vec![0f32; rows];
            if !y_ptr.is_null() {
                ptr::copy_nonoverlapping(y_ptr, out.as_mut_ptr(), rows);
            }
            Some(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_q8(rows: usize, width: usize) -> Vec<u8> {
        let blocks = width.div_ceil(Q8_K);
        let mut out = vec![0u8; rows * blocks * Q8_BLOCK];
        let one = 0x3c00u16.to_le_bytes();
        for r in 0..rows {
            for b in 0..blocks {
                let off = (r * blocks + b) * Q8_BLOCK;
                out[off..off + 2].copy_from_slice(&one);
                for j in 0..Q8_K {
                    let i = b * Q8_K + j;
                    if i < width {
                        out[off + 2 + j] = i8::from(i == r % width) as u8;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn metal_q8_matches_cpu_when_available() {
        let rows = 64usize;
        let width = 64usize;
        let weights = encode_q8(rows, width);
        let x = vec![1.0f32; width];
        let cpu = crate::generate::matvec_q8_bytes(&weights, &x).unwrap();
        let Ok(metal) = crate::metal_wrap::wrap_shared(&weights) else {
            return;
        };
        let Some(gpu) = q8_matvec(&metal, 0, rows, width, &x) else {
            return;
        };
        assert_eq!(cpu, gpu, "Metal Q8 matvec must match CPU");

        let mut padded = vec![0u8; 128];
        padded.extend_from_slice(&weights);
        let metal = crate::metal_wrap::wrap_shared(&padded).unwrap();
        let gpu_off = q8_matvec(&metal, 128, rows, width, &x).unwrap();
        assert_eq!(cpu, gpu_off, "Metal Q8 matvec must honor a non-zero offset");
    }
}
