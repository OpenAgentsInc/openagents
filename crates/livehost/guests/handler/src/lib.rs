//! The demo route handler, a WebAssembly component built for wasm32-wasip2.
//! It renders `/hello` from the request's query string.

wit_bindgen::generate!({ world: "route", path: "../../wit" });

use exports::openagents::livehost::handler::{Guest, Request, Response};

/// `livehost bench` rewrites this line to make a handler change.
const STAMP: &str = "handler-0";

struct Handler;

impl Guest for Handler {
    fn handle(request: Request) -> Response {
        let name = request
            .query
            .iter()
            .find(|(key, _)| key == "name")
            .map(|(_, value)| value.as_str())
            .unwrap_or("there");
        let style = request
            .assets
            .iter()
            .find(|(path, _)| path == "static/style.css")
            .map(|(_, url)| url.as_str())
            .unwrap_or("");
        let body = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"livehost-manifest\" content=\"{manifest}\">\
             <title>Hello</title>\
             <link rel=\"stylesheet\" href=\"{style}\" data-asset=\"static/style.css\"></head>\
             <body><main><h1>Hello, {name}</h1>\
             <p id=\"stamp\">Rendered by a handler component ({STAMP}) for {method} {path}.</p>\
             </main></body></html>\n",
            manifest = request.manifest,
            name = escape(name),
            method = request.method,
            path = escape(&request.path),
        );
        Response {
            status: 200,
            content_type: "text/html; charset=utf-8".into(),
            body,
        }
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

export!(Handler);
