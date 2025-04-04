import { app, BrowserWindow, Menu, Tray, dialog, ipcMain } from 'electron'; // Added ipcMain
import path from 'node:path';
import fs from 'node:fs';
import { installExtension, REACT_DEVELOPER_TOOLS } from 'electron-devtools-installer';
import { serverApp } from './server';
import { serve } from '@hono/node-server';
import { Server } from 'http';
import type { ServerType } from '@hono/node-server';
import { initMCPClients, cleanupMCPClients } from './server/mcp-clients';
import { setApiPort } from './helpers/ipc/api-port/api-port-listeners';
import registerListeners from './helpers/ipc/listeners-register';
import { getDatabase, cleanupDatabase, getDbStatus } from './main/dbService'; // Import from new service

// --- Early Setup ---
app.setName('Coder');
if (process.platform === 'darwin') {
  app.setAboutPanelOptions({
    applicationName: 'Coder',
    applicationVersion: app.getVersion(),
    copyright: '© 2025 OpenAgents',
    version: app.getVersion(),
    credits: 'OpenAgents Team',
  });
}

const inDevelopment = !app.isPackaged; // Use app.isPackaged for reliable env check

// --- Global Variables ---
const DEFAULT_API_PORT = 3001;
let LOCAL_API_PORT = DEFAULT_API_PORT;
let serverInstance: ServerType | null = null;
const ALTERNATIVE_PORTS = [3002, 3003, 3004, 3005, 3006, 3007, 3008, 3009, 3010];
let tray: Tray | null = null;
let mainWindow: BrowserWindow | null = null;

