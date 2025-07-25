# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an OpenAgents monorepo with desktop and mobile applications, built with:
- **Desktop**: Tauri app (React + TypeScript + Vite frontend, Rust backend)
- **Mobile**: Expo app (React Native + TypeScript)
- **Backend**: Shared Convex backend
- **Package Manager**: Bun workspaces

### Project Structure
This is a **Bun workspace monorepo** with the following structure:
```
openagents/               # Root directory
├── apps/                 # Applications
│   ├── desktop/          # Tauri desktop app
│   │   ├── src/          # Frontend React/TypeScript code
│   │   ├── src-tauri/    # Backend Rust code
│   │   └── package.json  # Desktop app dependencies
│   └── mobile/           # Expo mobile app
│       ├── src/          # React Native code
│       └── package.json  # Mobile app dependencies
├── packages/             # Shared packages
│   ├── convex/           # Shared Convex backend
│   └── shared/           # Shared utilities and types
├── docs/                 # Documentation
├── scripts/              # Build and utility scripts
└── package.json          # Root workspace configuration
```

**Work from the root directory** for workspace commands, or **`apps/desktop/`** for desktop-specific commands.

## Commands

### Workspace Commands (from root)
```bash
bun install               # Install all workspace dependencies
bun run dev:desktop       # Run desktop app in development mode
bun run dev:mobile        # Run mobile app in development mode
bun run build:desktop     # Build desktop app
bun run build:mobile      # Build mobile app
bun run clean            # Clean all node_modules and dist folders
```

### Desktop Development (from apps/desktop/)
```bash
cd apps/desktop
bun install           # Install dependencies (if needed)
bun run dev           # Run Tauri app in development mode (no HMR by default)
bun run dev:hmr       # Run Tauri app with HMR (may steal focus)
bun run dev:vite      # Start Vite dev server only (no HMR)
bun run dev:vite:hmr  # Start Vite dev server only with HMR
```

### Desktop Building (from apps/desktop/)
```bash
cd apps/desktop
bun run build        # Build frontend (runs tsc && vite build)
bun run tauri build  # Build the complete Tauri app
```

### Package Management
**IMPORTANT**: This is a Bun workspace, so dependency management differs:

**Workspace dependencies (shared across apps)**: 
```bash
# From root directory
bun add <package> -w          # Add to workspace root
```

**App-specific dependencies**:
```bash
# Desktop app dependencies
bun add <package> --cwd apps/desktop

# Mobile app dependencies  
bun add <package> --cwd apps/mobile

# Rust dependencies (from apps/desktop/src-tauri/)
cd apps/desktop/src-tauri
cargo add <package>
```

**Shared package dependencies**:
```bash
# Convex package
bun add <package> --cwd packages/convex

# Shared utilities
bun add <package> --cwd packages/shared
```

### Other Commands
```bash
# From apps/desktop/
bun run preview      # Preview production build

# From root (workspace)
bun run install:all  # Reinstall all dependencies
```

**IMPORTANT**: Never run `bun run dev`, `bun run dev:hmr`, or similar development server commands automatically. The user will run these commands themselves. You should only compile/build to check for errors using commands like `cargo build` or `cargo check`.

## Architecture

### Frontend Structure (within `apps/desktop/` directory)
- Entry point: `apps/desktop/src/main.tsx`
- Main component: `apps/desktop/src/App.tsx`
- Uses Tauri API for IPC: `@tauri-apps/api/core`
- TypeScript configuration: `apps/desktop/tsconfig.json`

### Backend Structure (within `apps/desktop/` directory)
- Rust entry point: `apps/desktop/src-tauri/src/main.rs`
- Application logic: `apps/desktop/src-tauri/src/lib.rs`
- Tauri commands are defined with `#[tauri::command]` macro
- Current commands:
  - `greet(name: &str)`: Example command that returns a greeting

### Frontend-Backend Communication
- Frontend calls Rust commands using `invoke()` from `@tauri-apps/api/core`
- Example: `await invoke("greet", { name: "World" })`
- Commands must be registered in `tauri::generate_handler![]` in `lib.rs`

### Configuration Files
**Desktop app** (within `apps/desktop/` directory):
- `apps/desktop/src-tauri/tauri.conf.json`: Main Tauri configuration
- `apps/desktop/src-tauri/Cargo.toml`: Rust dependencies
- `apps/desktop/package.json`: Frontend dependencies and scripts

**Workspace** (root level):
- `package.json`: Workspace configuration and shared scripts
- `packages/convex/convex.json`: Convex backend configuration
- `packages/shared/package.json`: Shared utilities package
- App identifier: `com.openagents.OpenAgents`

### Key Dependencies
**Desktop App**:
- Tauri plugins: `tauri-plugin-opener`
- Frontend: React 18, Vite, TypeScript
- Backend: Tauri 2, Serde for serialization

**Mobile App**:
- React Native, Expo SDK
- TypeScript

**Shared**:
- Convex: Real-time backend
- Bun: Package manager and runtime
- TypeScript: Type safety across all packages