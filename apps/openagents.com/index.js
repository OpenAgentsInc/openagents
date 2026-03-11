const HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>OpenAgents</title>
  <style>
    @font-face {
      font-family: 'Berkeley Mono';
      src: url('/fonts/BerkeleyMono-Regular.woff2') format('woff2');
      font-weight: 400;
      font-style: normal;
    }
    @font-face {
      font-family: 'Berkeley Mono';
      src: url('/fonts/BerkeleyMono-Italic.woff2') format('woff2');
      font-weight: 400;
      font-style: italic;
    }
    @font-face {
      font-family: 'Berkeley Mono';
      src: url('/fonts/BerkeleyMono-Bold.woff2') format('woff2');
      font-weight: 700;
      font-style: normal;
    }
    @font-face {
      font-family: 'Berkeley Mono';
      src: url('/fonts/BerkeleyMono-BoldItalic.woff2') format('woff2');
      font-weight: 700;
      font-style: italic;
    }
    :root {
      --bg: oklch(.1 0 0);
      --text-primary: oklch(.9 0 0);
      --text-secondary: oklch(.75 0 0);
      --text-tertiary: oklch(.4 0 0);
      --border: oklch(.2 0 0);
      --selection: oklch(.2 0 0);
      --code-bg: oklch(.15 0 0);
      --mark: oklch(.3 .05 90);
      --font-size-m: 1rem;
    }
    * { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { height: 100%; overflow: hidden; font-family: 'Berkeley Mono', monospace; }
    html { height: 100vh; }
    body {
      height: 100vh;
      min-height: 100vh;
      overflow: hidden;
      background: var(--bg);
      color: var(--text-primary);
      display: flex;
      align-items: center;
      justify-content: center;
    }
    ::selection { background: var(--selection); }
    .screen {
      height: 100vh;
      min-height: 100vh;
      width: 100%;
      display: flex;
      align-items: center;
      justify-content: center;
      overflow: hidden;
    }
    .content {
      max-width: 35rem;
      --content-width: 35rem;
      width: 100%;
      padding: 0 1rem;
      text-align: left;
      color: var(--text-primary);
    }
    .content .title { font-size: 1rem; margin-bottom: 1.5rem; color: var(--text-primary); }
    .compute-market-box { border: 1px solid var(--border); border-radius: 0; padding: 1rem; width: max-content; max-width: 35rem; }
    .content .section-header { font-size: 0.75rem; color: var(--text-tertiary); margin-top: 0; margin-bottom: 0.35rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .content p { font-size: 1rem; line-height: 1.6; color: var(--text-secondary); }
    .content .prose > p { margin-bottom: 1.25rem; }
    .content code { background: var(--code-bg); padding: 0.1em 0.3em; }
    .content mark { background: var(--mark); }
    .content .text-tertiary { color: var(--text-tertiary); }
    .prose * { margin: 0; padding: 0; }
    .prose p, .prose li, .prose td, .prose th { font-size: var(--font-size-m); }
    .prose p, .prose li { font-size: var(--font-size-m); }
    .prose ul { list-style-type: disc; list-style-position: outside; padding-left: 1.25em; margin-left: 0; margin-bottom: 0; margin-top: 0; line-height: 1.75; }
    .prose ul li { position: relative; padding-left: 0.25em; margin-bottom: 0.5em; color: var(--text-secondary); }
    .prose ul li:last-child { margin-bottom: 0; }
    @media (prefers-reduced-motion: reduce) {
      * { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; animation-iteration-count: 1 !important; scroll-behavior: auto !important; }
    }
  </style>
</head>
<body data-centered="true">
  <div class="screen">
    <div class="content">
      <div class="title">OpenAgents</div>
      <div class="compute-market-box">
        <div class="section-header">COMPUTE MARKET</div>
        <div class="prose">
          <p>Sell your spare compute for bitcoin.</p>
          <ul>
            <li>Live now for Apple Silicon.</li>
            <li>Coming soon for NVIDIA &amp; AMD.</li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</body>
</html>`;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/" || url.pathname === "") {
      return new Response(HTML, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      });
    }
    return env.ASSETS.fetch(request);
  },
};
