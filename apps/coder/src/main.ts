import { app, BrowserWindow, Menu, Tray } from "electron";

// IMPORTANT: Set app name before anything else (this affects macOS dock name)
app.setName("Coder");

// Set macOS specific about panel options
if (process.platform === 'darwin') {
  app.setAboutPanelOptions({
    applicationName: "Coder",
    applicationVersion: app.getVersion(),
    copyright: "© 2025 OpenAgents",
    version: app.getVersion(),
    credits: "OpenAgents Team"
  });
}

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
import { Server } from 'http';
import { initMCPClients, cleanupMCPClients } from './server/mcp-clients';

const inDevelopment = process.env.NODE_ENV === "development";

// Define a port for the local API server
const LOCAL_API_PORT = 3001; // Or another available port

// Keep track of the server instance for graceful shutdown
let serverInstance: ReturnType<typeof serve> | null = null;

// Keep track of tray instance
let tray: Tray | null = null;

// Create application menu
function createAppMenu() {
  // Create the application menu
  const template = [
    ...(process.platform === 'darwin' ? [{
      label: app.name,
      submenu: [
        { role: 'about' },
        { type: 'separator' },
        { role: 'services' },
        { type: 'separator' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit' }
      ]
    }] : []),
    {
      label: 'File',
      submenu: [
        process.platform === 'darwin'
          ? { role: 'close' }
          : { role: 'quit' }
      ]
    },
    {
      label: 'Edit',
      submenu: [
        { role: 'undo' },
        { role: 'redo' },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        ...(process.platform === 'darwin' ? [
          { role: 'delete' },
          { role: 'selectAll' },
        ] : [
          { role: 'delete' },
          { type: 'separator' },
          { role: 'selectAll' }
        ])
      ]
    },
    {
      label: 'View',
      submenu: [
        { role: 'reload' },
        { role: 'forceReload' },
        { role: 'toggleDevTools' },
        { type: 'separator' },
        { role: 'resetZoom' },
        { role: 'zoomIn' },
        { role: 'zoomOut' },
        { type: 'separator' },
        { role: 'togglefullscreen' }
      ]
    },
    {
      label: 'Window',
      submenu: [
        { role: 'minimize' },
        { role: 'zoom' },
        ...(process.platform === 'darwin' ? [
          { type: 'separator' },
          { role: 'front' },
          { type: 'separator' },
          { role: 'window' }
        ] : [
          { role: 'close' }
        ])
      ]
    }
  ];

  // @ts-ignore - template is typed but not perfectly
  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}

function createWindow() {
  const preload = path.join(__dirname, "preload.js");

  // Determine icon path based on development or production mode and platform
  let iconPath;
  if (inDevelopment) {
    // In development mode, use the appropriate icon for the platform
    if (process.platform === 'darwin') {
      // macOS prefers .icns
      iconPath = path.join(process.cwd(), 'src', 'images', 'icon.icns');
    } else if (process.platform === 'win32') {
      // Windows prefers .ico
      iconPath = path.join(process.cwd(), 'src', 'images', 'icon.ico');
    } else {
      // Linux can use .png
      iconPath = path.join(process.cwd(), 'src', 'images', 'icon.png');
    }
  } else {
    // In production mode
    if (process.platform === 'darwin') {
      iconPath = path.join(process.resourcesPath, 'images', 'icon.icns');
    } else if (process.platform === 'win32') {
      iconPath = path.join(process.resourcesPath, 'images', 'icon.ico');
    } else {
      iconPath = path.join(process.resourcesPath, 'images', 'icon.png');
    }
  }

  const mainWindow = new BrowserWindow({
    width: 1200,
    height: 950,
    webPreferences: {
      devTools: inDevelopment,
      contextIsolation: true,
      nodeIntegration: true,
      nodeIntegrationInSubFrames: false,
      preload: preload,
      webSecurity: true, // Enable web security
      allowRunningInsecureContent: false,
    },
    titleBarStyle: "hidden",
    icon: iconPath,
    title: "Coder",
  });
  
  // Set up custom web request handling to allow connections to our local API server
  // This fixes the "Failed to fetch" errors when the app is making network requests
  mainWindow.webContents.session.webRequest.onBeforeRequest((details, callback) => {
    try {
      // Check if this is an API request to our local server
      const isApiRequest = details.url.includes('/api/');
      const isLocalhost = details.url.startsWith(`http://localhost:${LOCAL_API_PORT}`);
      
      if (isApiRequest && !isLocalhost) {
        let redirectUrl;
        try {
          // Parse the URL to extract path and query parameters
          const urlObj = new URL(details.url);
          redirectUrl = `http://localhost:${LOCAL_API_PORT}${urlObj.pathname}${urlObj.search || ''}`;
        } catch (parseError) {
          // Fallback for URLs that may not parse correctly
          const urlPath = details.url.split('/api/')[1] || '';
          redirectUrl = `http://localhost:${LOCAL_API_PORT}/api/${urlPath}`;
        }
        
        console.log(`[Main Process] Redirecting API request from ${details.url} to ${redirectUrl}`);
        callback({ redirectURL: redirectUrl });
      } else {
        callback({});
      }
    } catch (error) {
      // Log the error but allow the request to continue
      console.error('[Main Process] Error in web request handler:', error);
      callback({}); // Don't block the request on error
    }
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

// Create tray icon
function createTray() {
  const isDev = process.env.NODE_ENV === "development";
  let iconPath: string;

  // For macOS, use the template icon which works well with both light/dark themes
  if (process.platform === 'darwin') {
    if (isDev) {
      iconPath = path.join(process.cwd(), 'src', 'images', 'iconTemplate.png');
    } else {
      iconPath = path.join(process.resourcesPath, 'images', 'iconTemplate.png');
    }
  } else {
    // For other platforms, use the regular icon
    if (isDev) {
      iconPath = path.join(process.cwd(), 'src', 'images', 'icon.png');
    } else {
      iconPath = path.join(process.resourcesPath, 'images', 'icon.png');
    }
  }

  tray = new Tray(iconPath);
  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Open Coder', click: () => {
        const windows = BrowserWindow.getAllWindows();
        if (windows.length > 0) {
          windows[0].show();
        } else {
          createWindow();
        }
      }
    },
    { type: 'separator' },
    { label: 'Quit', click: () => app.quit() }
  ]);

  tray.setToolTip('Coder');
  tray.setContextMenu(contextMenu);
}

