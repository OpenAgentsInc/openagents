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
      console.log(`[Main Process] Starting local API server on port ${LOCAL_API_PORT}...`);
      
      if (!serverApp) {
        throw new Error('serverApp is undefined or null');
      }
      
      if (typeof serverApp.fetch !== 'function') {
        throw new Error('serverApp.fetch is not a function');
      }
      
      // Start the server with Hono's serve adapter
      serverInstance = serve({
        fetch: serverApp.fetch, // Pass the fetch handler from our Hono app
        port: LOCAL_API_PORT,
      }, (info) => {
        console.log(`[Main Process] ✅ Local API server listening on http://localhost:${info.port}`);
      });
      
      // Add error handling
      if (serverInstance && serverInstance.server) {
        serverInstance.server.on('error', (error) => {
          console.error('[Main Process] Server error event:', error);
        });
      }
    } catch (error) {
      console.error('[Main Process] Failed to start local API server:', error);
    }
    
    // Create window and install extensions
    return createWindow();
  })
  .then(installExtensions);

// Graceful shutdown
app.on('will-quit', () => {
  if (serverInstance && typeof serverInstance.close === 'function') {
    console.log('[Main Process] Closing local API server...');
    serverInstance.close();
    serverInstance = null;
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
