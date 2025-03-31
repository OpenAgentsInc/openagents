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
};

const handleFetch = (app: Hono<any>) => {
    ipcMain.handle('electron-fetch', async (event, { input, init }: { input: string | URL | Request; init?: RequestInit }): Promise<FetchReturnValue> => {
        console.log('[IPC Fetch Handler] Request:', input, init);
        try {
            // Hono's app.request expects a URL string or Request object
            const request = new Request(input, init);
            const response = await app.request(request);

            const responseHeaders: Record<string, string> = {};
            response.headers.forEach((value, key) => {
                responseHeaders[key] = value;
            });

            // Important: Read the body *once*
            const body = await response.text();

            console.log('[IPC Fetch Handler] Response Status:', response.status);

            return {
                status: response.status,
                statusText: response.statusText,
                headers: responseHeaders,
                body: body, // Send the consumed body text
                ok: response.ok,
                redirected: response.redirected,
                type: response.type,
                url: response.url,
            };
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