import { app, BrowserWindow, Menu, Tray, dialog } from 'electron';
import path from 'path';
import { installExtension, REACT_DEVELOPER_TOOLS } from 'electron-devtools-installer';
import { serverApp } from './server';
import { serve } from '@hono/node-server';
import { Server } from 'http';
import { initMCPClients, cleanupMCPClients } from './server/mcp-clients';
import { setApiPort } from './helpers/ipc/api-port/api-port-listeners';
import registerListeners from './helpers/ipc/listeners-register';

// --- Early Setup ---

// IMPORTANT: Set app name before anything else (this affects macOS dock name)
app.setName('Coder');

// Set macOS specific about panel options
if (process.platform === 'darwin') {
  app.setAboutPanelOptions({
    applicationName: 'Coder',
    applicationVersion: app.getVersion(),
    copyright: '© 2025 OpenAgents',
    version: app.getVersion(),
    credits: 'OpenAgents Team',
  });
}

const inDevelopment = process.env.NODE_ENV === 'development';

// --- Global Variables ---

// Define a port for the local API server
const DEFAULT_API_PORT = 3001;
let LOCAL_API_PORT = DEFAULT_API_PORT; // Will be updated if the default port is in use

// Keep track of the server instance for graceful shutdown
let serverInstance: ReturnType<typeof serve> | null = null;

// Alternative ports to try if the default port is in use
const ALTERNATIVE_PORTS = [3002, 3003, 3004, 3005, 3006, 3007, 3008, 3009, 3010];

// Keep track of tray instance
let tray: Tray | null = null;

// Keep track of the main window
let mainWindow: BrowserWindow | null = null;

// --- Single Instance Lock ---

