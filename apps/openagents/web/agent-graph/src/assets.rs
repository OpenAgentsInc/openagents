//! The fonts the graph draws with: the desktop's Geist and Geist Mono. In a
//! browser the page fetches them from the site's shared font files
//! (`/gpui-fonts/`, the same files `/demo/gpui` and `/gpui` load), so the
//! module carries none; natively, for `first-frame-capture`, they're
//! compiled in from the same files.

/// The faces the graph draws, by file name under
/// `apps/openagents/crates/ui/assets/fonts/`, in the order the page hands
/// them to [`crate::mount`].
pub const FONT_FILES: [&str; 4] = [
    "Geist.ttf",
    "Geist-Medium.ttf",
    "Geist-SemiBold.ttf",
    "GeistMono.ttf",
];

#[cfg(not(target_family = "wasm"))]
macro_rules! font {
    ($path:literal) => {
        include_bytes!(concat!("../../../crates/ui/assets/fonts/", $path))
    };
}

/// The [`FONT_FILES`] faces, compiled in, for native captures.
#[cfg(not(target_family = "wasm"))]
pub fn font_faces() -> Vec<std::borrow::Cow<'static, [u8]>> {
    let faces: [&'static [u8]; 4] = [
        font!("Geist.ttf"),
        font!("Geist-Medium.ttf"),
        font!("Geist-SemiBold.ttf"),
        font!("GeistMono.ttf"),
    ];
    faces.into_iter().map(std::borrow::Cow::Borrowed).collect()
}
