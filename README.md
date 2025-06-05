# OpenAgents

OpenAgents is a platform for AI agents using open protocols.

Our previous flagship product (v4) is an agentic chat app live at [openagents.com](https://openagents.com).

This repo holds our new cross-platform version (v5), a work in progress.

## Packages

This monorepo contains the following packages:

### Core Domain
- **`@openagentsinc/domain`** - Core business logic, API contracts, and shared types
  - Defines API specifications using Effect Schema
  - Contains branded types, error definitions, and domain entities
  - Schema-first development approach for type safety
  - Foundation package that other packages depend on

### Backend Services
- **`@openagentsinc/server`** - HTTP server implementation
  - Implements API contracts defined in domain package
  - Built with Effect's HTTP platform
  - Contains repository implementations and service handlers
  - Provides REST API endpoints for the application

### User Interfaces
- **`@openagentsinc/cli`** - Command-line interface client
  - Interactive CLI tool built with Effect CLI framework
  - Consumes server APIs via generated HTTP clients
  - Integrates with AI package for intelligent features
  - User-facing tool for system interaction

- **`@openagentsinc/ui`** - Shared UI component library
  - Modern React 19 components with Tailwind CSS v4
  - Includes pane system (draggable/resizable windows)
  - Hotbar navigation with keyboard shortcuts
  - Built with Radix UI primitives and Framer Motion
  - Platform-agnostic design for future mobile support

- **`@openagentsinc/playground`** - UI component testing environment
  - Vite-based development playground
  - Showcases and tests UI components
  - Private package for internal development
  - Integrates AI features for component demos

### AI Integration
- **`@openagentsinc/ai`** - Unified AI provider integration
  - Effect-based abstraction for multiple AI services
  - Full Claude Code CLI/SDK integration for MAX subscribers
  - Session management and streaming support
  - Provider-agnostic API with extensible architecture
  - Used by both CLI and Playground packages
