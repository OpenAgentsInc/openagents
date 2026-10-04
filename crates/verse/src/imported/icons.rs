//! Action-bar icons: game-icons.net SVGs, rasterized and tinted for the atlas.
//!
//! Each hotbar sprite key maps to one vendored SVG under
//! `assets/verse/icons/game-icons/`, its author for the CC BY 3.0 credit, and a
//! tint. The SVGs are white on transparent, so the tint is the icon's color.
use resvg::{tiny_skia, usvg};

/// One hotbar icon and the credit its license requires.
pub struct Icon {
    /// The atlas sprite key that `Ability::icon` or `SpellDef::icon` names.
    pub key: &'static str,
    /// Path under `assets/verse/icons/game-icons/`.
    pub file: &'static str,
    pub svg: &'static [u8],
    pub author: &'static str,
    /// Linear RGB multiplier for the white artwork.
    pub tint: [f32; 3],
    /// Mirrors the artwork top to bottom.
    pub flip: bool,
}

macro_rules! icon {
    ($key:literal, $file:literal, $author:literal, $tint:expr) => {
        icon!($key, $file, $author, $tint, false)
    };
    ($key:literal, $file:literal, $author:literal, $tint:expr, $flip:expr) => {
        Icon {
            key: $key,
            file: $file,
            svg: include_bytes!(concat!("../../../../assets/verse/icons/game-icons/", $file)),
            author: $author,
            tint: $tint,
            flip: $flip,
        }
    };
}

const PARCHMENT: [f32; 3] = [0.93, 0.85, 0.66];
const FIRE: [f32; 3] = [1.0, 0.55, 0.2];
const ARCANE: [f32; 3] = [0.85, 0.55, 1.0];
const MIST: [f32; 3] = [0.72, 0.86, 1.0];
const STORM: [f32; 3] = [0.55, 0.75, 1.0];
const SILK: [f32; 3] = [0.86, 0.86, 0.8];
const OIL: [f32; 3] = [0.86, 0.7, 0.36];
const SUNLIGHT: [f32; 3] = [1.0, 0.9, 0.5];
const WARD: [f32; 3] = [0.45, 0.7, 1.0];
const TRANSMUTATION: [f32; 3] = [0.76, 0.62, 1.0];
const STONE: [f32; 3] = [0.82, 0.76, 0.66];
const WIND: [f32; 3] = [0.6, 0.95, 0.9];
const ROT: [f32; 3] = [0.55, 0.9, 0.5];
const METEOR: [f32; 3] = [1.0, 0.42, 0.18];

/// Every hotbar icon: the original ten abilities, then the SRD spell slots.
pub const ICONS: &[Icon] = &[
    icon!(
        "bow-icon",
        "delapouite/bow-arrow.svg",
        "Delapouite",
        PARCHMENT
    ),
    icon!("fire-bolt-icon", "lorc/fire-ray.svg", "Lorc", FIRE),
    icon!(
        "magic-missile-icon",
        "lorc/energy-arrow.svg",
        "Lorc",
        ARCANE
    ),
    icon!("fireball-icon", "lorc/fireball.svg", "Lorc", FIRE),
    icon!("misty-step-icon", "lorc/teleport.svg", "Lorc", MIST),
    icon!("thunderwave-icon", "lorc/sonic-boom.svg", "Lorc", STORM),
    icon!("web-icon", "lorc/spider-web.svg", "Lorc", SILK),
    icon!("grease-icon", "lorc/dripping-goo.svg", "Lorc", OIL),
    icon!("light-icon", "lorc/lantern-flame.svg", "Lorc", SUNLIGHT),
    icon!("shield-icon", "lorc/magic-shield.svg", "Lorc", WARD),
    icon!(
        "telekinesis-icon",
        "lorc/juggler.svg",
        "Lorc",
        TRANSMUTATION
    ),
    icon!(
        "wall-of-stone-icon",
        "delapouite/stone-wall.svg",
        "Delapouite",
        STONE
    ),
    icon!(
        "levitate-icon",
        "delapouite/body-height.svg",
        "Delapouite",
        TRANSMUTATION
    ),
    icon!(
        "feather-fall-icon",
        "lorc/feather.svg",
        "Lorc",
        TRANSMUTATION
    ),
    icon!("gust-of-wind-icon", "lorc/wind-slap.svg", "Lorc", WIND),
    icon!("wind-wall-icon", "lorc/whirlwind.svg", "Lorc", WIND),
    icon!(
        "black-tentacles-icon",
        "delapouite/tentacles-barrier.svg",
        "Delapouite",
        ROT
    ),
    icon!(
        "meteor-swarm-icon",
        "lorc/meteor-impact.svg",
        "Lorc",
        METEOR
    ),
    icon!(
        "reverse-gravity-icon",
        "delapouite/gravitation.svg",
        "Delapouite",
        TRANSMUTATION,
        true
    ),
];

/// Edge length of a rasterized icon. The slot draws 36 logical units at up to
/// about 2.75 pixels each on a Retina display, so 128 pixels stays sharp.
pub const SIZE: u32 = 128;
/// The slot background behind the artwork.
const BACKGROUND: [f32; 3] = [18. / 255., 21. / 255., 29. / 255.];
/// Fraction of the edge left empty on each side of the artwork.
const PADDING: f32 = 0.12;

