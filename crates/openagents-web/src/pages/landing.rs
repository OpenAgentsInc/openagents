//! The homepage for visitors who aren't signed in: `/` shows this page
//! (`super::home`); a signed-in person's `/` is their new chat, and anyone
//! can start a chat at `/new`. `/landing`, its address while it was built,
//! goes to `/`.
//!
//! Round one of the UI system plan
//! (docs/ui/2026-10-15-shadcn-converter-and-ui-system-plan.md): the page
//! follows the section order of Initium's landing page (initium.sh), built
//! from blocks named as Initium and shadcn name them (navbar, hero section,
//! feature section, stats section, steps, pricing section, FAQ section, CTA
//! section, footer) and its free Button styles (default, outline, 3D),
//! written here in Maud and CSS over shadcn's color roles. No Initium or
//! shadcnblocks source is in the repository: Initium's Pro blocks may not
//! go in a public one, so only the free styles' look and the page's order
//! are followed. The words, figures, diagrams and screenshots are ours.
//!
//! Every figure is one we publish: the cost and time per accepted fix from
//! the docs' "How Coder works" diagrams and the research write-up, and
//! Pro's price from the plan terms `/pricing` sells.

use axum::Router;
use axum::http::{HeaderMap, header};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};

use crate::App;
use crate::pages::download::{CODER_PS1, CODER_SH};
use openagents_ui::content::CodeBlock;

const CSS: &str = include_str!("../../static/landing/landing.css");
const CSS_PATH: &str = "/landing/landing.css";
const RESEARCH: &str =
    "https://github.com/OpenAgentsInc/openagents/blob/main/docs/inference/briefed-agent-ab.md";
const HOW: &str = "/docs/coder-how-it-works";
/// The Coder 1.0 announcement on the blog.
const ANNOUNCEMENT: &str = "/blog/coder-1-0";
const GITHUB: &str = "https://github.com/OpenAgentsInc/openagents";
const X: &str = "https://x.com/OpenAgentsInc";
const DESCRIPTION: &str =
    "Coder sits in front of every coding agent you use, and sends each step to the model it needs.";

/// The one screenshot: an example of what people make with OpenAgents.
const CAD_SHOT: (&str, &[u8]) = (
    "/landing/web-cad.jpg",
    include_bytes!("../../static/landing/web-cad.jpg"),
);

/// The agents Coder sits in front of, as the docs' diagram names them.
const AGENTS: [&str; 6] = [
    "Claude Code",
    "Codex",
    "OpenCode",
    "Grok Build",
    "Devin CLI",
    "Cursor",
];

/// The page as `/` answers a visitor who isn't signed in.
pub(crate) fn respond(headers: &HeaderMap) -> axum::response::Response {
    Html(page(crate::theme::from_headers(headers)).into_string()).into_response()
}

pub(crate) fn routes() -> Router<App> {
    let (path, bytes) = CAD_SHOT;
    Router::new()
        .route(
            "/landing",
            get(|| async { axum::response::Redirect::permanent("/") }),
        )
        .route(
            CSS_PATH,
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS) }),
        )
        .route(
            path,
            get(move || async move {
                ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response()
            }),
        )
}

/// Initium's Button styles, as a link: `default`, `outline`, and `3d`.
fn button(label: &str, href: &str, variant: &str) -> Markup {
    html! {
        a class=(format!("button button-{variant}")) data-slot="button" href=(href) { (label) }
    }
}

/// A section's small label over its heading (Initium's eyebrow).
fn eyebrow(text: &str) -> Markup {
    html! { p.eyebrow { (text) } }
}

/// The section links, in the order the wide header and the menu list them.
const SECTIONS: [(&str, &str); 6] = [
    ("How it works", "#how"),
    ("Agents", "#agents"),
    ("Pricing", "#pricing"),
    ("Docs", "/docs"),
    ("Blog", "/blog"),
    ("FAQ", "#faq"),
];

