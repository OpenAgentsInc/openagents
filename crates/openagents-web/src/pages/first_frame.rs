//! The first frame `/agent/foreman` carries inline, so Foreman's graph shows
//! after one round trip instead of after the 1.5 MB module.
//!
//! The AgentGraph build writes `first-frame/` beside the module
//! (`build-web-demo.sh --app agent-graph`): a paint-only WebGL2 painter
//! (`painter.wasm.gz`, its `painter.js` glue, and `boot.js`), and the
//! view's first frame for each size class, captured natively by
//! `first-frame-capture` (`foreman-CLASS.ffs.gz`), with `manifest.json`
//! naming the classes and the digest of the graph they show.
//!
//! The page then carries, after the graph box, the painter and one class's
//! frame as base64 in inert script elements, and the glue and boot script
//! inline, allowed by its hash in the page's policy. A phone's user agent
//! gets the phone class, any other the desktop class. The boot script
//! paints only when the box measures what the frame was captured at, and
//! the app's own first frame replaces it. A build whose graph digest isn't
//! this server's graph is left out, so the page never shows a stale graph.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, header};
use base64::Engine as _;
use maud::{Markup, PreEscaped, html};
use sha2::{Digest, Sha256};

/// The directory inside the AgentGraph build.
pub(crate) const DIRECTORY: &str = "first-frame";

/// The capture tool inside it (`first-frame-capture`, native).
pub(crate) const CAPTURE: &str = "first-frame-capture";

/// Where a page whose box matches no class asks for its frame:
/// `?v=DIGEST&w=WIDTH&h=HEIGHT&dpr=SCALE`.
pub(crate) const ON_DEMAND: &str = "/agent/foreman/first-frame";

/// One size class's frame, ready to inline.
#[derive(Debug, PartialEq)]
pub(crate) struct Class {
    pub(crate) name: String,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) scale: f64,
    /// The gzipped scene, base64.
    pub(crate) scene: String,
}

/// A build's first frame, read once.
#[derive(Debug, PartialEq)]
pub(crate) struct FirstFrame {
    /// The painter module, gzipped, base64.
    pub(crate) painter: String,
    /// The glue and the boot script, as one inline script.
    pub(crate) script: String,
    /// `'sha256-…'` for the inline script.
    pub(crate) hash: String,
    pub(crate) classes: Vec<Class>,
    /// The graph's digest, which names the on-demand frames' addresses.
    pub(crate) digest: String,
    /// The capture tool, for window sizes no class covers.
    pub(crate) capture: Option<PathBuf>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl FirstFrame {
    /// Reads `build/first-frame`, or `None` when it's missing, unreadable,
    /// or captured from another graph than `graph_json`.
    pub(crate) fn read(build: &Path, graph_json: &str) -> Option<FirstFrame> {
        let dir = build.join(DIRECTORY);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
        if manifest["schema"] != "openagents.first-frame.v1"
            || manifest["graph_sha256"].as_str()? != sha256_hex(graph_json.as_bytes())
        {
            return None;
        }
        let b64 = |bytes: Vec<u8>| base64::engine::general_purpose::STANDARD.encode(bytes);
        let mut classes = vec![];
        for class in manifest["classes"].as_array()? {
            let file = class["file"].as_str()?;
            // The manifest names files in its own directory only.
            if file.contains('/') || file.contains('\\') || file.starts_with('.') {
                return None;
            }
            classes.push(Class {
                name: class["name"].as_str()?.to_owned(),
                width: class["width"].as_f64()?,
                height: class["height"].as_f64()?,
                scale: class["scale"].as_f64()?,
                scene: b64(std::fs::read(dir.join(file)).ok()?),
            });
        }
        let glue = std::fs::read_to_string(dir.join("painter.js")).ok()?;
        let boot = std::fs::read_to_string(dir.join("boot.js")).ok()?;
        let script = format!("{glue}\n{boot}");
        if script.to_ascii_lowercase().contains("</script") {
            return None;
        }
        let hash = format!(
            "'sha256-{}'",
            base64::engine::general_purpose::STANDARD.encode(Sha256::digest(script.as_bytes()))
        );
        let capture = dir.join(CAPTURE);
        Some(FirstFrame {
            painter: b64(std::fs::read(dir.join("painter.wasm.gz")).ok()?),
            script,
            hash,
            classes,
            digest: sha256_hex(graph_json.as_bytes()),
            capture: capture.is_file().then_some(capture),
        })
    }

    /// [`FirstFrame::read`], once per build directory and graph.
    pub(crate) fn cached(build: &Path, graph_json: &str) -> Option<Arc<FirstFrame>> {
        static CACHE: Mutex<Option<HashMap<(PathBuf, String), Option<Arc<FirstFrame>>>>> =
            Mutex::new(None);
        let key = (build.to_path_buf(), sha256_hex(graph_json.as_bytes()));
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .get_or_insert_with(HashMap::new)
            .entry(key)
            .or_insert_with(|| FirstFrame::read(build, graph_json).map(Arc::new))
            .clone()
    }

    /// The class for this request: phones by their user agent's `Mobi` or
    /// `Sec-CH-UA-Mobile: ?1`, anything else desktop.
    pub(crate) fn class(&self, headers: &HeaderMap) -> Option<&Class> {
        let phone = headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ua| ua.contains("Mobi"))
            || headers
                .get("sec-ch-ua-mobile")
                .is_some_and(|v| v.as_bytes() == b"?1");
        let want = if phone { "phone" } else { "desktop" };
        self.classes.iter().find(|c| c.name == want)
    }

