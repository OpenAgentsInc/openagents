//! The desktop app ships at the phone app's version. The source of truth is
//! `MARKETING_VERSION` in `bins/openagents-ios/host/project.yml` (the
//! TestFlight version, which the Android build also reads); this crate's
//! version, the bundle's `Info.plist`, and Android's fallback must equal it.

const PROJECT_YML: &str = include_str!("../../../bins/openagents-ios/host/project.yml");
const MAC_INFO_PLIST: &str = include_str!("../../../bins/openagents-desktop-macos/Info.plist");
const ANDROID_GRADLE: &str =
    include_str!("../../../bins/openagents-android/host/app/build.gradle.kts");

fn phone_version() -> &'static str {
    PROJECT_YML
        .lines()
        .find_map(|line| line.trim().strip_prefix("MARKETING_VERSION:"))
        .map(str::trim)
        .expect("MARKETING_VERSION in bins/openagents-ios/host/project.yml")
}

#[test]
fn the_desktop_app_has_the_phone_apps_version() {
    assert_eq!(
        env!("CARGO_PKG_VERSION"),
        phone_version(),
        "crates/openagents-desktop/Cargo.toml's version must equal MARKETING_VERSION \
         in bins/openagents-ios/host/project.yml; change them together"
    );
}

#[test]
fn the_mac_info_plist_has_the_phone_apps_version() {
    let after_key = MAC_INFO_PLIST
        .split("<key>CFBundleShortVersionString</key>")
        .nth(1)
        .expect("CFBundleShortVersionString in bins/openagents-desktop-macos/Info.plist");
    let value = after_key
        .split("<string>")
        .nth(1)
        .and_then(|rest| rest.split("</string>").next())
        .expect("a string value");
    assert_eq!(value.trim(), phone_version());
}

#[test]
fn the_android_fallback_has_the_phone_apps_version() {
    assert!(
        ANDROID_GRADLE.contains(&format!(
            "gradleProperty(\"openagentsVersionName\").orElse(\"{}\")",
            phone_version()
        )),
        "bins/openagents-android/host/app/build.gradle.kts's versionName fallback must equal \
         MARKETING_VERSION"
    );
}
