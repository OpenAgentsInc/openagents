//! `/lander5`: a landing page over the Grid at twilight, a full-screen
//! procedural shader built from Shader Skills (gokayfem/shader-skills, MIT;
//! `crates/lander5-web`).
//!
//! The page is one response: the title and the Download link as HTML over
//! a canvas, the two fragment shaders as inline data blocks, and the
//! WebGL2 runner's wasm-bindgen glue and module (base64) in one inline
//! module script. The runner's build (`scripts/build-lander5-web.sh`) is
//! read from the directory the server was started with (`--lander5 DIR`,
//! default `/srv/lander5`):
//!
//! ```text
//! DIR/lander5_web.js          wasm-bindgen's `--target web` glue
//! DIR/lander5_web_bg.wasm     the module
//! ```
//!
//! The document is assembled once per build and kept with its brotli and
//! gzip copies. Without the build the page is the same title and link over
//! a still gradient and runs no script, which is also what a browser
//! without WebGL2 or JavaScript shows. The policy admits exactly the
//! inline style and script by their SHA-256.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::App;
use crate::pages::everglade::{accepts, accepts_gzip};

/// The page.
pub(crate) const LANDER5_PATH: &str = "/lander5";

/// The build's files, as `scripts/build-lander5-web.sh` names them.
pub(crate) const LANDER5_GLUE: &str = "lander5_web.js";
pub(crate) const LANDER5_WASM: &str = "lander5_web_bg.wasm";

/// Where the Download button goes: the same page the home page's Download
/// buttons open.
const DOWNLOAD: &str = "/download";
const TITLE: &str = "OpenAgents";
const DESCRIPTION: &str = "Download OpenAgents.";

/// The page's look. The body's gradient stands in for the scene until the
/// first frame, and stays when there is no WebGL2: near-black sky, the warm
/// band at the horizon (42% down, where the shader puts it), dark floor.
const STYLE: &str = "\
:root{color-scheme:dark}\
html,body{margin:0;height:100%}\
body{background:#0a0a0a linear-gradient(180deg,#090909 0%,#0d0a09 22%,#1c0f09 36%,#3b1a0b 41.6%,#120c0a 42.2%,#0b0a0a 60%,#0a0a0a 100%);\
color:#ededed;font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,\"Helvetica Neue\",Arial,sans-serif;overflow:hidden}\
canvas{position:fixed;inset:0;width:100%;height:100%;display:block}\
main{position:relative;box-sizing:border-box;min-height:100%;display:flex;flex-direction:column;align-items:center;\
justify-content:center;gap:clamp(28px,5vh,48px);padding:0 16px 22vh;text-align:center}\
h1{margin:0;font-size:clamp(48px,10vw,132px);font-weight:600;letter-spacing:-.035em;line-height:1;\
text-shadow:0 2px 30px rgba(0,0,0,.55)}\
a{display:inline-block;padding:15px 34px;border-radius:999px;background:#ededed;color:#0a0a0a;\
font-size:17px;font-weight:600;text-decoration:none;box-shadow:0 0 0 1px rgba(255,255,255,.2),0 8px 40px rgba(255,140,40,.18)}\
a:hover{background:#fff}\
a:focus-visible{outline:2px solid #ffb35c;outline-offset:4px}";

/// The favicon, rendered as `/favicon.svg` renders it, inline so the page
/// needs no second request.
const ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E\
%3Crect width='64' height='64' fill='%230a0a0a'/%3E%3Cpath d='M22 18a18 18 0 1 0 20 0' fill='none' \
stroke='%23ededed' stroke-width='5' stroke-linecap='round'/%3E%3Cpath d='M32 10v22' stroke='%23ededed' \
stroke-width='5' stroke-linecap='round'/%3E%3C/svg%3E";

/// A page, ready to send.
pub(super) struct Document {
    pub(super) html: String,
    brotli: Vec<u8>,
    gzip: Vec<u8>,
    pub(super) policy: HeaderValue,
}

/// Which build a kept document was made from.
pub(super) type BuildKey = Option<(PathBuf, SystemTime, SystemTime)>;

/// The last document made, and the build it came from.
static KEPT: Mutex<Option<(BuildKey, Arc<Document>)>> = Mutex::new(None);

pub(crate) fn routes() -> Router<App> {
    Router::new().route(LANDER5_PATH, get(page))
}

async fn page(State(app): State<App>, request: HeaderMap) -> Response {
    let directory = app.config.lander5.clone();
    let document = tokio::task::spawn_blocking(move || document(directory.as_deref()))
        .await
        .unwrap_or_else(|_| Arc::new(assemble(None)));
    send(&document, &request)
}

