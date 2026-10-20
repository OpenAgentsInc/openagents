//! `/lander4`: the landing page in one response.
//!
//! A black page, the Grid's white floor, "OpenAgents", and a Download
//! button, sent as one HTML document with everything inlined: the styles,
//! the painter (`crates/lander-gl`, a raw WebGL2 wasm module, built by
//! `scripts/build-lander-gl.sh` into `static/lander4/`), and its glue. The
//! module draws the first frame synchronously while the page's one script
//! runs, so the first frame needs one round trip and no font, image, or
//! second file.
//!
//! The words are real HTML: an `<h1>` and an `<a>` to `/download`, the
//! same address the site's landing pages and header link. The painter
//! reads their boxes and paints over them (their text turns transparent),
//! so pointer, keyboard, focus, readers and search all use the real link.
//! Without WebGL2 or scripts, the page's CSS draws the floor and the
//! button instead.
//!
//! The document is built and compressed with brotli once per process; a
//! browser that accepts `br` gets the compressed bytes.

use std::io::Write;
use std::sync::LazyLock;

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::App;

/// The painter and its wasm-bindgen glue, checked in by
/// `scripts/build-lander-gl.sh`.
const WASM: &[u8] = include_bytes!("../../static/lander4/lander_gl_bg.wasm");
const GLUE: &str = include_str!("../../static/lander4/lander_gl.js");

/// Where the button goes: the site's download page, as on `/` and the
/// landing pages.
pub(crate) const DOWNLOAD: &str = "/download";

/// The page's styles. The painter reads the title's box height as its type
/// size (so the line height is 1) and draws the label at 17px; keep those
/// in step with `crates/lander-gl/src/lib.rs`. Without the painter, the
/// floor is a CSS grid tilted away from the viewer.
const STYLE: &str = "html,body{margin:0;height:100%;overflow:hidden;background:#000;color:#fff}\
body{font:600 17px/1 ui-sans-serif,-apple-system,system-ui,\"Segoe UI\",sans-serif;\
-webkit-font-smoothing:antialiased}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
main{position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;\
justify-content:center;gap:clamp(24px,4vh,40px);padding:0 16px}\
h1{margin:0;font-size:clamp(48px,13vw,128px);font-weight:600;line-height:1;\
letter-spacing:-.01em;white-space:nowrap}\
a{display:flex;align-items:center;justify-content:center;box-sizing:border-box;width:168px;\
height:48px;border:1px solid rgba(255,255,255,.26);border-radius:10px;background:#111;\
color:#fff;font-size:17px;text-decoration:none;box-shadow:0 6px 28px rgba(0,0,0,.7)}\
a:hover{background:#242424;border-color:rgba(255,255,255,.5)}\
a:focus-visible{outline:2px solid #fff;outline-offset:3px}\
.gl h1,.gl a{color:transparent;background:none;border-color:transparent;box-shadow:none}\
body::before{content:\"\";position:fixed;left:-50%;right:-50%;top:52%;height:60%;\
background:linear-gradient(90deg,rgba(255,255,255,.45) 1px,transparent 1px) 50% 0/48px 48px,\
linear-gradient(rgba(255,255,255,.45) 1px,transparent 1px) 0 0/48px 48px;\
transform:perspective(240px) rotateX(62deg);transform-origin:50% 0;\
-webkit-mask:linear-gradient(transparent,#000 70%);mask:linear-gradient(transparent,#000 70%)}\
.gl body::before{content:none}";

/// The site's mark, white on black, inline so the tab needs no request.
const ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E\
%3Crect width='64' height='64' fill='%23000'/%3E%3Cpath d='M22 18a18 18 0 1 0 20 0' fill='none' \
stroke='%23fff' stroke-width='5' stroke-linecap='round'/%3E%3Cpath d='M32 10v22' stroke='%23fff' \
stroke-width='5' stroke-linecap='round'/%3E%3C/svg%3E";

/// The module's bytes as a JavaScript string literal, one character a
/// byte. Inside HTML, a string of Latin-1 characters stays byte-aligned
/// under brotli and compresses smaller than base64 of the same bytes. The
/// escapes keep it a valid string in a classic script: no quote,
/// backslash, line break, NUL (the HTML parser would replace it), or `<`
/// (which could close the script).
fn latin1_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3 / 2 + 2);
    out.push('"');
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            0 if bytes.get(index + 1).is_some_and(u8::is_ascii_digit) => out.push_str("\\x00"),
            0 => out.push_str("\\0"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'<' => out.push_str("\\x3c"),
            _ => out.push(char::from(byte)),
        }
    }
    out.push('"');
    out
}

/// The page's one script: the glue, then the bytes, compiled and started
/// synchronously so the first frame is drawn before the first paint.
fn script() -> String {
    format!(
        "{GLUE}\ntry{{const s={};const b=new Uint8Array(s.length);\
for(let i=0;i<s.length;i++)b[i]=s.charCodeAt(i);wasm_bindgen.initSync({{module:b}})}}catch(e){{}}",
        latin1_literal(WASM)
    )
}

fn hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!(
        "'sha256-{}'",
        base64::engine::general_purpose::STANDARD.encode(digest)
    )
}

