Here’s a concise “under‑the‑hood” tour of Visual Studio Code (VS Code)—what it’s built with, how syntax highlighting works, the major packages it pulls in, and how the whole thing fits together.

---

## Quick answers to your direct questions

**Is it Electron and TypeScript?**
Yes. VS Code is a desktop app built on **Electron** (Chromium + Node.js), and its **core is implemented in TypeScript**. Microsoft’s docs call out Electron explicitly, and the project wiki states: “The core of VS Code is fully implemented in TypeScript.” ([Visual Studio Code][1])

**What does it use for syntax highlighting?**
By default, VS Code colorizes code using **TextMate grammars** interpreted by the `vscode-textmate` library with **Oniguruma** regular expressions (via a WASM binding, `vscode-oniguruma`). Modern “semantic highlighting” can layer on top using tokens from Language Servers. In early 2025, VS Code added an **experimental** Tree‑sitter path for TypeScript only (opt‑in flag), but TextMate remains the default for most languages. ([Visual Studio Code][2])

---

## Big‑picture architecture (how it’s put together)

At a high level, VS Code divides responsibilities across processes so heavy work can’t freeze the UI:

* **Electron Main process** – boots the app, creates windows, owns OS integrations. ([Visual Studio Code][1])
* **Renderer processes** – each workbench window is a Chromium page running the UI and the **Monaco Editor** (the code editor that powers VS Code). ([GitHub][3])
* **Shared / utility processes** – background services (e.g., extension install, file watching) moved into sandboxed “utility processes” as part of security hardening. ([Visual Studio Code][4])
* **Extension Host** – a separate Node.js runtime that loads third‑party extensions so they cannot block the UI and can be restarted independently. Web builds use a “web extension host.” ([Visual Studio Code][5])
* **Language features (LSP)** – many IntelliSense features come from Language Servers speaking the **Language Server Protocol**. VS Code ships the Node client and server libraries (`vscode-languageserver*`). ([Visual Studio Code][6])
* **Debugging (DAP)** – the UI talks to language/runtimes via the **Debug Adapter Protocol** so debuggers can be reused across tools. ([Microsoft GitHub][7])
* **Search** – full‑text search uses **ripgrep** via the `vscode-ripgrep` wrapper. ([GitHub][8])
* **Integrated terminal** – front end is **xterm.js**; a **node-pty** “pty host” process connects to your shell (ConPTY on Windows). ([Visual Studio Code][9])
* **File watching** – native watchers (e.g., parcel‑watcher) run out of process to scale on large repos. ([GitHub][10])
* **VS Code for the Web** – runs in the browser with the same core; for languages without a server, the **Anycode** extension uses **Tree‑sitter** (WASM) to provide “Outline / Go to Symbol” basics. ([GitHub][3])

---

## What packages/libraries does VS Code use?

There’s a long list (see the repo’s `ThirdPartyNotices.txt`), but these are the **load‑bearing** ones you’ll run into when building or extending VS Code:

| Purpose                         | Key package(s)                        | Notes                                                                        |
| ------------------------------- | ------------------------------------- | ---------------------------------------------------------------------------- |
| **Editor**                      | `monaco-editor`                       | The code editor inside VS Code. ([Visual Studio Code][5])                    |
| **Tokenization / colorization** | `vscode-textmate`, `vscode-oniguruma` | TextMate grammar interpreter + Oniguruma regex engine (WASM). ([GitHub][11]) |
| **Semantic highlighting**       | LSP semantic tokens                   | Layered on top of TextMate scopes. ([Visual Studio Code][12])                |
| **Search**                      | `vscode-ripgrep`                      | Downloads the right ripgrep binary for your platform. ([GitHub][8])          |
| **Terminal**                    | `xterm.js`, `node-pty`                | UI emulator + OS pseudo‑terminal bridge. ([Visual Studio Code][9])           |
| **Language features**           | `vscode-languageserver-*`             | Official LSP client/server libs. ([GitHub][13])                              |
| **Debugging**                   | Debug Adapter Protocol (DAP)          | Protocol + many adapters (Node, Go/Delve, etc.). ([Microsoft GitHub][7])     |
| **File watching**               | parcel‑watcher (utility process)      | High‑performance native file watches. ([GitHub][10])                         |
| **Runtime / shell**             | Electron                              | Chromium renderer + Node.js integration. ([Visual Studio Code][1])           |

(For a complete inventory, Microsoft’s repo includes a **Third‑Party Notices** file and related manifests.) ([GitHub][14])

---

## How syntax highlighting actually works

**1) Tokenization (TextMate):**
When a file opens or you type, the editor asks the TextMate engine (`vscode-textmate`) to apply the language’s grammar. Grammars are JSON/PLIST files full of **Oniguruma** regex rules that assign **scopes** to pieces of text (e.g., `keyword.control`, `string.quoted`). ([Visual Studio Code][2])

**2) Theming:**
Themes map those scopes to colors/styles. The result is the fast, regex‑based “syntax highlighting” you see instantly. ([Visual Studio Code][2])

**3) Semantic tokens (optional, layered):**
Once the language server warms up, it can provide **semantic tokens** (e.g., “this identifier is a class”, “this is a parameter”) which VS Code blends on top of the TextMate result for more precise coloring. ([Visual Studio Code][12])

**4) Experimental Tree‑sitter (TypeScript only, opt‑in):**
In January 2025 (v1.97), VS Code introduced an **experimental** Tree‑sitter highlighter for **TypeScript** behind `editor.experimental.preferTreeSitter`. It’s not the default and was added to explore accuracy/perf trade‑offs where TextMate grammars are hard to maintain. ([Visual Studio Code][15])

---

## How common editor features are wired

