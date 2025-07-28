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
bun run desktop           # Run desktop app in development mode
bun run mobile            # Run mobile app in development mode
bun run ios               # Run mobile app on iOS simulator
bun run android           # Run mobile app on Android emulator
bun run build:desktop     # Build desktop app
bun run build:mobile      # Build mobile app
bun run clean             # Clean all node_modules and dist folders
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

# CRITICAL: NEVER manually edit Cargo.toml files
# ALWAYS use `cargo add` and `cargo remove` commands for Rust dependencies
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

## Effect-TS Integration

**IMPORTANT**: Before doing any work with Effect-TS in this codebase, you MUST:
1. Read the comprehensive Effect-TS integration guide at `docs/effect/README.md`
2. Review relevant linked documentation and PRs mentioned in that guide
3. Understand the current migration phase (Phase 3 of 4 completed)

### Key Effect-TS Requirements
- **Service Architecture**: All new features should use Effect services when appropriate
- **Error Handling**: Use tagged errors (e.g., `StorageError`, `AuthError`)
- **State Management**: Use STM for atomic updates in sync scenarios
- **Streaming**: Replace polling with Effect streams where possible
- **Testing**: Follow Effect testing patterns with 90%+ coverage goal

### Current Effect Integration
- **Confect**: Using Confect (Effect-TS + Convex) for database integration
- **Services**: Storage, APM, Authentication services already implemented
- **Performance**: <1ms latency for streaming (vs 25ms polling)

See `docs/effect/README.md` for patterns, examples, and migration status.

## Code Review Process with CodeRabbit

This repository uses **CodeRabbit AI** for automated code review. Here's how it works:

### How CodeRabbit Works
- **Automatic Triggering**: CodeRabbit automatically analyzes every pull request when opened
- **Analysis Time**: Takes 2-3 minutes to complete initial analysis and post review comments
- **Integration Requirement**: You should work with CodeRabbit to address relevant feedback before merging PRs

### Interacting with CodeRabbit

There are **3 ways** to chat with CodeRabbit:

**1. Review Comments**: Directly reply to a CodeRabbit review comment
```
Examples:
- "I pushed a fix in commit <commit_id>, please review it."
- "Explain this complex logic."
- "Open a follow-up GitHub issue for this discussion."
```

**2. Files and Specific Lines**: Tag @coderabbitai in a review comment on specific code
```
Examples:
- "@coderabbitai explain this code block."
- "@coderabbitai modularize this function."
```

**3. PR Comments**: Tag @coderabbitai in a PR comment for general questions
```
Examples:
- "@coderabbitai gather interesting stats about this repository and render them as a table."
- "@coderabbitai read src/utils.ts and explain its main purpose."
- "@coderabbitai help me debug CodeRabbit configuration file."
```

### Best Practices
- **Address Actionable Feedback**: Focus on "actionable" comments that suggest specific improvements
- **Collaborate Until Satisfied**: Continue the conversation until both you and CodeRabbit are satisfied with the code quality
- **Use for Learning**: Ask CodeRabbit to explain complex logic or suggest improvements
- **Quality Gate**: Treat CodeRabbit review as a quality gate before merging

### CodeRabbit Commands
Use these commands in PR comments:
- `@coderabbitai review` - Trigger incremental review
- `@coderabbitai full review` - Complete review from scratch  
- `@coderabbitai summary` - Regenerate PR summary
- `@coderabbitai generate unit tests` - Generate tests for the PR
- `@coderabbitai resolve` - Mark all review comments as resolved

## Implementation Timeline Expectations

**CRITICAL**: When implementing features or fixes:

### Same-Day Delivery
- **Default Expectation**: Complete implementation within the same day when requested
- **No Week-Long Estimates**: Avoid providing timeline estimates in weeks or days unless specifically complex architectural changes
- **Immediate Action**: Begin implementation immediately rather than providing lengthy planning phases
- **Iterative Progress**: Make continuous progress with regular updates and commits

### Planning vs. Execution Balance
- **Brief Planning**: Limit planning to essential architecture decisions (max 15-30 minutes)
- **Rapid Prototyping**: Implement working solutions quickly, then iterate
- **Progressive Enhancement**: Start with basic functionality, enhance incrementally
- **Continuous Integration**: Make frequent commits to track progress

### Communication Style
- **Progress Updates**: Provide frequent updates on implementation progress
- **Issue Comments**: Update GitHub issues at key milestones, not just at completion
- **Commit Messages**: Use clear, descriptive commit messages for tracking
- **No Timeline Promises**: Avoid specific time estimates unless absolutely certain

### Implementation Approach
1. **Start Coding Immediately**: Begin with core functionality
2. **Test While Building**: Write tests alongside implementation
3. **Document as You Go**: Update documentation with each major component
4. **Deploy Early**: Get working versions ready for testing ASAP