pub fn icon(key: &str) -> Option<&'static Icon> {
    ICONS.iter().find(|i| i.key == key)
}

/// The vendored license and credits, for the asset inventory.
pub const LICENSE: &[u8] = include_bytes!("../../../../assets/verse/icons/game-icons/license.txt");
pub const CREDITS: &[u8] = include_bytes!("../../../../assets/verse/icons/game-icons/CREDITS.md");

/// Rasterizes an icon to `SIZE`×`SIZE` straight-alpha RGBA: the tinted
/// artwork over the opaque slot background.
pub fn rasterize(icon: &Icon) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_data(icon.svg, &usvg::Options::default())
        .map_err(|e| format!("Invalid icon {}: {e}", icon.file))?;
    let mut pixmap = tiny_skia::Pixmap::new(SIZE, SIZE).ok_or("Invalid icon size")?;
    let source = tree.size();
    let inner = SIZE as f32 * (1. - 2. * PADDING);
    let scale = inner / source.width().max(source.height());
    let offset = SIZE as f32 * PADDING;
    let mut transform = tiny_skia::Transform::from_row(scale, 0., 0., scale, offset, offset);
    if icon.flip {
        transform = transform.post_concat(tiny_skia::Transform::from_row(
            1.,
            0.,
            0.,
            -1.,
            0.,
            SIZE as f32,
        ));
    }
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut out = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for pixel in pixmap.pixels() {
        // White artwork: premultiplied red equals coverage.
        let coverage = f32::from(pixel.alpha()) / 255.;
        for (tint, background) in icon.tint.iter().zip(BACKGROUND) {
            let value = background * (1. - coverage) + tint * coverage;
            out.push((value.clamp(0., 1.) * 255.).round() as u8);
        }
        out.push(255);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ability_and_spell_slot_has_an_icon() {
        for ability in verse_world::play::Ability::ALL {
            assert!(icon(ability.icon()).is_some(), "{}", ability.icon());
        }
        for spell in verse_world::spells::CATALOG {
            assert!(icon(spell.icon).is_some(), "{} has no icon", spell.icon);
        }
        let mut keys: Vec<_> = ICONS.iter().map(|i| i.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ICONS.len(), "duplicate icon keys");
    }

    #[test]
    fn every_icon_is_credited() {
        let credits = std::str::from_utf8(CREDITS).unwrap();
        for icon in ICONS {
            assert!(credits.contains(icon.file), "{} is not credited", icon.file);
            assert!(
                credits.contains(&format!("Icons made by {}", icon.author)),
                "{} has no credit line",
                icon.author
            );
            assert!(icon.file.starts_with(&icon.author.to_lowercase()));
        }
    }

    #[test]
    fn icons_rasterize_deterministically_with_visible_artwork() {
        for icon in ICONS {
            let first = rasterize(icon).unwrap();
            assert_eq!(first.len(), (SIZE * SIZE * 4) as usize);
            assert_eq!(first, rasterize(icon).unwrap(), "{}", icon.key);
            // The artwork covers a real share of the slot but not all of it.
            let lit = first
                .chunks(4)
                .filter(|p| p[..3].iter().map(|&c| u32::from(c)).sum::<u32>() > 300)
                .count() as f32
                / (SIZE * SIZE) as f32;
            assert!((0.05..0.75).contains(&lit), "{} covers {lit}", icon.key);
        }
    }

    #[test]
    fn reverse_gravity_is_the_flipped_artwork() {
        let flipped = icon("reverse-gravity-icon").unwrap();
        let upright = Icon {
            flip: false,
            ..*flipped
        };
        let (a, b) = (rasterize(flipped).unwrap(), rasterize(&upright).unwrap());
        let row = (SIZE * 4) as usize;
        let mirrored: Vec<u8> = b.chunks(row).rev().flatten().copied().collect();
        let differing = a.iter().zip(&mirrored).filter(|(x, y)| x != y).count();
        assert!(differing < a.len() / 100, "{differing} bytes differ");
    }

    /// Writes every rasterized icon in one row to the PNG that
    /// `VERSE_ICON_SHEET` names, for visual review of the atlas input.
    #[test]
    #[ignore = "writes review evidence; run with VERSE_ICON_SHEET=path"]
    fn write_icon_sheet() {
        let path = std::env::var("VERSE_ICON_SHEET").expect("VERSE_ICON_SHEET");
        let (count, size) = (ICONS.len(), SIZE as usize);
        let mut sheet = vec![0u8; count * size * size * 4];
        for (i, icon) in ICONS.iter().enumerate() {
            let pixels = rasterize(icon).unwrap();
            for y in 0..size {
                let row = &pixels[y * size * 4..(y + 1) * size * 4];
                let at = (y * count * size + i * size) * 4;
                sheet[at..at + size * 4].copy_from_slice(row);
            }
        }
        let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let mut encoder = png::Encoder::new(file, (count * size) as u32, SIZE);
        encoder.set_color(png::ColorType::Rgba);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&sheet)
            .unwrap();
    }
}
