//! The Pro plan on Settings (#11006): what it includes, this month's
//! hours, saved storage, and the extra-hours choice with its monthly cap
//! (`docs/cloud/retail-environment-contract.md`).
//!
//! The numbers come from the environment meter
//! ([`retail_cloud::environment`]) kept at `--plan-meter`. Without it this
//! server can't show hours or take the extra-hours choice, and Settings
//! says so.
//!
//! With `--plan-checkout PLAN` (and the meter), Subscribe opens the
//! account service's Stripe Checkout for that plan on the person's own
//! workspace, and Manage subscription opens Stripe's billing page (card,
//! cancel). Stripe's signed event, not the browser coming back, starts the
//! month: the gateway writes it into the same meter, so Settings shows Pro
//! once it lands (#11072). Without `--plan-checkout`, Settings says
//! subscribing isn't open instead of showing a button.

use std::path::Path;
use std::sync::Mutex;

use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType};
use openagents_ui::forms::{Checkbox, Field, Input, InputType};
use retail_cloud::environment::{
    self, EnvironmentPlan, ExtraHours, Standing, Summary, day_label, usd,
};
use retail_cloud::journal::Journal;

pub(crate) const EXTRA: &str = "/settings/plan/extra";
pub(crate) const CSRF_SCOPE: &str = "plan-extra-hours";
pub(crate) const SUBSCRIBE: &str = "/settings/plan/subscribe";
pub(crate) const MANAGE: &str = "/settings/plan/manage";
pub(crate) const CHECKOUT_SCOPE: &str = "plan-checkout";
/// The highest monthly cap the form takes, in dollars.
const CAP_DOLLARS_MAX: u64 = 10_000;

/// The plan this server shows and the meter behind it.
pub struct Plans {
    pub(crate) terms: EnvironmentPlan,
    meter: Option<Mutex<Journal>>,
    /// The billing plan Subscribe buys, when checkout is set up here.
    checkout: Option<String>,
}

impl Plans {
    /// The checked-in plan, the meter journal at `meter` if given, and the
    /// billing plan id Subscribe buys if checkout is set up here.
    ///
    /// # Errors
    ///
    /// The meter cannot be opened.
    pub fn open(meter: Option<&Path>, checkout: Option<String>) -> Result<Self, String> {
        let meter = meter
            .map(|path| Journal::open(path).map(Mutex::new))
            .transpose()
            .map_err(|error| format!("--plan-meter: {error}"))?;
        Ok(Self {
            terms: environment::plan(),
            meter,
            checkout,
        })
    }

    /// A plan over an existing journal, for tests and fixtures.
    #[must_use]
    pub fn with_meter(meter: Option<Journal>, checkout: Option<String>) -> Self {
        Self {
            terms: environment::plan(),
            meter: meter.map(Mutex::new),
            checkout,
        }
    }

    pub(crate) fn has_meter(&self) -> bool {
        self.meter.is_some()
    }

    /// The plan Subscribe buys: only with a meter, so a paid month always
    /// has somewhere to show up.
    pub(crate) fn checkout(&self) -> Option<&str> {
        self.checkout.as_deref().filter(|_| self.has_meter())
    }

    /// What Settings shows for `account` now. `returned`: the browser just
    /// came back from checkout.
    pub(crate) fn view(&self, account: &str, now: i64, returned: bool) -> View {
        let summary = self.meter.as_ref().map(|meter| {
            meter
                .lock()
                .ok()
                .and_then(|j| environment::summary(&j, &self.terms, account, now).ok())
        });
        View {
            terms: self.terms.clone(),
            summary,
            checkout: self.checkout().is_some(),
            returned,
        }
    }

    /// Save the account's extra-hours choice.
    pub(crate) fn set_extra(&self, account: &str, choice: ExtraHours) -> Result<(), ()> {
        let meter = self.meter.as_ref().ok_or(())?;
        let mut journal = meter.lock().map_err(|_| ())?;
        environment::set_extra_hours(&mut journal, account, choice).map_err(|_| ())
    }
}

/// One account's plan section.
pub(crate) struct View {
    pub terms: EnvironmentPlan,
    /// `None`: no meter here. `Some(None)`: the meter can't be read.
    pub summary: Option<Option<Summary>>,
    /// Subscribe and Manage subscription work here.
    pub checkout: bool,
    /// Back from checkout, before Stripe's event has landed.
    pub returned: bool,
}