/// The header. Wide: the brand, the section links, Try it, Log in, and
/// Download. At phone and tablet width (900px and under): the brand,
/// Download, and a Menu button that opens the rest. The menu is a
/// `<details>`, so it opens and closes with no script, from the keyboard
/// too (Tab to it, Enter or Space).
fn navbar() -> Markup {
    let log_in = crate::auth::login_href("/", false);
    html! {
        header.navbar data-slot="navbar" {
            a.navbar-brand href="/" {
                img src="/favicon.svg" alt="" width="24" height="24";
                span { "OpenAgents" }
            }
            nav.navbar-links aria-label="Sections" {
                @for (label, href) in SECTIONS { a href=(href) { (label) } }
            }
            div.navbar-account {
                a.navbar-login href=(super::home::NEW) { "Try it in your browser" }
                a.navbar-login href=(log_in) { "Log in" }
            }
            (button("Download Coder", "/download", "3d"))
            details.navbar-menu {
                summary.navbar-menu-button aria-label="Menu" {
                    span.navbar-menu-icon aria-hidden="true" { span {} span {} span {} }
                }
                nav.navbar-menu-panel aria-label="Menu" {
                    @for (label, href) in SECTIONS { a href=(href) { (label) } }
                    hr;
                    a href=(super::home::NEW) { "Try it in your browser" }
                    a href=(log_in) { "Log in" }
                }
            }
        }
    }
}

fn hero_section() -> Markup {
    html! {
        section.hero-section data-slot="hero-section" aria-labelledby="hero-title" {
            div.hero-copy {
                a.hero-pill href=(ANNOUNCEMENT) {
                    span.hero-pill-tag { "New" }
                    span { "Coder 1.0 is out" }
                    span.hero-pill-arrow aria-hidden="true" { "\u{2192}" }
                }
                h1 #hero-title { "One Coder in front of every coding agent you use" }
                p.hero-lead {
                    "Coder works out what each step needs. Small steps go to small, fast models; only the hard parts reach the big one."
                }
                div.hero-actions {
                    (button("Download Coder", "/download", "3d"))
                    (button("How it works", HOW, "outline"))
                }
            }
            (hero_visual())
        }
    }
}

/// The hero's picture: Coder's prompt with the agents it can run, and the
/// cards around it say what the docs show (a held step, the study's cost).
fn hero_visual() -> Markup {
    html! {
        div.hero-visual aria-hidden="true" {
            div.prompt-card {
                div.prompt-chips {
                    span.chip.chip-active { "Auto" }
                    @for agent in &AGENTS[..3] { span.chip { (agent) } }
                    span.chip { "+3" }
                }
                p.prompt-text { "Fix the failing login test, then open a PR" span.prompt-caret {} }
                div.prompt-steps {
                    p { span.dot.dot-small {} "Find the test" em { "small model" } }
                    p { span.dot.dot-small {} "Read the code" em { "small model" } }
                    p { span.dot.dot-big {} "Write the fix" em { "big model" } }
                }
            }
            div.float-card.float-stat {
                p.float-label { "Per accepted fix" }
                p.float-value { "$1.84" }
                p.float-note { "instead of $4.00" }
            }
            div.float-card.float-hold {
                p.float-label { "Waiting for you" }
                p.float-value-small { "Merge pull request" }
                p.float-keys { kbd { "Y" } " allow " kbd { "N" } " deny" }
            }
        }
    }
}

fn problem_section() -> Markup {
    html! {
        section.problem-section data-slot="text-section" {
            p {
                "On its own, a coding agent sends most steps to one big model: finding a file, reading it, running a command, writing the fix. You pay big-model prices, and wait big-model times, for work a small model can do."
            }
            p.muted {
                "Coder sits in front of the agent you already use and decides, step by step, what each one needs. The agent stays the same; the bill and the wait get smaller."
            }
        }
    }
}

/// A feature: text on one side, our picture on the other; with `wide`,
/// the text over the full width and the pictures below it, for a section
/// with more than one picture.
#[allow(clippy::too_many_arguments)]
fn feature_section(
    id: &str,
    label: &str,
    title: &str,
    body: &[&str],
    action: Option<(&str, &str)>,
    picture: Markup,
    flipped: bool,
    wide: bool,
) -> Markup {
    let class = match (wide, flipped) {
        (true, _) => "feature-section feature-wide",
        (false, true) => "feature-section feature-flipped",
        (false, false) => "feature-section",
    };
    html! {
        section class=(class) id=(id) data-slot="feature-section" aria-labelledby=(format!("{id}-title")) {
            div.feature-copy {
                (eyebrow(label))
                h2 id=(format!("{id}-title")) { (title) }
                @for line in body { p { (line) } }
                @if let Some((label, href)) = action {
                    (button(label, href, "outline"))
                }
            }
            div.feature-picture { (picture) }
        }
    }
}

fn diagram(path: &str, alt: &str) -> Markup {
    html! { img.diagram src=(path) alt=(alt) loading="lazy" width="640" height="420"; }
}

