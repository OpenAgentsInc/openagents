# OpenAgents

Voice-first AI development platform that lets you orchestrate multiple coding agents from anywhere. Built with Tauri for true cross-platform support on desktop and mobile.

## Voice-Driven Development

Control AI coding agents through natural conversation. Start a task on your phone during your commute, have your desktop execute it, and review results from anywhere. No more being tethered to your IDE.

## Multi-Agent Architecture

- **Universal Orchestration**: Run Claude Code, Amp, Codex, or local models side-by-side
- **Shared Context**: All agents access the same conversation history and learned patterns  
- **Intelligent Routing**: Automatically selects the best agent for each task
- **Extensible**: Add new agents via standardized interfaces

## Privacy-First Design

- **Local Execution**: Everything runs on your devices
- **Your Data**: Conversations and code never leave your control
- **Optional Sync**: End-to-end encrypted sync between devices
- **Open Protocols**: Built on MCP (Model Context Protocol) and open standards

## Cross-Platform Experience

Built with Tauri + React + Rust for native performance everywhere:

### Mobile
- Push-to-talk voice interface
- Real-time task monitoring
- Background execution status

### Desktop  
- Multi-window agent management
- Local model support
- File system integration
- Full code execution environment

## Getting Started

```bash
# Install dependencies
bun install

# Development
bun run tauri dev

# Build for production
bun run tauri build
```

## Beyond Traditional IDEs

While others build better VS Code forks, OpenAgents reimagines development as conversation. Focus on what you want to build, not how to navigate complex UIs.