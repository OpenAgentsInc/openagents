//! The carrot ladder: one shade more orange for every garden won, from
//! snow white (shade 0) to neon orange (shade 20).

/// The ladder's last shade.
pub const MAX_SHADE: u32 = 20;

/// The fur colour of each shade, sRGB.
pub const SHADES: [u32; 21] = [
    0xFFFFFF, 0xFEFBF8, 0xFFF6EE, 0xFFF1E4, 0xFFECDB, 0xFFE6D0, 0xFFE0C5, 0xFFD9BB, 0xFED3B1,
    0xFFCCA5, 0xFFC59A, 0xFEBE90, 0xFFB684, 0xFFAE79, 0xFFA56C, 0xFE9D61, 0xFE9455, 0xFF8A45,
    0xFE8137, 0xFF7623, 0xFE6B04,
];

/// The shade a win count earns; it never passes the last.
#[must_use]
pub fn shade_for_wins(wins: u32) -> u32 {
    wins.min(MAX_SHADE)
}

/// The fur colour for a win count.
#[must_use]
pub fn fur(wins: u32) -> u32 {
    SHADES[shade_for_wins(wins) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_runs_white_to_neon_and_caps() {
        assert_eq!(fur(0), 0xFFFFFF);
        assert_eq!(fur(20), 0xFE6B04);
        assert_eq!(fur(500), 0xFE6B04);
        assert_eq!(shade_for_wins(7), 7);
        // Each step is no lighter in green and blue than the one before.
        for pair in SHADES.windows(2) {
            assert!((pair[1] & 0xFF) <= (pair[0] & 0xFF));
        }
    }
}
