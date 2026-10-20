//! The fonts the graph draws with, bundled into the module: the desktop's
//! Geist and Geist Mono, the same files `/demo/gpui` ships.

use std::borrow::Cow;

macro_rules! font {
    ($path:literal) => {
        include_bytes!(concat!("../../../crates/ui/assets/fonts/", $path))
    };
}

pub fn font_faces() -> Vec<Cow<'static, [u8]>> {
    let faces: [&'static [u8]; 4] = [
        font!("Geist.ttf"),
        font!("Geist-Medium.ttf"),
        font!("Geist-SemiBold.ttf"),
        font!("GeistMono.ttf"),
    ];
    faces.into_iter().map(Cow::Borrowed).collect()
}
