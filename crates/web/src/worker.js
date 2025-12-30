// Cloudflare Worker for WGPUI Web Demo
// Adds required headers for WebGPU and SharedArrayBuffer support

export default {
  async fetch(request, env, ctx) {
    // Fetch the asset from the static assets binding
    const response = await env.ASSETS.fetch(request);

    // Clone response to modify headers
    const newResponse = new Response(response.body, response);

    // Add COOP/COEP headers required for SharedArrayBuffer (WebGPU threading)
    newResponse.headers.set('Cross-Origin-Opener-Policy', 'same-origin');
    newResponse.headers.set('Cross-Origin-Embedder-Policy', 'require-corp');

    // Set correct MIME types
    const url = new URL(request.url);
    if (url.pathname.endsWith('.wasm')) {
      newResponse.headers.set('Content-Type', 'application/wasm');
    } else if (url.pathname.endsWith('.js')) {
      newResponse.headers.set('Content-Type', 'application/javascript');
    }

    // Cache immutable assets aggressively
    if (url.pathname.startsWith('/pkg/')) {
      newResponse.headers.set('Cache-Control', 'public, max-age=31536000, immutable');
    }

    return newResponse;
  },
};
