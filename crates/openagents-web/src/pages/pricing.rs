//! `/pricing`: Free, Pro, and Teams side by side, then a few questions
//! (`docs/pro/README.md`, "Pricing page").
//!
//! Pro's numbers come from the checked-in plan terms
//! ([`retail_cloud::environment::plan`]), the same ones Settings → Plan
//! shows, so the page can't drift from what Pro sells. Every button goes
//! somewhere real: Pro's to Settings → Plan, where Subscribe opens
//! checkout (or Settings says subscribing isn't open on this server), and
//! signed out through sign-up first.

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, Render, html};
use openagents_ui::content::{Heading, MarkdownRoot, PageColumn, Paragraph, PlanCard, PlanCards};
use retail_cloud::environment::{EnvironmentPlan, usd};

use crate::App;
use crate::account::Account;
use crate::ui_page::UiPage;

/// The page's path.
pub(crate) const PRICING: &str = "/pricing";
/// Where Pro is bought and managed.
const PLAN: &str = "/settings#settings-plan";
/// Where teams and companies write to us.
const CONTACT: &str = "/contact?topic=sales";

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PRICING, get(pricing))
}

/// Whether this page is served. It promises the 3-day trial and Pro
/// features that aren't built yet (`docs/pro/README.md`, build steps 2-6),
/// so a deployment shows it only with `OPENAGENTS_PRICING_PAGE=1`;
/// otherwise `/pricing` goes to the pricing guide, as it did before.
fn served() -> bool {
    cfg!(test) || std::env::var("OPENAGENTS_PRICING_PAGE").is_ok_and(|v| v == "1")
}

/// Who is looking: what each card's button says and where it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visitor {
    /// This server doesn't sign people in.
    Anonymous,
    SignedOut,
    Free,
    Pro,
}

async fn pricing(State(app): State<App>, headers: HeaderMap) -> Response {
    if !served() {
        return axum::response::Redirect::temporary("/docs/pricing").into_response();
    }
    let visitor = match crate::account::current() {
        Account::Unknown => Visitor::Anonymous,
        Account::SignedOut => Visitor::SignedOut,
        Account::SignedIn { .. } => {
            if on_pro(&app, &headers).await {
                Visitor::Pro
            } else {
                Visitor::Free
            }
        }
    };
    let terms = app
        .config
        .plan
        .as_deref()
        .map_or_else(retail_cloud::environment::plan, |p| p.terms.clone());
    UiPage::new("Pricing")
        .path(PRICING)
        .description(
            "OpenAgents is free to start. Pro adds cloud time and cloud agents for $20 a month.",
        )
        .scriptless()
        .content(body(&terms, visitor))
        .respond(&headers)
}

/// Whether the signed-in viewer has a paid Pro month running here.
async fn on_pro(app: &App, headers: &HeaderMap) -> bool {
    let Some(service) = app.config.cloud.as_deref() else {
        return false;
    };
    match service.authenticate(headers).await {
        Ok(viewer) => crate::plan::on_pro(app, &viewer.account_id),
        Err(_) => false,
    }
}

/// `6,000`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn free(visitor: Visitor) -> PlanCard {
    let (label, href) = match visitor {
        Visitor::Anonymous => ("Download Coder", crate::layout::DOWNLOAD.to_owned()),
        Visitor::SignedOut => ("Sign up free", crate::auth::login_href("/", true)),
        Visitor::Free => ("Current plan", PLAN.to_owned()),
        Visitor::Pro => ("Start a chat", "/".to_owned()),
    };
    PlanCard::new("Free", "Free")
        .note("For people and agents")
        .lead("All of Coder, and:")
        .item("Coder on terminal, desktop, web and phone")
        .item_with(
            "Every coding agent: Claude Code, Codex, OpenCode, Grok Build, Devin and Cursor",
            [
                "Use your Claude, ChatGPT and other subscriptions",
                "Bring your own keys",
                "No fee on models you bring",
            ],
        )
        .item_with(
            "System 1 in front of every agent",
            ["Routing, briefs and checks that save you money"],
        )
        .item("A free model allowance every day")
        .item("Unlimited projects and chats")
        .item("Sync on every device")
        .item("Public share links")
        .item("Plugins: use any, publish your own")
        .action(label, href)
}

fn pro(t: &EnvironmentPlan, visitor: Visitor) -> PlanCard {
    let (label, href) = match visitor {
        Visitor::Pro => ("Current plan", PLAN.to_owned()),
        Visitor::SignedOut => (
            "Start 3-day free trial",
            crate::auth::login_href(crate::settings::PAGE, true),
        ),
        Visitor::Anonymous | Visitor::Free => ("Start 3-day free trial", PLAN.to_owned()),
    };
    PlanCard::new("Pro", usd(t.price_usd_micros))
        .per("/mo.")
        .note("Starts with a 3-day free trial")
        .lead("Everything in Free, and:")
        .item_with(
            format!(
                "{} minutes of cloud time a month",
                thousands(t.included_machine_hours * 60)
            ),
            [format!(
                "{} vCPU, {} GB machines \u{b7} {} at once",
                t.machine.vcpus, t.machine.memory_gb, t.machines_at_once
            )],
        )
        .item_with(
            "Cloud agents that keep working with your laptop closed",
            ["Issue runs, long jobs and scheduled runs"],
        )
        .item_with("Image and file uploads", ["In chats and as project files"])
        .item_with(
            "Advanced projects",
            [format!(
                "Project knowledge, secrets, multiple repos and {} GB of saved setups",
                t.storage_gb
            )],
        )
        .item_with(
            "Model usage included each month",
            ["On top of the free daily allowance"],
        )
        .item_with(
            "CAD Lab",
            ["Design parts by talking, measure them, and compare versions"],
        )
        .item_with(
            format!("Extra time at {} an hour", usd(t.extra_hour_usd_micros)),
            ["Only if you turn it on, up to a monthly limit you set"],
        )
        .action(label, href)
        .featured(true)
}

