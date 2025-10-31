use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use std::fs;
use std::path::Path;
use serde_json::json;

/// Wrapper handle for a zoomable, pannable SVG viewer window.
/// Construct via `render_mermaid(svg_source)` then call `run()`
/// to open the window and process events.
pub struct MermaidView {
    html: String,
    /// Optional window title. Defaults to "Mermaid Viewer".
    pub title: String,
    /// Initial window size (physical pixels).
    pub width: u32,
    pub height: u32,
}

impl MermaidView {
    /// Launches the viewer window and blocks the current thread
    /// until the window is closed.
    pub fn run(&self) -> Result<()> {
        use tao::event::{Event, WindowEvent};
        use tao::event_loop::{ControlFlow, EventLoop};
        use tao::window::WindowBuilder;
        use wry::WebViewBuilder;

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(self.title.clone())
            .with_inner_size(tao::dpi::LogicalSize::new(self.width as f64, self.height as f64))
            .build(&event_loop)
            .map_err(|e| anyhow!("create window: {e}"))?;

        let _webview = WebViewBuilder::new()
            .with_html(self.html.clone())
            .build(&window)
            .map_err(|e| anyhow!("build webview: {e}"))?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            if let Event::WindowEvent { event, .. } = event {
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                }
            }
        });
    }
}

/// Build a viewer for an SVG (produced from Mermaid elsewhere).
/// The SVG is shown on a dark background with light text and
/// supports smooth pan/zoom interactions with mouse and touch.
pub fn render_mermaid(svg_source: &str) -> Result<MermaidView> {
    let html = build_html(svg_source);
    Ok(MermaidView {
        html,
        title: "Mermaid Viewer".to_string(),
        width: 1024,
        height: 768,
    })
}

/// Build a viewer with a left sidebar that lists all Mermaid/SVG docs found
/// under the provided directory (e.g., `docs/tinyvex`). Clicking a doc loads
/// it into the viewer without leaving the window.
pub fn render_mermaid_docs_index(dir: impl AsRef<Path>) -> Result<MermaidView> {
    let dir = dir.as_ref();
    let mut docs: Vec<serde_json::Value> = Vec::new();
    let exts = ["md", "markdown", "mmd", "svg"]; 
    for entry in fs::read_dir(dir).map_err(|e| anyhow!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| anyhow!("read_dir entry: {e}"))?;
        let p = entry.path();
        if !p.is_file() { continue; }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !exts.contains(&ext.as_str()) { continue; }
        let raw = fs::read_to_string(&p).map_err(|e| anyhow!("read {}: {e}", p.display()))?;
        let (kind, content) = if ext == "svg" || raw.trim_start().starts_with("<svg") {
            ("svg", raw)
        } else if ext == "md" || ext == "markdown" {
            match extract_first_mermaid_block(&raw) {
                Some(code) => ("mermaid", code),
                None => continue, // skip MD without a mermaid block to avoid syntax errors
            }
        } else {
            ("mermaid", raw)
        };
        docs.push(json!({
            "name": name,
            "kind": kind,
            "content": content,
        }));
    }
    // Sort by name for stable nav
    docs.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
    let docs_json = serde_json::to_string(&docs).unwrap_or("[]".into());
    let html = build_html_docs_index(&docs_json);
    Ok(MermaidView { html, title: "Mermaid Viewer".into(), width: 1280, height: 840 })
}

fn extract_first_mermaid_block(md: &str) -> Option<String> {
    let fence = "```mermaid";
    let i = md.find(fence)?;
    let rest = &md[i + fence.len()..];
    let j = rest.find("```")?;
    Some(rest[..j].trim().to_string())
}

/// Build a viewer from Mermaid diagram source text.
/// Renders via mermaid.js inside the webview, then applies the same
/// pan/zoom behavior to the produced SVG.
pub fn render_mermaid_code(mermaid_code: &str) -> Result<MermaidView> {
    let html = build_html_from_mermaid(mermaid_code);
    Ok(MermaidView {
        html,
        title: "Mermaid Viewer".to_string(),
        width: 1024,
        height: 768,
    })
}