    /// The elements after the graph box: the painter, the frame, and the
    /// script that paints it.
    pub(crate) fn markup(&self, class: &Class) -> Markup {
        html! {
            script type="application/octet-stream" id="first-frame-painter" { (self.painter) }
            script type="application/octet-stream" id="first-frame-scene"
                data-width=(class.width) data-height=(class.height) data-scale=(class.scale)
                data-other=[self.capture.as_ref().map(|_| format!("{ON_DEMAND}?v={}", self.digest))] { (class.scene) }
            script { (PreEscaped(&self.script)) }
        }
    }

    /// `policy` with this script's hash allowed.
    pub(crate) fn policy(&self, policy: &str) -> String {
        policy.replacen(
            "script-src 'self'",
            &format!("script-src 'self' {}", self.hash),
            1,
        )
    }
}

/// A frame size a page may ask for: layout pixels to 1/64 and a scale to
/// 1/1000, as integers, so one size has one cache entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Size {
    w64: u32,
    h64: u32,
    dpr1000: u32,
}

impl Size {
    /// Checks and rounds a page's request: a box from 100 to 4096 layout
    /// pixels a side at a scale from 0.5 to 4.
    pub(crate) fn parse(w: f64, h: f64, dpr: f64) -> Option<Size> {
        let fine = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !fine(w, 100.0, 4096.0) || !fine(h, 100.0, 4096.0) || !fine(dpr, 0.5, 4.0) {
            return None;
        }
        Some(Size {
            w64: (w * 64.0).round() as u32,
            h64: (h * 64.0).round() as u32,
            dpr1000: (dpr * 1000.0).round() as u32,
        })
    }

    fn args(&self) -> [String; 3] {
        [
            (self.w64 as f64 / 64.0).to_string(),
            (self.h64 as f64 / 64.0).to_string(),
            (self.dpr1000 as f64 / 1000.0).to_string(),
        ]
    }
}

/// Frames captured on request, by size; at most this many are kept.
const KEPT: usize = 256;