* **IntelliSense / “Go to Definition” / Rename:**
  The VS Code UI is the client; the heavy lifting comes from **Language Servers** over **LSP** (completion, hovers, diagnostics, symbols, semantic tokens). Extensions can spawn a server or connect to one. ([Visual Studio Code][6])

* **Debugging:**
  The editor talks to a **Debug Adapter** over **DAP** (JSON messages). The adapter bridges to your runtime (Node, Python, .NET, etc.). This separation lets the same adapter work in different tools. ([Microsoft GitHub][7])

* **Search across files:**
  The workbench invokes a background process that shells out to **ripgrep** (via `vscode-ripgrep`) with the right flags and ignore settings. ([GitHub][8])

* **Integrated terminal:**
  The UI uses **xterm.js**; a “pty host” running **node-pty** connects to your shell. On Windows, VS Code uses **ConPTY** for modern pseudo‑console support. ([Visual Studio Code][9])

* **File changes & large repos:**
  File watching runs in a sandboxed utility process using native watchers (e.g., parcel‑watcher). This keeps the UI smooth and scales on giant workspaces. ([GitHub][10])

* **Extensions & safety:**
  Extensions load in the **Extension Host** (separate process or web worker), isolated from the UI for stability/security. Their `package.json` declares contributions and activation events; their entry point implements `activate/deactivate`. ([Visual Studio Code][5])

---

## A note on “what VS Code is made of” vs “what ships where”

* **Code base & layers:** The **core** lives under `src/vs/` and is layered (base → platform → editor/Monaco → workbench → code). The workbench hosts Monaco and stitches features together for desktop (Electron) and web builds, reusing the same TypeScript code wherever possible. ([GitHub][3])
* **Distribution:** “VS Code” the product is built from the open‑source **Code‑OSS** repo with Microsoft’s product bits layered on top. Releases are monthly. ([GitHub][14])
* **Web vs Desktop:** The web version (vscode.dev, github.dev) runs entirely in the browser; where full language servers can’t run, **Anycode** (Tree‑sitter) fills in a basic experience. Desktop can run local and remote extension hosts (Dev Containers/SSH/WSL). ([Visual Studio Code][16])

---

## Where to look next (official, architectural resources)

* **“Why VS Code”** (Electron + web tech background). ([Visual Studio Code][1])
* **Source Code Organization** (layers, environments, Monaco relationship, TypeScript core). ([GitHub][3])
* **Syntax highlighting & Semantic tokens** (TextMate + semantic overlay). ([Visual Studio Code][2])
* **Extension Host** (process model) & **VS Code API**. ([Visual Studio Code][5])
* **LSP** & **DAP** specs (how IntelliSense and debugging integrate). ([Microsoft GitHub][17])

---

### TL;DR

* VS Code is an **Electron** desktop app whose **core is TypeScript**, with the **Monaco Editor** at its heart. ([Visual Studio Code][1])
* **Syntax highlighting** is **TextMate + Oniguruma** by default, optionally layered with **semantic tokens** from language servers; there’s an **experimental** Tree‑sitter highlighter for TypeScript behind a setting. ([Visual Studio Code][2])
* Under the hood, VS Code keeps the UI responsive by pushing work to dedicated processes (Extension Host, search via **ripgrep**, terminal via **xterm.js/node‑pty**, file watching via native watchers). ([GitHub][8])

If you want, I can zoom in on any specific subsystem (e.g., how a language server is wired up, how grammars are authored, or how the terminal’s pty host works) and include minimal sample code.

[1]: https://code.visualstudio.com/docs/editor/whyvscode "Why did we build Visual Studio Code?"
[2]: https://code.visualstudio.com/api/language-extensions/syntax-highlight-guide?utm_source=chatgpt.com "Syntax Highlight Guide | Visual Studio Code Extension API"
[3]: https://github.com/microsoft/vscode/wiki/source-code-organization "Source Code Organization · microsoft/vscode Wiki · GitHub"
[4]: https://code.visualstudio.com/blogs/2022/11/28/vscode-sandbox?utm_source=chatgpt.com "Migrating VS Code to Process Sandboxing"
[5]: https://code.visualstudio.com/api/advanced-topics/extension-host?utm_source=chatgpt.com "Extension Host"
[6]: https://code.visualstudio.com/api/language-extensions/language-server-extension-guide?utm_source=chatgpt.com "Language Server Extension Guide"
[7]: https://microsoft.github.io/debug-adapter-protocol//?utm_source=chatgpt.com "Official page for Debug Adapter Protocol"
[8]: https://github.com/microsoft/vscode-ripgrep?utm_source=chatgpt.com "microsoft/vscode-ripgrep"
[9]: https://code.visualstudio.com/docs/terminal/advanced?utm_source=chatgpt.com "Terminal Advanced"
[10]: https://github.com/microsoft/vscode/wiki/File-Watcher-Internals?utm_source=chatgpt.com "File Watcher Internals · microsoft/vscode Wiki"
[11]: https://github.com/microsoft/vscode-textmate?utm_source=chatgpt.com "microsoft/vscode-textmate: A library that helps tokenize text ..."
[12]: https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide?utm_source=chatgpt.com "Semantic Highlight Guide | Visual Studio Code Extension API"
[13]: https://github.com/microsoft/vscode-languageserver-node?utm_source=chatgpt.com "microsoft/vscode-languageserver-node: Language server ..."
[14]: https://github.com/microsoft/vscode "GitHub - microsoft/vscode: Visual Studio Code"
[15]: https://code.visualstudio.com/updates/v1_97 "January 2025 (version 1.97)"
[16]: https://code.visualstudio.com/docs/setup/vscode-web?utm_source=chatgpt.com "Visual Studio Code for the Web"
[17]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/?utm_source=chatgpt.com "Language Server Protocol Specification - 3.17"