fn build_html(svg_source: &str) -> String {
    // Embed Berkeley Mono (Regular) from the repo for consistent typography.
    // Falls back to local fonts if loading fails.
    const BERKELEY_MONO_REGULAR: &[u8] = include_bytes!(
        "../../../expo/assets/fonts/BerkeleyMono-Regular.ttf"
    );
    let font_b64 = BASE64.encode(BERKELEY_MONO_REGULAR);
    // Inline the SVG into a themed HTML page. We attach listeners to
    // implement viewBox-based zooming and panning for crisp vector scaling.
    // Theme constants
    let bg = "#08090a";
    let text = "#f7f8f8";
    let stroke = "#6c7075"; // muted grey
    // Encode SVG to avoid raw injection; we will sanitize in JS before attaching
    let svg_b64 = BASE64.encode(svg_source.as_bytes());

    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Mermaid Viewer</title>
    <style>
      /* Disable selection across the viewer */
      *, *::before, *::after {{ -webkit-user-select: none; -moz-user-select: none; user-select: none; -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent; }}
      #toolbar, #toolbar * {{ -webkit-user-select: none !important; user-select: none !important; pointer-events: auto; }}
      @font-face {{
        font-family: 'Berkeley Mono Viewer';
        src: url('data:font/ttf;base64,{font_b64}') format('truetype');
        font-weight: 400;
        font-style: normal;
        font-display: swap;
      }}
      :root {{
        --bg: {bg};
        --fg: {text};
        --stroke: {stroke};
      }}
      html, body {{
        margin: 0; padding: 0; height: 100%; width: 100%;
        background: var(--bg);
        color: var(--fg);
        font-family: 'Berkeley Mono Viewer', 'Berkeley Mono', ui-monospace, Menlo, Consolas, monospace;
        -webkit-font-smoothing: antialiased;
        text-rendering: optimizeLegibility;
        -webkit-text-size-adjust: 100%;
      }}
      #toolbar {{
        position: fixed; top: 12px; right: 12px; z-index: 10;
        background: rgba(20,22,24,0.7);
        border: 1px solid #2a2d31; border-radius: 6px;
        padding: 6px 8px; display: flex; gap: 6px; align-items: center;
        box-shadow: 0 4px 10px rgba(0,0,0,0.25);
      }}
      #toolbar button {{
        background: #14181b; color: var(--fg);
        border: 1px solid #2a2d31; border-radius: 4px;
        padding: 4px 8px; cursor: pointer;
      }}
      #toolbar button:hover {{ border-color: #3b4046; }}
      #view {{ position: absolute; inset: 0; overflow: hidden; }}
      svg {{
        height: 100%; width: 100%; display: block; background: var(--bg);
        text-rendering: geometricPrecision;
        -webkit-font-smoothing: antialiased;
      }}
      /* Prevent any selection/dragging on SVG and descendants, and route events to container */
      svg, svg * {{ -webkit-user-select: none !important; user-select: none !important; -webkit-user-drag: none; }}
      svg *, svg text {{ pointer-events: none; }}
      svg text {{
        fill: var(--fg) !important;
        font-family: 'Berkeley Mono Viewer', 'Berkeley Mono', ui-monospace, Menlo, monospace !important;
        font-variant-ligatures: none;
        font-synthesis: none;
        text-rendering: geometricPrecision;
      }}
      svg rect, svg path, svg line, svg circle, svg ellipse, svg polygon {{
        stroke: var(--stroke);
        vector-effect: non-scaling-stroke;
        shape-rendering: geometricPrecision;
      }}
    </style>
  </head>
  <body>
    <div id="toolbar">
      <button id="fit">Fit</button>
      <button id="zoom_in">+</button>
      <button id="zoom_out">-</button>
      <span id="status" style="opacity:.8;font-size:12px;">100%</span>
    </div>
    <div id="view"></div>
    <script>
      (function(){{
        // Block any selection attempts globally
        document.addEventListener('selectstart', (e) => e.preventDefault());
        document.addEventListener('dragstart', (e) => e.preventDefault());
        const container = document.getElementById('view');
        // Decode and sanitize SVG before attaching
        const raw = atob('{svg_b64}');
        const doc = new DOMParser().parseFromString(raw, 'image/svg+xml');
        let svg = doc && doc.documentElement && doc.documentElement.tagName.toLowerCase() === 'svg' ? doc.documentElement : null;
        if (!svg) return;
        svg.querySelectorAll('script, foreignObject').forEach(n => n.remove());
        const walker = document.createTreeWalker(svg, NodeFilter.SHOW_ELEMENT);
        const els = [];
        while (walker.nextNode()) els.push(walker.currentNode);
        els.forEach(el => {{
          if (!el.getAttributeNames) return;
          for (const name of el.getAttributeNames()) {{
            if (/^on/i.test(name)) el.removeAttribute(name);
            if ((name === 'href' || name.endsWith(':href'))) {{
              const v = (el.getAttribute(name) || '').trim();
              if (/^(javascript:|data:)/i.test(v)) el.removeAttribute(name);
            }}
          }}
        }});
        container.innerHTML = '';
        container.appendChild(svg);

        // Ensure viewBox exists
        function ensureViewBox(){{
          const vb = svg.viewBox.baseVal;
          if (vb && (vb.width > 0 && vb.height > 0)) return;
          const w = parseFloat(svg.getAttribute('width')) || container.clientWidth || 1024;
          const h = parseFloat(svg.getAttribute('height')) || container.clientHeight || 768;
          svg.setAttribute('viewBox', '0 0 ' + w + ' ' + h);
        }}
        ensureViewBox();

        // Track viewBox state and remember initial values for Fit
        let vx = svg.viewBox.baseVal.x;
        let vy = svg.viewBox.baseVal.y;
        let vw = svg.viewBox.baseVal.width || container.clientWidth || 1024;
        let vh = svg.viewBox.baseVal.height || container.clientHeight || 768;
        const init = {{ x: vx, y: vy, w: vw, h: vh }};

        function round(n){{ return Math.round(n*1000)/1000; }}
        function applyViewBox(){{
          // Round values to avoid micro-blur from sub-pixel jitter
          svg.setAttribute('viewBox', round(vx) + ' ' + round(vy) + ' ' + round(vw) + ' ' + round(vh));
          updateStatus();
        }}

        // Ensure note labels are vertically centered within their rectangles
        function centerNotes(){{
          const rects = svg.querySelectorAll('g[class*="note"] rect, rect.note, rect[class*="note"]');
          rects.forEach((rect) => {{
            const rb = rect.getBBox();
            const cy = rb.y + rb.height / 2;
            const g = rect.parentNode;
            if (!g) return;
            const texts = g.querySelectorAll('text');
            texts.forEach((t) => {{
              const tb = t.getBBox();
              const currentCenter = tb.y + tb.height / 2;
              const curY = parseFloat(t.getAttribute('y') || '0');
              const fudge = Math.max(0, tb.height * 0.15); // push a few px down
              const newY = curY + (cy - currentCenter) + fudge;
              t.setAttribute('y', newY);
              t.setAttribute('dominant-baseline', 'middle');
              t.setAttribute('alignment-baseline', 'middle');
              t.setAttribute('dy', '0');
            }});
          }});
        }}

        function getPt(evt){{
          const rect = container.getBoundingClientRect();
          const px = evt.clientX - rect.left;
          const py = evt.clientY - rect.top;
          const sx = px / rect.width; // 0..1
          const sy = py / rect.height;
          const ux = vx + sx * vw; // in viewBox units
          const uy = vy + sy * vh;
          return {{ ux, uy, sx, sy, px, py, rect }};
        }}

        function zoomAt(factor, evt){{
          const {{ ux, uy }} = getPt(evt);
          const newW = vw / factor;
          const newH = vh / factor;
          vx = ux - (ux - vx) * (newW / vw);
          vy = uy - (uy - vy) * (newH / vh);
          vw = newW; vh = newH;
          applyViewBox();
        }}

        // Prevent context menu so right-click can be used for panning
        container.addEventListener('contextmenu', (e) => {{ e.preventDefault(); }});

        // Mouse wheel zoom (low sensitivity)
        container.addEventListener('wheel', (e) => {{
          e.preventDefault();
          // Map deltaY to an exponential scale factor with low sensitivity.
          // Smaller step -> less sensitive zoom; tuned for trackpads and wheels.
          const step = 0.0007; // doubled sensitivity from 0.00035
          const factor = Math.exp(-e.deltaY * step);
          zoomAt(factor, e);
        }}, {{ passive: false }});

        // Drag to pan
        let dragging = false, startX = 0, startY = 0, svx = 0, svy = 0;
        container.addEventListener('pointerdown', (e) => {{
          const isMouse = e.pointerType === 'mouse';
          const isLeft = e.button === 0;
          const isRight = e.button === 2;
          if (!isMouse || isLeft || isRight) {{
            dragging = true; startX = e.clientX; startY = e.clientY; svx = vx; svy = vy;
            container.setPointerCapture(e.pointerId);
          }}
        }});
        container.addEventListener('pointermove', (e) => {{
          if (!dragging) return;
          const rect = container.getBoundingClientRect();
          const dx = e.clientX - startX; const dy = e.clientY - startY;
          vx = svx - dx * (vw / rect.width);
          vy = svy - dy * (vh / rect.height);
          applyViewBox();
        }});
        container.addEventListener('pointerup', () => dragging = false);
        container.addEventListener('pointercancel', () => dragging = false);

        // Toolbar actions
        document.getElementById('zoom_in').onclick = () => {{
          const centerEvt = new MouseEvent('wheel', {{ clientX: container.clientWidth/2, clientY: container.clientHeight/2, deltaY: -1 }});
          zoomAt(1.08, centerEvt); // gentler step
        }};
        document.getElementById('zoom_out').onclick = () => {{
          const centerEvt = new MouseEvent('wheel', {{ clientX: container.clientWidth/2, clientY: container.clientHeight/2, deltaY: 1 }});
          zoomAt(1/1.08, centerEvt);
        }};
        function fit(){{
          // Restore initial viewBox captured on load
          vx = init.x; vy = init.y; vw = init.w; vh = init.h;
          applyViewBox();
        }}
        document.getElementById('fit').onclick = fit;
        window.addEventListener('keydown', (e) => {{ if (e.key === 'f' || e.key === 'F') fit(); }});

        const status = document.getElementById('status');
        function updateStatus(){{
          // Compute zoom percent relative to initial width
          const percent = (init.w / vw) * 100;
          status.textContent = percent.toFixed(0) + '%';
        }}
        centerNotes();
        applyViewBox();
      }})();
    </script>
  </body>
 </html>
        "#,
        bg = bg,
        text = text,
        stroke = stroke,
        svg_b64 = svg_b64
    )
}

