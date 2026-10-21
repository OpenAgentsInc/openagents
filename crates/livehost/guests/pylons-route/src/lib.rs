//! `/pylons`' list of online pylons as a handler component. The rendering
//! is `openagents-web`'s `pages::pylons_field::list_html` over the shared
//! `pylons-view` types. The host has no interface yet for the live field,
//! so the snapshot comes from the `snapshot` query parameter as JSON, and
//! an empty one otherwise.

wit_bindgen::generate!({ world: "route", path: "../../wit" });

use exports::openagents::livehost::handler::{Guest, Request, Response};
use pylons_view::Snapshot;

struct Route;

impl Guest for Route {
    fn handle(request: Request) -> Response {
        let snapshot: Snapshot = request
            .query
            .iter()
            .find(|(key, _)| key == "snapshot")
            .and_then(|(_, json)| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let style = request
            .assets
            .iter()
            .find(|(path, _)| path == "static/style.css")
            .map(|(_, url)| url.as_str())
            .unwrap_or("");
        let body = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"livehost-manifest\" content=\"{manifest}\">\
             <title>Pylons</title>\
             <link rel=\"stylesheet\" href=\"{style}\" data-asset=\"static/style.css\"></head>\
             <body><main><h1>Pylons</h1>\
             <section id=\"pylons-panel\" aria-labelledby=\"pylons-count\">{list}</section>\
             </main></body></html>\n",
            manifest = request.manifest,
            list = list_html(&snapshot),
        );
        Response {
            status: 200,
            content_type: "text/html; charset=utf-8".into(),
            body,
        }
    }
}

/// `openagents-web`'s `list_html`, unchanged.
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

export!(Route);
