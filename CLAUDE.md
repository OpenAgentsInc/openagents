# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents Tauri desktop application built with:
- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust with Tauri framework
- **Package Manager**: Bun (not npm/yarn)
- **Main Project Directory**: `/openagents-tauri/`

## Commands

### Development
```bash
cd openagents-tauri
bun install           # Install dependencies
bun run dev           # Run Tauri app in development mode (no HMR by default)
bun run dev:hmr       # Run Tauri app with HMR (may steal focus)
bun run dev:vite      # Start Vite dev server only (no HMR)
bun run dev:vite:hmr  # Start Vite dev server only with HMR
```

### Building
```bash
cd openagents-tauri
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

### Frontend Structure
- Entry point: `src/main.tsx`
- Main component: `src/App.tsx`
- Uses Tauri API for IPC: `@tauri-apps/api/core`
- TypeScript configuration: `tsconfig.json`

### Backend Structure
- Rust entry point: `src-tauri/src/main.rs`
- Application logic: `src-tauri/src/lib.rs`
- Tauri commands are defined with `#[tauri::command]` macro
- Current commands:
  - `greet(name: &str)`: Example command that returns a greeting

### Frontend-Backend Communication
- Frontend calls Rust commands using `invoke()` from `@tauri-apps/api/core`
- Example: `await invoke("greet", { name: "World" })`
- Commands must be registered in `tauri::generate_handler![]` in `lib.rs`

### Configuration Files
- `src-tauri/tauri.conf.json`: Main Tauri configuration
- `src-tauri/Cargo.toml`: Rust dependencies
- `package.json`: Frontend dependencies and scripts
- App identifier: `com.openagents.OpenAgents`

### Key Dependencies
- Tauri plugins: `tauri-plugin-opener`
- Frontend: React 18, Vite, TypeScript
- Backend: Tauri 2, Serde for serialization