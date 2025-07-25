# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Tauri desktop application built with:
- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust with Tauri framework
- **Package Manager**: Bun (not npm/yarn)

### Project Structure
**All application code is located in the `desktop/` directory.** The repository root contains documentation and configuration, but the actual Tauri application lives entirely within:
```
desktop/                  # Main project directory
├── src/                  # Frontend React/TypeScript code
├── src-tauri/            # Backend Rust code
├── package.json          # Frontend dependencies
├── bun.lock             # Bun lockfile
└── ...                  # Other frontend config files
```

**Always work from the `desktop/` directory** when running commands or referencing files.

## Commands

### Development
```bash
cd desktop
bun install           # Install dependencies
bun run dev           # Run Tauri app in development mode (no HMR by default)
bun run dev:hmr       # Run Tauri app with HMR (may steal focus)
bun run dev:vite      # Start Vite dev server only (no HMR)
bun run dev:vite:hmr  # Start Vite dev server only with HMR
```

### Building
```bash
cd desktop
bun run build        # Build frontend (runs tsc && vite build)
bun run tauri build  # Build the complete Tauri app
```

### Package Management
**IMPORTANT**: Never manually edit `Cargo.toml` or `package.json` to add dependencies. Always use the proper package managers:
- **Rust dependencies**: Use `cargo add <package>` in the `src-tauri` directory
- **Frontend dependencies**: Use `bun add <package>` for runtime deps or `bun add -d <package>` for dev deps
This ensures you get the latest compatible versions and proper lockfile updates.

### Other Commands
```bash
bun run preview      # Preview production build
```

**IMPORTANT**: Never run `bun run dev`, `bun run dev:hmr`, or similar development server commands automatically. The user will run these commands themselves. You should only compile/build to check for errors using commands like `cargo build` or `cargo check`.

## Architecture

### Frontend Structure (within `desktop/` directory)
- Entry point: `desktop/src/main.tsx`
- Main component: `desktop/src/App.tsx`
- Uses Tauri API for IPC: `@tauri-apps/api/core`
- TypeScript configuration: `desktop/tsconfig.json`

### Backend Structure (within `desktop/` directory)
- Rust entry point: `desktop/src-tauri/src/main.rs`
- Application logic: `desktop/src-tauri/src/lib.rs`
- Tauri commands are defined with `#[tauri::command]` macro
- Current commands:
  - `greet(name: &str)`: Example command that returns a greeting

### Frontend-Backend Communication
- Frontend calls Rust commands using `invoke()` from `@tauri-apps/api/core`
- Example: `await invoke("greet", { name: "World" })`
- Commands must be registered in `tauri::generate_handler![]` in `lib.rs`

### Configuration Files (within `desktop/` directory)
- `desktop/src-tauri/tauri.conf.json`: Main Tauri configuration
- `desktop/src-tauri/Cargo.toml`: Rust dependencies
- `desktop/package.json`: Frontend dependencies and scripts
- App identifier: `com.openagents.OpenAgents`

### Key Dependencies
- Tauri plugins: `tauri-plugin-opener`
- Frontend: React 18, Vite, TypeScript
- Backend: Tauri 2, Serde for serialization