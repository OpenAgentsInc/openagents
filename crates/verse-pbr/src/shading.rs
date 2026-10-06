//! Shared local-light and coverage semantics before backend shader translation.
pub(crate) fn source(source: &str) -> String {
    source.replace("// VERSE_SHARED_SHADING", include_str!("shading.wgsl"))
}
