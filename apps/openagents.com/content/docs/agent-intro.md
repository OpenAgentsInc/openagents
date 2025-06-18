---
title: OpenAgents Codebase Overview
date: 2025-06-18
summary: Comprehensive overview of the OpenAgents architecture and technology stack
category: reference
order: 10
---

# OpenAgents Codebase Overview

## Project Overview

OpenAgents is a platform for building Bitcoin-powered AI agents using open protocols. The project is structured as a TypeScript monorepo using pnpm workspaces, with a clear separation between reusable packages (libraries) and user-facing applications. The architecture emphasizes type safety, functional programming patterns through Effect.js, and local-first AI inference for privacy preservation.

## Technical Stack

The codebase is built on modern JavaScript tooling with TypeScript as the primary language. The runtime environment is Bun, chosen for its performance and built-in TypeScript support. For web applications, we use Elysia as the HTTP server framework. The entire project follows functional programming principles using Effect.js, which provides dependency injection, type-safe error handling, and service-oriented architecture patterns. Build tooling includes Vite for bundling, Vitest for testing, and custom Effect build utilities for multi-format package distribution.

## Package Architecture

The monorepo contains several core packages that form the foundation of the platform. The SDK package (`@openagentsinc/sdk`) provides the agent runtime with Bitcoin wallet integration and AI inference capabilities. The Nostr package (`@openagentsinc/nostr`) implements the decentralized protocol for agent communication and identity. Psionic (`@openagentsinc/psionic`) is our custom web framework that emphasizes server-side rendering and hypermedia patterns. The UI package (`@openagentsinc/ui`) provides WebTUI, a terminal-inspired component library using attribute-based styling rather than traditional CSS classes.

## Web Framework Philosophy

Psionic, our web framework, takes a deliberately minimalist approach focusing on server-side rendering and HTML-over-the-wire patterns. It includes a built-in component explorer for documentation and development. The framework uses template literals for HTML generation and includes a markdown service for content management. All styling is handled through CSS-in-JS patterns with support for multiple theme systems. The framework is designed specifically for the OpenAgents ecosystem and prioritizes simplicity over feature completeness.

## UI Component System

WebTUI, our UI library, provides terminal-inspired components that use semantic HTML with attribute-based styling. Components are styled using custom attributes like `is-="button"` and `box-="square"` rather than CSS classes. The system includes comprehensive theming support with predefined color schemes (Zinc, Catppuccin, Gruvbox, Nord). All components support ASCII-style box drawing for a distinctive retro-futuristic aesthetic. The library is framework-agnostic and works with any HTML rendering system.

## AI Integration Architecture

The AI package provides abstraction over multiple inference providers, though currently Ollama is the primary integration for local, privacy-preserving AI inference. The system supports streaming responses, embeddings generation, and multi-model selection. All AI operations are wrapped in Effect services for consistent error handling and dependency injection. The architecture is designed to support future integration with cloud providers while maintaining the local-first philosophy.

## Build System and Distribution

Each package follows a multi-step build process to support both ESM and CommonJS formats. The build pipeline includes TypeScript compilation, Babel transformation for annotations and CommonJS conversion, and Effect build utilities for final packaging. All packages are configured for tree-shaking with pure function annotations. The monorepo uses shared TypeScript configurations with composite builds for efficient compilation. Testing is handled by Vitest with Effect-specific testing utilities.

## Development Tooling and Standards

The project enforces code quality through ESLint configuration tailored for Effect.js patterns, pre-push hooks that run linting, type checking, and tests, and automated code generation for Effect package exports. Documentation is built into the platform itself using our own tools, with markdown content processed by the Psionic framework. The development environment supports hot reloading through Bun's built-in capabilities. All code follows strict TypeScript settings with Effect-specific language service enhancements for improved developer experience.