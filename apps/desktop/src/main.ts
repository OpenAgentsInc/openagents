import { app, BrowserWindow, ipcMain } from "electron";
import path from "node:path";
import started from "electron-squirrel-startup";
import { Effect } from "effect";
import type { InvoicePaymentRequest } from "@openagentsinc/lightning-effect/contracts";

import { LND_RUNTIME_CHANNELS } from "./main/lndRuntimeIpc";
import { LND_WALLET_CHANNELS } from "./main/lndWalletIpc";
import { SPARK_WALLET_CHANNELS } from "./main/sparkWalletIpc";
import {
  LndRuntimeManagerService,
  projectLndRuntimeSnapshotForRenderer,
  toRuntimeManagerError,
} from "./main/lndRuntimeManager";
import { makeLndRuntimeManagedRuntime } from "./main/lndRuntimeRuntime";
import {
  LndWalletManagerService,
  projectLndWalletSnapshotForRenderer,
} from "./main/lndWalletManager";
import {
  SparkWalletManagerService,
  projectSparkWalletSnapshotForRenderer,
  toSparkWalletManagerError,
} from "./main/sparkWalletManager";

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

const registerLndWalletIpcHandlers = (): void => {
  ipcMain.handle(LND_WALLET_CHANNELS.snapshot, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndWalletManagerService;
        const snapshot = yield* manager.snapshot();
        return projectLndWalletSnapshotForRenderer(snapshot);
      }),
    ),
  );

  ipcMain.handle(
    LND_WALLET_CHANNELS.initialize,
    async (_event, input: { readonly passphrase: string; readonly seedMnemonic?: ReadonlyArray<string> }) =>
      runLndRuntime(
        Effect.gen(function* () {
          const manager = yield* LndWalletManagerService;
          yield* manager.initializeWallet(input);
        }),
      ),
  );

  ipcMain.handle(
    LND_WALLET_CHANNELS.unlock,
    async (_event, input?: { readonly passphrase?: string }) =>
      runLndRuntime(
        Effect.gen(function* () {
          const manager = yield* LndWalletManagerService;
          yield* manager.unlockWallet(input);
        }),
      ),
  );

  ipcMain.handle(LND_WALLET_CHANNELS.lock, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndWalletManagerService;
        yield* manager.lockWallet();
      }),
    ),
  );

  ipcMain.handle(LND_WALLET_CHANNELS.acknowledgeSeedBackup, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndWalletManagerService;
        yield* manager.acknowledgeSeedBackup();
      }),
    ),
  );

  ipcMain.handle(LND_WALLET_CHANNELS.prepareRestore, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndWalletManagerService;
        yield* manager.prepareRestore();
      }),
    ),
  );

  ipcMain.handle(
    LND_WALLET_CHANNELS.restore,
    async (
      _event,
      input: {
        readonly passphrase: string;
        readonly seedMnemonic: ReadonlyArray<string>;
        readonly recoveryWindowDays?: number;
      },
    ) =>
      runLndRuntime(
        Effect.gen(function* () {
          const manager = yield* LndWalletManagerService;
          yield* manager.restoreWallet(input);
        }),
      ),
  );
};

const registerSparkWalletIpcHandlers = (): void => {
  ipcMain.handle(SPARK_WALLET_CHANNELS.snapshot, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        const snapshot = yield* manager.snapshot();
        return projectSparkWalletSnapshotForRenderer(snapshot);
      }),
    ),
  );

  ipcMain.handle(SPARK_WALLET_CHANNELS.bootstrap, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        yield* manager.bootstrap();
      }),
    ),
  );

  ipcMain.handle(SPARK_WALLET_CHANNELS.refresh, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        const snapshot = yield* manager.refresh();
        return projectSparkWalletSnapshotForRenderer(snapshot);
      }),
    ),
  );

  ipcMain.handle(
    SPARK_WALLET_CHANNELS.payInvoice,
    async (_event, input: InvoicePaymentRequest) =>
      runLndRuntime(
        Effect.gen(function* () {
          const manager = yield* SparkWalletManagerService;
          return yield* manager.payInvoice(input);
        }),
      ),
  );

  ipcMain.handle(SPARK_WALLET_CHANNELS.disconnect, async () =>
    runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        yield* manager.disconnect();
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

const bootstrapLndWallet = async (): Promise<void> => {
  try {
    await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* LndWalletManagerService;
        yield* manager.bootstrap();
      }),
    );
  } catch (error) {
    console.error(`[desktop:lnd-wallet] bootstrap failed: ${String(error)}`);
  }
};

const bootstrapSparkWallet = async (): Promise<void> => {
  try {
    await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        yield* manager.bootstrap();
        const snapshot = yield* manager.snapshot();
        return snapshot;
      }),
    );

    const status = await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        return yield* manager.snapshot();
      }),
    );
    console.info(
      `[desktop:spark] wallet lifecycle=${status.lifecycle} network=${status.network} balanceSats=${status.balanceSats ?? "n/a"}`,
    );
  } catch (error) {
    const mapped = toSparkWalletManagerError(error);
    console.error(`[desktop:spark] bootstrap failed (${mapped.code}): ${mapped.message}`);
  }
};

const disconnectSparkWallet = async (): Promise<void> => {
  try {
    await runLndRuntime(
      Effect.gen(function* () {
        const manager = yield* SparkWalletManagerService;
        yield* manager.disconnect();
      }),
    );
  } catch (error) {
    console.error(`[desktop:spark] disconnect failed: ${String(error)}`);
  }
};

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on("ready", () => {
  void (async () => {
    registerLndRuntimeIpcHandlers();
    registerLndWalletIpcHandlers();
    registerSparkWalletIpcHandlers();
    if (!(await startLndRuntime())) {
      app.quit();
      return;
    }
    await bootstrapLndWallet();
    await bootstrapSparkWallet();
    createWindow();
  })();
});

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on("window-all-closed", () => {
  void disconnectSparkWallet();
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
  void disconnectSparkWallet();
  void stopLndRuntime();
});

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and import them here.
