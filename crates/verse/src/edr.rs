//! Extended dynamic range on macOS: the screen's headroom and the layer tag.
//!
//! wgpu turns on `wantsExtendedDynamicRangeContent` for an RGBA16F surface but
//! leaves the layer's color space unset. An extended-range frame needs the
//! extended linear sRGB space so values above 1.0 display brighter than
//! reference white (Apple, "Displaying HDR content in a Metal layer"). iOS
//! hosts tag their own layer; this module does it for the desktop window.

use std::ffi::c_void;

use objc2::encode::{Encoding, RefEncode};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

#[repr(C)]
struct ColorSpace {
    _opaque: [u8; 0],
}

// SAFETY: `CGColorSpaceRef` is a pointer to the opaque `CGColorSpace` struct.
unsafe impl RefEncode for ColorSpace {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGColorSpace", &[]));
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGColorSpaceExtendedLinearSRGB: *const c_void;
    fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut ColorSpace;
    fn CGColorSpaceRelease(space: *mut ColorSpace);
}

fn main_screen() -> Option<*mut AnyObject> {
    // SAFETY: `+[NSScreen mainScreen]` has no arguments; called on the main
    // thread, where the window and renderer live.
    let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
    (!screen.is_null()).then_some(screen)
}

/// How far above reference white the main screen could ever display.
#[must_use]
pub fn potential_headroom() -> f32 {
    main_screen().map_or(1.0, |screen| {
        // SAFETY: an NSScreen responds to this CGFloat property (macOS 10.15+).
        let value: f64 = unsafe {
            msg_send![
                screen,
                maximumPotentialExtendedDynamicRangeColorComponentValue
            ]
        };
        value as f32
    })
}

/// How far above reference white the main screen displays right now.
#[must_use]
pub fn current_headroom() -> f32 {
    main_screen().map_or(1.0, |screen| {
        // SAFETY: an NSScreen responds to this CGFloat property.
        let value: f64 =
            unsafe { msg_send![screen, maximumExtendedDynamicRangeColorComponentValue] };
        value as f32
    })
}

/// Tags a CAMetalLayer as extended linear sRGB.
///
/// # Safety
/// `layer` must be a live CAMetalLayer used on the main thread.
pub unsafe fn tag_extended_linear(layer: *mut AnyObject) {
    // SAFETY: the constant is a CFString the framework defines; creating and
    // releasing the color space follows Core Foundation ownership rules, and
    // the layer retains its own reference.
    unsafe {
        let space = CGColorSpaceCreateWithName(kCGColorSpaceExtendedLinearSRGB);
        if space.is_null() {
            return;
        }
        let _: () = msg_send![layer, setColorspace: space];
        CGColorSpaceRelease(space);
    }
}
