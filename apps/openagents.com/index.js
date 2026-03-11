const HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>OpenAgents</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { min-height: 100vh; }
    body {
      background: #000;
      color: #fff;
      font-family: monospace;
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
  async fetch() {
    return new Response(HTML, {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  },
};