fn routing_pictures() -> Markup {
    html! {
        div.compare {
            figure {
                (diagram("/static/docs/coder-harness.svg", "Another harness: every step goes through the big model."))
                figcaption { "Another harness" }
            }
            figure {
                (diagram("/static/docs/coder-system-1.svg", "Coder: quick decisions in front, only the fix goes to the big model."))
                figcaption { "Coder" }
            }
        }
    }
}

/// A typical Coder turn, step by step, as the transcript shows it: what it
/// read, what it ran, what it changed, and the check that passed.
fn transcript_demo() -> Markup {
    html! {
        div.transcript data-slot="transcript" aria-label="A Coder turn, step by step" {
            p.transcript-ask { "The total in the cart is off by one. Fix it." }
            ol.transcript-steps {
                li.transcript-step {
                    span.step-mark.step-ok {} span.step-tool { "Read" } code { "src/cart.rs" }
                    em { "small model" }
                }
                li.transcript-step {
                    span.step-mark.step-fail {} span.step-tool { "Bash" } code { "cargo test cart" }
                    em { "1 failed" }
                }
                li.transcript-step.transcript-open {
                    span.step-mark.step-ok {} span.step-tool { "Edit" } code { "src/cart.rs" }
                    em { "big model" }
                    pre.transcript-diff {
                        span.diff-del { "-    (0..=items.len()).map(|i| items[i].price).sum()" } "\n"
                        span.diff-add { "+    items.iter().map(|item| item.price).sum()" }
                    }
                }
                li.transcript-step {
                    span.step-mark.step-ok {} span.step-tool { "Bash" } code { "cargo test cart" }
                    em { "12 passed" }
                }
            }
            p.transcript-answer { "Fixed: the sum read one item past the end. The cart tests pass." }
        }
    }
}

/// The example card: one thing people make with OpenAgents.
fn cad_example() -> Markup {
    html! {
        figure.example-card data-slot="card" {
            img src=(CAD_SHOT.0) alt="A fan mounting bracket designed in a chat on OpenAgents" loading="lazy";
            figcaption {
                span.example-tag { "Example" }
                strong { "Design a part by talking" }
                span { "Ask for a 40 mm fan bracket, then pick a face and change it. Every version is kept to compare. CAD Lab is part of Pro." }
            }
        }
    }
}

fn everywhere_pictures() -> Markup {
    html! {
        div.everywhere { (transcript_demo()) (cad_example()) }
    }
}

fn stats_section() -> Markup {
    html! {
        section.stats-section data-slot="stats-section" aria-label="What the study measured" {
            div.stat { p.stat-value { "$1.84" } p.stat-label { "per accepted fix, instead of $4.00" } }
            div.stat { p.stat-value { "121 s" } p.stat-label { "median time, instead of 284 s" } }
            div.stat { p.stat-value { "21" } p.stat-label { "real issues, same model both ways" } }
            p.stats-source { a href=(RESEARCH) { "Read the study" } }
        }
    }
}

fn steps_section() -> Markup {
    html! {
        section.steps-section #how data-slot="steps-section" aria-labelledby="how-title" {
            (eyebrow("How it works"))
            h2 #how-title { "Three steps" }
            ol.steps {
                li.step {
                    span.step-number { "01" }
                    h3 { "Install" }
                    p { "One line in your terminal:" }
                    (CodeBlock::new(CODER_SH).language("bash").title("macOS and Linux"))
                    (CodeBlock::new(CODER_PS1).language("powershell").title("Windows, in PowerShell"))
                }
                li.step {
                    span.step-number { "02" }
                    h3 { "Pick your agents" }
                    p { "Coder finds the agents you have installed. Shift+Tab steps between them, or leave it on Auto." }
                }
                li.step {
                    span.step-number { "03" }
                    h3 { "Work" }
                    p { "Ask for what you want. Coder sends each step where it belongs and holds the risky ones for you." }
                }
            }
        }
    }
}