// --- Single Instance Lock ---
function requestSingleInstanceLock(): boolean {
  const gotTheLock = app.requestSingleInstanceLock();
  if (!gotTheLock) {
    console.log('[Main Process] Another instance is already running. Quitting.');
    app.quit();
    return false;
  }
  app.on('second-instance', () => {
    console.log('[Main Process] Second instance detected. Focusing existing window.');
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
  console.log('[Main Process] Single instance lock acquired.');
  return true;
}

// --- Initialization Functions ---

// Database initialization using the new service
async function initializeDatabaseService() {
  console.time('initializeDatabaseService');
  console.log('[Main Process] Initializing Database Service...');
  try {
    await getDatabase(); // This calls the singleton logic in dbService
    console.log('[Main Process] Database Service Initialized successfully.');
  } catch (error) {
    console.error('[Main Process] CRITICAL: Database Service failed to initialize:', error);
    // Error dialog is shown within dbService, rethrow to halt initialization
    throw error;
  }
  console.timeEnd('initializeDatabaseService');
}

// Database cleanup using the new service
async function cleanupDatabaseService() {
  console.log('[Main Process] Cleaning up Database Service...');
  try {
    await cleanupDatabase(); // Calls the cleanup logic in dbService
  } catch(error) {
      console.error('[Main Process] Error during database cleanup:', error);
  }
  console.log('[Main Process] Database Service Cleaned Up.');
}

async function initializeMcpClients() {
  console.time('initializeMcpClients');
  console.log('[Main Process] Initializing MCP clients...');
  try {
    await initMCPClients();
    console.log('[Main Process] MCP clients initialized successfully.');
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error('[Main Process] Error initializing MCP clients:', error);
    dialog.showErrorBox('MCP Initialization Failed', `Failed to initialize MCP clients: ${errorMessage}. Some features might be unavailable.`);
    // Optionally make fatal: throw error;
  }
  console.timeEnd('initializeMcpClients');
}

async function initializeCoreServices() {
  console.time('initializeCoreServices');
  console.log('[Main Process] Initializing Core Services...');
  // TODO: Add config loading if needed here

  // Initialize Database first
  await initializeDatabaseService();

  // Initialize MCP Clients (potentially depends on DB/config)
  await initializeMcpClients();

  console.log('[Main Process] Core Services Initialized.');
  console.timeEnd('initializeCoreServices');
}

async function startApiServer(): Promise<ServerType | null> {
  console.time('startApiServer');
  console.log('[Main Process] Starting Local API Server...');
  // ... (rest of startApiServer function remains largely the same as previous refactor)
  try {
    if (!serverApp) throw new Error('serverApp is undefined or null');
    if (typeof serverApp.fetch !== 'function') throw new Error('serverApp.fetch is not a function');

    const startServerWithFallback = async (port: number, attemptedPorts: number[] = []): Promise<ServerType> => {
      console.log(`[Main Process] Attempting to start local API server on port ${port}...`);
      return new Promise((resolve, reject) => {
        const server = serve({ fetch: serverApp.fetch, port }, (info) => {
          console.log(`[Main Process] ✅ Local API server listening on http://localhost:${info.port}`);
          LOCAL_API_PORT = info.port;
          setApiPort(info.port);
          resolve(server);
        });
        server.on('error', (error: Error & { code?: string }) => {
          if (error.code === 'EADDRINUSE') {
            console.warn(`[Main Process] Port ${port} already in use`);
            const nextPortIndex = ALTERNATIVE_PORTS.findIndex(p => !attemptedPorts.includes(p));
            if (nextPortIndex >= 0) {
              const nextPort = ALTERNATIVE_PORTS[nextPortIndex];
              console.log(`[Main Process] Trying alternative port: ${nextPort}`);
              server.close(() => { startServerWithFallback(nextPort, [...attemptedPorts, port]).then(resolve).catch(reject); });
            } else {
              reject(new Error(`All designated API ports are in use. Tried: ${DEFAULT_API_PORT}, ${ALTERNATIVE_PORTS.join(', ')}`));
            }
          } else {
            console.error('[Main Process] Server startup error:', error);
            reject(error);
          }
        });
      });
    };

    const startedServer = await startServerWithFallback(LOCAL_API_PORT);
    startedServer.on('error', (error: Error) => { console.error('[Main Process] Runtime Server error event:', error); });
    console.log(`[Main Process] Exposing API port ${LOCAL_API_PORT} to renderer process via IPC.`);
    console.timeEnd('startApiServer');
    return startedServer;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error('[Main Process] Failed to start local API server:', error);
    dialog.showErrorBox('Server Startup Failed', `Could not start the local API server: ${errorMessage}. The application might not function correctly.`);
    console.timeEnd('startApiServer');
    return null;
  }
}

function createMainWindow() {
  // If window is already being created or exists, return it
  if (mainWindow) {
    console.log('[Main Process] Window already exists, showing it');
    mainWindow.show();
    return mainWindow;
  }

  console.time('createMainWindow');
  console.log('[Main Process] Creating main window...');
  const preload = path.join(__dirname, "preload.js");
  let iconPath;
  const imageDir = inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images');
  if (process.platform === 'darwin') iconPath = path.join(imageDir, 'icon.icns');
  else if (process.platform === 'win32') iconPath = path.join(imageDir, 'icon.ico');
  else iconPath = path.join(imageDir, 'icon.png');

  const newWindow = new BrowserWindow({
    width: 1200, height: 950,
    webPreferences: { devTools: inDevelopment, contextIsolation: true, preload, webSecurity: true, allowRunningInsecureContent: false },
    titleBarStyle: 'hidden', icon: iconPath, title: 'Coder', show: false, // Keep show: false initially
  });

  // Assign to global reference before setting up event handlers
  mainWindow = newWindow;

  // --- Add Error Handling for Loading ---
  mainWindow.webContents.on('did-fail-load', (event, errorCode, errorDescription, validatedURL) => {
    console.error(`[Main Process] Failed to load window content: ${errorDescription} (Code: ${errorCode}) URL: ${validatedURL}`);
    // Optionally show an error dialog to the user
    if (!inDevelopment) { // Avoid annoying dialogs during hot-reloads in dev
        dialog.showErrorBox('Content Load Error', `Failed to load application content: ${errorDescription}`);
    }
    // Decide if the app should quit or retry? For now, just log.
  });
  // --- End Error Handling ---

  mainWindow.webContents.session.webRequest.onBeforeRequest((details, callback) => {
    try {
      const url = new URL(details.url);
      if (url.pathname.startsWith('/api/') && url.host !== `localhost:${LOCAL_API_PORT}`) {
          const redirectUrl = `http://localhost:${LOCAL_API_PORT}${url.pathname}${url.search}`;
          console.log(`[Main Process] Redirecting API request from ${details.url} to ${redirectUrl}`);
          callback({ redirectURL: redirectUrl }); return;
      }
    } catch (error) { console.error('[Main Process] Error in web request handler:', error); }
    callback({});
  });

  registerListeners(mainWindow);

  console.log(`[Main Process] Loading renderer content. MAIN_WINDOW_VITE_DEV_SERVER_URL: ${MAIN_WINDOW_VITE_DEV_SERVER_URL}`);
  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    console.log(`[Main Process] Loading URL: ${MAIN_WINDOW_VITE_DEV_SERVER_URL}`);
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    // --- Simplified Production Path ---
    // Assumes index.html is in '<outputDir>/renderer/main_window/' relative to main process root
    // VitePlugin default output is usually '.vite/renderer/main_window'
    const indexPath = path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`);
    console.log(`[Main Process] Loading file: ${indexPath}`);
    if (fs.existsSync(indexPath)) {
        mainWindow.loadFile(indexPath);
    } else {
        console.error(`[Main Process] Renderer index.html not found at expected path: ${indexPath}`);
        dialog.showErrorBox('Application Load Error', `Could not find the required file to load the application interface: ${indexPath}`);
        // App likely cannot function, consider quitting
        // app.quit();
    }
    // --- End Simplified Path ---
  }

  mainWindow.once('ready-to-show', () => {
    if (mainWindow) { // Check if window still exists
      console.log('[Main Process] Window ready-to-show event fired. Showing window.');
      mainWindow.show();
      console.log('[Main Process] Main window shown.');
      // Open DevTools automatically in development for debugging
      if (inDevelopment) {
           mainWindow.webContents.openDevTools({ mode: 'detach' }); // Detach dev tools
      }
    } else {
        console.warn('[Main Process] Window was destroyed before ready-to-show fired.');
    }
  });

  mainWindow.on('closed', () => {
    console.log('[Main Process] Main window closed.');
    mainWindow = null;
  });

  console.log('[Main Process] Main window created, waiting for content to load...');
  console.timeEnd('createMainWindow');
  return mainWindow;
}


function createAppMenu() {
    console.log('[Main Process] Creating application menu...');
    // ... (createAppMenu function remains the same as previous refactor)
    // Define the menu template with explicit types for roles
    const template: Electron.MenuItemConstructorOptions[] = [
        ...(process.platform === 'darwin' ? [{
          label: app.name,
          submenu: [
            { role: 'about' }, 
            { type: 'separator' }, 
            { role: 'services' }, 
            { type: 'separator' },
            { role: 'hide' }, 
            { role: 'hideothers' as any }, // Type as any for hideOthers
            { role: 'unhide' as any }, // Type as any for unhide
            { type: 'separator' }, 
            { role: 'quit' }
          ] as Electron.MenuItemConstructorOptions[] // Type assertion here
        }] : []),
        { 
          label: 'File', 
          submenu: [process.platform === 'darwin' ? { role: 'close' } : { role: 'quit' }] as Electron.MenuItemConstructorOptions[] 
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
            ...(process.platform === 'darwin' 
              ? [{ role: 'delete' }, { role: 'selectall' as any }] // Type as any for selectAll
              : [{ role: 'delete' }, { type: 'separator' }, { role: 'selectall' as any }]
            )
          ] as Electron.MenuItemConstructorOptions[] 
        },
        { 
          label: 'View', 
          submenu: [
            { role: 'reload' }, 
            { role: 'forcereload' as any }, // Type as any for forceReload
            { role: 'toggledevtools' as any }, // Type as any for toggleDevTools
            { type: 'separator' },
            { role: 'resetzoom' as any }, // Type as any for resetZoom
            { role: 'zoomin' as any }, // Type as any for zoomIn
            { role: 'zoomout' as any }, // Type as any for zoomOut
            { type: 'separator' }, 
            { role: 'togglefullscreen' }
          ] as Electron.MenuItemConstructorOptions[] 
        },
        { 
          label: 'Window', 
          submenu: [
            { role: 'minimize' }, 
            { role: 'zoom' },
            ...(process.platform === 'darwin' 
              ? [{ type: 'separator' }, { role: 'front' }, { type: 'separator' }, { role: 'window' as any }] 
              : [{ role: 'close' }]
            )
          ] as Electron.MenuItemConstructorOptions[] 
        }
      ];
    const menu = Menu.buildFromTemplate(template);
    Menu.setApplicationMenu(menu);
    console.log('[Main Process] Application menu created.');
}

function createTray() {
    console.log('[Main Process] Creating tray icon...');
    // ... (createTray function remains the same as previous refactor)
    let iconPath: string;
    const imageDir = inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images');
    iconPath = process.platform === 'darwin' ? path.join(imageDir, 'iconTemplate.png') : path.join(imageDir, 'icon.png');
    try {
        tray = new Tray(iconPath);
        const contextMenu = Menu.buildFromTemplate([
          { label: 'Open Coder', click: () => { if (mainWindow) mainWindow.show(); else createMainWindow(); } },
          { type: 'separator' }, { label: 'Quit', click: () => app.quit() }
        ]);
        tray.setToolTip('Coder');
        tray.setContextMenu(contextMenu);
        tray.on('click', () => { if (mainWindow) { mainWindow.isVisible() ? mainWindow.hide() : mainWindow.show(); } else { createMainWindow(); } });
        console.log('[Main Process] Tray icon created.');
      } catch (error) { console.error('[Main Process] Failed to create tray icon:', error); }
}

async function installDevExtensions() {
    console.time('installDevExtensions');
    console.log('[Main Process] Installing development extensions...');
    // ... (installDevExtensions function remains the same)
    try {
        const result = await installExtension(REACT_DEVELOPER_TOOLS);
        console.log(`[Main Process] Extension installed: ${result.name}`);
      } catch (error) { console.error('[Main Process] Failed to install React DevTools:', error); }
      console.timeEnd('installDevExtensions');
}

// --- IPC Handlers ---

// Track registered handlers to prevent duplication
const registeredIpcHandlers = new Set<string>();

function registerIpcHandlers() {
    console.log('[Main Process] Registering IPC handlers...');

    // Only register if not already registered
    if (!registeredIpcHandlers.has('get-db-status')) {
        ipcMain.handle('get-db-status', () => {
            console.log('[Main Process] IPC: get-db-status requested');
            return getDbStatus(); // Return status from dbService
        });
        registeredIpcHandlers.add('get-db-status');
    }

    // Add other main process handlers here if needed
    console.log('[Main Process] IPC handlers registered.');
}

// --- Main Application Initialization Orchestrator ---

async function initializeApp() {
  console.time('initializeApp');
  console.log('[Main Process] Starting application initialization...');
  try {
    // 0. Register IPC Handlers early
    registerIpcHandlers();

    // 1. Create menus
    createAppMenu();

    // 2. Create main window early so it can be displayed while other services initialize
    createMainWindow();

    // 3. Create tray
    createTray();

    // 4. Initialize core services (DB, MCP) in the background
    initializeCoreServices().catch(error => {
      console.error('[Main Process] Error during core services initialization:', error);
      dialog.showErrorBox('Service Initialization Error',
        `Failed to initialize core services: ${error.message}
Some features may be unavailable.`);
    });

    // 5. Start API server - do this in the background in dev mode
    if (inDevelopment) {
      // Non-blocking in dev mode
      startApiServer().then(server => {
        serverInstance = server;
        if (!serverInstance) {
          console.warn("[Main Process] API Server failed to start in dev mode.");
        }
      }).catch((error: unknown) => {
        const errorMessage = error instanceof Error ? error.message : String(error);
        console.error("[Main Process] Error starting API server in dev mode:", errorMessage);
      });
    } else {
      // Blocking in production mode (might reconsider if server start is slow)
      serverInstance = await startApiServer();
      if (!serverInstance) {
        console.error("[Main Process] API Server failed to start. Application may be unstable.");
      }
    }

    // 6. Install dev extensions
    if (inDevelopment) {
      await installDevExtensions();
    }

    // 7. Setup macOS Dock
    if (process.platform === 'darwin') {
      const iconPath = path.join(inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images'), 'icon.png');
      // Use native fs.existsSync instead of fs-extra.pathExists
      if(fs.existsSync(iconPath)) app.dock.setIcon(iconPath);
      const dockMenu = Menu.buildFromTemplate([{ label: 'New Window', click() { if (!mainWindow) createMainWindow(); } }]);
      app.dock.setMenu(dockMenu);
    }

    console.log('[Main Process] Application initialization sequence complete.');
  } catch (error) {
    console.error('[Main Process] CRITICAL ERROR during initialization:', error);
    const errorMessage = error instanceof Error ? error.message : String(error);
    dialog.showErrorBox('Application Initialization Failed', `A critical error occurred during startup: ${errorMessage}

The application will now exit.`);
    app.quit();
  } finally {
    console.timeEnd('initializeApp');
  }
}

// --- Electron App Event Handling ---

if (!requestSingleInstanceLock()) {
  // Quit called by lock function
} else {
  app.whenReady().then(initializeApp);
}

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    console.log('[Main Process] All windows closed. Quitting.');
    app.quit();
  } else {
    console.log('[Main Process] All windows closed on macOS. App remains active.');
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    console.log('[Main Process] App activated with no windows open. Creating new window.');
    // Ensure app is ready and window doesn't already exist somehow
    if (app.isReady() && !mainWindow) {
        createMainWindow();
    } else if (mainWindow) {
         console.log('[Main Process] App activated, window exists but might be hidden. Showing.');
         mainWindow.show(); // Ensure it's shown if it exists but was hidden
    } else {
        console.warn('[Main Process] App activated but not ready yet. Window creation deferred.');
    }
  } else {
     console.log('[Main Process] App activated, window(s) already exist.');
     if(mainWindow) mainWindow.show(); // Bring existing window to front
  }
});


app.on('will-quit', async (event) => {
  console.log('[Main Process] App is quitting...');
  event.preventDefault(); // Prevent immediate quit

  // 1. Close server
  if (serverInstance) {
    console.log('[Main Process] Closing local API server...');
    await new Promise<void>(resolve => { serverInstance?.close(err => { if(err) console.error("[Main Process] Error closing server:", err); else console.log("[Main Process] Server closed."); resolve(); }); serverInstance = null; });
  }
  // 2. Cleanup MCP
  console.log('[Main Process] Cleaning up MCP clients...');
  await cleanupMCPClients();
  // 3. Cleanup DB
  await cleanupDatabaseService(); // Use the new service function
  // 4. Cleanup Tray
  if (tray) { console.log('[Main Process] Destroying tray icon...'); tray.destroy(); tray = null; }

  console.log('[Main Process] Cleanup complete. Exiting now.');
  app.exit();
});

process.on('uncaughtException', (error) => {
  console.error('[Main Process] Uncaught Exception:', error);
  dialog.showErrorBox('Unhandled Error', `An unexpected error occurred: ${error.message}.`);
});
process.on('unhandledRejection', (reason, promise) => {
  console.error('[Main Process] Unhandled Rejection at:', promise, 'reason:', reason);
  dialog.showErrorBox('Unhandled Promise Rejection', `An unexpected promise rejection occurred: ${reason}.`);
});

app.on('ready', () => { if (!inDevelopment) app.clearRecentDocuments(); });
