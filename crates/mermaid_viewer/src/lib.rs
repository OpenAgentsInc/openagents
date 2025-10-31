use anyhow::{anyhow, Result};

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

fn build_html(svg_source: &str) -> String {
    // Inline the SVG into a themed HTML page. We attach listeners to
    // implement viewBox-based zooming and panning for crisp vector scaling.
    // Theme constants
    let bg = "#08090a";
    let text = "#f7f8f8";
    let stroke = "#6c7075"; // muted grey

    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Mermaid Viewer</title>
    <style>
      :root {{
        --bg: {bg};
        --fg: {text};
        --stroke: {stroke};
      }}
      html, body {{
        margin: 0; padding: 0; height: 100%; width: 100%;
        background: var(--bg);
        color: var(--fg);
        font-family: 'Berkeley Mono', ui-monospace, Menlo, Consolas, monospace;
        -webkit-font-smoothing: antialiased; text-rendering: optimizeLegibility;
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
      svg {{ height: 100%; width: 100%; display: block; background: var(--bg); }}
      svg text {{ fill: var(--fg) !important; font-family: 'Berkeley Mono', ui-monospace, Menlo, monospace !important; }}
      svg rect, svg path, svg line, svg circle, svg ellipse, svg polygon {{ stroke: var(--stroke); }}
      * {{ shape-rendering: geometricPrecision; }}
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
      {svg}
    </div>
    <script>
      (function(){{
        const container = document.getElementById('view');
        const svg = container.querySelector('svg');
        if (!svg) return;

        // Ensure viewBox exists
        function ensureViewBox(){{
          const vb = svg.viewBox.baseVal;
          if (vb && (vb.width > 0 && vb.height > 0)) return;
          const w = parseFloat(svg.getAttribute('width')) || container.clientWidth || 1024;
          const h = parseFloat(svg.getAttribute('height')) || container.clientHeight || 768;
          svg.setAttribute('viewBox', '0 0 ' + w + ' ' + h);
        }}
        ensureViewBox();

        // Track viewBox state
        let vx = svg.viewBox.baseVal.x;
        let vy = svg.viewBox.baseVal.y;
        let vw = svg.viewBox.baseVal.width || container.clientWidth || 1024;
        let vh = svg.viewBox.baseVal.height || container.clientHeight || 768;

        function applyViewBox(){{
          svg.setAttribute('viewBox', vx + ' ' + vy + ' ' + vw + ' ' + vh);
          updateStatus();
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

        // Mouse wheel zoom (low sensitivity)
        container.addEventListener('wheel', (e) => {{
          e.preventDefault();
          // Map deltaY to an exponential scale factor with low sensitivity.
          // Smaller step -> less sensitive zoom; tuned for trackpads and wheels.
          const step = 0.00035; // was effectively ~0.095 per notch; now ~0.035
          const factor = Math.exp(-e.deltaY * step);
          zoomAt(factor, e);
        }}, {{ passive: false }});

        // Drag to pan
        let dragging = false, startX = 0, startY = 0, svx = 0, svy = 0;
        container.addEventListener('pointerdown', (e) => {{
          dragging = true; startX = e.clientX; startY = e.clientY; svx = vx; svy = vy;
          container.setPointerCapture(e.pointerId);
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
          // Fit SVG content to container based on initial bounding box or viewBox.
          const vb = svg.viewBox.baseVal;
          vx = vb.x; vy = vb.y; vw = vb.width; vh = vb.height;
          applyViewBox();
        }}
        document.getElementById('fit').onclick = fit;
        window.addEventListener('keydown', (e) => {{ if (e.key === 'f' || e.key === 'F') fit(); }});

        const status = document.getElementById('status');
        function updateStatus(){{
          // Compute zoom percent relative to initial viewBox or SVG width/height
          const vb0 = svg.viewBox.baseVal;
          const w0 = vb0.width; // assume initial = current after first ensure
          const percent = (w0 / vw) * 100;
          status.textContent = percent.toFixed(0) + '%';
        }}
        applyViewBox();
      }})();
    </script>
  </body>
 </html>
        "#,
        bg = bg,
        text = text,
        stroke = stroke,
        svg = svg_source
    )
}