// Clear any cached data on app start
if (app.isReady()) {
  app.clearRecentDocuments();
  if (process.platform === 'darwin') {
    app.dock.setIcon(path.join(inDevelopment ? process.cwd() : process.resourcesPath, 'src', 'images', 'icon.png'));
  }
}

app.whenReady()
  .then(async () => {
    console.log('[Main Process] App is ready.');

    // Create application menu
    createAppMenu();

    // On macOS, set the dock icon explicitly and configure dock menu
    if (process.platform === 'darwin') {
      // For macOS dock, PNG actually works better than ICNS for dynamic updates
      const iconPath = path.join(inDevelopment ? process.cwd() : process.resourcesPath, 'src', 'images', 'icon.png');
      app.dock.setIcon(iconPath);

      // Set up a dock menu
      const dockMenu = Menu.buildFromTemplate([
        {
          label: 'New Window',
          click() { createWindow(); }
        }
      ]);
      app.dock.setMenu(dockMenu);
    }

    // Start the local Hono server using the Node adapter
    try {
      console.log(`[Main Process] Starting local API server on port ${LOCAL_API_PORT}...`);

      if (!serverApp) {
        throw new Error('serverApp is undefined or null');
      }

      if (typeof serverApp.fetch !== 'function') {
        throw new Error('serverApp.fetch is not a function');
      }

      // We'll initialize MCP clients after the window is created
      console.log('[Main Process] MCP client initialization will happen after window creation');

      // Start the server with Hono's serve adapter
      serverInstance = serve({
        fetch: serverApp.fetch, // Pass the fetch handler from our Hono app
        port: LOCAL_API_PORT,
      }, (info) => {
        console.log(`[Main Process] ✅ Local API server listening on http://localhost:${info.port}`);
      });

      // Add error handling
      if (serverInstance) {
        serverInstance.on('error', (error: Error) => {
          console.error('[Main Process] Server error event:', error);
        });
      }
    } catch (error) {
      console.error('[Main Process] Failed to start local API server:', error);
    }

    // Create tray
    createTray();

    // Create window and install extensions
    createWindow();
    
    // Now initialize MCP clients after window is created
    setTimeout(async () => {
      try {
        console.log('[Main Process] Initializing MCP clients now that window is created...');
        await initMCPClients();
        console.log('[Main Process] MCP clients initialized successfully');
      } catch (error) {
        console.error('[Main Process] Error initializing MCP clients:', error);
      }
    }, 3000); // Delay by 3 seconds to ensure everything is ready
    
    return installExtensions();
  });

// Graceful shutdown
app.on('will-quit', () => {
  if (serverInstance && typeof serverInstance.close === 'function') {
    console.log('[Main Process] Closing local API server...');
    serverInstance.close();
    serverInstance = null;
  }

  // Clean up tray
  if (tray) {
    tray.destroy();
    tray = null;
  }

  // Clean up MCP clients
  console.log('[Main Process] Cleaning up MCP clients...');
  cleanupMCPClients();
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
