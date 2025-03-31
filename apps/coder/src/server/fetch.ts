// apps/coder/src/server/fetch.ts
import { ipcMain } from 'electron';
import type { Hono } from 'hono';

// Define a type for the return value to ensure consistency
export type FetchReturnValue = {
    status: number;
    statusText: string;
    headers: Record<string, string>; // Capture headers
    body: string; // Send body as text initially
    ok: boolean;
    redirected: boolean;
    type: ResponseType;
    url: string;
    isStream?: boolean; // Indicates if this is a streaming response
};

const handleFetch = (app: Hono<any>) => {
    // Log handler registration
    console.log('[IPC Fetch Handler] Registering handler for electron-fetch channel');
    
    // Check if the channel is already registered
    const existingHandlers = ipcMain.listeners('electron-fetch');
    if (existingHandlers.length > 0) {
        console.log('[IPC Fetch Handler] Warning: electron-fetch channel already has handlers registered:', existingHandlers.length);
        ipcMain.removeAllListeners('electron-fetch');
        console.log('[IPC Fetch Handler] Removed existing handlers');
    }
    
    // Add a simple test channel to verify IPC is working at all
    ipcMain.handle('test-ipc', async () => {
        console.log('[IPC Test] Test channel called');
        return { success: true, message: 'IPC is working' };
    });
    console.log('[IPC Test] Registered test-ipc channel');
    
    ipcMain.handle('electron-fetch', async (event, { input, init }: { input: string | URL | Request; init?: RequestInit }): Promise<FetchReturnValue> => {
        console.log('[IPC Fetch Handler] Received request:', input, init);
        try {
            // Convert relative URLs to absolute URLs with a dummy base
            let url: string | URL;
            if (typeof input === 'string' && input.startsWith('/')) {
                // Use a dummy base URL for relative paths
                url = new URL(input, 'http://localhost');
                console.log('[IPC Fetch Handler] Converted relative URL to:', url.toString());
            } else {
                url = input;
            }
            
            // Hono's app.request expects a URL string or Request object
            // Create a clean init object without signal
            const { signal, ...safeInit } = init || {};
            console.log('[IPC Fetch Handler] Removed signal from init object for request constructor');
            
            const request = new Request(url, safeInit);
            const response = await app.request(request);

            const responseHeaders: Record<string, string> = {};
            response.headers.forEach((value, key) => {
                responseHeaders[key] = value;
            });

            console.log('[IPC Fetch Handler] Response Status:', response.status);
            console.log('[IPC Fetch Handler] Response Content-Type:', response.headers.get('content-type'));

            // Check if this is a streaming response (SSE)
            if (response.headers.get('content-type')?.includes('text/event-stream')) {
                console.log('[IPC Fetch Handler] Detected SSE stream response');
                
                // For SSE, we don't want to consume the body as text
                // Return a special indicator that this is a streaming response
                return {
                    status: response.status,
                    statusText: response.statusText,
                    headers: responseHeaders,
                    body: '', // Body will be streamed
                    ok: response.ok,
                    redirected: response.redirected,
                    type: response.type,
                    url: response.url,
                    isStream: true, // Add a flag to indicate this is a stream
                };
            } else {
                // For non-streaming responses, read the body as text
                const body = await response.text();
                
                return {
                    status: response.status,
                    statusText: response.statusText,
                    headers: responseHeaders,
                    body: body, // Send the consumed body text
                    ok: response.ok,
                    redirected: response.redirected,
                    type: response.type,
                    url: response.url,
                    isStream: false,
                };
            }
        } catch (error: any) {
            console.error('[IPC Fetch Handler] Error:', error);
            // Return an error structure that the preload script can handle
            return {
                status: 500,
                statusText: 'Internal Server Error',
                headers: {'content-type': 'application/json'},
                body: JSON.stringify({ error: error.message || 'Failed to handle fetch request' }),
                ok: false,
                redirected: false,
                type: 'basic',
                url: typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url,
            };
        }
    });
    console.log('[IPC Fetch Handler] Registered listener for "electron-fetch".');
};

export default handleFetch;