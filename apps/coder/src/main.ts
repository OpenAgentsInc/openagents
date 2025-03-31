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
import { serve } from '@hono/node-server'; // Import serve from Hono's adapter

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

    // Start the local Hono server using the Node adapter
    try {
      console.log(`[Main Process] Attempting to start local API server on port ${LOCAL_API_PORT}...`);
      
      // Verify serverApp exists and has the required fetch method
      console.log('[Main Process] ServerApp type:', typeof serverApp);
      console.log('[Main Process] ServerApp keys:', Object.keys(serverApp));
      
      if (!serverApp) {
        throw new Error('serverApp is undefined or null');
      }
      
      if (typeof serverApp.fetch !== 'function') {
        throw new Error('serverApp.fetch is not a function');
      }
      
      // Start the server with Hono's serve adapter
      console.log('[Main Process] Initializing server with serve...');
      serverInstance = serve({
        fetch: serverApp.fetch, // Pass the fetch handler from our Hono app
        port: LOCAL_API_PORT,
      }, (info) => {
        console.log(`[Main Process] ✅ Local API server listening on http://localhost:${info.port}`);
      });
      
      console.log('[Main Process] Server instance created:', !!serverInstance);
      
      // Add more error handling
      if (serverInstance && serverInstance.server) {
        console.log('[Main Process] Adding error handlers to server...');
        
        serverInstance.server.on('error', (error) => {
          console.error('[Main Process] Server error event:', error);
        });
        
        serverInstance.server.on('listening', () => {
          console.log('[Main Process] Server is listening');
        });
      } else {
        console.warn('[Main Process] Server instance created but server property is missing');
      }
    } catch (error) {
      console.error('[Main Process] ❌ Failed to start local API server:', error);
      console.error('[Main Process] Error stack:', error instanceof Error ? error.stack : 'No stack trace');
      // Log important information for debugging
      console.log('[Main Process] Node version:', process.versions.node);
      console.log('[Main Process] Electron version:', process.versions.electron);
      
      // Try to log the serverApp state
      try {
        console.log('[Main Process] ServerApp JSON:', JSON.stringify(serverApp));
      } catch (jsonError) {
        console.error('[Main Process] Failed to stringify serverApp:', jsonError);
      }
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
