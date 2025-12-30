// Simple dev server for WGPUI web demo
// Run with: bun serve.ts

const mimeTypes: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.json': 'application/json',
};

Bun.serve({
  port: 3000,
  async fetch(req) {
    const url = new URL(req.url);
    let path = url.pathname;

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
          'Cross-Origin-Opener-Policy': 'same-origin',
          'Cross-Origin-Embedder-Policy': 'require-corp',
        },
      });
    }

    return new Response('Not found', { status: 404 });
  },
});

console.log('WGPUI Web Demo server running at http://localhost:3000');
