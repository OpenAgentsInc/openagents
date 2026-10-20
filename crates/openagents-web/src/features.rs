//! The features that aren't ready for everyone yet, and the one switch that
//! turns them on (#12336, `docs/audits/2026-10-20-gpui-everywhere.md`).
//!
//! Each [`Feature`] here is off unless the deployment lists it in
//! `OPENAGENTS_WEB_FEATURES` (a comma list; `all` turns every one on, and
//! `-name` turns one off again after `all`). A feature that is off is
//! hidden: [`gate`] answers 404 for its routes, and the pages leave out
//! its links, buttons, and sections instead of showing them disabled. The
//! GPUI pages get the features that are on in their `openagents-features`
//! meta tag ([`Features::meta`]), so the canvas hides the same things.
//!
//! The older per-feature switches still work: `OPENAGENTS_WEB_AGENT_REPLIES=on`
//! turns on [`Feature::AgentReplies`] and `OPENAGENTS_PRICING_PAGE=1` turns
//! on [`Feature::Billing`].
//!
//! A [`crate::Config`] built in code ([`crate::Config::development`], which
//! the tests use) has every feature on, so each feature's own tests keep
//! running; the binary starts from [`Features::from_env`], where every
//! feature is off. A feature leaves this list when it's ready: delete its
//! variant and its checks, and it's on for everyone.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use crate::App;

/// The environment variable that lists the features to turn on.
pub const ENV: &str = "OPENAGENTS_WEB_FEATURES";

/// The page's meta tag that names the features that are on.
pub const META: &str = "openagents-features";

/// A feature that isn't ready for everyone yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// The browser wallet that asks the phone to pay (U5, #12258):
    /// `/v1/wallet/browser*`. The phone's approvals and the Lightning
    /// address (#12307) haven't passed their checks.
    Wallet,
    /// Earn on a paired computer (#12294): the GPUI Settings page. Its
    /// scratch-host check hasn't run.
    Earn,
    /// The Pro plan and its checkout: Settings' plan section,
    /// `/settings/plan/*`, and the `/pricing` page (else the pricing
    /// guide). Checkout isn't configured in production.
    Billing,
    /// The affiliate program: `/affiliates` and Settings' affiliate
    /// earnings. It pays from sales that need Stripe (#11727).
    Affiliates,
    /// Selling CAD parts for sats: `/cad-shop*`, `/cad-order/*`,
    /// `/cad-purchases`, and selling a version from a design.
    CadShop,
    /// The live payment pictures `/live` and `/stats`, which show no
    /// payments until the network takes real ones.
    PaymentStats,
    /// Published agents answer mentions and messages (#12256).
    AgentReplies,
    /// Hand tracking in `/verse` (#12309): the Hands button.
    VerseHands,
    /// Moving a computer's chat to a cloud computer from the GPUI chat
    /// (#12282). Bringing one back stays.
    MoveToCloud,
}

impl Feature {
    pub const ALL: [Feature; 9] = [
        Feature::Wallet,
        Feature::Earn,
        Feature::Billing,
        Feature::Affiliates,
        Feature::CadShop,
        Feature::PaymentStats,
        Feature::AgentReplies,
        Feature::VerseHands,
        Feature::MoveToCloud,
    ];

    /// The feature's name in [`ENV`] and in the meta tag.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Feature::Wallet => "wallet",
            Feature::Earn => "earn",
            Feature::Billing => "billing",
            Feature::Affiliates => "affiliates",
            Feature::CadShop => "cad_shop",
            Feature::PaymentStats => "payment_stats",
            Feature::AgentReplies => "agent_replies",
            Feature::VerseHands => "verse_hands",
            Feature::MoveToCloud => "move_to_cloud",
        }
    }

    /// The feature `name` names, if any.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|feature| feature.name() == name)
    }

    const fn bit(self) -> u32 {
        1 << self as u32
    }

    /// Whether `path` is one of this feature's routes.
    #[must_use]
    pub fn owns(self, path: &str) -> bool {
        let under = |base: &str| {
            path.strip_prefix(base)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        };
        match self {
            Feature::Wallet => under("/v1/wallet/browser"),
            Feature::Billing => under("/settings/plan"),
            Feature::Affiliates => under("/affiliates"),
            Feature::CadShop => {
                under("/cad-shop")
                    || under("/cad-order")
                    || under("/cad-purchases")
                    || path
                        .strip_prefix("/cad/")
                        .is_some_and(|rest| rest.ends_with("/unlist"))
            }
            Feature::PaymentStats => path == "/live" || path == "/stats",
            Feature::Earn | Feature::AgentReplies | Feature::VerseHands | Feature::MoveToCloud => {
                false
            }
        }
    }
}

/// The features that are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features(u32);

impl Default for Features {
    /// Every feature off, as production starts.
    fn default() -> Self {
        Self::none()
    }
}

impl Features {
    /// Every feature off.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// Every feature on.
    #[must_use]
    pub fn all() -> Self {
        Self(Feature::ALL.iter().fold(0, |bits, f| bits | f.bit()))
    }

