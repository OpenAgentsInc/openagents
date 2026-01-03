// Simple dev server for WGPUI web demo
// Run with: bun serve.ts

const mimeTypes: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.json': 'application/json',
};

const PORT = parseInt(process.env.PORT || '3000');
const GGUF_ROUTE = '/gpt-oss-20b-Q8_0.gguf';
const GGUF_PATH = '../ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf';
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

    if (path === '/') {
      path = '/index.html';
    }

    // Map requests to files
    let filePath: string;
    if (path.startsWith('/pkg/')) {
      filePath = `./pkg${path.slice(4)}`;
    } else {
      filePath = `.${path}`;
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
