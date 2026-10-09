//! The phone apps draw in the web's fonts (#11120): the system faces of the
//! `--font-sans` and `--font-mono` stacks (`oa_tokens::typography`), SF Pro
//! and SF Mono on iOS, Roboto and Droid Sans Mono on Android, at the web's
//! type scale. These tests read the iOS and Android host sources and fail on
//! a bundled font, a font family outside the stacks, or an iOS text-style
//! size off the scale.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The host sources: iOS's app, tests, and project, and Android's app
/// sources, resources, and build script.
fn host_roots() -> Vec<PathBuf> {
    let root = repo();
    vec![
        root.join("bins/openagents-ios/host/App"),
        root.join("bins/openagents-ios/host/UITests"),
        root.join("bins/openagents-ios/host/project.yml"),
        root.join("bins/openagents-android/host/app/src/main"),
        root.join("bins/openagents-android/host/app/build.gradle.kts"),
    ]
}

/// Every file under the host roots, with its text when it is text.
fn host_files() -> Vec<(PathBuf, Option<String>)> {
    let mut stack = host_roots();
    let mut files = Vec::new();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("a source directory") {
                stack.push(entry.expect("an entry").path());
            }
        } else if path.is_file() {
            let text = std::fs::read_to_string(&path).ok();
            files.push((path, text));
        }
    }
    assert!(files.len() > 50, "found only {} host files", files.len());
    files
}

/// The phone hosts bundle no font, list none for UIKit, and name no face
/// outside the web's stacks: no Paper Mono and no custom face lookups.
#[test]
fn the_phones_bundle_and_name_no_other_font() {
    let banned = [
        "PaperMono",
        "Paper Mono",
        "paper_mono",
        "paper-mono",
        "UIAppFonts",
        ".custom(",
        "UIFont(name:",
        "R.font.",
        "@font/",
        "createFromAsset",
        "/fonts/",
        "Geist",
        "JetBrains",
        "Fira",
        "Cascadia",
        ".otf",
        ".woff",
    ];
    for (path, text) in host_files() {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        assert!(
            !name.ends_with(".ttf") && !name.ends_with(".otf") && !name.ends_with(".woff2"),
            "{} is a bundled font",
            path.display()
        );
        assert!(
            !path.ends_with("res/font"),
            "{} is a font resource directory",
            path.display()
        );
        let Some(text) = text else { continue };
        for word in banned {
            assert!(!text.contains(word), "{} names {word}", path.display());
        }
    }
}

/// Every family an Android resource or typeface names is one the token
/// stacks name (Android resolves `sans-serif` to Roboto and `monospace` to
/// Droid Sans Mono).
#[test]
fn every_android_family_is_a_token_family() {
    let mut stacks = oa_tokens::typography::sans();
    stacks.extend(oa_tokens::typography::mono());
    let mut named = Vec::new();
    for (path, text) in host_files() {
        let Some(text) = text else { continue };
        for (open, close) in [
            ("android:fontFamily\">", "<"),
            ("android:fontFamily=\"", "\""),
            ("TypefaceSpan(\"", "\""),
            ("Typeface.create(\"", "\""),
        ] {
            for (index, _) in text.match_indices(open) {
                let rest = &text[index + open.len()..];
                let family = &rest[..rest.find(close).unwrap_or(rest.len())];
                named.push((path.clone(), family.to_string()));
            }
        }
    }
    assert!(
        named.iter().any(|(_, family)| family == "sans-serif"),
        "the theme sets no family"
    );
    for (path, family) in named {
        assert!(
            stacks.contains(&family),
            "{} names {family}, outside the token stacks",
            path.display()
        );
    }
}

/// The iOS text styles are the web's type scale, and the app draws them in
/// the system faces.
#[test]
fn the_ios_text_styles_are_the_web_scale() {
    let path = repo().join("bins/openagents-ios/host/App/Typeface.swift");
    let source = std::fs::read_to_string(&path).expect("Typeface.swift");
    let mut sizes = 0;
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("case .").or(line.strip_prefix("default:")) else {
            continue;
        };
        let value = rest.rsplit(':').next().unwrap_or(rest);
        let value = value.split("//").next().unwrap_or(value).trim();
        let Ok(size) = value.parse::<f32>() else {
            continue;
        };
        assert!(
            oa_tokens::typography::on_scale(size),
            "{line} is off the web's type scale"
        );
        sizes += 1;
    }
    assert!(sizes >= 20, "found only {sizes} text-style sizes");
    for face in [
        ".systemFont(ofSize:",
        ".monospacedSystemFont(ofSize:",
        "design: .monospaced",
    ] {
        assert!(source.contains(face), "Typeface.swift draws no {face}");
    }
    let body = |style: &str| {
        source
            .lines()
            .find(|line| line.contains(&format!("case .{style}")))
            .and_then(|line| line.split("//").next())
            .and_then(|line| line.rsplit(':').next())
            .and_then(|size| size.trim().parse::<f32>().ok())
    };
    assert_eq!(
        body("body, .callout"),
        Some(oa_tokens::typography::conversation::BODY.size)
    );
    assert_eq!(
        body("subheadline"),
        Some(oa_tokens::typography::menu::ROW.size)
    );
}
