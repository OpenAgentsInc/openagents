//! The remaining desktop bundle matches its crate version.

const MAC_INFO_PLIST: &str = include_str!("../../../bins/openagents-desktop-macos/Info.plist");

#[test]
fn the_mac_info_plist_has_the_desktop_crate_version() {
    let after_key = MAC_INFO_PLIST
        .split("<key>CFBundleShortVersionString</key>")
        .nth(1)
        .expect("CFBundleShortVersionString in bins/openagents-desktop-macos/Info.plist");
    let value = after_key
        .split("<string>")
        .nth(1)
        .and_then(|rest| rest.split("</string>").next())
        .expect("a string value");
    assert_eq!(value.trim(), env!("CARGO_PKG_VERSION"));
}