/// The page's policy: its one inline script and stylesheet by hash, wasm
/// compilation, and the inline icon. Nothing else loads.
fn policy(script: &str) -> String {
    format!(
        "default-src 'none'; script-src {} 'wasm-unsafe-eval'; style-src {}; img-src data:; \
base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        hash(script),
        hash(STYLE)
    )
}

fn document(script: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>OpenAgents</title>\
<meta name=\"description\" content=\"Download the OpenAgents desktop app.\">\
<link rel=\"icon\" href=\"{ICON}\"><style>{STYLE}</style></head><body>\
<canvas id=\"grid\" aria-hidden=\"true\"></canvas><main>\
<h1 id=\"title\">OpenAgents</h1><a id=\"download\" href=\"{DOWNLOAD}\">Download</a></main>\
<script>{script}</script></body></html>"
    )
}

pub(crate) struct Lander {
    pub(crate) html: String,
    pub(crate) brotli: Vec<u8>,
    pub(crate) policy: HeaderValue,
}

pub(crate) static PAGE: LazyLock<Lander> = LazyLock::new(|| {
    let script = script();
    let html = document(&script);
    let mut brotli = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
    brotli
        .write_all(html.as_bytes())
        .expect("compressing in memory");
    let brotli = brotli.into_inner();
    Lander {
        policy: HeaderValue::from_str(&policy(&script)).expect("an ASCII policy"),
        html,
        brotli,
    }
});

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/lander4", get(page))
}

async fn page(request: HeaderMap) -> Response {
    let page = &*PAGE;
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        ),
        (header::CONTENT_SECURITY_POLICY, page.policy.clone()),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=300"),
        ),
        (header::VARY, HeaderValue::from_static("accept-encoding")),
    ];
    if super::everglade::accepts(&request, "br") {
        let mut response = (headers, page.brotli.clone()).into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
        response
    } else {
        (headers, page.html.clone()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    /// The literal decodes, as the page's script does, to the module's
    /// exact bytes.
    #[test]
    fn the_inline_module_decodes_to_its_bytes() {
        let literal = latin1_literal(WASM);
        let inner = &literal[1..literal.len() - 1];
        let mut bytes = Vec::new();
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                bytes.push(u8::try_from(u32::from(c)).unwrap());
                continue;
            }
            match chars.next().unwrap() {
                '0' => bytes.push(0),
                'n' => bytes.push(b'\n'),
                'r' => bytes.push(b'\r'),
                'x' => {
                    let hex: String = [chars.next().unwrap(), chars.next().unwrap()]
                        .iter()
                        .collect();
                    bytes.push(u8::from_str_radix(&hex, 16).unwrap());
                }
                other => bytes.push(other as u8),
            }
        }
        assert_eq!(bytes, WASM);
        assert!(!inner.contains(['<', '\n', '\r', '\0']));
        assert!(WASM.starts_with(b"\0asm"));
    }

    /// The compressed page is the page.
    #[test]
    fn the_brotli_copy_is_the_page() {
        let mut out = String::new();
        brotli::Decompressor::new(PAGE.brotli.as_slice(), 4096)
            .read_to_string(&mut out)
            .unwrap();
        assert_eq!(out, PAGE.html);
    }

    /// One request: nothing in the page loads anything else.
    #[test]
    fn the_page_loads_nothing_else() {
        let html = &PAGE.html;
        for needle in [
            "src=",
            "<link rel=\"stylesheet\"",
            "url(",
            "fetch(\"",
            "@import",
        ] {
            let outside_glue = html.replace(GLUE, "");
            assert!(!outside_glue.contains(needle), "{needle}");
        }
        assert_eq!(html.matches("<script").count(), 1);
    }
}
