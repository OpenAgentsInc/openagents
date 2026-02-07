export const VIEWER_HTML = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Effuse Test Runner</title>
    <style>
      :root {
        --bg: #0b0f17;
        --panel: #121a28;
        --text: #e6edf7;
        --muted: #9db0cc;
        --ok: #3ddc97;
        --bad: #ff5c7a;
        --stroke: rgba(255, 255, 255, 0.12);
        --node: rgba(255, 255, 255, 0.06);
        --node2: rgba(255, 255, 255, 0.09);
      }
      html,
      body {
        height: 100%;
        margin: 0;
        background: var(--bg);
        color: var(--text);
        font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial;
      }
      header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 10px 14px;
        border-bottom: 1px solid var(--stroke);
        background: linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent);
      }
      header .title {
        font-weight: 700;
        letter-spacing: 0.2px;
      }
      header .status {
        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
        color: var(--muted);
      }
      main {
        display: grid;
        grid-template-columns: 420px 1fr;
        height: calc(100% - 48px);
      }
      .events {
        border-right: 1px solid var(--stroke);
        background: var(--panel);
        overflow: auto;
      }
      .events .row {
        padding: 10px 12px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        font-size: 12px;
        line-height: 1.35;
      }
      .events .row .type {
        font-weight: 700;
      }
      .events .row .meta {
        color: var(--muted);
        margin-top: 2px;
        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
      }
      .graph {
        position: relative;
        overflow: auto;
      }
      svg {
        width: 1600px;
        height: 1200px;
      }
      .node rect {
        fill: var(--node);
        stroke: var(--stroke);
      }
      .node.ok rect {
        stroke: rgba(61, 220, 151, 0.45);
      }
      .node.bad rect {
        stroke: rgba(255, 92, 122, 0.55);
      }
      .node text {
        fill: var(--text);
        font-size: 12px;
        dominant-baseline: middle;
      }
      .node .kind {
        fill: var(--muted);
        font-size: 11px;
      }
      .edge {
        stroke: rgba(255, 255, 255, 0.16);
        stroke-width: 1.5;
        fill: none;
      }
      @media (max-width: 900px) {
        main {
          grid-template-columns: 1fr;
        }
        .events {
          display: none;
        }
      }
    </style>
  </head>
  <body>
    <header>
      <div class="title">Effuse Test Runner</div>
      <div class="status" id="status">connecting...</div>
    </header>
    <main>
      <section class="events" id="events"></section>
      <section class="graph">
        <svg id="svg" viewBox="0 0 1600 1200" preserveAspectRatio="xMinYMin meet"></svg>
      </section>
    </main>
    <script>
      const statusEl = document.getElementById('status');
      const eventsEl = document.getElementById('events');
      const svgEl = document.getElementById('svg');

      const spans = new Map();

      function escapeHtml(s) {
        return String(s)
          .replaceAll('&', '&amp;')
          .replaceAll('<', '&lt;')
          .replaceAll('>', '&gt;')
          .replaceAll('\"', '&quot;')
          .replaceAll(\"'\", '&#39;');
      }

      function depthOf(span) {
        let d = 0;
        let cur = span;
        while (cur && cur.parentSpanId) {
          const p = spans.get(cur.parentSpanId);
          if (!p) break;
          d += 1;
          cur = p;
          if (d > 20) break;
        }
        return d;
      }

      function renderGraph() {
        const list = Array.from(spans.values()).sort((a, b) => (a.startTs ?? 0) - (b.startTs ?? 0));
        const nodeW = 260;
        const nodeH = 44;
        const xGap = 46;
        const yGap = 20;

        const nodes = list.map((s, i) => {
          const depth = depthOf(s);
          const x = 20 + depth * (nodeW + xGap);
          const y = 20 + i * (nodeH + yGap);
          return { ...s, depth, x, y };
        });

        const nodeById = new Map(nodes.map((n) => [n.spanId, n]));

        let edges = '';
        for (const n of nodes) {
          if (!n.parentSpanId) continue;
          const p = nodeById.get(n.parentSpanId);
          if (!p) continue;
          const x1 = p.x + nodeW;
          const y1 = p.y + nodeH / 2;
          const x2 = n.x;
          const y2 = n.y + nodeH / 2;
          const mx = (x1 + x2) / 2;
          edges += '<path class=\"edge\" d=\"M ' + x1 + ' ' + y1 + ' C ' + mx + ' ' + y1 + ', ' + mx + ' ' + y2 + ', ' + x2 + ' ' + y2 + '\" />';
        }

        let nodesSvg = '';
        for (const n of nodes) {
          const status = n.status === 'failed' ? 'bad' : n.status === 'passed' ? 'ok' : '';
          nodesSvg +=
            '<g class=\"node ' + status + '\">' +
            '<rect x=\"' +
            n.x +
            '\" y=\"' +
            n.y +
            '\" rx=\"10\" ry=\"10\" width=\"' +
            nodeW +
            '\" height=\"' +
            nodeH +
            '\" />' +
            '<text x=\"' +
            (n.x + 12) +
            '\" y=\"' +
            (n.y + 18) +
            '\">' +
            escapeHtml(n.name || n.spanId) +
            '</text>' +
            '<text class=\"kind\" x=\"' +
            (n.x + 12) +
            '\" y=\"' +
            (n.y + 34) +
            '\">' +
            escapeHtml(n.kind || '') +
            '</text>' +
            '</g>';
        }

        svgEl.setAttribute('height', String(Math.max(1200, 40 + nodes.length * (nodeH + yGap))));
        svgEl.innerHTML = edges + nodesSvg;
      }

      function appendEventRow(evt) {
        const div = document.createElement('div');
        div.className = 'row';
        div.innerHTML =
          '<div class=\"type\">' +
          escapeHtml(evt.type) +
          '</div>' +
          '<div class=\"meta\">' +
          escapeHtml(JSON.stringify(evt).slice(0, 260)) +
          '</div>';
        eventsEl.prepend(div);
      }

      function onEvent(evt) {
        appendEventRow(evt);
        if (evt.type === 'span.started') {
          spans.set(evt.spanId, {
            spanId: evt.spanId,
            parentSpanId: evt.parentSpanId,
            name: evt.name,
            kind: evt.kind,
            startTs: evt.ts,
          });
          renderGraph();
        } else if (evt.type === 'span.finished') {
          const cur = spans.get(evt.spanId);
          if (cur) {
            cur.endTs = evt.ts;
            cur.status = evt.status;
            cur.durationMs = evt.durationMs;
            spans.set(evt.spanId, cur);
            renderGraph();
          }
        } else if (evt.type === 'run.finished') {
          statusEl.textContent = evt.status + ' (' + evt.durationMs + 'ms)';
        }
      }

      const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = proto + '//' + location.host + '/ws';
      const ws = new WebSocket(wsUrl);
      ws.addEventListener('open', () => (statusEl.textContent = 'connected'));
      ws.addEventListener('close', () => (statusEl.textContent = 'disconnected'));
      ws.addEventListener('message', (e) => {
        try {
          onEvent(JSON.parse(String(e.data)));
        } catch (err) {
          console.error(err);
        }
      });
    </script>
  </body>
</html>
`