    /// The features `list` turns on, and the names in it that aren't
    /// features. `all` turns every one on; `-name` turns one off.
    #[must_use]
    pub fn parse(list: &str) -> (Self, Vec<String>) {
        let mut features = Self::none();
        let mut unknown = Vec::new();
        for item in list.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let (off, name) = match item.strip_prefix('-') {
                Some(name) => (true, name),
                None => (false, item),
            };
            if name == "all" {
                features = if off { Self::none() } else { Self::all() };
                continue;
            }
            match Feature::named(name) {
                Some(feature) => features.set(feature, !off),
                None => unknown.push(item.to_owned()),
            }
        }
        (features, unknown)
    }

    /// The deployment's features: [`ENV`], then the older switches.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        let (features, unknown) = Self::from_vars(var);
        for name in unknown {
            eprintln!("openagents-web: {ENV}: no feature is named {name:?}");
        }
        features
    }

    /// [`Self::from_env`] over `var`.
    fn from_vars(var: impl Fn(&str) -> Option<String>) -> (Self, Vec<String>) {
        let (mut features, unknown) = Self::parse(&var(ENV).unwrap_or_default());
        if var("OPENAGENTS_WEB_AGENT_REPLIES").as_deref() == Some("on") {
            features.set(Feature::AgentReplies, true);
        }
        if var("OPENAGENTS_PRICING_PAGE").as_deref() == Some("1") {
            features.set(Feature::Billing, true);
        }
        (features, unknown)
    }

    pub fn set(&mut self, feature: Feature, on: bool) {
        if on {
            self.0 |= feature.bit();
        } else {
            self.0 &= !feature.bit();
        }
    }

    /// The same features with `feature` on or off.
    #[must_use]
    pub fn with(mut self, feature: Feature, on: bool) -> Self {
        self.set(feature, on);
        self
    }

    #[must_use]
    pub const fn on(self, feature: Feature) -> bool {
        self.0 & feature.bit() != 0
    }

    /// The feature that is off and owns `path`, if any.
    #[must_use]
    pub fn hides(self, path: &str) -> Option<Feature> {
        Feature::ALL
            .into_iter()
            .find(|feature| !self.on(*feature) && feature.owns(path))
    }

    /// The features that are on, as the meta tag's comma list.
    #[must_use]
    pub fn meta(self) -> String {
        Feature::ALL
            .into_iter()
            .filter(|feature| self.on(*feature))
            .map(Feature::name)
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Answers 404 for a hidden feature's routes, as if they weren't there:
/// the site's not-found page, or a JSON refusal under `/v1/`.
pub(crate) async fn gate(State(app): State<App>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if app.config.features.hides(path).is_none() {
        return next.run(request).await;
    }
    if path.starts_with("/v1/") {
        return crate::coder_sync::refused(StatusCode::NOT_FOUND, "not_found", "Not found.");
    }
    crate::not_found_for(request.headers())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_is_off_unless_listed() {
        let (none, unknown) = Features::parse("");
        assert!(unknown.is_empty());
        for feature in Feature::ALL {
            assert!(!none.on(feature), "{feature:?}");
            assert_eq!(Feature::named(feature.name()), Some(feature));
        }
        assert_eq!(Features::default(), Features::none());
    }

    #[test]
    fn the_list_turns_features_on_and_off() {
        let (features, unknown) = Features::parse(" wallet, earn ,nope");
        assert!(features.on(Feature::Wallet) && features.on(Feature::Earn));
        assert!(!features.on(Feature::Billing));
        assert_eq!(unknown, ["nope"]);
        let (features, _) = Features::parse("all,-cad_shop");
        assert!(features.on(Feature::Billing));
        assert!(!features.on(Feature::CadShop));
        assert_eq!(features.meta().split(',').count(), Feature::ALL.len() - 1);
    }

    #[test]
    fn the_older_switches_still_turn_their_feature_on() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        let (features, _) = Features::from_vars(vars(&[
            ("OPENAGENTS_WEB_AGENT_REPLIES", "on"),
            ("OPENAGENTS_PRICING_PAGE", "1"),
        ]));
        assert!(features.on(Feature::AgentReplies) && features.on(Feature::Billing));
        let (features, _) = Features::from_vars(vars(&[("OPENAGENTS_WEB_AGENT_REPLIES", "1")]));
        assert_eq!(features, Features::none());
    }

    #[test]
    fn each_route_belongs_to_its_feature() {
        let off = Features::none();
        for (path, feature) in [
            ("/v1/wallet/browser", Feature::Wallet),
            ("/v1/wallet/browser/requests/abc", Feature::Wallet),
            ("/settings/plan/subscribe", Feature::Billing),
            ("/affiliates", Feature::Affiliates),
            ("/cad-shop", Feature::CadShop),
            ("/cad-shop/tok/checkout", Feature::CadShop),
            ("/cad-order/o1/qr", Feature::CadShop),
            ("/cad-purchases", Feature::CadShop),
            ("/cad/bracket/unlist", Feature::CadShop),
            ("/live", Feature::PaymentStats),
            ("/stats", Feature::PaymentStats),
        ] {
            assert_eq!(off.hides(path), Some(feature), "{path}");
            assert_eq!(off.with(feature, true).hides(path), None, "{path}");
        }
        for path in [
            "/",
            "/settings",
            "/v1/wallet",
            "/v1/wallet/browserx",
            "/cad",
            "/cad/bracket",
            "/cad-shopping",
            "/livestream",
            "/api/stats",
            "/pricing",
        ] {
            assert_eq!(off.hides(path), None, "{path}");
        }
    }
}