fn pricing_section() -> Markup {
    let plan = retail_cloud::environment::plan();
    let price = retail_cloud::environment::usd(plan.price_usd_micros);
    html! {
        section.pricing-section #pricing data-slot="pricing-section" aria-labelledby="pricing-title" {
            (eyebrow("Pricing"))
            h2 #pricing-title { "Free to start" }
            div.pricing-cards {
                div.price-card data-slot="card" {
                    h3 { "Free" }
                    p.price { "$0" }
                    ul {
                        li { "Coder in front of the agents you use" }
                        li { "A free daily model allowance" }
                        li { "OpenAgents on the web and your phone" }
                    }
                    (button("Download Coder", "/download", "outline"))
                }
                div.price-card.price-card-featured data-slot="card" {
                    h3 { "Pro" }
                    p.price { (price) span { "/mo." } }
                    ul {
                        li { "Everything in Free" }
                        li { "Cloud agents that keep working with your laptop closed" }
                        li { "Cloud time, uploads, and advanced projects" }
                        li { "Model usage included each month" }
                        li { "CAD Lab: design parts by talking" }
                    }
                    (button("See plans", "/pricing", "3d"))
                }
                div.price-card data-slot="card" {
                    h3 { "Enterprise" }
                    p.price { "Custom" }
                    ul {
                        li { "Everything in Pro" }
                        li { "Deploy on our cloud, your cloud, or your own machines" }
                        li { "SAML single sign-on and SCIM" }
                        li { "Uptime SLA and priority support" }
                        li { "Audit logs, invoicing, and a security review" }
                    }
                    (button("Contact sales", "/contact?topic=sales", "outline"))
                }
            }
        }
    }
}

fn faq_section() -> Markup {
    let questions: [(&str, Markup); 4] = [
        (
            "Which agents does Coder work with?",
            html! { (AGENTS.join(", ")) ", and Auto, which picks for you." },
        ),
        (
            "Do I have to change how I work?",
            html! { "No. You keep the agent you use; Coder sits in front of it." },
        ),
        (
            "Is it open source?",
            html! { "Yes. " a href=(GITHUB) { "The code is on GitHub" } "." },
        ),
        (
            "What waits for me?",
            html! { "Steps that are hard to undo: a deploy, a merge, a secret, a payment, a DNS change. Press Y to allow or N to deny." },
        ),
    ];
    html! {
        section.faq-section #faq data-slot="faq-section" aria-labelledby="faq-title" {
            (eyebrow("FAQ"))
            h2 #faq-title { "Questions" }
            div.faq {
                @for (question, answer) in questions {
                    details.faq-item {
                        summary { (question) }
                        p { (answer) }
                    }
                }
            }
        }
    }
}

fn cta_section() -> Markup {
    html! {
        section.cta-section data-slot="cta-section" aria-labelledby="cta-title" {
            h2 #cta-title { "Pay for the hard parts only" }
            p { "Install Coder and keep the agent you already use." }
            div.hero-actions { (button("Download Coder", "/download", "3d")) }
        }
    }
}

fn footer() -> Markup {
    html! {
        footer.footer data-slot="footer" {
            div.footer-brand {
                a.navbar-brand href="/" {
                    img src="/favicon.svg" alt="" width="24" height="24";
                    span { "OpenAgents" }
                }
                p.muted { "Open source. Plugin authors paid in Bitcoin." }
            }
            nav.footer-columns aria-label="Footer" {
                div { h3 { "Product" } a href="/download" { "Download" } a href="/pricing" { "Pricing" } a href="/efficiency" { "Efficiency" } }
                div { h3 { "Resources" } a href="/docs" { "Docs" } a href="/blog" { "Blog" } a href=(RESEARCH) { "Research" } a href=(GITHUB) { "GitHub" } a href=(X) { "X (Twitter)" } }
                div { h3 { "Legal" } a href="/privacy" { "Privacy" } a href="/terms" { "Terms" } }
                div { h3 { "Contact" } a href="/contact" { "Contact us" } a href=(format!("mailto:{}", crate::agent_ready::CONTACT)) { "Email" } }
            }
        }
    }
}

