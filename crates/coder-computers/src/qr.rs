//! Invitation QR codes, rendered on this device.
//!
//! The modules come from the same local renderer `coder-connect` uses for
//! pairing codes, so an invitation never reaches a QR-generation service. A
//! phone host draws [`modules`] itself; the terminal and desktop adapters
//! draw [`text`] inside the Rust Native tree.
use coder_access::protocol::INVITATION_PREFIX;

/// The QR modules of a `coder-host:` invitation, row by row, with a
/// four-module quiet zone; `true` is a dark module. `None` for a string that
/// is not an invitation or does not fit.
#[must_use]
pub fn modules(code: &str) -> Option<Vec<Vec<bool>>> {
    coder_connect::pairing::qr_modules_prefixed(INVITATION_PREFIX, code).ok()
}

/// The code as text: two module rows per line in half blocks. Adapters draw
/// text lit on a dark background, so the lit cells are the light modules and
/// the quiet zone.
#[must_use]
pub fn text(modules: &[Vec<bool>]) -> String {
    let light = |y: usize, x: usize| modules.get(y).and_then(|row| row.get(x)) == Some(&false);
    let width = modules.first().map_or(0, Vec::len);
    let mut output = String::new();
    for y in (0..modules.len()).step_by(2) {
        if y > 0 {
            output.push('\n');
        }
        for x in 0..width {
            // A missing bottom row past an odd height stays dark.
            output.push(match (light(y, x), light(y + 1, x)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_draws_light_modules_two_rows_per_line() {
        let modules = vec![
            vec![false, true, false],
            vec![false, false, true],
            vec![true, false, false],
        ];
        assert_eq!(text(&modules), "█▄▀\n ▀▀");
    }

    #[test]
    fn only_host_invitations_render() {
        assert!(modules("coder-pair:AAAA").is_none());
        assert!(modules("coder-host:not valid!").is_none());
        let rows = modules("coder-host:AAAA").unwrap();
        assert!(rows.len() >= 29 && rows.iter().all(|row| row.len() == rows.len()));
    }
}
