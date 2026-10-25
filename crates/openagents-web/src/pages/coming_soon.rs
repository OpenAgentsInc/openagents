//! `/`: "Coming soon", centered over a slow WebGPU field. The launch flow
//! is reviewed behind the private preview ([`crate::preview`]) until it
//! opens. A `/` address carrying one of the Grid's query parameters still
//! goes to the Grid at `/home` with its query.
//!
//! Black, with Aethyr's Noir content color (`oa_tokens::noir::CONTENT`,
//! `#ededed`) for the text. Without WebGPU, or with reduced
//! motion, the page keeps its still gradient.

use axum::{
    Router,
    extract::RawQuery,
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};

use crate::App;

pub(crate) const PATH: &str = "/";
const SCRIPT_PATH: &str = "/coming-soon.js";
const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OpenAgents</title>
<meta name="description" content="OpenAgents. Coming soon.">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<script type="application/ld+json">{"@context":"https://schema.org","@type":"Organization","name":"OpenAgents","url":"https://openagents.com/"}</script>
<link rel="alternate" type="text/markdown" href="/index.md">
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{display:grid;place-items:center;overflow:hidden;font-family:"Geist","Inter",ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif}
canvas{position:fixed;inset:0;width:100%;height:100%;opacity:0;transition:opacity 1.6s ease}
canvas.on{opacity:1}
.still{position:fixed;inset:0;background:#000}
main{position:relative;text-align:center;padding:0 16px;user-select:none;-webkit-user-select:none}
::selection{background:#ededed;color:#000}
h1{margin:0;font-size:13px;font-weight:500;letter-spacing:.6em;text-indent:.6em;text-transform:uppercase;line-height:1}
</style>
</head>
<body>
<div class="still"></div>
<canvas id="field" aria-hidden="true"></canvas>
<main><h1>Coming soon</h1></main>
<script type="module" src="/coming-soon.js"></script>
</body>
</html>
"##;

const SCRIPT: &str = r##"const canvas = document.getElementById("field");
const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
const shader = `
struct U { size: vec2f, time: f32, pad: f32 }
@group(0) @binding(0) var<uniform> u: U;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn fbm(q: vec2f) -> f32 {
  var p = q; var v = 0.0; var a = 0.5;
  for (var k = 0; k < 5; k++) { v += a * noise(p); p = p * 2.03 + vec2f(1.7, 9.2); a *= 0.5; }
  return v;
}
@fragment fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let uv = (pos.xy - 0.5 * u.size) / u.size.y;
  let t = u.time * 0.035;
  let w = vec2f(fbm(uv * 1.6 + vec2f(t, -t)), fbm(uv * 1.6 + vec2f(-t, t * 0.7) + 4.0));
  let n = fbm(uv * 2.2 + 2.4 * w + vec2f(0.0, t));
  let glow = exp(-2.2 * dot(uv, uv));
  let base = vec3f(0.0);
  let tint = mix(vec3f(0.05, 0.055, 0.075), vec3f(0.20, 0.21, 0.27), n * n);
  var c = base + tint * glow * glow * (0.2 + 0.8 * n);
  let star = step(0.9975, hash(floor(pos.xy * 0.5))) * (0.5 + 0.5 * sin(u.time * 1.3 + hash(floor(pos.xy)) * 40.0));
  c += vec3f(star * 0.5 * (1.0 - glow * 0.6));
  return vec4f(c, 1.0);
}`;
async function start() {
  if (!navigator.gpu) return;
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return;
  const device = await adapter.requestDevice();
  const context = canvas.getContext("webgpu");
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque" });
  const module = device.createShaderModule({ code: shader });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format }] },
  });
  const uniforms = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: uniforms } }] });
  const begin = performance.now();
  const frame = () => {
    const scale = Math.min(devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.floor(canvas.clientWidth * scale));
    const h = Math.max(1, Math.floor(canvas.clientHeight * scale));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    const time = still ? 20 : 20 + (performance.now() - begin) / 1000;
    device.queue.writeBuffer(uniforms, 0, new Float32Array([w, h, time, 0]));
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }] });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, group);
    pass.draw(3);
    pass.end();
    device.queue.submit([encoder.finish()]);
    canvas.classList.add("on");
    if (!still) requestAnimationFrame(frame);
  };
  frame();
  if (still) addEventListener("resize", frame);
}
start().catch(() => {});
"##;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(page))
        .route(SCRIPT_PATH, get(script))
}

/// `/` query parameters that belong to the Grid at `/home`.
const GRID_QUERY: [&str; 6] = ["chat", "suggest", "lab", "ui", "project", "gl"];

async fn page(RawQuery(query): RawQuery) -> Response {
    if let Some(query) = query.as_deref()
        && url::form_urlencoded::parse(query.as_bytes())
            .any(|(k, _)| GRID_QUERY.contains(&k.as_ref()))
    {
        return Redirect::temporary(&format!("{}?{query}", super::grid_home::GRID_HOME_PATH))
            .into_response();
    }
    let mut response = axum::response::Html(PAGE).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=60"));
    response
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        SCRIPT,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_says_only_coming_soon_and_loads_its_own_script() {
        assert!(PAGE.contains("<h1>Coming soon</h1>"));
        assert!(PAGE.contains(r#"src="/coming-soon.js""#));
        for gone in ["/playtest", "/waitlist", "/beta", "/new-lander", "<form", "<a "] {
            assert!(!PAGE.contains(gone), "{gone}");
        }
        assert!(SCRIPT.contains("navigator.gpu") && SCRIPT.contains("prefers-reduced-motion"));
    }
}
