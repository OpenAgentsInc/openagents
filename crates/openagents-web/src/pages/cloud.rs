//! Public Cloud information, with prices from the checked retail contract.
//! Reading terms creates no offer, reservation, enrollment, or execution.

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::actions::{Button, ButtonLink};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use route_contract::price_book::{Charge, ModelPayer, Placement, PriceBook};
use serde::Deserialize;

use crate::App;
use crate::layout::{GITHUB, escape};
use crate::ui_page::{UiPage, action_link};

const PRICE_BOOK: &str = include_str!("../../../route-contract/fixtures/price-book-v1.json");
const COMPUTER: &str = "retail-boat-large-v1";
const TASK: &str = "retail-repo-change-v1";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/cloud", get(cloud))
}

#[derive(Deserialize)]
struct Published {
    book: PriceBook,
}

fn published_book() -> Result<PriceBook, ()> {
    let published: Published = serde_json::from_str(PRICE_BOOK).map_err(|_| ())?;
    published.book.check().map_err(|_| ())?;
    Ok(published.book)
}

/// This computes the published maximum without creating a customer offer.
fn prices(book: &PriceBook) -> Result<String, ()> {
    book.check().map_err(|_| ())?;
    let class = book
        .classes
        .iter()
        .find(|class| class.computer == COMPUTER && class.task == TASK)
        .ok_or(())?;
    let quote = book
        .quote(
            &Placement::Retail {
                computer: COMPUTER.into(),
                task: TASK.into(),
            },
            class.max_seconds,
            None,
        )
        .map_err(|_| ())?
        .ok_or(())?;
    quote.check(book).map_err(|_| ())?;
    let compute = quote
        .lines
        .iter()
        .find(|line| line.resource == Charge::Compute)
        .ok_or(())?;
    let ModelPayer::CallerKey { provider } = &class.model;
    Ok(format!(
        "<p class=\"oa-page-eyebrow\">Published terms · activation pending</p>\
<p>Price book <code>{version}</code>. One credit is exactly {credit_sats} sat.</p>\
<table><caption>Retail v1 prices</caption><thead><tr><th scope=\"col\">Resource</th>\
<th scope=\"col\">Price</th><th scope=\"col\">Payer</th></tr></thead><tbody>\
<tr><th scope=\"row\">Compute</th><td>{rate} millisatoshis per metered second</td><td>Your purchased balance</td></tr>\
<tr><th scope=\"row\">Coordination</th><td>{coordination} sats per started task</td><td>Your purchased balance</td></tr>\
<tr><th scope=\"row\">Model</th><td>Billed directly by <code>{provider}</code></td><td>Your own provider key</td></tr>\
</tbody></table>\
<p>For at most {seconds} seconds, the published maximum is <strong>{maximum} sats / {credits} credits</strong>: \
{compute} sats for compute and {coordination} sats for coordination. Your provider's model bill is separate.</p>\
<p class=\"oa-page-meta\">Book digest: <code>{digest}</code>. These published terms are not a purchase offer.</p>\
<p>Confirming an admitted offer reserves its maximum before provisioning. A stop request does not stop the meter; \
unknown usage stays held until reconciliation. Unused reserved funds are released separately from settled charges. \
Purchased retail balance has no Lightning redemption in v1.</p>",
        version = escape(&book.version),
        credit_sats = book.credit.sats,
        rate = class.compute_msats_per_second,
        coordination = class.coordination_sats,
        provider = escape(provider),
        seconds = quote.max_seconds,
        maximum = quote.max_sats,
        credits = quote.max_credits,
        compute = compute.max_sats,
        digest = escape(quote.book.as_str()),
    ))
}

fn execution_choices() -> &'static str {
    "<section aria-labelledby=\"execution-title\"><h2 id=\"execution-title\">Choose where work runs</h2>\
<ul class=\"oa-item-list\">\
<li data-availability=\"unavailable\"><p class=\"oa-item-title\">Your computer · Unavailable in this browser</p>\
<p>Use the <a href=\"/download\">OpenAgents apps</a> with your own model login or key. Local work needs no compute purchase. \
Browser enrollment and task supervision need a current host grant.</p></li>\
<li data-availability=\"unavailable\"><p class=\"oa-item-title\">Another enrolled computer · Unavailable in this browser</p>\
<p>Work needs an explicitly enrolled host, an admitted workspace, current rights, and separate disclosure approval. \
Account sign-in alone enrolls no computer.</p></li>\
<li data-availability=\"unavailable\"><p class=\"oa-item-title\">Operator Boat · Unavailable in this browser</p>\
<p>Authorized operators can select an integrated agent or headless Coder through the native tools. \
The browser has no operator credential or cloud-job connection.</p></li>\
<li data-availability=\"unavailable\"><p class=\"oa-item-title\">Operator GCE · Unavailable in this browser</p>\
<p>Headless Coder uses an explicitly granted pool and its actual capacity. Integrated Boat agents are not a GCE option. \
An operator pool is separate from retail compute.</p></li>\
<li data-availability=\"proposed\"><p class=\"oa-item-title\">Retail Cloud v1 · Proposed</p>\
<p>The browser purchase lane is disabled until native delegation, funded checks, and commercial activation qualify. \
The supported contract is one Boat repository task with its own frozen offer.</p></li></ul></section>"
}

