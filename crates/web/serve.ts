// Simple dev server for WGPUI web demo
// Run with: bun serve.ts

const mimeTypes: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.json': 'application/json',
};

const ROOT_DIR = import.meta.dir;
const PORT = parseInt(process.env.PORT || '3000');
const GGUF_ROUTE = '/gpt-oss-20b-Q8_0.gguf';
const GGUF_PATH = `${ROOT_DIR}/../ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf`;
const ISOLATION_HEADERS = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};
const RANGE_HEADERS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'Range',
  'Access-Control-Allow-Methods': 'GET,HEAD,OPTIONS',
  'Access-Control-Expose-Headers': 'Content-Range,Accept-Ranges,Content-Length',
  'Accept-Ranges': 'bytes',
};
const GPTOSS_HTML = `<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>GPT-OSS Pipeline Viz - OpenAgents</title>
    <meta name="description" content="GPT-OSS model loading + inference telemetry visualization.">
    <link rel="stylesheet" href="/static/MyWebfontsKit.css">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Vera Mono', 'DejaVu Sans Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
            overflow: hidden;
            width: 100%;
            height: 100%;
        }
        #hud-container {
            width: 100vw;
            height: 100vh;
            overflow: hidden;
        }
        canvas {
            width: 100%;
            height: 100%;
            display: block;
        }
    </style>
</head>
<body>
    <div id="hud-container">
        <canvas id="canvas"></canvas>
    </div>
    <script type="module">
        window.GPTOSS_PAGE = true;

        const params = new URLSearchParams(window.location.search);
        const dataUrl = params.get("data");
        if (dataUrl) {
            window.GPTOSS_DATA_URL = dataUrl;
        }

        import init, { start_demo } from '/pkg/openagents_web_client.js';

        async function run() {
            await init();
            await start_demo('canvas');
        }

        run().catch(console.error);
    </script>
</body>
</html>
`;

async function serveGguf(req: Request): Promise<Response> {
  if (req.method === 'OPTIONS') {
    return new Response(null, { status: 204, headers: { ...RANGE_HEADERS, ...ISOLATION_HEADERS } });
  }

  const file = Bun.file(GGUF_PATH);
  if (!(await file.exists())) {
    return new Response(`GGUF not found at ${GGUF_PATH}`, {
      status: 404,
      headers: { ...RANGE_HEADERS, ...ISOLATION_HEADERS },
    });
  }

  const size = file.size;
  const range = req.headers.get('Range');
  if (range) {
    const match = /^bytes=(\d+)-(\d+)?$/.exec(range);
    if (!match) {
      return new Response('Invalid Range', {
        status: 416,
        headers: {
          ...RANGE_HEADERS,
          ...ISOLATION_HEADERS,
          'Content-Range': `bytes */${size}`,
        },
      });
    }
    const start = Number(match[1]);
    const rawEnd = match[2] ? Number(match[2]) : size - 1;
    if (!Number.isFinite(start) || !Number.isFinite(rawEnd) || start > rawEnd) {
      return new Response('Invalid Range', {
        status: 416,
        headers: {
          ...RANGE_HEADERS,
          ...ISOLATION_HEADERS,
          'Content-Range': `bytes */${size}`,
        },
      });
    }
    if (start >= size) {
      return new Response('Range Not Satisfiable', {
        status: 416,
        headers: {
          ...RANGE_HEADERS,
          ...ISOLATION_HEADERS,
          'Content-Range': `bytes */${size}`,
        },
      });
    }

    const end = Math.min(rawEnd, size - 1);
    const chunk = file.slice(start, end + 1);
    return new Response(chunk, {
      status: 206,
      headers: {
        ...RANGE_HEADERS,
        ...ISOLATION_HEADERS,
        'Content-Type': 'application/octet-stream',
        'Content-Length': `${end - start + 1}`,
        'Content-Range': `bytes ${start}-${end}/${size}`,
      },
    });
  }

  return new Response(file, {
    status: 200,
    headers: {
      ...RANGE_HEADERS,
      ...ISOLATION_HEADERS,
      'Content-Type': 'application/octet-stream',
      'Content-Length': `${size}`,
    },
  });
}

// Kill any process using the port
try {
  await Bun.$`fuser -k ${PORT}/tcp 2>/dev/null`.quiet();
} catch {
  // No process on port, that's fine
}

const server = Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);
    let path = url.pathname;

    if (path === GGUF_ROUTE) {
      return serveGguf(req);
    }

    if (path === '/gptoss' || path === '/gptoss/') {
      return new Response(GPTOSS_HTML, {
        headers: {
          'Content-Type': 'text/html; charset=utf-8',
          'X-Frame-Options': 'SAMEORIGIN',
          ...ISOLATION_HEADERS,
        },
      });
    }

    if (path === '/') {
      path = '/index.html';
    }

    // Map requests to files (anchored to script directory)
    let filePath: string;
    if (path.startsWith('/pkg/')) {
      filePath = `${ROOT_DIR}/pkg${path.slice(4)}`;
    } else {
      filePath = `${ROOT_DIR}${path}`;
    }

    const file = Bun.file(filePath);
    if (await file.exists()) {
      const ext = filePath.match(/\.[^.]+$/)?.[0] || '';
      const contentType = mimeTypes[ext] || 'application/octet-stream';

      return new Response(file, {
        headers: {
          'Content-Type': contentType,
          ...ISOLATION_HEADERS,
        },
      });
    }

    return new Response('Not found', { status: 404 });
  },
});

console.log(`WGPUI Web Demo server running at http://localhost:${server.port}`);
