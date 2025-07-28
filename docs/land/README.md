# Land Code Editor

This is describing the land repo - This repository contains the source code and documentation for **Land**, a next-generation, cross-platform code editor inspired by Visual Studio Code. It is engineered for high performance and resource efficiency using a modern technology stack.

### 1. Overview & Vision

*   **Project Name:** Land
*   **Goal:** To build a high-performance, resource-efficient, and reliable code editor.
*   **Core Technologies:**
    *   **Backend:** Rust and Tauri for native performance (`Mountain`).
    *   **Application Logic:** TypeScript with Effect-TS for declarative, type-safe, and testable code.
    *   **UI Layer:** Astro for the UI components (`Sky`).
*   **Key Differentiator:** The entire application is built on a declarative, effects-based architecture, ensuring all side effects (I/O, UI updates, etc.) are handled in a structured and stable manner.
*   **Funding:** The project is funded by the NGI0 Commons Fund via NLnet.
*   **License:** The project is in the public domain under the Creative Commons CC0 Universal license.

### 2. Core Architecture

The application is split into several distinct, interacting components:

*   **`Mountain` (Rust Backend):** The main Tauri application. It manages the native window, OS operations (filesystem, process management), and hosts a gRPC server.
*   **`Wind` & `Sky` (TypeScript Frontend):** The UI layer running in the Tauri webview. `Wind` is an Effect-TS reimplementation of VS Code's workbench services, managing all UI state and logic. `Sky` contains the UI components (built with Astro) that render this state.
*   **`Cocoon` (TypeScript Extension Host):** A separate Node.js process that runs existing VS Code extensions with high compatibility. It provides a `vscode` API shim built with Effect-TS and communicates with `Mountain` for any privileged operations.
*   **`Common` (Rust Library):** An abstract core library defining the application's shared language through traits and data transfer objects (DTOs), without any concrete implementation.
*   **Inter-Process Communication (IPC):**
    *   **`Wind` <-> `Mountain`:** Uses standard Tauri events and commands.
    *   **`Cocoon` <-> `Mountain`:** Uses gRPC (`Vine` element) for a strongly-typed and performant API contract.

### 3. Modular "Elements" Structure

The project is highly modular, broken down into "Elements," which are managed as separate repositories/submodules:

*   **Core:** `Common` (abstract traits), `Mountain` (backend app), `Wind`/`Sky` (frontend), `Cocoon` (extension host).
*   **Libraries:** `Echo` (task scheduler), `River`/`Sun` (filesystem read/write), `Vine` (gRPC protocol).
*   **Build/Dependencies:** `Editor` (VS Code source submodule), `Rest` (JS bundler), `Output` (bundled JS).
*   **Utilities:** `Maintain` (CI/CD scripts), `Worker` (web workers), `Mist` (WebSocket logic).
*   **Future Vision:** `Grove`, a planned native Rust extension host to eventually replace the Node.js-based `Cocoon`.

### 4. Documented Workflows

The `docs/Workflow` directory details the end-to-end interactions for key features, including:
*   Application Startup & Handshake
*   Opening and Saving Files (including Save Participants for formatting)
*   Executing Commands from the Command Palette
*   Invoking Language Features (e.g., Hover Provider)
*   Creating Webview Panels and Integrated Terminals
*   Source Control Management (SCM) via a built-in Git extension
*   User Data Synchronization (settings, extensions)
*   Running Extension Tests in a separate "Development Host"

### 5. Project Governance

*   **Code of Conduct & Contributing:** The project has adopted the Contributor Covenant, emphasizing a welcoming, inclusive, and harassment-free community. Both `CODE_OF_CONDUCT.md` and `CONTRIBUTING.md` detail these standards.
*   **Security Policy:** A comprehensive security policy is defined in `SECURITY.md`. Vulnerabilities must be reported privately to **Security@Editor.Land** and should not be disclosed publicly in issues. The policy covers supported versions and the vulnerability management process.
*   **Changelog:** The project is in its initial version (`0.0.1`), as noted in `CHANGELOG.md`.

**Land Repository:** https://github.com/CodeEditorLand/Land