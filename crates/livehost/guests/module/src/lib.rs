//! The demo browser module. The page reads a greeting from its memory, so
//! a change here is visible as soon as the browser loads the new module.
#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable()
}

/// `livehost bench` rewrites this line to make a module change.
const STAMP: &str = "module-0";

static mut GREETING: [u8; 128] = [0; 128];
static mut LEN: usize = 0;

/// Renders the greeting into the module's memory and returns its address.
#[unsafe(no_mangle)]
pub extern "C" fn greeting_ptr() -> *const u8 {
    let parts: [&[u8]; 3] = [b"Hello from a WebAssembly module (", STAMP.as_bytes(), b")"];
    let mut len = 0;
    // SAFETY: the module is single-threaded, and the page calls the two
    // exports one after the other.
    unsafe {
        let buffer = &mut *core::ptr::addr_of_mut!(GREETING);
        for part in parts {
            let end = (len + part.len()).min(buffer.len());
            buffer[len..end].copy_from_slice(&part[..end - len]);
            len = end;
        }
        LEN = len;
        buffer.as_ptr()
    }
}

/// The length of the greeting that `greeting_ptr` rendered.
#[unsafe(no_mangle)]
pub extern "C" fn greeting_len() -> usize {
    // SAFETY: see `greeting_ptr`.
    unsafe { LEN }
}