fn build_html_from_mermaid(code: &str) -> String {
    // Use only colors from expo/constants/theme.ts
    let bg = "#08090a"; // background
    let text = "#f7f8f8"; // foreground
    let border = "#23252a"; // border
    let tertiary = "#8a8f98"; // tertiary (muted grey)
    let quaternary = "#62666d"; // quaternary
    let sidebar_bg = "#0e0e12"; // sidebarBackground

    const BERKELEY_MONO_REGULAR: &[u8] = include_bytes!(
        "../../../expo/assets/fonts/BerkeleyMono-Regular.ttf"
    );
    let font_b64 = BASE64.encode(BERKELEY_MONO_REGULAR);

    // Escape closing </script> sequences if any (extremely unlikely in Mermaid text)
    let safe_code = code.replace("</script>", r"<\/script>");

    // Vendor mermaid.js and embed via data URL to avoid CDN
    const MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid.min.js");
    let mermaid_js_b64 = BASE64.encode(MERMAID_JS);

    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Mermaid Viewer</title>
    <style>
      @font-face {{
        font-family: 'Berkeley Mono Viewer';
        src: url('data:font/ttf;base64,{font_b64}') format('truetype');
        font-weight: 400;
        font-style: normal;
        font-display: swap;
      }}
      :root {{ --bg: {bg}; --fg: {text}; --stroke: {tertiary}; }}
      html, body {{
        margin: 0; padding: 0; height: 100%; width: 100%;
        background: var(--bg); color: var(--fg);
        font-family: 'Berkeley Mono Viewer', 'Berkeley Mono', ui-monospace, Menlo, Consolas, monospace;
        -webkit-font-smoothing: antialiased; text-rendering: optimizeLegibility;
      }}
      #toolbar {{
        position: fixed; top: 12px; right: 12px; z-index: 10;
        background: rgba(20,22,24,0.7);
        border: 1px solid #2a2d31; border-radius: 6px;
        padding: 6px 8px; display: flex; gap: 6px; align-items: center;
        box-shadow: 0 4px 10px rgba(0,0,0,0.25);
      }}
      #toolbar button {{ background: #14181b; color: var(--fg); border: 1px solid #2a2d31; border-radius: 4px; padding: 4px 8px; cursor: pointer; }}
      #toolbar button:hover {{ border-color: #3b4046; }}
      #view {{ position: absolute; inset: 0; overflow: hidden; }}
      .mermaid {{ height: 100%; width: 100%; display: block; background: var(--bg); }}
      .mermaid, .mermaid * {{ -webkit-user-select: none !important; user-select: none !important; -webkit-user-drag: none; }}
      .mermaid svg {{ height: 100%; width: 100%; display: block; background: var(--bg); text-rendering: geometricPrecision; -webkit-font-smoothing: antialiased; }}
      .mermaid svg *, .mermaid svg text {{ pointer-events: none; }}
      .mermaid svg text {{ fill: var(--fg) !important; font-family: 'Berkeley Mono Viewer', 'Berkeley Mono', ui-monospace, Menlo, monospace !important; font-variant-ligatures: none; font-synthesis: none; text-rendering: geometricPrecision; }}
      .mermaid svg rect, .mermaid svg path, .mermaid svg line, .mermaid svg circle, .mermaid svg ellipse, .mermaid svg polygon {{ stroke: var(--stroke); vector-effect: non-scaling-stroke; shape-rendering: geometricPrecision; }}
    </style>
  </head>
  <body>
    <div id="toolbar">
      <button id="fit">Fit</button>
      <button id="zoom_in">+</button>
      <button id="zoom_out">-</button>
      <span id="status" style="opacity:.8;font-size:12px;">100%</span>
    </div>
    <div id="view">
      <div id="mermaid" class="mermaid">{code}</div>
    </div>
    <script src="data:text/javascript;base64,{mermaid_js_b64}"></script>
    <script>
      (function(){{
        // Block any selection attempts globally
        document.addEventListener('selectstart', (e) => e.preventDefault());
        document.addEventListener('dragstart', (e) => e.preventDefault());
        const container = document.getElementById('view');
        const mroot = document.getElementById('mermaid');
        if (!mroot) return;

        // Prevent context menu so right-click can be used for panning
        container.addEventListener('contextmenu', (e) => {{ e.preventDefault(); }});

        mermaid.initialize({{
          startOnLoad: true,
          securityLevel: 'loose',
          theme: 'dark',
          flowchart: {{ htmlLabels: true }},
          themeVariables: {{
            background: '{bg}',
            primaryColor: '{bg}',
            primaryTextColor: '{text}',
            fontFamily: 'Berkeley Mono Viewer, Berkeley Mono, ui-monospace, Menlo, monospace',

            lineColor: '{tertiary}',
            signalColor: '{tertiary}',
            sequenceMessageArrowColor: '{tertiary}',

            actorTextColor: '{text}',
            actorBorder: '{quaternary}',
            actorBkg: '{bg}',

            noteTextColor: '{text}',
            noteBkgColor: '{sidebar_bg}',
            noteBorderColor: '{border}',

            activationBorderColor: '{border}',
            activationBkgColor: '{sidebar_bg}',

            sequenceNumberColor: '{tertiary}',
            altBackground: '{sidebar_bg}',

            labelBoxBkgColor: '{bg}',
            labelBoxBorderColor: '{quaternary}',
            loopTextColor: '{tertiary}',
          }},
        }});

        function afterRender(){{
          // Attach pan/zoom to the generated SVG
          const svg = container.querySelector('svg');
          if (!svg) return;

          function ensureViewBox(){{
            const vb = svg.viewBox.baseVal;
            if (vb && (vb.width > 0 && vb.height > 0)) return;
            const w = svg.getBBox().width || container.clientWidth || 1024;
            const h = svg.getBBox().height || container.clientHeight || 768;
            svg.setAttribute('viewBox', '0 0 ' + w + ' ' + h);
          }}
          ensureViewBox();

          let vx = svg.viewBox.baseVal.x;
          let vy = svg.viewBox.baseVal.y;
          let vw = svg.viewBox.baseVal.width || container.clientWidth || 1024;
          let vh = svg.viewBox.baseVal.height || container.clientHeight || 768;
          const init = {{ x: vx, y: vy, w: vw, h: vh }};
          function round(n){{ return Math.round(n*1000)/1000; }}
          function updateStatus(){{
            const percent = (init.w / vw) * 100;
            document.getElementById('status').textContent = percent.toFixed(0) + '%';
          }}
          function applyViewBox(){{
            svg.setAttribute('viewBox', round(vx) + ' ' + round(vy) + ' ' + round(vw) + ' ' + round(vh));
            updateStatus();
          }}
          function centerNotes(){{
            const rects = svg.querySelectorAll('g[class*="note"] rect, rect.note, rect[class*="note"]');
            rects.forEach((rect) => {{
              const rb = rect.getBBox();
              const cy = rb.y + rb.height / 2;
              const g = rect.parentNode;
              if (!g) return;
              const texts = g.querySelectorAll('text');
              texts.forEach((t) => {{
                const tb = t.getBBox();
                const currentCenter = tb.y + tb.height / 2;
                const curY = parseFloat(t.getAttribute('y') || '0');
                const fudge = Math.max(0, tb.height * 0.15);
                const newY = curY + (cy - currentCenter) + fudge;
                t.setAttribute('y', newY);
                t.setAttribute('dominant-baseline', 'middle');
                t.setAttribute('alignment-baseline', 'middle');
                t.setAttribute('dy', '0');
              }});
            }});
          }}
          function getPt(evt){{
            const rect = container.getBoundingClientRect();
            const px = evt.clientX - rect.left; const py = evt.clientY - rect.top;
            const sx = px / rect.width; const sy = py / rect.height;
            const ux = vx + sx * vw; const uy = vy + sy * vh;
            return {{ ux, uy, sx, sy, px, py, rect }};
          }}
          function zoomAt(factor, evt){{
            const {{ ux, uy }} = getPt(evt);
            const newW = vw / factor; const newH = vh / factor;
            vx = ux - (ux - vx) * (newW / vw);
            vy = uy - (uy - vy) * (newH / vh);
            vw = newW; vh = newH; applyViewBox();
          }}
          container.addEventListener('wheel', (e) => {{ e.preventDefault(); const step = 0.0007; const factor = Math.exp(-e.deltaY * step); zoomAt(factor, e); }}, {{ passive: false }});
          let dragging=false,startX=0,startY=0,svx=0,svy=0;
          container.addEventListener('pointerdown', (e)=>{{
            const isMouse = e.pointerType === 'mouse';
            const isLeft = e.button === 0;
            const isRight = e.button === 2;
            if (!isMouse || isLeft || isRight) {{
              dragging=true; startX=e.clientX; startY=e.clientY; svx=vx; svy=vy; container.setPointerCapture(e.pointerId);
            }}
          }});
          container.addEventListener('pointermove', (e)=>{{ if(!dragging) return; const rect=container.getBoundingClientRect(); const dx=e.clientX-startX; const dy=e.clientY-startY; vx=svx - dx*(vw/rect.width); vy=svy - dy*(vh/rect.height); applyViewBox(); }});
          container.addEventListener('pointerup', ()=> dragging=false);
          container.addEventListener('pointercancel', ()=> dragging=false);
          document.getElementById('zoom_in').onclick = () => {{ const e = new MouseEvent('wheel', {{ clientX: container.clientWidth/2, clientY: container.clientHeight/2, deltaY: -1 }}); zoomAt(1.08, e); }};
          document.getElementById('zoom_out').onclick = () => {{ const e = new MouseEvent('wheel', {{ clientX: container.clientWidth/2, clientY: container.clientHeight/2, deltaY: 1 }}); zoomAt(1/1.08, e); }};
          document.getElementById('fit').onclick = () => {{ vx=init.x; vy=init.y; vw=init.w; vh=init.h; centerNotes(); applyViewBox(); }};
          window.addEventListener('keydown',(e)=>{{ if(e.key==='f'||e.key==='F') document.getElementById('fit').click(); }});
          centerNotes();
          applyViewBox();
        }}

        // Mermaid renders async; wait for it then attach handlers
        if (typeof mermaid !== 'undefined' && mermaid.run) {{
          mermaid.run({{ querySelector: '#mermaid' }}).then(afterRender).catch(afterRender);
        }} else {{
          // Fallback: attach after a short delay
          setTimeout(afterRender, 200);
        }}
      }})();
    </script>
  </body>
 </html>
        "#,
        font_b64 = font_b64,
        bg = bg,
        text = text,
        border = border,
        tertiary = tertiary,
        quaternary = quaternary,
        sidebar_bg = sidebar_bg,
        code = safe_code,
        mermaid_js_b64 = mermaid_js_b64,
    )
}

