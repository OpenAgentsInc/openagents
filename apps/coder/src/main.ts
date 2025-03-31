import { app, BrowserWindow } from "electron";
import registerListeners from "./helpers/ipc/listeners-register";
// "electron-squirrel-startup" seems broken when packaging with vite
//import started from "electron-squirrel-startup";
import path from "path";
import {
  installExtension,
  REACT_DEVELOPER_TOOLS,
} from "electron-devtools-installer";
// import { setupElectronCommandExecutor } from "@openagents/core";
import { serverApp } from "./server";
import { createServer } from "node:http"; // Use Node's native HTTP server

const inDevelopment = process.env.NODE_ENV === "development";

// Define a port for the local API server
const LOCAL_API_PORT = 3001; // Or another available port

// Keep track of the server instance for graceful shutdown
let serverInstance: ReturnType<typeof serve> | null = null;

function createWindow() {
  const preload = path.join(__dirname, "preload.js");
  const mainWindow = new BrowserWindow({
    width: 1200,
    height: 950,
    webPreferences: {
      devTools: inDevelopment,
      contextIsolation: true,
      nodeIntegration: true,
      nodeIntegrationInSubFrames: false,
      preload: preload,
    },
    titleBarStyle: "hidden",
  });
  registerListeners(mainWindow);

  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(
      path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`),
    );
  }
}

async function installExtensions() {
  try {
    const result = await installExtension(REACT_DEVELOPER_TOOLS);
    console.log(`Extensions installed successfully: ${result.name}`);
  } catch {
    console.error("Failed to install extensions");
  }
}

// Initialize command execution before anything else
// setupElectronCommandExecutor();
// console.log('✨ Command execution setup complete');

app.whenReady()
  .then(async () => {
    console.log('[Main Process] App is ready.');

    // Start the local HTTP server to handle requests
    try {
      console.log(`[Main Process] Attempting to start local API server on port ${LOCAL_API_PORT}...`);
      
      // Create a simple Node.js HTTP server
      const httpServer = createServer(async (req, res) => {
        console.log(`[Main Process] Received request: ${req.method} ${req.url}`);
        
        // Set CORS headers for all responses
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
        res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, Accept, X-Requested-With');
        res.setHeader('Access-Control-Max-Age', '86400');
        
        // Handle preflight OPTIONS requests
        if (req.method === 'OPTIONS') {
          console.log('[Main Process] Handling OPTIONS preflight request');
          res.writeHead(204);
          res.end();
          return;
        }
        
        // Test endpoint
        if (req.url === '/api/test-cors' && req.method === 'GET') {
          console.log('[Main Process] Handling /api/test-cors request');
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ success: true, message: 'CORS is working!' }));
          return;
        }
        
        // Basic health check
        if (req.url === '/api/ping' && req.method === 'GET') {
          console.log('[Main Process] Handling /api/ping request');
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ message: 'pong' }));
          return;
        }
        
        // Forward to Hono app for all other requests
        try {
          // Create a Request object from the Node.js request
          const chunks: Buffer[] = [];
          
          // Collect the body if there is one
          for await (const chunk of req) {
            chunks.push(Buffer.from(chunk));
          }
          const bodyText = Buffer.concat(chunks).toString('utf-8');
          
          // Prepare headers
          const headers = new Headers();
          for (const [key, value] of Object.entries(req.headers)) {
            if (value) headers.append(key, Array.isArray(value) ? value.join(', ') : value);
          }
          
          // Create a Request object for Hono
          const url = new URL(req.url || '/', `http://localhost:${LOCAL_API_PORT}`);
          const request = new Request(url, {
            method: req.method,
            headers,
            body: bodyText.length > 0 ? bodyText : undefined
          });
          
          // Pass the request to Hono
          console.log(`[Main Process] Forwarding to Hono: ${req.method} ${req.url}`);
          const honoResponse = await serverApp.fetch(request);
          
          // Write the status and headers
          res.writeHead(honoResponse.status, Object.fromEntries(honoResponse.headers.entries()));
          
          // Check if it's a streaming response
          if (honoResponse.headers.get('content-type')?.includes('text/event-stream')) {
            console.log('[Main Process] Handling stream response');
            
            // For streaming responses, pipe the Hono response body to the HTTP response
            const reader = honoResponse.body?.getReader();
            if (reader) {
              try {
                while (true) {
                  const { done, value } = await reader.read();
                  if (done) break;
                  res.write(value);
                }
              } finally {
                reader.releaseLock();
                res.end();
              }
            } else {
              res.end();
            }
          } else {
            // For regular responses, just get the body and send it
            const body = await honoResponse.text();
            res.end(body);
          }
        } catch (error) {
          console.error('[Main Process] Error forwarding to Hono:', error);
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'Internal Server Error' }));
        }
      });
      
      // Start the server
      httpServer.listen(LOCAL_API_PORT, () => {
        console.log(`[Main Process] ✅ Local API server listening on http://localhost:${LOCAL_API_PORT}`);
      });
      
      // Store the server instance
      serverInstance = httpServer;
      
      // Add error handler
      httpServer.on('error', (error) => {
        console.error('[Main Process] Server error:', error);
      });
      
    } catch (error) {
      console.error('[Main Process] ❌ Failed to start local API server:', error);
      console.error('[Main Process] Error stack:', error instanceof Error ? error.stack : 'No stack trace');
      // Log important information for debugging
      console.log('[Main Process] Node version:', process.versions.node);
      console.log('[Main Process] Electron version:', process.versions.electron);
    }
    
    // Create window and install extensions
    return createWindow();
  })
  .then(installExtensions);

// Graceful shutdown
app.on('will-quit', () => {
  if (serverInstance) {
    console.log('[Main Process] Closing local API server...');
    
    // Check if it's a server with a close method
    if (typeof serverInstance.close === 'function') {
      serverInstance.close(() => {
        console.log('[Main Process] Local API server closed.');
      });
      // Don't set to null immediately as the callback might not fire
    }
    
    // In case there are any open connections, force them to close after a timeout
    setTimeout(() => {
      console.log('[Main Process] Ensuring all server resources are released.');
      serverInstance = null;
    }, 1000);
  }
});

//osX only
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
//osX only ends
