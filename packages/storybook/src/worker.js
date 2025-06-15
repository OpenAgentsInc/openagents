import { getAssetFromKV } from '@cloudflare/kv-asset-handler'

// Export a default object containing event handlers
export default {
  // The fetch handler is invoked when this worker receives a HTTP(S) request
  // and should return a Response (optionally wrapped in a Promise)
  async fetch(request, env, ctx) {
    try {
      // Add logic to decide whether to serve an asset or run your original worker code
      const url = new URL(request.url)
      
      // Serve index.html for root path
      if (url.pathname === '/') {
        url.pathname = '/index.html'
      }
      
      // Attempt to serve a static asset
      return await getAssetFromKV(
        {
          request,
          waitUntil: ctx.waitUntil.bind(ctx),
        },
        {
          ASSET_NAMESPACE: env.__STATIC_CONTENT,
          ASSET_MANIFEST: JSON.parse(env.__STATIC_CONTENT_MANIFEST),
        }
      )
    } catch (e) {
      // If an error is thrown try to serve a 404.html file
      try {
        const notFoundResponse = await getAssetFromKV(
          {
            request: new Request(`${new URL(request.url).origin}/404.html`),
            waitUntil: ctx.waitUntil.bind(ctx),
          },
          {
            ASSET_NAMESPACE: env.__STATIC_CONTENT,
            ASSET_MANIFEST: JSON.parse(env.__STATIC_CONTENT_MANIFEST),
          }
        )
        return new Response(notFoundResponse.body, {
          ...notFoundResponse,
          status: 404,
        })
      } catch {
        // If no 404.html is found, return a simple 404
        return new Response('Not Found', { status: 404 })
      }
    }
  },
}