/// The frame for `size`, captured once by the build's capture tool (one
/// capture at a time, ten seconds at most) and kept.
pub(crate) async fn on_demand(first: &FirstFrame, size: Size) -> Option<Arc<Vec<u8>>> {
    static FRAMES: Mutex<Option<HashMap<(String, Size), Arc<Vec<u8>>>>> = Mutex::new(None);
    static ONE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let key = (first.digest.clone(), size);
    let cached = |key: &(String, Size)| {
        FRAMES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .get(key)
            .cloned()
    };
    if let Some(frame) = cached(&key) {
        return Some(frame);
    }
    let tool = first.capture.as_ref()?;
    let _turn = ONE.acquire().await.ok()?;
    if let Some(frame) = cached(&key) {
        return Some(frame);
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(tool)
            .arg("--frame")
            .args(size.args())
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    let frame = Arc::new(output.stdout);
    let mut frames = FRAMES.lock().unwrap_or_else(|e| e.into_inner());
    let frames = frames.get_or_insert_with(HashMap::new);
    if frames.len() >= KEPT {
        frames.clear();
    }
    frames.insert(key, frame.clone());
    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(graph: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join(DIRECTORY);
        std::fs::create_dir_all(&first).unwrap();
        let manifest = serde_json::json!({
            "schema": "openagents.first-frame.v1",
            "graph_sha256": sha256_hex(graph.as_bytes()),
            "classes": [
                {"name": "desktop", "width": 990.0, "height": 736.0, "scale": 2.0, "file": "d.ffs.gz"},
                {"name": "phone", "width": 356.0, "height": 605.671875, "scale": 2.0, "file": "p.ffs.gz"},
            ],
        });
        std::fs::write(first.join("manifest.json"), manifest.to_string()).unwrap();
        std::fs::write(first.join("d.ffs.gz"), b"desktop").unwrap();
        std::fs::write(first.join("p.ffs.gz"), b"phone").unwrap();
        std::fs::write(first.join("painter.wasm.gz"), b"wasm").unwrap();
        std::fs::write(first.join("painter.js"), "let wasm_bindgen;").unwrap();
        std::fs::write(first.join("boot.js"), "(function () {})();").unwrap();
        dir
    }

    #[test]
    fn a_build_for_this_graph_inlines_its_frame_and_hashes_its_script() {
        let dir = build("{\"g\":1}");
        let first = FirstFrame::read(dir.path(), "{\"g\":1}").unwrap();
        assert_eq!(first.script, "let wasm_bindgen;\n(function () {})();");
        let digest = base64::engine::general_purpose::STANDARD
            .encode(Sha256::digest(first.script.as_bytes()));
        assert_eq!(first.hash, format!("'sha256-{digest}'"));
        let policy = first.policy(crate::pages::foreman::GRAPH_POLICY);
        assert!(policy.contains(&format!(
            "script-src 'self' {} 'wasm-unsafe-eval';",
            first.hash
        )));
        assert!(!policy.contains("unsafe-inline"));

        let mut headers = HeaderMap::new();
        let desktop = first.class(&headers).unwrap();
        assert_eq!(desktop.scene, "ZGVza3RvcA==");
        headers.insert(
            header::USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Mobile/15E148"
                .parse()
                .unwrap(),
        );
        let phone = first.class(&headers).unwrap();
        assert_eq!((phone.name.as_str(), phone.height), ("phone", 605.671875));
        let html = first.markup(phone).into_string();
        assert!(html.contains("id=\"first-frame-painter\">d2FzbQ==</script>"));
        assert!(html.contains(
            "data-width=\"356\" data-height=\"605.671875\" data-scale=\"2\">cGhvbmU=</script>"
        ));
        assert!(html.contains("<script>let wasm_bindgen;\n(function () {})();</script>"));
    }

    #[test]
    fn a_build_for_another_graph_or_a_broken_build_is_left_out() {
        let dir = build("{\"g\":1}");
        assert!(FirstFrame::read(dir.path(), "{\"g\":2}").is_none());
        std::fs::remove_file(dir.path().join(DIRECTORY).join("boot.js")).unwrap();
        assert!(FirstFrame::read(dir.path(), "{\"g\":1}").is_none());
        assert!(FirstFrame::read(Path::new("/nowhere"), "{}").is_none());

        let dir = build("{}");
        let manifest = dir.path().join(DIRECTORY).join("manifest.json");
        let text = std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("d.ffs.gz", "../d.ffs.gz");
        std::fs::write(&manifest, text).unwrap();
        assert!(FirstFrame::read(dir.path(), "{}").is_none());
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[test]
    fn requested_sizes_are_bounded_and_rounded() {
        let s = Size::parse(990.0, 605.671875, 2.0).unwrap();
        assert_eq!(
            s.args(),
            ["990".to_owned(), "605.671875".to_owned(), "2".to_owned()]
        );
        assert_eq!(
            Size::parse(990.004, 605.67, 2.0001),
            Some(Size {
                w64: 63360,
                h64: 38763,
                dpr1000: 2000
            })
        );
        assert!(Size::parse(50.0, 600.0, 2.0).is_none());
        assert!(Size::parse(600.0, 5000.0, 2.0).is_none());
        assert!(Size::parse(600.0, 600.0, 0.0).is_none());
        assert!(Size::parse(f64::NAN, 600.0, 2.0).is_none());
        assert!(Size::parse(600.0, 600.0, 9.0).is_none());
    }
}
