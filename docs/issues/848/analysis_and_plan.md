# Analysis and Plan for Issue #848 - Application Initialization Refactor

Based on review of issue #848, related documentation (`initialization.md`, `mcp-client-initialization.md`, `rxdb-setup-learnings.md`, `error-handling.md`), and code structure, the following analysis and plan have been developed.

## Analysis of Potential Root Causes

1.  **Database Issues (Locks/Wipes):**
    *   **Late/Renderer Initialization:** Triggering DB initialization from `HomePage.tsx` (React component) is a primary concern. It occurs late, is subject to React lifecycle complexities (StrictMode), mixes UI with core logic, and increases risk of race conditions or multiple init attempts.
    *   **Inter-Version Conflicts:** Standard file locking might not prevent issues if different application versions (with potentially different schemas) access the same database files.
    *   **Cleanup Reliability:** The effectiveness of DB cleanup on app quit needs verification to prevent stale locks or corrupted state.

2.  **MCP Client Initialization Timing:**
    *   **Implicit Dependencies:** While initialization occurs in `main.ts`, potential implicit dependencies (e.g., config loaded from DB) might not be met *before* MCP clients are initialized.
    *   **Synchronization:** Need to ensure services depending on MCP clients (like the Hono API server) strictly `await` the completion of `initMCPClients()`.

3.  **macOS Window Activation:**
    *   **`activate` Event Handling:** The standard Electron `app.on('activate', ...)` handler might be missing, incorrect, or racing with initial window creation, causing the reported issue on macOS.

4.  **Code Structure:**
    *   **Lack of Centralization:** Core initialization (DB) triggered from the renderer process violates separation of concerns.
    *   **Modularity:** Initialization steps in `main.ts` might lack clear sequencing and modularity.
    *   **Error Handling:** Error handling seems fragmented; a unified strategy for critical main process initialization failures appears absent.

## Refactoring Plan

1.  **Centralize Core Initialization in `main.ts`:**
    *   Move all critical initializations (DB, MCP Clients, Config) to the Electron `main` process.
    *   Execute sequentially and early within an `async` function called from `app.whenReady()`.
    *   **Strict Order:** Config -> Database -> MCP Clients -> Create Window -> Load Renderer.
    *   Use IPC (`ipcMain.handle`/`preload`/`ipcRenderer.invoke`) for renderer to check status (e.g., `isDbReady()`) and request data *after* main process services are ready.

2.  **Make Database Initialization Robust:**
    *   Relocate DB logic (`getDatabase`, `createDatabase`) to `main.ts` or a dedicated `src/main/dbService.ts`.
    *   Use `app.requestSingleInstanceLock()` rigorously.
    *   Implement version checking/namespacing for the DB directory/files to prevent inter-version conflicts.
    *   Ensure robust schema migration handling in the main process init sequence.
    *   Verify reliable DB cleanup (destroy, release locks) in `app.on('will-quit')`.

3.  **Ensure Synchronous MCP Client Initialization:**
    *   Verify `initMCPClients()` explicitly awaits necessary preconditions (config, network).
    *   Confirm dependent services (Hono server) `await` `initMCPClients()` completion.
    *   Consider adding basic health checks/retry logic during init.

4.  **Fix macOS Window Activation:**
    *   Review/Implement `app.on('activate', ...)` handler in `main.ts`.
    *   Ensure it calls `createWindow()` *only* if `BrowserWindow.getAllWindows().length === 0`.
    *   Guard `createWindow` against concurrent calls.

5.  **Improve Code Structure & Electron Best Practices:**
    *   Refactor `main.ts` initialization into smaller, named `async` functions (e.g., `initializeAppConfig`, `initializeDatabaseService`, `initializeMcpService`, `createMainWindow`).
    *   Remove *all* DB initialization from renderer components (`HomePage.tsx`, etc.).
    *   Review `preload.ts` for secure exposure via `contextBridge` and ensure `contextIsolation` is enabled.
    *   Implement clear handling for fatal init errors in `main.ts` (e.g., `dialog.showErrorBox` and quit).

6.  **Add Basic Startup Profiling:**
    *   Use `console.time` / `console.timeEnd` around major init phases in `main.ts` (DB, MCP, Window Create/Load).

7.  **Update Documentation:**
    *   Significantly revise `docs/initialization.md` to reflect the new main-process-driven flow.
