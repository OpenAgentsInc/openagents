# OpenAgents

**AI-Powered Development Platform** - Seamlessly integrate Claude Code's AI development capabilities across desktop and mobile with **real-time synchronization**. Work on your GitHub repositories anywhere, anytime.

Built as a Bun workspace monorepo with Convex real-time backend and Effect-TS for robust, type-safe functionality.

Being built in public. See [intro video](https://x.com/OpenAgentsInc/status/1948214004268064771)

## 🎯 What is OpenAgents?

OpenAgents transforms how you interact with Claude Code by providing:

- **🔄 Cross-Platform Continuity**: Start a coding session on desktop, continue on mobile, switch back seamlessly
- **📱 Mobile-First Development**: Access your GitHub repositories and AI assistance from anywhere
- **🤖 AI-Powered Code Analysis**: Leverage Claude's advanced code understanding across your entire project
- **🔐 Enterprise-Ready Security**: OAuth-based GitHub integration with support for private repositories
- **⚡ Real-Time Collaboration**: Instant synchronization between all your devices

### Use Cases

- **Code Review on the Go**: Review pull requests and get AI insights during commute
- **Quick Bug Fixes**: Address urgent issues from mobile when away from desk  
- **Architecture Planning**: Brainstorm solutions with AI assistance anywhere
- **Learning & Documentation**: Ask Claude about unfamiliar codebases while mobile
- **Project Management**: Track development progress across devices

## Quick Start

```bash
# Install all dependencies
bun install

# Run desktop app
bun run desktop

# Run mobile app  
bun run mobile

# Start Convex backend
bun run convex
```

## ✨ Core Features

### 🔄 Real-Time Synchronization
- **Bidirectional Session Sync**: Claude Code sessions automatically sync between desktop and mobile
- **Message Threading**: All conversations maintain context across devices
- **Live Updates**: See changes instantly as they happen on other devices
- **Session Handoff**: Seamlessly switch between platforms mid-conversation

### 🐙 GitHub Integration  
- **Repository Management**: Browse and select from your GitHub repositories
- **Private Repository Support**: Full OAuth integration with private repo access
- **Smart Onboarding**: Guided setup connecting your most-used repositories
- **Repository Context**: Claude has full understanding of your project structure

### 📱 Mobile Experience
- **Native Performance**: Built with Expo/React Native for smooth mobile experience
- **Touch-Optimized UI**: Designed specifically for mobile interaction patterns
- **Offline Resilience**: Graceful handling of network connectivity changes
- **Push Notifications**: Stay updated on session activity (planned)

### 🖥️ Desktop Integration
- **Tauri Architecture**: Lightweight, secure desktop application
- **System Integration**: Native OS interactions and file system access
- **Hot Module Replacement**: Fast development workflow with HMR support
- **Multi-Window Support**: Work with multiple Claude sessions simultaneously

### 🔐 Security & Privacy
- **OAuth 2.0 Authentication**: Industry-standard GitHub authentication
- **Secure Token Storage**: Encrypted credential management across platforms
- **Zero-Trust Architecture**: All communications encrypted and authenticated
- **Audit Logging**: Comprehensive logging for enterprise compliance

## Commands

**Development:**
- `bun run desktop` - Run desktop app in development
- `bun run mobile` - Run mobile app in development  
- `bun run ios` - Run on iOS simulator
- `bun run android` - Run on Android emulator
- `bun run convex` - Start Convex backend

**Building:**
- `bun run build:desktop` - Build desktop app
- `bun run build:ios` - Build iOS production app
- `bun run build:android` - Build Android production app

**Deployment:**
- `bun run submit` - Submit iOS app to App Store
- `bun run update` - Publish OTA update to production

**Utilities:**
- `bun install` - Install all dependencies
- `bun run clean` - Clean node_modules and dist folders

## 🏗️ Architecture

### Tech Stack
- **Frontend**: React 19 + TypeScript for universal components
- **Desktop**: Tauri 2 (Rust backend + React frontend)
- **Mobile**: Expo SDK 53 + React Native 0.79
- **Backend**: Convex (real-time database + serverless functions)
- **Functional Programming**: Effect-TS for robust error handling and data flow
- **Authentication**: OpenAuth + GitHub OAuth 2.0
- **Package Management**: Bun workspaces for monorepo management

### Project Structure
```
openagents/
├── apps/
│   ├── desktop/          # Tauri desktop application
│   │   ├── src/          # React/TypeScript frontend
│   │   └── src-tauri/    # Rust backend
│   ├── mobile/           # Expo mobile application
│   │   ├── src/          # React Native/TypeScript
│   │   └── convex/       # Mobile-specific Convex integration
│   └── auth/             # OpenAuth server (Cloudflare Workers)
├── packages/
│   ├── convex/           # Shared Convex backend
│   │   ├── confect/      # Effect-TS + Convex integration
│   │   └── convex/       # Database schema and functions
│   └── shared/           # Shared utilities and types
└── docs/                 # Documentation and guides
```

### Data Flow Architecture
1. **Authentication**: OAuth flow via OpenAuth server stores GitHub tokens
2. **Repository Sync**: GitHub API integration fetches user's repositories  
3. **Session Management**: Convex manages Claude Code session state
4. **Real-Time Updates**: Convex subscriptions push changes to all clients
5. **Cross-Platform State**: Effect-TS ensures type-safe data transformations

## 🔄 Development Workflow

### Getting Started
1. **Clone & Install**: `git clone` and `bun install` 
2. **GitHub OAuth**: Configure OAuth app credentials in environment
3. **Convex Setup**: Deploy Convex schema and functions
4. **Auth Server**: Deploy OpenAuth server to Cloudflare Workers
5. **Development**: Run desktop/mobile apps with `bun run desktop/mobile`

### Key Development Patterns
- **Effect-TS First**: All business logic uses Effect for error handling
- **Confect Integration**: Custom Effect-TS + Convex integration layer
- **Type Safety**: End-to-end TypeScript from frontend to backend
- **Real-Time by Default**: All data mutations automatically sync
- **Mobile-First UI**: Components designed for touch with desktop adaptation

## 📊 Current Status & Roadmap

### ✅ Implemented Features (v0.1)
- **Repository Selection Onboarding**: Complete GitHub integration with OAuth
- **Two-Way Session Sync**: Desktop ↔ Mobile Claude Code sessions  
- **Real-Time Messaging**: Cross-platform message synchronization
- **GitHub Repository Access**: Support for private and public repositories
- **Mobile Session Creation**: Start Claude Code sessions from mobile
- **Effect-TS Integration**: Type-safe functional programming patterns
- **Secure Authentication**: OpenAuth + GitHub OAuth 2.0 implementation

### 🚧 In Development
- **Enhanced Mobile UI**: Improved touch interactions and animations
- **Offline Support**: Local caching and sync when connection restored
- **Push Notifications**: Real-time alerts for session activity
- **Multi-Repository Support**: Work with multiple repositories simultaneously
- **Session Templates**: Predefined conversation starters for common tasks

### 🎯 Planned Features
- **Team Collaboration**: Shared sessions and collaborative code review
- **CI/CD Integration**: Connect with GitHub Actions and deployment workflows  
- **Code Analysis Dashboard**: Visual insights into codebase patterns and AI suggestions
- **Voice Interaction**: Voice-to-text for mobile code discussions
- **IDE Extensions**: Integration with VS Code, JetBrains IDEs
- **Enterprise SSO**: SAML, LDAP, and other enterprise authentication methods

### 🚀 Performance Metrics
- **Session Sync Latency**: <100ms average across platforms
- **Repository Load Time**: <2s for typical repositories (5-1000 files)
- **Mobile App Size**: <15MB (optimized for global distribution)
- **Desktop Memory Usage**: <200MB typical, <500MB peak
- **Real-Time Updates**: <50ms P95 latency via Convex WebSockets

---

## 🤝 Contributing

OpenAgents is built in public. We welcome contributions from developers interested in advancing AI-powered development tools.

### Development Setup
1. Fork the repository
2. Follow the Quick Start guide above  
3. Check out our [Development Guide](docs/DEVELOPMENT.md) for detailed setup
4. Submit PRs with comprehensive tests and documentation

### Project Goals
- **Developer Experience First**: Every feature should enhance, not complicate, the development workflow
- **Cross-Platform Parity**: Mobile and desktop should offer equivalent capabilities
- **Performance Obsessed**: Real-time sync must feel instantaneous
- **Security by Design**: Enterprise-grade security from day one
- **Open Source Sustainability**: Build a thriving ecosystem around AI development tools

---

*OpenAgents - Where AI meets development, everywhere.*