/// `12.5`, `100`.
fn hours(seconds: u64) -> String {
    let tenths = seconds.div_ceil(360);
    if tenths % 10 == 0 {
        format!("{}", tenths / 10)
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}

/// What the plan includes, in one line.
pub(crate) fn includes(t: &EnvironmentPlan) -> String {
    format!(
        "{} a month: {} hours a month on a {} vCPU, {} GB machine, {} machines at once, and {} GB of saved environments. Unused hours don't roll over. Models run on your own Claude or Codex key or subscription.",
        usd(t.price_usd_micros),
        t.included_machine_hours,
        t.machine.vcpus,
        t.machine.memory_gb,
        t.machines_at_once,
        t.storage_gb,
    )
}

/// Dollars typed into the cap field, as millionths: `10`, `10.5`, `10.50`.
pub(crate) fn parse_cap(text: &str) -> Option<u64> {
    let text = text.trim().trim_start_matches('$');
    let (whole, cents) = match text.split_once('.') {
        Some((whole, cents)) => (whole, cents),
        None => (text, ""),
    };
    if whole.is_empty() && cents.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit())
        || !cents.bytes().all(|b| b.is_ascii_digit())
        || cents.len() > 2
        || whole.len() > 6
    {
        return None;
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    if whole > CAP_DOLLARS_MAX {
        return None;
    }
    let cents: u64 = format!("{cents:0<2}").parse().ok()?;
    Some(whole * 1_000_000 + cents * 10_000)
}

fn cap_text(micros: u64) -> String {
    usd(micros).trim_start_matches('$').to_owned()
}

/// One button that posts a ticketed form.
fn post_button(action: &str, label: &str, ticket: (&str, &str)) -> Markup {
    html! {
        form method="post" action=(action) {
            input type="hidden" name="csrf" value=(ticket.0);
            input type="hidden" name="request" value=(ticket.1);
            (Button::new(label).kind(ButtonType::Submit))
        }
    }
}

/// The Plan section. `csrf` is the extra-hours form's ticket and request;
/// `checkout` is the Subscribe / Manage subscription forms' ticket.
pub(crate) fn section(
    view: &View,
    csrf: Option<(&str, &str)>,
    checkout: Option<(&str, &str)>,
) -> Markup {
    let t = &view.terms;
    let active = match &view.summary {
        Some(Some(s)) => match &s.standing {
            Standing::Active { period } => Some((s, period.end)),
            _ => None,
        },
        _ => None,
    };
    html! {
        section class="oa-settings-group" aria-labelledby="settings-plan" {
            h2 #settings-plan { "Plan" }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { (t.name) }
                    span class="oa-settings-hint" { (includes(t)) }
                    span class="oa-settings-hint" {
                        @if let Some((_, renews)) = active {
                            "You're on " (t.name) ". It renews " (day_label(renews)) " unless you cancel."
                        } @else if view.checkout && view.returned {
                            "Thanks. Stripe is confirming your payment. Reload this page in a minute to see " (t.name) "."
                        } @else if view.checkout {
                            "You're not subscribed."
                        } @else {
                            "Subscribing isn't open on this server yet."
                        }
                    }
                }
                @if let (true, Some(ticket)) = (view.checkout, checkout) {
                    div class="oa-settings-control" {
                        @if active.is_some() {
                            (post_button(MANAGE, "Manage subscription", ticket))
                        } @else {
                            (post_button(SUBSCRIBE, "Subscribe", ticket))
                        }
                    }
                }
            }
            @match (&view.summary, active) {
                (None, _) => {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Hours this month" }
                            span class="oa-settings-hint" { "This server doesn't track hours yet." }
                        }
                    }
                }
                (Some(None), _) => {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Hours this month" }
                            span class="oa-settings-hint" { "Your hours can't be read right now." }
                        }
                    }
                }
                (Some(Some(_)), None) => {}
                (Some(Some(_)), Some((s, resets))) => {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Hours this month" }
                            span class="oa-settings-hint" {
                                (hours(s.used_seconds)) " of " (t.included_machine_hours)
                                " hours used. Resets " (day_label(resets)) "."
                            }
                        }
                    }
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "Saved environments" }
                            span class="oa-settings-hint" {
                                (s.storage_gb) " of " (t.storage_gb) " GB, "
                                (s.versions) " of " (t.saved_versions) " saved versions."
                            }
                        }
                    }
                    (extra_row(t, s, resets, csrf))
                }
            }
        }
    }
}

