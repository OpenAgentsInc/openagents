//! The pairing code as QR modules.
//!
//! The code text is encoded with `qrcodegen` at error-correction level M,
//! which keeps a `openagents-connect:` payload (up to about 560
//! characters) readable from a laptop screen. The window paints the modules
//! black on a white square with a four-module quiet zone, as scanners
//! expect; an inverted code reads poorly on some phones.

use qrcodegen::{QrCode, QrCodeEcc};

/// The modules around the code a scanner needs clear.
pub const QUIET_ZONE: usize = 4;

/// A QR code's modules, row by row; `true` is dark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modules {
    pub size: usize,
    pub dark: Vec<bool>,
}

impl Modules {
    /// Whether the module at `x`, `y` is dark.
    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.dark[y * self.size + x]
    }
}

/// The modules for `text`, or `None` when it does not fit a QR code.
pub fn modules(text: &str) -> Option<Modules> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let size = code.size() as usize;
    let mut dark = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            dark.push(code.get_module(x as i32, y as i32));
        }
    }
    Some(Modules { size, dark })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_size_code_fits() {
        let text = format!("openagents-connect:{}", "A".repeat(540));
        let modules = modules(&text).expect("fits");
        // Version 19 or so: well under the largest QR code.
        assert!(modules.size < 120, "{}", modules.size);
        // The finder pattern's corner is dark.
        assert!(modules.get(0, 0));
    }
}