/// The page, in the visitor's chosen theme (Coder Light or Coder Noir), or
/// following their system without a choice, as every page on the site.
pub(crate) fn page(theme: Option<openagents_ui::shell::Theme>) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" data-theme=[theme.map(openagents_ui::shell::Theme::as_str)] {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { "OpenAgents" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                // The metadata every page carries for people and agents:
                // previews, llms.txt, the Markdown twin, the catalog, and
                // our contact (`crate::agent_ready::head`).
                (maud::PreEscaped(crate::agent_ready::head(
                    "OpenAgents",
                    Some(DESCRIPTION),
                    Some("/"),
                )))
                (maud::PreEscaped(crate::theme::style_tag()))
                // The design kit's script, for the install lines' Copy buttons.
                (maud::PreEscaped(crate::theme::script_tags()))
                (maud::PreEscaped(crate::agent_ready::webmcp_tag()))
                link rel="stylesheet" href=(CSS_PATH);
            }
            body.landing {
                (navbar())
                main {
                    (hero_section())
                    (problem_section())
                    (feature_section(
                        "routing", "Routing", "Only the hard parts reach the big model",
                        &["Every message stops first at a ring of quick decisions: route, brief, trim, gate, check. Finding and reading go to small, fast models; only the fix goes on to the big one."],
                        Some(("How Coder works", HOW)), routing_pictures(), false, true,
                    ))
                    (stats_section())
                    (feature_section(
                        "agents", "Agents", "Keep the agent you use",
                        &["Claude Code, Codex, OpenCode, Grok Build, Devin CLI and Cursor all run behind the same Coder. Shift+Tab steps from one to the next; Auto picks for you."],
                        Some(("Download Coder", "/download")),
                        diagram("/static/docs/coder-agents.svg", "One Coder in front of every agent."), true, false,
                    ))
                    (feature_section(
                        "holds", "Safety", "Hard-to-undo steps wait for you",
                        &["Reading, editing, running tests and pushing go straight through. A deploy, a merge, a secret, a payment or a DNS change stops and waits until you press Y or N."],
                        None,
                        diagram("/static/docs/coder-holds.svg", "Hard-to-undo steps wait for you."), false, false,
                    ))
                    (feature_section(
                        "everywhere", "Every step in view", "See what it read, ran, and changed",
                        &["Each step shows as it happens, with the model that did it. Follow along in the terminal, on the web, or on your phone, and build more than code."],
                        None, everywhere_pictures(), true, true,
                    ))
                    (steps_section())
                    (pricing_section())
                    (faq_section())
                    (cta_section())
                }
                (footer())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_landing_page_has_every_section_and_real_links() {
        let page = page(None).into_string();
        for slot in [
            "navbar",
            "hero-section",
            "feature-section",
            "stats-section",
            "steps-section",
            "pricing-section",
            "faq-section",
            "cta-section",
            "footer",
        ] {
            assert!(page.contains(&format!("data-slot=\"{slot}\"")), "{slot}");
        }
        assert!(page.contains("href=\"/download\""));
        assert!(page.contains(RESEARCH));
        // The announcement pill, and Blog in the header, the menu, and the
        // footer.
        assert!(page.contains(&format!("class=\"hero-pill\" href=\"{ANNOUNCEMENT}\"")));
        assert!(crate::pages::blog::post("coder-1-0").is_some());
        assert_eq!(page.matches("href=\"/blog\"").count(), 3);
        // The menu for narrow screens works without a script.
        assert!(page.contains("<details class=\"navbar-menu\">"));
        assert!(page.contains("<summary class=\"navbar-menu-button\" aria-label=\"Menu\">"));
        assert!(page.contains(CODER_SH) && page.contains(CODER_PS1));
        for svg in [
            "coder-harness",
            "coder-system-1",
            "coder-agents",
            "coder-holds",
        ] {
            assert!(page.contains(&format!("/static/docs/{svg}.svg")), "{svg}");
        }
        assert!(page.contains(CAD_SHOT.0) && CAD_SHOT.1.starts_with(&[0xFF, 0xD8]));
        assert!(page.contains("data-slot=\"transcript\""));
        // Pro's price is the one /pricing sells.
        let plan = retail_cloud::environment::plan();
        assert!(page.contains(&retail_cloud::environment::usd(plan.price_usd_micros)));
        assert!(page.contains("Enterprise") && page.contains("href=\"/contact?topic=sales\""));
        // Every link goes somewhere: in-page ones to a section that exists.
        for anchor in ["how", "agents", "pricing", "faq"] {
            assert!(page.contains(&format!("href=\"#{anchor}\"")));
            assert!(page.contains(&format!("id=\"{anchor}\"")), "#{anchor}");
        }
        assert!(!page.contains("href=\"#\""));
        // No inline styles or scripts: the site's policy allows neither.
        // Scripts load from the site's own files only (the Copy buttons).
        assert!(!page.contains("style="));
        assert!(!page.contains("<script>"));
        // Scripts load from the site's own files; the one inline block is
        // the structured data every page carries, which runs nothing.
        assert_eq!(
            page.matches("<script").count(),
            page.matches("<script src=\"/static/").count()
                + page
                    .matches("<script type=\"application/ld+json\">")
                    .count()
        );
        assert!(page.contains("data-oa-copy") || page.contains("Copy"));
        crate::copy_guard::assert_plain("/landing", &page);
        // The site's Coder Light / Coder Noir stylesheet, and the chosen
        // theme pinned like every other page.
        assert!(page.contains(crate::theme::STYLESHEET_PATH));
        assert!(!page.contains("data-theme"));
        let light = super::page(Some(openagents_ui::shell::Theme::Light)).into_string();
        assert!(light.contains("data-theme=\"light\""));
    }
}
