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
    * { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { height: 100%; overflow: hidden; font-family: 'Berkeley Mono', monospace; }
    html { height: 100vh; }
    body {
      height: 100vh;
      min-height: 100vh;
      overflow: hidden;
      background: #000;
      color: #fff;
      display: flex;
      align-items: center;
      justify-content: center;
    }
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
      color: oklch(.9 0 0);
    }
    .content .title { font-size: 1.25rem; margin-bottom: 1rem; }
    .content p { font-size: 1rem; line-height: 1.6; }
  </style>
</head>
<body data-centered="true">
  <div class="screen">
    <div class="content">
      <div class="title">OpenAgents</div>
      <p>Sell compute for bitcoin.</p>
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
