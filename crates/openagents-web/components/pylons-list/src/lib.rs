//! `/pylons`' list of online pylons as a handler component. The site reads
//! the live field's snapshot for it (`data.read("pylons-snapshot")`) and
//! puts the returned HTML in the page; `pages::pylons_field::list_html` is
//! the native copy it falls back to.

wit_bindgen::generate!({ world: "site-route", path: "../../../livehost/wit" });

use exports::openagents::livehost::handler::{Guest, Request, Response};
use openagents::livehost::data;
use pylons_view::Snapshot;

struct List;

impl Guest for List {
    fn handle(_request: Request) -> Response {
        let snapshot: Snapshot = data::read("pylons-snapshot")
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Response {
            status: 200,
            content_type: "text/html; charset=utf-8".into(),
            body: list_html(&snapshot),
        }
    }
}

fn list_html(snapshot: &Snapshot) -> String {
    let mut html = String::new();
    let count = snapshot.pylons.len() + snapshot.more;
    let heading = match count {
        0 if !snapshot.synced => "Looking for pylons".to_owned(),
        0 => "No pylons online".to_owned(),
        1 => "1 pylon online".to_owned(),
        n => format!("{n} pylons online"),
    };
    html.push_str(&format!(
        "<h2 id=\"pylons-count\">{heading}</h2><ul id=\"pylons-list\">"
    ));
    for pylon in &snapshot.pylons {
        html.push_str(&format!(
            "<li data-id=\"{}\"><span class=\"n\">{}</span> <span class=\"d\">{}</span></li>",
            escape(&pylon.id),
            escape(&pylon.name),
            escape(&pylon.summary()),
        ));
    }
    html.push_str("</ul>");
    if snapshot.more > 0 {
        html.push_str(&format!("<p>And {} more.</p>", snapshot.more));
    }
    html
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

export!(List);
