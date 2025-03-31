// apps/coder/src/helpers/ipc/fetch/fetch-context.ts
import { contextBridge, ipcRenderer } from 'electron';
import type { FetchReturnValue } from '../../../server/fetch';

export function exposeFetchContext() {
  contextBridge.exposeInMainWorld('electron', {
    ...((window as any).electron || {}),
    fetch: async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      try {
        // Use the channel name defined in fetch.ts
        const result: FetchReturnValue = await ipcRenderer.invoke('electron-fetch', { input, init });

        // Reconstruct a Response object for the renderer
        const responseBody = result.body; // Body is already text
        const responseHeaders = new Headers(result.headers);

        const response = new Response(responseBody, {
          status: result.status,
          statusText: result.statusText,
          headers: responseHeaders,
        });

        // Define properties directly on the response instance (as Response properties are read-only)
        Object.defineProperty(response, 'ok', { value: result.ok });
        Object.defineProperty(response, 'redirected', { value: result.redirected });
        Object.defineProperty(response, 'type', { value: result.type });
        Object.defineProperty(response, 'url', { value: result.url });

        console.log('[Preload] Reconstructed Response:', response.status);
        return response;
      } catch (error: any) {
        console.error('[Preload] Error invoking electron-fetch:', error);
        // Create a synthetic error response
        return new Response(JSON.stringify({ error: error.message || 'IPC fetch failed' }), {
          status: 500,
          statusText: 'IPC Error',
          headers: { 'Content-Type': 'application/json' }
        });
      }
    }
  });

  console.log('[Preload] Exposed electron.fetch');
}