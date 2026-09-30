//! The pairing code as QR modules.
//!
//! The QR code carries the code's link form,
//! `https://openagents.com/connect#<payload>` ([`code_modules`]), so the
//! phone's own camera opens the OpenAgents app, not only the app's scanner.
//! It is encoded with `qrcodegen` at error-correction level M, which keeps
//! even the longest link (666 characters) readable from a laptop screen. The window paints the modules
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

/// The modules of the QR code for a connect code in either form: its link.
/// `None` when `code` is not a connect code or does not fit.
pub fn code_modules(code: &str) -> Option<Modules> {
    modules(&openagents_connect::code::link(code)?)
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

    use openagents_connect::code::{LINK_PREFIX, MAX_LINK_BYTES, PREFIX};

    #[test]
    fn a_full_size_code_fits() {
        // The longest link a code can make.
        let payload = "A".repeat(MAX_LINK_BYTES - LINK_PREFIX.len());
        let modules = code_modules(&format!("{PREFIX}{payload}")).expect("fits");
        // Version 24 at most: well under the largest QR code.
        assert!(modules.size <= 17 + 4 * 24, "{}", modules.size);
        // The finder pattern's corner is dark.
        assert!(modules.get(0, 0));
    }

    #[test]
    fn the_qr_code_carries_the_link_form() {
        let text = format!("{PREFIX}AQID");
        let link = format!("{LINK_PREFIX}AQID");
        assert_eq!(code_modules(&text), modules(&link));
        assert_eq!(code_modules(&link), modules(&link));
        assert_eq!(code_modules("coder-host:AQID"), None);
    }
}
