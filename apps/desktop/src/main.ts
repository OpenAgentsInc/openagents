import { app, BrowserWindow } from "electron";
import path from "node:path";
import started from "electron-squirrel-startup";

import { LndBinaryResolverError, resolveAndVerifyLndBinary } from "./main/lndBinaryResolver";

// Handle creating/removing shortcuts on Windows when installing/uninstalling.
if (started) {
  app.quit();
}

const createWindow = () => {
  const isDev = Boolean(MAIN_WINDOW_VITE_DEV_SERVER_URL);

  // Create the browser window.
  const mainWindow = new BrowserWindow({
    width: 1024,
    height: 760,
    minWidth: 900,
    minHeight: 640,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
    autoHideMenuBar: true,
    show: false,
  });

  // and load the index.html of the app.
  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(
      path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`),
    );
  }

  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  if (isDev) {
    mainWindow.webContents.openDevTools({ mode: "detach" });
  }
};

const ensureLndBinaryReady = (): boolean => {
  try {
    const resolved = resolveAndVerifyLndBinary({
      appPath: app.getAppPath(),
      resourcesPath: process.resourcesPath,
      isPackaged: app.isPackaged,
      env: process.env,
    });
    console.info(
      `[desktop:lnd] binary ready target=${resolved.target} source=${resolved.source} sha256=${resolved.sha256}`,
    );
    return true;
  } catch (error) {
    if (error instanceof LndBinaryResolverError) {
      console.error(`[desktop:lnd] binary resolution failed (${error.code}): ${error.message}`);
    } else {
      console.error(`[desktop:lnd] binary resolution failed: ${String(error)}`);
    }

    // Production builds fail closed if binary staging or checksum validation fails.
    return !app.isPackaged;
  }
};

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on("ready", () => {
  if (!ensureLndBinaryReady()) {
    app.quit();
    return;
  }
  createWindow();
});

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  // On OS X it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and import them here.
