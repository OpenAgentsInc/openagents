//! The site's features that aren't ready for everyone yet (#12336).
//!
//! The server lists the features that are on in the page's
//! `openagents-features` meta tag (`crates/openagents-web/src/features.rs`).
//! A feature that is off is hidden: the app leaves out its controls. A page
//! with no such tag comes from a server older than the registry, which
//! hid nothing, so every feature counts as on there.

/// The page's `<meta>` that lists the features that are on.
pub const META: &str = "openagents-features";
/// Earn on a paired computer (#12294).
pub const EARN: &str = "earn";
/// Moving a computer's chat to a cloud computer (#12282).
pub const MOVE_TO_CLOUD: &str = "move_to_cloud";

/// Whether `list`, the meta tag's content, names `feature`; `None` (no
/// tag) turns every feature on.
pub fn listed(list: Option<&str>, feature: &str) -> bool {
    list.is_none_or(|list| list.split(',').any(|name| name.trim() == feature))
}

/// Whether the page turns `feature` on.
#[cfg(target_family = "wasm")]
pub fn on(feature: &str) -> bool {
    let list = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| {
            d.query_selector(&format!("meta[name=\"{META}\"]"))
                .ok()
                .flatten()
        })
        .and_then(|e| e.get_attribute("content"));
    listed(list.as_deref(), feature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_names_the_features_that_are_on() {
        assert!(listed(Some("wallet,earn"), EARN));
        assert!(!listed(Some("wallet"), EARN));
        assert!(!listed(Some(""), MOVE_TO_CLOUD));
        assert!(listed(None, MOVE_TO_CLOUD));
    }
}
