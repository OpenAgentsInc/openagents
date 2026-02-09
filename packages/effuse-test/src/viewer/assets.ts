import { FLOW_STYLES_CSS } from "../../../effuse-flow/src/index.ts"

export const VIEWER_HTML = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Effuse Test Runner</title>
    <style>
      ${FLOW_STYLES_CSS}

      :root {
        --bg: #0b0f17;
        --panel: #121a28;
        --text: #e6edf7;
        --muted: #9db0cc;
        --stroke: rgba(255, 255, 255, 0.12);
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
        overflow: hidden;
      }

      #graph-root {
        width: 100%;
        height: 100%;
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
      <section class="graph" id="graph">
        <div id="graph-root"></div>
      </section>
    </main>
    <script type="module" src="/viewer.js"></script>
  </body>
</html>
`