fn teams() -> PlanCard {
    PlanCard::new("Enterprise", "Custom")
        .note("For companies and custom deployments")
        .lead("Everything in Pro, and:")
        .item_with(
            "Deploy where you need it",
            ["Our cloud, your own cloud account, or your own machines"],
        )
        .item("Single sign-on (SAML) and user provisioning (SCIM)")
        .item("Uptime SLA and priority support")
        .item("Audit logs and data retention controls")
        .item("Shared billing, invoicing and spend limits")
        .item("Security review and a custom agreement")
        .action("Contact sales", CONTACT)
}

/// The questions under the cards.
const FAQ: &[(&str, &str)] = &[
    (
        "How does the free trial work?",
        "Start it with a card. Cancel anytime in the first 3 days and you pay nothing. We remind you the day before the trial ends.",
    ),
    (
        "What if I use more than Pro includes?",
        "We never stop your work at a limit we chose, only at one you set. Extra time is charged only if you turn it on, up to your own monthly limit.",
    ),
    (
        "How do I pay?",
        "Pay by card, or with Bitcoin over Lightning. The 3-day free trial is on card; with Bitcoin, Pro starts when you pay. Credits for extra use can be bought either way.",
    ),
    (
        "Do I pay for the models I bring?",
        "No. Your own Claude, ChatGPT and other subscriptions, and your own keys, are never charged by us.",
    ),
    (
        "Can my agents use OpenAgents?",
        "Yes. Agents get the same plans as people, and an agent can read its plan through the API.",
    ),
];

fn body(t: &EnvironmentPlan, visitor: Visitor) -> Markup {
    let cards = PlanCards::new("Plans")
        .heading("Pricing")
        .lead("Start free. Upgrade when you need the cloud.")
        .cards([free(visitor), pro(t, visitor), teams()]);
    PageColumn::new(html! {
        (cards)
        (MarkdownRoot::new(html! {
            (Heading::new(2, "Questions").id("questions"))
            @for (question, answer) in FAQ {
                (Heading::new(3, *question))
                (Paragraph::new(*answer))
            }
        }).label("Questions"))
    })
    .wide()
    .render()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(html: &str) -> String {
        crate::copy_guard::assert_plain(PRICING, html);
        oa_copy::visible_text(html)
    }

    #[test]
    fn three_cards_with_minutes_and_buttons_that_go_somewhere() {
        let terms = retail_cloud::environment::plan();
        for visitor in [
            Visitor::Anonymous,
            Visitor::SignedOut,
            Visitor::Free,
            Visitor::Pro,
        ] {
            let html = body(&terms, visitor).into_string();
            let t = text(&html);
            assert_eq!(html.matches("<article class=\"oa-plan-card\"").count(), 3);
            for name in [
                "aria-label=\"Free\"",
                "aria-label=\"Pro\"",
                "aria-label=\"Enterprise\"",
            ] {
                assert!(html.contains(name), "{name}");
            }
            assert!(t.contains("6,000 minutes of cloud time a month"), "{t}");
            assert!(t.contains("2 vCPU, 8 GB machines \u{b7} 2 at once"), "{t}");
            assert!(
                html.contains("<span class=\"oa-plan-card__amount\">$20</span><span class=\"oa-plan-card__per\">/mo.</span>"),
                "{html}"
            );
            assert!(t.contains("Extra time at $0.18 an hour"), "{t}");
            assert!(html.contains(&format!("href=\"{CONTACT}\"")));
            // Every button is a link with a real target; none is disabled.
            assert_eq!(html.matches("class=\"oa-plan-card__action\"").count(), 3);
            assert!(!html.contains("aria-disabled") && !html.contains("<button"));
            let pro = &html[html.find("aria-label=\"Pro\"").unwrap()..];
            let pro = &pro[..pro.find("</article>").unwrap()];
            let expected = match visitor {
                Visitor::SignedOut => "href=\"/signup?return_to=%2Fsettings\"",
                _ => "href=\"/settings#settings-plan\"",
            };
            assert!(pro.contains(expected), "{visitor:?}: {pro}");
            assert_eq!(
                t.contains("Start 3-day free trial"),
                visitor != Visitor::Pro,
                "{visitor:?}"
            );
            assert_eq!(
                t.contains("Current plan"),
                matches!(visitor, Visitor::Free | Visitor::Pro)
            );
        }
    }

    #[test]
    fn minutes_read_with_commas() {
        assert_eq!(thousands(6000), "6,000");
        assert_eq!(thousands(600), "600");
        assert_eq!(thousands(1_200_000), "1,200,000");
    }
}