async fn cloud(State(app): State<App>, headers: HeaderMap) -> Response {
    let pricing = published_book()
        .and_then(|book| prices(&book))
        .unwrap_or_else(|()| {
            "<p>Unavailable: this server cannot verify the published retail price book.</p>".into()
        });
    let ready = crate::cloud::ready(&app)
        && app
            .config
            .cloud
            .as_ref()
            .is_some_and(|service| service.health().is_ok());
    let workspace = if ready {
        html! {
            section.oa-card aria-labelledby="workspace-title" data-availability="available" {
                (MarkdownRoot::new(html! {
                    h2 id="workspace-title" { "Workspace" }
                    p {
                        "Sign in through the configured native account service and choose your \
        workspace. Computer, private work, and spending connections require their own grants."
                    }
                }))
                div.oa-page-actions { (ButtonLink::new("Open workspace", "/cloud/app")) }
            }
        }
    } else {
        html! {
            section.oa-card aria-labelledby="workspace-title" data-availability="unavailable" {
                (MarkdownRoot::new(html! {
                    h2 id="workspace-title" { "Workspace \u{b7} Unavailable" }
                    p {
                        "This server has no configured native account connection. Private tasks, \
        files, controls, and balances are unavailable."
                    }
                }))
                div.oa-page-actions {
                    (Button::new("Open workspace")
                        .disabled(true)
                        .attr("aria-describedby", "workspace-reason"))
                }
                p.oa-page-meta id="workspace-reason" {
                    "The operator must configure the native account service and browser privacy \
        runtime."
                }
            }
        }
    };
    let terms = format!(
        "{choices}\
<section aria-labelledby=\"retail-title\"><h2 id=\"retail-title\">Retail v1 terms</h2>\
<p><code>{COMPUTER}</code> runs one <code>{TASK}</code> task from a public HTTPS GitHub repository at an exact commit, \
with 1–8 frozen checks and your own OpenAI key. The service permits at most four concurrent retail sandboxes across customers.</p>\
<p>The sandbox is deleted after the task. The patch, check outputs, summary, and scrubbed trace remain available for 30 days. \
Private repositories, publication, a customer terminal, fan-out, hosted inference, and continuation are outside this contract.</p>\
{pricing}\
<p><a href=\"{GITHUB}/blob/main/docs/cloud/retail-contract.md\">Read the retail contract</a> · \
<a href=\"{GITHUB}/blob/main/docs/cloud/retail-prices.md\">Read the charge and refund terms</a></p></section>\
<section aria-labelledby=\"verse-title\"><h2 id=\"verse-title\">Verse and your agents</h2>\
<p>Explore <a href=\"/grid\">the Grid</a> or <a href=\"/everglade\">Everglade</a> when this server has its world build. \
World, computer, and private work connections have separate identities and rights. Joining a world starts no task \
and grants no access to private work. Connected Alice and Studio supervision are proposed for this workspace.</p>\
<p><a href=\"/docs/verse\">Read the Verse guide</a></p></section>",
        choices = execution_choices(),
    );
    let content = PageColumn::new(html! {
        section {
            (MarkdownRoot::new(html! {
                p.oa-page-eyebrow { "Coder Cloud" }
                h1 { "Your work, in the browser" }
                p.oa-page-lead {
                    "Direct Coder, follow your agents, and review their results across your \
    computers and qualified cloud capacity."
                }
                p {
                    "The shared web components are available now. Private work and paid browser \
    execution are proposed."
                }
            }))
            div.oa-page-actions {
                (ButtonLink::new("Explore components", "/components"))
                (action_link("Download OpenAgents", "/download"))
            }
        }
        (workspace)
        (MarkdownRoot::new(PreEscaped(terms)))
    });
    // The page offers nothing to submit: no script, and no form at all, so
    // not even the theme toggle's no-script fallback.
    UiPage::new("Coder Cloud")
        .path("/cloud")
        .scriptless()
        .without_toggle()
        .content(content)
        .respond(&headers)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_prices_follow_the_checked_book_and_refuse_invalid_units() {
        let mut book = published_book().unwrap();
        let html = prices(&book).unwrap();
        assert!(html.contains(&book.version));
        assert!(html.contains(book.digest().as_str()));
        assert!(html.contains("244 sats / 244 credits"));
        book.classes[0].compute_msats_per_second = 50;
        let revised = prices(&book).unwrap();
        assert!(revised.contains("280 sats / 280 credits"));
        assert!(revised.contains(book.digest().as_str()));
        book.credit.sats = 2;
        assert!(prices(&book).is_err());
    }

    #[test]
    fn price_book_text_is_escaped() {
        let mut book = published_book().unwrap();
        book.version.push_str("<script>");
        let html = prices(&book).unwrap();
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
