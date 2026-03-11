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
    html, body { min-height: 100vh; font-family: 'Berkeley Mono', monospace; }
    body {
      background: #000;
      color: #fff;
      display: flex;
      align-items: center;
      justify-content: center;
    }
  </style>
</head>
<body>
  <span>OpenAgents</span>
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
