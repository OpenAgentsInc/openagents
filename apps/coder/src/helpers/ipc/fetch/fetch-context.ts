// apps/coder/src/helpers/ipc/fetch/fetch-context.ts
import { contextBridge, ipcRenderer } from 'electron';
import type { FetchReturnValue } from '../../../server/fetch';

export function exposeFetchContext() {
  console.log('[Preload] Exposing fetch context through contextBridge');
  
  // Get existing electron object if it exists
  const existingElectronApi = (window as any).electron || {};
  console.log('[Preload] Existing electron API keys:', Object.keys(existingElectronApi));
  
  contextBridge.exposeInMainWorld('electron', {
    ...existingElectronApi,
    // Add a test function for basic IPC
    testIpc: async () => {
      console.log('[Preload] Calling test-ipc channel');
      return await ipcRenderer.invoke('test-ipc');
    },
    fetch: async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      console.log('[Preload] electron.fetch called with:', input);
      try {
        // Use the channel name defined in fetch.ts
        const result: FetchReturnValue = await ipcRenderer.invoke('electron-fetch', { input, init });

        // Get headers
        const responseHeaders = new Headers(result.headers);
        
        // Check if this is a streaming response
        if (result.isStream) {
          console.log('[Preload] Processing streaming response');
          
          // For streaming responses, we need to create a special response
          // that emulates a streaming fetch
          
          // Create a ReadableStream that emits events as they come in
          const encoder = new TextEncoder();
          const readableStream = new ReadableStream({
            start(controller) {
              // Set up an event source to listen for server-sent events
              // This is a dummy operation since we can't actually set up
              // a real event source here, but we need to return a valid Response
              
              // Add a dummy chunk to make sure the stream is seen as valid
              controller.enqueue(encoder.encode(''));
            }
          });
          
          // Create a Response with the stream
          const response = new Response(readableStream, {
            status: result.status,
            statusText: result.statusText,
            headers: responseHeaders,
          });
          
          console.log('[Preload] Created streaming Response');
          return response;
        }
        
        // For regular responses
        console.log('[Preload] Processing regular response');
        const responseBody = result.body; // Body is already text

        // Create a Blob from the responseBody to ensure it's properly processed
        const bodyBlob = new Blob([responseBody], { type: responseHeaders.get('content-type') || 'text/plain' });
        
        // Create a proper Response object using the Blob
        const response = new Response(bodyBlob, {
          status: result.status,
          statusText: result.statusText,
          headers: responseHeaders,
        });

        // Define properties directly on the response instance (as Response properties are read-only)
        Object.defineProperty(response, 'ok', { value: result.ok });
        Object.defineProperty(response, 'redirected', { value: result.redirected });
        Object.defineProperty(response, 'type', { value: result.type });
        Object.defineProperty(response, 'url', { value: result.url });
        
        // Log the response for debugging
        console.log('[Preload] Created Response object with status:', response.status);
        console.log('[Preload] Response has text method:', typeof response.text === 'function');

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