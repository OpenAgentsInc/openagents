//! Metal shared wrap of a GGUF mmap.
//!
//! Metal.framework and libobjc are opened with `dlopen` only when
//! `wrap_shared` runs. A compile-time `metal`/`objc` link aborted parallel
//! `openagents-cli` lib tests (`mach_msg failed`).

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_void, CStr};
    use std::ptr;
    use std::sync::OnceLock;

    const RTLD_LAZY: i32 = 1;
    /// `MTLResourceStorageModeShared` (Apple Silicon unified memory).
    const STORAGE_MODE_SHARED: usize = 0;

    extern "C" {
        fn dlopen(filename: *const i8, flags: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    }

    type CreateDevice = unsafe extern "C" fn() -> *mut c_void;
    type SelRegister = unsafe extern "C" fn(*const i8) -> *const c_void;
    type ObjcRelease = unsafe extern "C" fn(*mut c_void);
    type MsgSendBuf = unsafe extern "C" fn(
        obj: *mut c_void,
        sel: *const c_void,
        bytes: *const c_void,
        length: usize,
        options: usize,
        deallocator: *mut c_void,
    ) -> *mut c_void;

    static FNS: OnceLock<Option<MetalFns>> = OnceLock::new();

    struct MetalFns {
        create_device: CreateDevice,
        release: ObjcRelease,
        new_buffer: MsgSendBuf,
        sel_new_buffer: *const c_void,
    }

    unsafe impl Send for MetalFns {}
    unsafe impl Sync for MetalFns {}

    fn fns() -> Option<&'static MetalFns> {
        FNS.get_or_init(|| unsafe { load() }).as_ref()
    }

    unsafe fn load() -> Option<MetalFns> {
        let objc_path = CStr::from_bytes_with_nul(b"/usr/lib/libobjc.A.dylib\0").ok()?;
        let metal_path =
            CStr::from_bytes_with_nul(b"/System/Library/Frameworks/Metal.framework/Metal\0")
                .ok()?;
        let objc = dlopen(objc_path.as_ptr(), RTLD_LAZY);
        let metal = dlopen(metal_path.as_ptr(), RTLD_LAZY);
        if objc.is_null() || metal.is_null() {
            return None;
        }
        let sel_register: SelRegister = transmute_sym(dlsym(objc, c_sym(b"sel_registerName\0")?))?;
        let release: ObjcRelease = transmute_sym(dlsym(objc, c_sym(b"objc_release\0")?))?;
        let msg_send = dlsym(objc, c_sym(b"objc_msgSend\0")?);
        if msg_send.is_null() {
            return None;
        }
        let create = dlsym(metal, c_sym(b"MTLCreateSystemDefaultDevice\0")?);
        if create.is_null() {
            return None;
        }
        let sel = sel_register(
            b"newBufferWithBytesNoCopy:length:options:deallocator:\0"
                .as_ptr()
                .cast(),
        );
        if sel.is_null() {
            return None;
        }
        Some(MetalFns {
            create_device: transmute_sym(create)?,
            release,
            new_buffer: transmute_sym(msg_send)?,
            sel_new_buffer: sel,
        })
    }

    fn c_sym(bytes: &[u8]) -> Option<*const i8> {
        CStr::from_bytes_with_nul(bytes).ok().map(|s| s.as_ptr())
    }

    unsafe fn transmute_sym<T>(ptr: *mut c_void) -> Option<T> {
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&ptr))
        }
    }

    pub struct MetalShared {
        device: *mut c_void,
        buffer: *mut c_void,
        pub length: u64,
        release: ObjcRelease,
    }

    unsafe impl Send for MetalShared {}
    unsafe impl Sync for MetalShared {}

    impl Drop for MetalShared {
        fn drop(&mut self) {
            unsafe {
                if !self.buffer.is_null() {
                    (self.release)(self.buffer);
                }
                if !self.device.is_null() {
                    (self.release)(self.device);
                }
            }
        }
    }

    pub fn wrap_shared(bytes: &[u8]) -> Result<MetalShared, String> {
        let fns = fns().ok_or_else(|| String::from("Metal.framework not loaded"))?;
        if bytes.is_empty() {
            return Err(String::from("empty mapping"));
        }
        unsafe {
            let device = (fns.create_device)();
            if device.is_null() {
                return Err(String::from("no Metal device"));
            }
            let buffer = (fns.new_buffer)(
                device,
                fns.sel_new_buffer,
                bytes.as_ptr().cast(),
                bytes.len(),
                STORAGE_MODE_SHARED,
                ptr::null_mut(),
            );
            if buffer.is_null() {
                (fns.release)(device);
                return Err(String::from("newBufferWithBytesNoCopy returned nil"));
            }
            Ok(MetalShared {
                device,
                buffer,
                length: bytes.len() as u64,
                release: fns.release,
            })
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{wrap_shared, MetalShared};

#[cfg(not(target_os = "macos"))]
pub struct MetalShared {
    pub length: u64,
}

#[cfg(not(target_os = "macos"))]
pub fn wrap_shared(_bytes: &[u8]) -> Result<MetalShared, String> {
    Err(String::from("Metal is not compiled on this target"))
}

pub fn metal_compiled() -> bool {
    cfg!(target_os = "macos")
}
