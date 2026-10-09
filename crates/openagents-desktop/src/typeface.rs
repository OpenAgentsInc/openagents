//! The window's fonts: the web's (#11120). Text draws in the first family
//! of the `--font-sans` stack this computer has, code in the first of the
//! `--font-mono` stack (`oa_tokens::typography`): SF Pro and SF Mono on a
//! Mac, Segoe UI and Consolas on Windows, Noto Sans and DejaVu Sans Mono or
//! Liberation Mono on Linux. The window bundles no font of its own; only a
//! computer with none of the stack's families falls back to the shaper's
//! bundled face.
//!
//! [`install`] runs once, first thing in `main`, before any text is
//! measured.

/// Font files for a family named in the token stacks, in the order to try.
/// A family the stacks name with no file here (an emoji face, a generic
/// keyword this platform resolves elsewhere) is skipped.
#[must_use]
pub fn files(family: &str) -> &'static [&'static str] {
    match family {
        // The platform's UI face.
        "ui-sans-serif" | "-apple-system" | "system-ui" => &[
            "/System/Library/Fonts/SFNS.ttf",
            r"C:\Windows\Fonts\SegUIVar.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ],
        "Segoe UI" => &[
            r"C:\Windows\Fonts\SegUIVar.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ],
        "Noto Sans" => &[
            "/usr/share/fonts/noto/NotoSans[wdth,wght].ttf",
            "/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/run/current-system/sw/share/X11/fonts/NotoSans[wdth,wght].ttf",
        ],
        "Helvetica" => &["/System/Library/Fonts/Helvetica.ttc"],
        "Arial" => &[
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Arial.ttf",
        ],
        "sans-serif" => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        ],
        // The platform's monospace face.
        "ui-monospace" | "SFMono-Regular" | "SF Mono" => &["/System/Library/Fonts/SFNSMono.ttf"],
        "Menlo" => &["/System/Library/Fonts/Menlo.ttc"],
        "Monaco" => &["/System/Library/Fonts/Monaco.ttf"],
        "Consolas" => &[r"C:\Windows\Fonts\consola.ttf"],
        "Liberation Mono" => &[
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
        ],
        "DejaVu Sans Mono" | "monospace" => &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
        ],
        "Courier New" => &[
            "/System/Library/Fonts/Supplemental/Courier New.ttf",
            r"C:\Windows\Fonts\cour.ttf",
        ],
        _ => &[],
    }
}

/// The first file of `stack`'s families that exists here, with the family
/// that named it.
#[must_use]
pub fn resolve(stack: &[String]) -> Option<(String, std::path::PathBuf)> {
    stack.iter().find_map(|family| {
        files(family)
            .iter()
            .map(std::path::Path::new)
            .find(|path| path.is_file())
            .map(|path| (family.clone(), path.to_path_buf()))
    })
}

/// Which faces the window draws in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub text: Option<(String, std::path::PathBuf)>,
    pub code: Option<(String, std::path::PathBuf)>,
}

/// The faces the token stacks pick on this computer.
#[must_use]
pub fn choose() -> Chosen {
    Chosen {
        text: resolve(&oa_tokens::typography::sans()),
        code: resolve(&oa_tokens::typography::mono()),
    }
}

/// Installs the stacks' faces for every view in this process. Without a
/// text face the shaper keeps its bundled one; without a code face, code
/// draws in the text face.
#[cfg(feature = "app")]
pub fn install() -> Chosen {
    let chosen = choose();
    let read = |pick: &Option<(String, std::path::PathBuf)>| -> Option<&'static [u8]> {
        let bytes = std::fs::read(&pick.as_ref()?.1).ok()?;
        Some(Box::leak(bytes.into_boxed_slice()))
    };
    if let Some(text) = read(&chosen.text) {
        let code = read(&chosen.code).unwrap_or(text);
        if let Err(problem) = rust_native::layout::shape::install_faces(text, code) {
            eprintln!("fonts: {problem}");
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every family the resolver knows is one the token stacks name, so the
    /// window can draw in no family outside the web's type system.
    #[test]
    fn every_font_family_is_a_token_family() {
        let mut stacks = oa_tokens::typography::sans();
        stacks.extend(oa_tokens::typography::mono());
        let known = [
            "ui-sans-serif",
            "-apple-system",
            "system-ui",
            "Segoe UI",
            "Noto Sans",
            "Helvetica",
            "Arial",
            "sans-serif",
            "ui-monospace",
            "SFMono-Regular",
            "SF Mono",
            "Menlo",
            "Monaco",
            "Consolas",
            "Liberation Mono",
            "DejaVu Sans Mono",
            "monospace",
            "Courier New",
        ];
        for family in known {
            assert!(!files(family).is_empty(), "{family} has no files");
            assert!(
                stacks.iter().any(|named| named == family),
                "{family} is not in the token stacks"
            );
        }
        assert!(files("Paper Mono").is_empty());
        assert!(files("Inter").is_empty());
    }

    /// The window's own code names no font family and bundles no font
    /// file: every face comes from the token stacks through this module.
    #[test]
    fn the_window_bundles_and_names_no_other_font() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut stack = vec![root.join("src")];
        let banned = [
            "Paper Mono",
            "PaperMono-",
            "paper_mono::",
            "Inter-",
            "Geist",
            "JetBrains",
            "Fira",
            "Cascadia",
            ".ttf\")",
            ".otf\")",
            ".woff",
        ];
        while let Some(path) = stack.pop() {
            for entry in std::fs::read_dir(&path).expect("a source directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.file_name().is_some_and(|name| name == "typeface.rs") {
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for word in banned {
                    assert!(!source.contains(word), "{} names {word}", path.display());
                }
            }
        }
        for entry in std::fs::read_dir(root).expect("the crate") {
            let name = entry.expect("an entry").file_name();
            let name = name.to_string_lossy().to_lowercase();
            assert!(
                !name.ends_with(".ttf") && !name.ends_with(".otf") && name != "fonts",
                "the crate bundles {name}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_mac_draws_in_sf_pro_and_sf_mono() {
        let chosen = choose();
        let text = chosen.text.expect("a text face");
        let code = chosen.code.expect("a code face");
        assert_eq!(text.0, "ui-sans-serif");
        assert!(text.1.ends_with("SFNS.ttf"));
        assert_eq!(code.0, "ui-monospace");
        assert!(code.1.ends_with("SFNSMono.ttf"));
        for path in [text.1, code.1] {
            let bytes = std::fs::read(&path).expect("the face");
            let face = swash::FontRef::from_index(&bytes, 0).expect("a font");
            assert!(face.charmap().map('A') != 0);
        }
    }
}