fn build_html_docs_index(docs_json: &str) -> String {
    // Dark theme palette
    let bg = "#08090a";
    let text = "#f7f8f8";
    let border = "#23252a";
    let tertiary = "#8a8f98";
    let quaternary = "#62666d";
    let sidebar_bg = "#0e0e12";

    const BERKELEY_MONO_REGULAR: &[u8] = include_bytes!(
        "../../../expo/assets/fonts/BerkeleyMono-Regular.ttf"
    );
    let font_b64 = BASE64.encode(BERKELEY_MONO_REGULAR);
    const MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid.min.js");
    let mermaid_js_b64 = BASE64.encode(MERMAID_JS);

    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Mermaid Viewer</title>
    <style>
      *, *::before, *::after {{ -webkit-user-select: none; user-select: none; -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent; }}
      @font-face {{ font-family: 'Berkeley Mono Viewer'; src: url('data:font/ttf;base64,{font_b64}') format('truetype'); font-weight: 400; font-style: normal; font-display: swap; }}
      :root {{ --bg:{bg}; --fg:{text}; --border:{border}; --muted:{tertiary}; --q:{quaternary}; --sidebar:{sidebar_bg}; }}
      html, body {{ margin:0; height:100%; background:var(--bg); color:var(--fg); font-family:'Berkeley Mono Viewer', ui-monospace, Menlo, monospace; }}
      #shell {{ display:grid; grid-template-columns: 280px 1fr; height:100%; width:100%; }}
      #nav {{ background:var(--sidebar); border-right:1px solid var(--border); overflow:auto; }}
      #nav h1 {{ font-size:14px; margin:12px; color:var(--muted); }}
      #files {{ display:flex; flex-direction:column; gap:6px; padding:0 8px 16px 8px; }}
      #files button {{ text-align:left; padding:8px 10px; background:#14181b; color:var(--fg); border:1px solid #2a2d31; border-radius:6px; cursor:pointer; }}
      #files button.active {{ border-color:#3b4046; background:#181c20; }}
      #panel {{ position:relative; overflow:hidden; }}
      #toolbar {{ position:absolute; top:12px; right:12px; z-index:10; background:rgba(20,22,24,.7); border:1px solid #2a2d31; border-radius:6px; padding:6px 8px; display:flex; gap:6px; align-items:center; }}
      #toolbar button {{ background:#14181b; color:var(--fg); border:1px solid #2a2d31; border-radius:4px; padding:4px 8px; cursor:pointer; }}
      #view {{ position:absolute; inset:0; overflow:hidden; }}
      .mermaid {{ height:100%; width:100%; display:block; background:var(--bg); }}
      .mermaid svg {{ height:100%; width:100%; display:block; background:var(--bg); text-rendering:geometricPrecision; -webkit-font-smoothing:antialiased; }}
      .mermaid svg *, .mermaid svg text {{ pointer-events:none; }}
      .mermaid svg text {{ fill:var(--fg) !important; font-family:'Berkeley Mono Viewer', ui-monospace, Menlo, monospace !important; font-variant-ligatures:none; font-synthesis:none; text-rendering:geometricPrecision; }}
      .mermaid svg rect, .mermaid svg path, .mermaid svg line, .mermaid svg circle, .mermaid svg ellipse, .mermaid svg polygon {{ stroke: {tertiary}; vector-effect: non-scaling-stroke; shape-rendering: geometricPrecision; }}
      svg {{ height:100%; width:100%; display:block; background:var(--bg); text-rendering:geometricPrecision; -webkit-font-smoothing:antialiased; }}
      svg, svg * {{ -webkit-user-select:none !important; user-select:none !important; -webkit-user-drag:none; }}
      svg *, svg text {{ pointer-events:none; }}
    </style>
  </head>
  <body>
    <div id="shell">
      <aside id="nav">
        <h1>Tinyvex Diagrams</h1>
        <div id="files"></div>
      </aside>
      <main id="panel">
        <div id="toolbar">
          <button id="fit">Fit</button>
          <button id="zoom_in">+</button>
          <button id="zoom_out">-</button>
          <span id="status" style="opacity:.8;font-size:12px;">100%</span>
        </div>
        <div id="view"><div id="mermaid" class="mermaid"></div></div>
      </main>
    </div>
    <script>window.__DOCS__ = {docs};</script>
    <script src="data:text/javascript;base64,{mermaid_js_b64}"></script>
    <script>
      (function(){{
        const container = document.getElementById('view');
        const nav = document.getElementById('files');
        const list = Array.isArray(window.__DOCS__) ? window.__DOCS__ : (window.__DOCS__?.docs || []);
        let currentIndex = -1;

        // Build nav
        list.forEach((d, i) => {{
          const b = document.createElement('button');
          b.textContent = d.name || ('doc ' + (i+1));
          b.onclick = () => loadDoc(i);
          nav.appendChild(b);
        }});

        function setActive(i){{
          [...nav.querySelectorAll('button')].forEach((el, idx)=>{{ el.classList.toggle('active', idx===i); }});
        }}

        function ensureViewBox(svg){{
          const vb = svg.viewBox.baseVal; if (vb && vb.width>0 && vb.height>0) return;
          const b = svg.getBBox(); svg.setAttribute('viewBox', '0 0 ' + (b.width||1024) + ' ' + (b.height||768));
        }}

        function centerNotes(svg){{
          const rects = svg.querySelectorAll('g[class*="note"] rect, rect.note, rect[class*="note"]');
          rects.forEach((rect)=>{{
            const rb = rect.getBBox(); const cy = rb.y + rb.height/2; const g = rect.parentNode; if(!g) return;
            const texts = g.querySelectorAll('text');
            texts.forEach((t)=>{{ const tb = t.getBBox(); const cc = tb.y + tb.height/2; const y0 = parseFloat(t.getAttribute('y')||'0'); const fudge = Math.max(0, tb.height*0.15); t.setAttribute('y', y0 + (cy-cc) + fudge); t.setAttribute('dominant-baseline','middle'); t.setAttribute('alignment-baseline','middle'); t.setAttribute('dy','0'); }});
          }});
        }}

        function attachPanZoom(svg){{
          ensureViewBox(svg);
          // State
          let vx = svg.viewBox.baseVal.x, vy = svg.viewBox.baseVal.y,
              vw = svg.viewBox.baseVal.width || container.clientWidth || 1024,
              vh = svg.viewBox.baseVal.height || container.clientHeight || 768;
          const init = {{ x:vx, y:vy, w:vw, h:vh }};
          function round(n){{ return Math.round(n*1000)/1000; }}
          function updateStatus(){{ const percent = (init.w / vw) * 100; document.getElementById('status').textContent = percent.toFixed(0) + '%'; }}
          function apply(){{ svg.setAttribute('viewBox', round(vx)+' '+round(vy)+' '+round(vw)+' '+round(vh)); updateStatus(); }}
          function getPt(evt){{ const r = container.getBoundingClientRect(); const px=evt.clientX-r.left, py=evt.clientY-r.top; const sx=px/r.width, sy=py/r.height; const ux=vx+sx*vw, uy=vy+sy*vh; return {{ux,uy,sx,sy,px,py,rect:r}}; }}
          function zoomAt(f, evt){{ const {{ux,uy}}=getPt(evt); const nw=vw/f, nh=vh/f; vx = ux - (ux - vx) * (nw / vw); vy = uy - (uy - vy) * (nh / vh); vw=nw; vh=nh; apply(); }}
          container.addEventListener('contextmenu', (e)=>e.preventDefault());
          container.addEventListener('wheel', (e)=>{{ e.preventDefault(); const step=0.0007; const f=Math.exp(-e.deltaY*step); zoomAt(f,e); }}, {{passive:false}});
          let dragging=false, sx=0, sy=0, svx=0, svy=0;
          container.addEventListener('pointerdown', (e)=>{{ const isMouse=e.pointerType==='mouse'; const left=e.button===0, right=e.button===2; if(!isMouse||left||right){{ dragging=true; sx=e.clientX; sy=e.clientY; svx=vx; svy=vy; container.setPointerCapture(e.pointerId); }} }});
          container.addEventListener('pointermove', (e)=>{{ if(!dragging) return; const r=container.getBoundingClientRect(); const dx=e.clientX-sx, dy=e.clientY-sy; vx=svx - dx*(vw/r.width); vy=svy - dy*(vh/r.height); apply(); }});
          container.addEventListener('pointerup', ()=> dragging=false);
          container.addEventListener('pointercancel', ()=> dragging=false);
          document.getElementById('zoom_in').onclick = ()=> {{ const e=new MouseEvent('wheel', {{clientX:container.clientWidth/2, clientY:container.clientHeight/2, deltaY:-1}}); zoomAt(1.08,e); }};
          document.getElementById('zoom_out').onclick= ()=> {{ const e=new MouseEvent('wheel', {{clientX:container.clientWidth/2, clientY:container.clientHeight/2, deltaY:1}});  zoomAt(1/1.08,e); }};
          document.getElementById('fit').onclick = ()=> {{ vx=init.x; vy=init.y; vw=init.w; vh=init.h; centerNotes(svg); apply(); }};
          window.addEventListener('keydown', (e)=>{{ if(e.key==='f'||e.key==='F') document.getElementById('fit').click(); }});
          centerNotes(svg);
          apply();
        }}

        function loadDoc(i){{
          if (i===currentIndex) return; currentIndex = i; setActive(i);
          const d = list[i]; if (!d) return;
          const view = document.getElementById('view');
          view.innerHTML = '<div id="mermaid" class="mermaid"></div>';
          if (String(d.kind||'mermaid') === 'svg' || String(d.content||'').trim().startsWith('<svg')) {{
            view.innerHTML = d.content;
            const svg = view.querySelector('svg'); if (svg) attachPanZoom(svg);
          }} else {{
            const mroot = document.getElementById('mermaid');
            // Normalize code: trim BOM/whitespace; drop any leading noise before keyword
            (function(){{
              const txt = String(d.content||'').replace(/^\uFEFF/, '');
              const re = /(flowchart|sequenceDiagram|erDiagram|classDiagram|stateDiagram|gantt|pie|journey)\b/;
              const m = re.exec(txt);
              mroot.textContent = m ? txt.slice(m.index) : txt;
            }})();
            mermaid.initialize({{ startOnLoad:false, securityLevel:'loose', theme:'dark', flowchart: {{ htmlLabels: true }}, themeVariables: {{ background:'{bg}', primaryColor:'{bg}', primaryTextColor:'{text}', lineColor:'{tertiary}', actorTextColor:'{text}', actorBorder:'{quaternary}', actorBkg:'{bg}', noteTextColor:'{text}', noteBkgColor:'{sidebar_bg}', noteBorderColor:'{border}', activationBorderColor:'{border}', activationBkgColor:'{sidebar_bg}', sequenceNumberColor:'{tertiary}', altBackground:'{sidebar_bg}', labelBoxBkgColor:'{bg}', labelBoxBorderColor:'{quaternary}', loopTextColor:'{tertiary}', fontFamily:'Berkeley Mono Viewer, ui-monospace, Menlo, monospace' }} }});
            mermaid.run({{ querySelector:'#mermaid' }}).then(()=>{{ const svg = view.querySelector('svg'); if (svg) attachPanZoom(svg); }}).catch(()=>{{ const svg = view.querySelector('svg'); if (svg) attachPanZoom(svg); }});
          }}
        }}

        // Populate and load first doc if present
        if (list.length) loadDoc(0);
      }})();
    </script>
  </body>
 </html>
        "#,
        font_b64 = font_b64,
        bg = bg,
        text = text,
        border = border,
        tertiary = tertiary,
        quaternary = quaternary,
        sidebar_bg = sidebar_bg,
        mermaid_js_b64 = mermaid_js_b64,
        docs = docs_json,
    )
}
