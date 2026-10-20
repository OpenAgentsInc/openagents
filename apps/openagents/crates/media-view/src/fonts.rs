//! Bundled interface fonts shared with image conversion.

pub static GEIST: [&[u8]; 8] = [
    include_bytes!("../../ui/assets/fonts/Geist.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-Italic.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-MediumItalic.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-SemiBoldItalic.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-Bold.ttf"),
    include_bytes!("../../ui/assets/fonts/Geist-BoldItalic.ttf"),
];

pub static GEIST_MONO: [&[u8]; 8] = [
    include_bytes!("../../ui/assets/fonts/GeistMono.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-Italic.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-MediumItalic.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-SemiBold.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-SemiBoldItalic.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-Bold.ttf"),
    include_bytes!("../../ui/assets/fonts/GeistMono-BoldItalic.ttf"),
];

/// Font faces shared by the interface and SVG text-to-path conversion.
pub fn bundled_font_faces() -> impl Iterator<Item = &'static [u8]> {
    GEIST.iter().chain(GEIST_MONO.iter()).copied()
}