fn extra_row(t: &EnvironmentPlan, s: &Summary, resets: i64, csrf: Option<(&str, &str)>) -> Markup {
    let cap = Field::new("plan-extra-cap", "Monthly limit, in dollars");
    html! {
        div class="oa-settings-row" {
            div class="oa-settings-text" {
                span class="oa-settings-label" { "Extra hours" }
                span class="oa-settings-hint" {
                    @if s.extra.enabled {
                        "On: " (usd(t.extra_hour_usd_micros)) " an hour from your credits after this month's "
                        (t.included_machine_hours) " hours, up to " (usd(s.extra.cap_usd_micros))
                        " a month. " (usd(s.extra_spent_usd_micros)) " used this month."
                    } @else {
                        "Off. When this month's hours run out, setups stop until "
                        (day_label(resets)) ". Turn this on to keep going at "
                        (usd(t.extra_hour_usd_micros)) " an hour from your credits, up to a monthly limit you set."
                    }
                }
                @if let Some((ticket, request)) = csrf {
                    form method="post" action=(EXTRA) {
                        input type="hidden" name="csrf" value=(ticket);
                        input type="hidden" name="request" value=(request);
                        p {
                            (Checkbox::new("enabled", "Use extra hours")
                                .value("on")
                                .checked(s.extra.enabled))
                        }
                        (cap.clone().control(
                            Input::new("cap")
                                .input_type(InputType::Text)
                                .inputmode("decimal")
                                .value(cap_text(s.extra.cap_usd_micros))
                                .aria(cap.aria()),
                        ))
                        p { (Button::new("Save").kind(ButtonType::Submit)) }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retail_cloud::environment::Period;

    const NOW: i64 = 1_791_504_000; // October 9, 2026.

    fn subscribed() -> Journal {
        let mut j = Journal::in_memory().unwrap();
        environment::record_period(
            &mut j,
            &Period {
                account: "acct".into(),
                plan: environment::plan().version,
                start: NOW - 86_400,
                end: NOW + 30 * 86_400,
            },
        )
        .unwrap();
        j
    }

    fn text(html: &str) -> String {
        let text = oa_copy::visible_text(html);
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
        text
    }

    #[test]
    fn caps_parse_as_dollars() {
        assert_eq!(parse_cap("10"), Some(10_000_000));
        assert_eq!(parse_cap("$10.5"), Some(10_500_000));
        assert_eq!(parse_cap("0.18"), Some(180_000));
        assert_eq!(parse_cap(""), None);
        assert_eq!(parse_cap("ten"), None);
        assert_eq!(parse_cap("1.234"), None);
        assert_eq!(parse_cap("10001"), None);
        assert_eq!(hours(45 * 60), "0.8");
        assert_eq!(hours(3600 * 100), "100");
    }

    #[test]
    fn no_checkout_says_so_instead_of_a_button() {
        let plans = Plans::with_meter(None, None);
        let html = section(&plans.view("acct", NOW, false), None, None).into_string();
        let t = text(&html);
        assert!(
            t.contains("Subscribing isn't open on this server yet."),
            "{t}"
        );
        assert!(t.contains("This server doesn't track hours yet."), "{t}");
        assert!(
            t.contains("$20 a month: 100 hours a month on a 2 vCPU, 8 GB machine"),
            "{t}"
        );
        assert!(!html.contains(">Subscribe<"));
        assert!(!html.contains("<form"));
        // Checkout without a meter would charge with nowhere to show the
        // month: still no button.
        let plans = Plans::with_meter(None, Some("pro".into()));
        let html = section(&plans.view("acct", NOW, false), None, Some(("t", "r"))).into_string();
        assert!(!html.contains("<form"), "{html}");
    }

    #[test]
    fn subscribe_opens_checkout_and_a_subscriber_can_manage_it() {
        let plans = Plans::with_meter(Some(Journal::in_memory().unwrap()), Some("pro".into()));
        let html = section(&plans.view("acct", NOW, false), None, Some(("t", "r"))).into_string();
        let t = text(&html);
        assert!(t.contains("You're not subscribed."), "{t}");
        assert!(
            html.contains("action=\"/settings/plan/subscribe\""),
            "{html}"
        );
        assert!(html.contains(">Subscribe<"), "{html}");
        // Back from Stripe before its event: say what's happening.
        let html = section(&plans.view("acct", NOW, true), None, Some(("t", "r"))).into_string();
        let t = text(&html);
        assert!(
            t.contains("Thanks. Stripe is confirming your payment. Reload this page in a minute to see Pro."),
            "{t}"
        );
        // After the event: Pro, and the billing page instead of Subscribe.
        let plans = Plans::with_meter(Some(subscribed()), Some("pro".into()));
        let html = section(
            &plans.view("acct", NOW, true),
            Some(("t", "r")),
            Some(("t", "r")),
        )
        .into_string();
        let t = text(&html);
        assert!(
            t.contains("You're on Pro. It renews November 8 unless you cancel."),
            "{t}"
        );
        assert!(html.contains("action=\"/settings/plan/manage\""), "{html}");
        assert!(!html.contains(">Subscribe<"), "{html}");
    }

    #[test]
    fn a_subscriber_sees_hours_storage_and_the_extra_hours_choice() {
        let plans = Plans::with_meter(Some(subscribed()), None);
        let html = section(&plans.view("acct", NOW, false), Some(("t", "r")), None).into_string();
        let t = text(&html);
        assert!(
            t.contains("You're on Pro. It renews November 8 unless you cancel."),
            "{t}"
        );
        assert!(t.contains("0 of 100 hours used. Resets November 8."), "{t}");
        assert!(t.contains("0 of 20 GB, 0 of 10 saved versions."), "{t}");
        assert!(t.contains("Off. When this month's hours run out"), "{t}");
        assert!(html.contains("action=\"/settings/plan/extra\""));
        plans
            .set_extra(
                "acct",
                ExtraHours {
                    enabled: true,
                    cap_usd_micros: 10_000_000,
                },
            )
            .unwrap();
        let html = section(&plans.view("acct", NOW, false), Some(("t", "r")), None).into_string();
        let t = text(&html);
        assert!(
            t.contains("On: $0.18 an hour from your credits after this month's 100 hours, up to $10 a month. $0 used this month."),
            "{t}"
        );
        assert!(html.contains("value=\"10\""), "{html}");
    }
}
