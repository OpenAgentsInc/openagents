//! The look's settings and rules that don't need a browser: quality tiers,
//! the page's options, and the `chroma` rule (only the bunny, its food and
//! power-ups are in colour; every other material is gray, OKLCH chroma at
//! most 0.02).

/// A quality tier, as `verse_engine::quality::Tier` names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Old or slow devices: drawn at one pixel per CSS pixel, thin lines.
    Low,
    /// Phones: up to 1.5 pixels per CSS pixel.
    Medium,
    /// Desktops: up to 2 pixels per CSS pixel.
    High,
}

impl Tier {
    /// The tier a browser gets without asking: phones (a coarse pointer)
    /// Medium, everything else High.
    #[must_use]
    pub fn default_for(coarse_pointer: bool) -> Self {
        if coarse_pointer {
            Self::Medium
        } else {
            Self::High
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    /// The most device pixels the scene is drawn with per CSS pixel.
    #[must_use]
    pub fn pixel_ratio(self, device: f32) -> f32 {
        let cap = match self {
            Self::Low => 1.0,
            Self::Medium => 1.5,
            Self::High => 2.0,
        };
        device.clamp(1.0, cap)
    }

    /// The scene's offscreen size as a share of the canvas: Low draws its
    /// fill and line buffers at three quarters.
    #[must_use]
    pub fn target_scale(self) -> f32 {
        if self == Self::Low { 0.75 } else { 1.0 }
    }

    /// How far, in target pixels, the line pass looks to each side: lines
    /// come out about twice that wide, 2 px at 1080 rows, scaled with the
    /// height.
    #[must_use]
    pub fn line_radius(self, rows: i32) -> f32 {
        let base = (1.25 * rows as f32 / 1080.0).max(1.0);
        if self == Self::Low { 1.0 } else { base }
    }
}

/// What the page's address fragment asks for, `#tier=low&contrast=high`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub tier: Option<Tier>,
    pub high_contrast: bool,
    /// A garden to open straight away (1 to 5), for testing.
    pub garden: Option<usize>,
    /// Captures: the farmer stays in his shed.
    pub quiet: bool,
    /// Captures: open the kit sheet instead of a garden.
    pub kit: bool,
}

impl Options {
    #[must_use]
    pub fn parse(hash: &str) -> Self {
        let mut options = Self::default();
        for pair in hash.trim_start_matches('#').split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            match key {
                "tier" => options.tier = Tier::parse(value),
                "contrast" => options.high_contrast = value == "high",
                "garden" => options.garden = value.parse().ok().filter(|n| (1..=99).contains(n)),
                "quiet" => options.quiet = true,
                "kit" => options.kit = true,
                _ => {}
            }
        }
        options
    }
}

/// OKLCH chroma of an sRGB colour with channels from 0 to 1.
#[must_use]
pub fn chroma(rgb: [f32; 3]) -> f32 {
    let linear = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = rgb.map(linear);
    let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (a * a + bb * bb).sqrt()
}

/// The `chroma` rule's limit for gray materials.
pub const GRAY_CHROMA: f32 = 0.02;

/// Admits a mesh that isn't flagged `chroma`: every vertex colour must be
/// gray.
pub fn admit_gray(mesh: &crate::mesh::Mesh) -> Result<(), String> {
    for vertex in mesh.data.chunks(crate::mesh::STRIDE) {
        let colour = [vertex[9], vertex[10], vertex[11]];
        if chroma(colour) > GRAY_CHROMA {
            return Err(format!("a gray material has colour {colour:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rgb;

    #[test]
    fn options_read_from_the_fragment() {
        let o = Options::parse("#tier=low&contrast=high&garden=3&quiet");
        assert_eq!(o.tier, Some(Tier::Low));
        assert!(o.high_contrast && o.quiet && !o.kit);
        assert_eq!(o.garden, Some(3));
        assert_eq!(Options::parse(""), Options::default());
        assert_eq!(Options::parse("#garden=x&tier=ultra").tier, None);
    }

    #[test]
    fn tiers_cap_the_pixel_ratio_and_scale_lines() {
        assert_eq!(Tier::High.pixel_ratio(3.0), 2.0);
        assert_eq!(Tier::Medium.pixel_ratio(3.0), 1.5);
        assert_eq!(Tier::Low.pixel_ratio(3.0), 1.0);
        assert_eq!(Tier::High.line_radius(1080), 1.25);
        assert_eq!(Tier::High.line_radius(2160), 2.5);
        assert_eq!(Tier::Low.line_radius(2160), 1.0);
        assert_eq!(Tier::default_for(true), Tier::Medium);
    }

    #[test]
    fn chroma_is_zero_for_grays_and_high_for_orange() {
        assert!(chroma(rgb(0xF4F4F2)) < 0.005);
        assert!(chroma(rgb(0x3A3A3A)) < 0.005);
        assert!(chroma(rgb(0xFE6B04)) > 0.18);
        // The ladder's last shade is the spec's 0.199.
        assert!((chroma(rgb(0xFE6B04)) - 0.199).abs() < 0.01);
    }
}