/// `document` as the response to `request`: brotli or gzip when accepted.
pub(super) fn send(document: &Document, request: &HeaderMap) -> Response {
    let (body, coding) = if accepts(&request, "br") {
        (document.brotli.clone(), Some("br"))
    } else if accepts_gzip(&request) {
        (document.gzip.clone(), Some("gzip"))
    } else {
        (document.html.clone().into_bytes(), None)
    };
    let mut response = ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, document.policy.clone());
    headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    if let Some(coding) = coding {
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(coding));
    }
    response
}

/// The build's glue and module.
pub(super) fn build(directory: Option<&Path>) -> Option<(String, Vec<u8>)> {
    let directory = directory?;
    let glue = std::fs::read_to_string(directory.join(LANDER5_GLUE)).ok()?;
    let wasm = std::fs::read(directory.join(LANDER5_WASM)).ok()?;
    Some((glue, wasm))
}

/// Which build is in `directory` now.
pub(super) fn build_key(directory: Option<&Path>) -> BuildKey {
    directory.and_then(|directory| {
        let modified = |file: &str| {
            std::fs::metadata(directory.join(file))
                .and_then(|m| m.modified())
                .ok()
        };
        Some((
            directory.to_path_buf(),
            modified(LANDER5_GLUE)?,
            modified(LANDER5_WASM)?,
        ))
    })
}

/// The document for the build in `directory`, made once per build.
fn document(directory: Option<&Path>) -> Arc<Document> {
    let key = build_key(directory);
    if let Ok(kept) = KEPT.lock()
        && let Some((kept_key, document)) = kept.as_ref()
        && *kept_key == key
    {
        return document.clone();
    }
    let document = Arc::new(assemble(build(directory)));
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, document.clone()));
    }
    document
}

/// The glue as a classic module body: its `export` keywords dropped, so it
/// can sit inline beside the line that starts it.
fn inline_glue(glue: &str) -> String {
    let mut out = String::with_capacity(glue.len());
    for line in glue.lines() {
        if line.starts_with("export {") || line.starts_with("export default ") {
            continue;
        }
        out.push_str(line.strip_prefix("export ").unwrap_or(line));
        out.push('\n');
    }
    out
}

/// `'sha256-…'` for a policy, over `text` exactly as the page carries it.
pub(super) fn source_hash(text: &str) -> String {
    format!(
        "'sha256-{}'",
        STANDARD.encode(Sha256::digest(text.as_bytes()))
    )
}

/// Whether `text` can sit inside a `<script>` element unchanged.
pub(super) fn fits_in_script(text: &str) -> bool {
    !text.to_ascii_lowercase().contains("</script")
}

/// What another page adds to this one (`/lander6`'s demo card): its own
/// stylesheet, markup after the Download link, and a module script that
/// runs after the runner's, loading files from this site. `main`, when
/// given, is the whole of `<main>` instead (`/lander7` has no Download
/// link).
pub(super) struct Addition {
    pub style: &'static str,
    pub in_main: &'static str,
    pub script: &'static str,
    /// Appended to the runner's script, after its start; the runner's
    /// start promise is `ready` there.
    pub runner_tail: &'static str,
    pub main: Option<&'static str>,
}

/// The page, with the runner when a build is given.
fn assemble(build: Option<(String, Vec<u8>)>) -> Document {
    assemble_with(build, None)
}

