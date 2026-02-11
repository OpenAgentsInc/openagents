import { app, BrowserWindow, ipcMain } from "electron";
import path from "node:path";
import started from "electron-squirrel-startup";
import { Effect } from "effect";

import { LND_RUNTIME_CHANNELS } from "./main/lndRuntimeIpc";
import {
  LndRuntimeManagerService,
  projectLndRuntimeSnapshotForRenderer,
  toRuntimeManagerError,
} from "./main/lndRuntimeManager";
import { makeLndRuntimeManagedRuntime } from "./main/lndRuntimeRuntime";

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

const lndRuntime = makeLndRuntimeManagedRuntime({
  appPath: app.getAppPath(),
  resourcesPath: process.resourcesPath,
  userDataPath: app.getPath("userData"),
  isPackaged: app.isPackaged,
  env: process.env,
});

const runLndRuntime = <A, E, R>(effect: Effect.Effect<A, E, R>): Promise<A> =>
  lndRuntime.runPromise(effect as Effect.Effect<A, E, never>);

const registerLndRuntimeIpcHandlers = (): void => {
  ipcMain.handle(LND_RUNTIME_CHANNELS.snapshot, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        const snapshot = yield* manager.snapshot();
        return projectLndRuntimeSnapshotForRenderer(snapshot);
      }),
    ),
  );

  ipcMain.handle(LND_RUNTIME_CHANNELS.start, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        yield* manager.start();
      }),
    ),
  );

  ipcMain.handle(LND_RUNTIME_CHANNELS.stop, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        yield* manager.stop();
      }),
    ),
  );

  ipcMain.handle(LND_RUNTIME_CHANNELS.restart, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        yield* manager.restart();
      }),
    ),
  );
};

const startLndRuntime = async (): Promise<boolean> => {
  try {
    await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        yield* manager.start();
        const snapshot = yield* manager.snapshot();
        return snapshot;
      }),
    );
    const resolved = await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        return yield* manager.snapshot();
      }),
    );
    console.info(
      `[desktop:lnd] runtime started lifecycle=${resolved.lifecycle} target=${resolved.target ?? "n/a"} pid=${resolved.pid ?? "n/a"}`,
    );
    return true;
  } catch (error) {
    const mapped = toRuntimeManagerError(error);
    console.error(`[desktop:lnd] runtime start failed (${mapped.code}): ${mapped.message}`);

    // Production builds fail closed if binary staging or checksum validation fails.
    return !app.isPackaged;
  }
};

const stopLndRuntime = async (): Promise<void> => {
  try {
    await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndRuntimeManagerService;
        yield* manager.stop();
      }),
    );
  } catch (error) {
    console.error(`[desktop:lnd] runtime stop failed: ${String(error)}`);
  }
};

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on("ready", () => {
  void (async () => {
    registerLndRuntimeIpcHandlers();
    if (!(await startLndRuntime())) {
      app.quit();
      return;
    }
    createWindow();
  })();
});

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on("window-all-closed", () => {
  void stopLndRuntime();
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

app.on("before-quit", () => {
  void stopLndRuntime();
});

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and import them here.