function requestSingleInstanceLock(): boolean {
  const gotTheLock = app.requestSingleInstanceLock();

  if (!gotTheLock) {
    console.log('[Main Process] Another instance is already running. Quitting.');
    app.quit();
    return false;
  }

  app.on('second-instance', (event, commandLine, workingDirectory) => {
    // Someone tried to run a second instance, we should focus our window.
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

// Placeholder for Database Initialization (Plan Item 2)
async function initializeDatabaseService() {
  console.time('initializeDatabaseService');
  console.log('[Main Process] Initializing Database Service...');
  // TODO: Implement database initialization logic here
  // - Use singleton pattern
  // - Handle locking robustly (consider app versions)
  // - Perform migrations
  // - Set up IPC handlers for renderer status checks/queries
  await new Promise(resolve => setTimeout(resolve, 50)); // Simulate async work
  console.log('[Main Process] Database Service Initialized (Placeholder).');
  console.timeEnd('initializeDatabaseService');
}

// Placeholder for Database Cleanup
async function cleanupDatabaseService() {
  console.log('[Main Process] Cleaning up Database Service...');
  // TODO: Implement database cleanup logic here (destroy, release locks)
  await new Promise(resolve => setTimeout(resolve, 20)); // Simulate async work
  console.log('[Main Process] Database Service Cleaned Up (Placeholder).');
}

async function initializeMcpClients() {
  console.time('initializeMcpClients');
  console.log('[Main Process] Initializing MCP clients...');
  try {
    await initMCPClients(); // Use the existing function
    console.log('[Main Process] MCP clients initialized successfully.');
  } catch (error) {
    console.error('[Main Process] Error initializing MCP clients:', error);
    // Decide if this is fatal or if the app can continue degraded
    dialog.showErrorBox('MCP Initialization Failed', `Failed to initialize MCP clients: ${error.message}. Some features might be unavailable.`);
    // Optionally: throw error; // To make it fatal
  }
  console.timeEnd('initializeMcpClients');
}

async function initializeCoreServices() {
  console.time('initializeCoreServices');
  console.log('[Main Process] Initializing Core Services...');
  // TODO: Add config loading if needed

  // Initialize Database first (as MCP might depend on config/DB)
  await initializeDatabaseService();

  // Initialize MCP Clients
  await initializeMcpClients();

  console.log('[Main Process] Core Services Initialized.');
  console.timeEnd('initializeCoreServices');
}

async function startApiServer(): Promise<Server | null> {
  console.time('startApiServer');
  console.log('[Main Process] Starting Local API Server...');

  try {
    if (!serverApp) {
      throw new Error('serverApp is undefined or null');
    }
    if (typeof serverApp.fetch !== 'function') {
      throw new Error('serverApp.fetch is not a function');
    }

    // Function to start server with port fallback
    const startServerWithFallback = async (port: number, attemptedPorts: number[] = []): Promise<Server> => {
      console.log(`[Main Process] Attempting to start local API server on port ${port}...`);
      return new Promise((resolve, reject) => {
        const server = serve(
          {
            fetch: serverApp.fetch,
            port: port,
          },
          (info) => {
            console.log(`[Main Process] ✅ Local API server listening on http://localhost:${info.port}`);
            LOCAL_API_PORT = info.port;
            setApiPort(info.port); // Update IPC
            resolve(server);
          }
        );

        server.on('error', (error: Error & { code?: string }) => {
          if (error.code === 'EADDRINUSE') {
            console.warn(`[Main Process] Port ${port} already in use`);
            const nextPortIndex = ALTERNATIVE_PORTS.findIndex(p => !attemptedPorts.includes(p));
            if (nextPortIndex >= 0) {
              const nextPort = ALTERNATIVE_PORTS[nextPortIndex];
              console.log(`[Main Process] Trying alternative port: ${nextPort}`);
              server.close(() => { // Ensure server is closed before retrying
                startServerWithFallback(nextPort, [...attemptedPorts, port])
                  .then(resolve)
                  .catch(reject);
              });
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

    // Add global error handling for the running server
    startedServer.on('error', (error: Error) => {
      console.error('[Main Process] Runtime Server error event:', error);
      // Potentially notify the user or attempt recovery
    });

    console.log(`[Main Process] Exposing API port ${LOCAL_API_PORT} to renderer process via IPC.`);
    console.timeEnd('startApiServer');
    return startedServer;

  } catch (error) {
    console.error('[Main Process] Failed to start local API server:', error);
    dialog.showErrorBox('Server Startup Failed', `Could not start the local API server: ${error.message}. The application might not function correctly.`);
    // Decide if this is fatal
    // throw error; // Uncomment to make server failure fatal
    console.timeEnd('startApiServer');
    return null; // Indicate server failed to start
  }
}


function createMainWindow() {
  console.time('createMainWindow');
  console.log('[Main Process] Creating main window...');
  const preload = path.join(__dirname, "preload.js");

  let iconPath;
  const imageDir = inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images');
  if (process.platform === 'darwin') {
    iconPath = path.join(imageDir, 'icon.icns');
  } else if (process.platform === 'win32') {
    iconPath = path.join(imageDir, 'icon.ico');
  } else {
    iconPath = path.join(imageDir, 'icon.png');
  }

  mainWindow = new BrowserWindow({
    width: 1200,
    height: 950,
    webPreferences: {
      devTools: inDevelopment,
      contextIsolation: true, // Keep true for security
      // nodeIntegration: false, // Recommended false with contextIsolation
      preload: preload,
      webSecurity: true,
      allowRunningInsecureContent: false,
    },
    titleBarStyle: 'hidden',
    icon: iconPath,
    title: 'Coder',
    show: false, // Don't show until ready
  });

  // Redirect local API requests correctly
  mainWindow.webContents.session.webRequest.onBeforeRequest((details, callback) => {
    try {
      const url = new URL(details.url);
      // Redirect requests aiming for '/api/' but not already targeting the correct local port
      if (url.pathname.startsWith('/api/') && url.host !== `localhost:${LOCAL_API_PORT}`) {
          const redirectUrl = `http://localhost:${LOCAL_API_PORT}${url.pathname}${url.search}`;
          console.log(`[Main Process] Redirecting API request from ${details.url} to ${redirectUrl}`);
          callback({ redirectURL: redirectUrl });
          return;
      }
    } catch (error) {
        console.error('[Main Process] Error in web request handler:', error);
    }
    callback({}); // Proceed normally
  });


  registerListeners(mainWindow); // Register IPC listeners

  // Load the renderer code
  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(
      path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`),
    );
  }

  // Show window gracefully when ready
  mainWindow.once('ready-to-show', () => {
    mainWindow?.show();
    console.log('[Main Process] Main window shown.');
  });

  // Dereference window object on close
  mainWindow.on('closed', () => {
    console.log('[Main Process] Main window closed.');
    mainWindow = null;
  });

  console.log('[Main Process] Main window created.');
  console.timeEnd('createMainWindow');

  return mainWindow;
}


function createAppMenu() {
  console.log('[Main Process] Creating application menu...');
  const template: (Electron.MenuItemConstructorOptions | Electron.MenuItem)[] = [
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
    }] as Electron.MenuItemConstructorOptions[] : []),
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

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
  console.log('[Main Process] Application menu created.');
}

function createTray() {
  console.log('[Main Process] Creating tray icon...');
  let iconPath: string;
  const imageDir = inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images');

  if (process.platform === 'darwin') {
    // Use template icon for macOS (adapts to light/dark mode)
    iconPath = path.join(imageDir, 'iconTemplate.png');
  } else {
    // Use regular icon for other platforms
    iconPath = path.join(imageDir, 'icon.png'); // Assuming icon.png exists
  }

  try {
    tray = new Tray(iconPath);
    const contextMenu = Menu.buildFromTemplate([
      {
        label: 'Open Coder', click: () => {
          if (mainWindow) {
            mainWindow.show();
          } else {
            createMainWindow();
          }
        }
      },
      { type: 'separator' },
      { label: 'Quit', click: () => app.quit() }
    ]);

    tray.setToolTip('Coder');
    tray.setContextMenu(contextMenu);

    // Optional: Handle click to show/hide window
    tray.on('click', () => {
        if (mainWindow) {
            mainWindow.isVisible() ? mainWindow.hide() : mainWindow.show();
        } else {
            createMainWindow();
        }
    });

    console.log('[Main Process] Tray icon created.');
  } catch (error) {
      console.error('[Main Process] Failed to create tray icon:', error);
      // App can likely continue without a tray icon
  }
}

async function installDevExtensions() {
  console.time('installDevExtensions');
  console.log('[Main Process] Installing development extensions...');
  try {
    const result = await installExtension(REACT_DEVELOPER_TOOLS);
    console.log(`[Main Process] Extension installed: ${result.name}`);
  } catch (error) {
    console.error('[Main Process] Failed to install React DevTools:', error);
  }
  console.timeEnd('installDevExtensions');
}

// --- Main Application Initialization Orchestrator ---

async function initializeApp() {
  console.time('initializeApp');
  console.log('[Main Process] Starting application initialization...');

  try {
    // 1. Create menus early (can be done before core services)
    createAppMenu();

    // 2. Initialize core backend services (DB, MCP, Config)
    await initializeCoreServices();

    // 3. Start the local API server (depends on core services like MCP)
    serverInstance = await startApiServer();
    if (!serverInstance && !inDevelopment) { // Make server failure fatal in production?
       // throw new Error("API Server failed to start, cannot continue.");
       console.error("[Main Process] API Server failed to start. Application may be unstable.");
    }

    // 4. Create the main UI window (depends on API port being set)
    createMainWindow();

    // 5. Create the system tray icon
    createTray();

    // 6. Install dev extensions if in development mode
    if (inDevelopment) {
      await installDevExtensions();
    }

    // 7. Setup macOS Dock Icon and Menu
    if (process.platform === 'darwin') {
      const iconPath = path.join(inDevelopment ? path.join(process.cwd(), 'src', 'images') : path.join(process.resourcesPath, 'images'), 'icon.png');
      app.dock.setIcon(iconPath);
      const dockMenu = Menu.buildFromTemplate([
        {
          label: 'New Window',
          click() {
            if (!mainWindow) createMainWindow();
           }
        }
      ]);
      app.dock.setMenu(dockMenu);
    }

    console.log('[Main Process] Application initialization complete.');

  } catch (error) {
    console.error('[Main Process] CRITICAL ERROR during initialization:', error);
    dialog.showErrorBox('Application Initialization Failed', `A critical error occurred during startup: ${error.message}\n\nThe application will now exit.`);
    app.quit(); // Exit if essential initialization fails
  } finally {
    console.timeEnd('initializeApp');
  }
}

// --- Electron App Event Handling ---

// Request single instance lock early
if (!requestSingleInstanceLock()) {
  // The lock function already called app.quit()
} else {
  // This method will be called when Electron has finished
  // initialization and is ready to create browser windows.
  // Some APIs can only be used after this event occurs.
  app.whenReady().then(initializeApp);
}


// Quit when all windows are closed, except on macOS.
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    console.log('[Main Process] All windows closed. Quitting.');
    app.quit();
  } else {
    console.log('[Main Process] All windows closed on macOS. App remains active.');
    // Optionally remove tray icon here if desired when window closed on mac
    // if (tray) {
    //   tray.destroy();
    //   tray = null;
    // }
  }
});

// On macOS, re-create a window when the dock icon is clicked and there are no other windows open.
app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    console.log('[Main Process] App activated with no windows open. Creating new window.');
    if (app.isReady()) { // Ensure app is ready before creating window
        createMainWindow();
    } else {
        console.warn('[Main Process] App activated but not ready yet. Window creation deferred to `whenReady`.');
        // initializeApp will handle window creation when ready
    }
  } else {
     console.log('[Main Process] App activated, window(s) already exist.');
     // Optionally bring existing window to front
     if(mainWindow) mainWindow.show();
  }
});


// Graceful shutdown
app.on('will-quit', async (event) => {
  console.log('[Main Process] App is quitting...');

  // Prevent immediate quit to allow cleanup
  event.preventDefault();

  // 1. Close local API server
  if (serverInstance && typeof serverInstance.close === 'function') {
    console.log('[Main Process] Closing local API server...');
    await new Promise<void>((resolve, reject) => {
        serverInstance?.close((err) => {
            if(err) {
                console.error("[Main Process] Error closing server:", err);
                reject(err); // Or just log and continue?
            } else {
                console.log("[Main Process] Server closed.");
                resolve();
            }
        });
        serverInstance = null;
    });
  }

  // 2. Clean up MCP clients
  console.log('[Main Process] Cleaning up MCP clients...');
  await cleanupMCPClients(); // Assuming this is synchronous or returns a promise

  // 3. Clean up Database Service
  await cleanupDatabaseService(); // Placeholder

  // 4. Clean up tray
  if (tray) {
    console.log('[Main Process] Destroying tray icon...');
    tray.destroy();
    tray = null;
  }

  // 5. Now allow the app to quit
  console.log('[Main Process] Cleanup complete. Exiting now.');
  app.exit();
});

// Handle unhandled exceptions/rejections
process.on('uncaughtException', (error) => {
  console.error('[Main Process] Uncaught Exception:', error);
  dialog.showErrorBox('Unhandled Error', `An unexpected error occurred: ${error.message}. The application might become unstable.`);
  // Consider quitting app.quit();
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('[Main Process] Unhandled Rejection at:', promise, 'reason:', reason);
  dialog.showErrorBox('Unhandled Promise Rejection', `An unexpected promise rejection occurred: ${reason}. The application might become unstable.`);
});

// Clear recent documents cache on start (optional)
app.on('ready', () => {
    if (!inDevelopment) { // Only clear cache in production/packaged builds?
        app.clearRecentDocuments();
    }
});