/// [`assemble`], with another page's [`Addition`].
pub(super) fn assemble_with(
    build: Option<(String, Vec<u8>)>,
    addition: Option<&Addition>,
) -> Document {
    let style = addition.map_or(STYLE, |addition| addition.style);
    let in_main = addition.map_or("", |addition| addition.in_main);
    let main = match addition.and_then(|addition| addition.main) {
        Some(main) => main.to_owned(),
        None => format!("<h1>{TITLE}</h1><a href=\"{DOWNLOAD}\">Download</a>{in_main}"),
    };
    let scene = build.and_then(|(glue, wasm)| {
        let sky = lander5_web::shaders::minify(&lander5_web::shaders::sky_source());
        let grid = lander5_web::shaders::minify(&lander5_web::shaders::grid_source());
        let script = match addition {
            None => format!(
                "{}const s=atob(\"{}\"),b=new Uint8Array(s.length);\
for(let i=0;i<s.length;i++)b[i]=s.charCodeAt(i);__wbg_init({{module_or_path:b}});",
                inline_glue(&glue),
                STANDARD.encode(&wasm)
            ),
            Some(addition) => format!(
                "{}const s=atob(\"{}\"),b=new Uint8Array(s.length);\
for(let i=0;i<s.length;i++)b[i]=s.charCodeAt(i);const ready=__wbg_init({{module_or_path:b}});{}",
                inline_glue(&glue),
                STANDARD.encode(&wasm),
                addition.runner_tail
            ),
        };
        [&sky, &grid, &script]
            .iter()
            .all(|text| fits_in_script(text))
            .then_some((sky, grid, script))
    });
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{TITLE}</title>\
<meta name=\"description\" content=\"{DESCRIPTION}\">\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"{ICON}\">\
<style>{style}</style></head><body>\
<canvas id=\"{canvas}\" aria-hidden=\"true\"></canvas>\
<main>{main}</main>",
        canvas = lander5_web::shaders::CANVAS_ID,
    );
    let mut script_hash = None;
    if let Some((sky, grid, script)) = &scene {
        let notice = lander5_web::shaders::library_notice();
        html.push_str(&format!(
            "<!--\n{notice}-->\
<script type=\"x-shader/x-fragment\" id=\"{sky_id}\">{sky}</script>\
<script type=\"x-shader/x-fragment\" id=\"{grid_id}\">{grid}</script>\
<script type=\"module\">{script}</script>",
            sky_id = lander5_web::shaders::SKY_ID,
            grid_id = lander5_web::shaders::GRID_ID,
        ));
        script_hash = Some(source_hash(script));
    }
    let mut connect = String::new();
    let script_src = match (addition, &script_hash) {
        (Some(addition), _) if fits_in_script(addition.script) => {
            html.push_str(&format!(
                "<script type=\"module\">{}</script>",
                addition.script
            ));
            connect = "connect-src 'self'; worker-src blob:; ".to_owned();
            let runner = script_hash.as_deref().unwrap_or_default();
            format!(
                "{runner} {} 'self' 'wasm-unsafe-eval'",
                source_hash(addition.script)
            )
            .trim_start()
            .to_owned()
        }
        (_, Some(hash)) => format!("{hash} 'wasm-unsafe-eval'"),
        (_, None) => "'none'".to_owned(),
    };
    html.push_str("</body></html>");

    let policy = format!(
        "default-src 'none'; style-src {style}; script-src {script_src}; {connect}img-src data:; \
base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        style = source_hash(style),
    );
    let policy = HeaderValue::from_str(&policy)
        .unwrap_or_else(|_| HeaderValue::from_static("default-src 'none'"));
    let mut brotli = Vec::new();
    let _ = brotli::BrotliCompress(
        &mut html.as_bytes(),
        &mut brotli,
        &brotli::enc::BrotliEncoderParams {
            quality: 11,
            lgwin: 22,
            ..Default::default()
        },
    );
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = std::io::Write::write_all(&mut gzip, html.as_bytes());
    let gzip = gzip.finish().unwrap_or_default();
    Document {
        html,
        brotli,
        gzip,
        policy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLUE: &str = "let wasm;\nexport function start() {}\n\
async function __wbg_init(module_or_path) { return module_or_path; }\n\
export { initSync, __wbg_init as default };\n";

    #[test]
    fn the_glue_loses_its_exports_only() {
        assert_eq!(
            inline_glue(GLUE),
            "let wasm;\nfunction start() {}\nasync function __wbg_init(module_or_path) { return module_or_path; }\n"
        );
    }

    #[test]
    fn the_policy_admits_the_inline_style_and_script_by_hash() {
        let document = assemble(Some((GLUE.to_owned(), b"\0asm\x01\0\0\0".to_vec())));
        let policy = document.policy.to_str().unwrap();
        let script = document
            .html
            .split("<script type=\"module\">")
            .nth(1)
            .and_then(|rest| rest.split("</script>").next())
            .unwrap();
        assert!(script.contains("__wbg_init({module_or_path:b})"));
        assert!(script.contains("AGFzbQEAAAA="), "the module, base64");
        assert!(policy.contains(&source_hash(script)));
        assert!(policy.contains(&source_hash(STYLE)));
        assert!(policy.contains("'wasm-unsafe-eval'"));
        assert!(!policy.contains("unsafe-inline"));
        assert!(document.html.contains("Copyright (c) 2026 gokayfem"));
    }

    #[test]
    fn without_a_build_the_page_runs_no_script() {
        let document = assemble(None);
        assert!(!document.html.contains("<script"));
        assert!(
            document
                .policy
                .to_str()
                .unwrap()
                .contains("script-src 'none'")
        );
    }

    #[test]
    fn a_build_that_could_close_its_script_is_left_out() {
        let document = assemble(Some((
            "</SCRIPT><script>alert(1)".to_owned(),
            b"\0asm".to_vec(),
        )));
        assert!(!document.html.contains("<script"));
    }

    #[test]
    fn the_compressed_copies_hold_the_page() {
        let document = assemble(None);
        let mut html = Vec::new();
        brotli::BrotliDecompress(&mut document.brotli.as_slice(), &mut html).unwrap();
        assert_eq!(html, document.html.as_bytes());
        let mut gunzip = flate2::read::GzDecoder::new(document.gzip.as_slice());
        let mut html = String::new();
        std::io::Read::read_to_string(&mut gunzip, &mut html).unwrap();
        assert_eq!(html, document.html);
    }
}
