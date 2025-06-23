---
title: Roadmap
date: 2025-06-18
summary: Current state and future plans for OpenAgents
category: guide
order: 6
---

# Roadmap

This document outlines the current state of OpenAgents and our vision for the future. As an early-stage project, priorities may shift based on community feedback and technical discoveries.

> **Important**: OpenAgents is heavily in development. APIs and features are subject to change.

## Current State (June 2025)

### ✅ Implemented

#### Core SDK
- **Basic agent creation**: Identity generation with placeholder Nostr keys
- **Ollama integration**: Local AI inference with streaming support
- **TypeScript-first**: Full type safety and modern tooling
- **Effect foundation**: Service architecture ready for expansion

#### Web Framework
- **Psionic**: Server-side rendering with component explorer
- **WebTUI**: Terminal-inspired UI components with theming
- **Documentation site**: Built with our own tools

#### AI Capabilities
- **Local inference**: Privacy-preserving AI through Ollama
- **Streaming responses**: Real-time token generation
- **Embeddings**: Semantic search and similarity
- **Multi-model support**: Any Ollama-compatible model

#### Developer Experience
- **Monorepo structure**: Clean package boundaries
- **Hot reload**: Fast development cycles
- **TypeScript**: End-to-end type safety
- **Documentation**: Comprehensive guides and API reference

### 🚧 In Development

#### Nostr Integration
- **NIP-06 key derivation**: Deterministic identities from mnemonics
- **Event handling**: Type-safe Nostr event system
- **Relay connections**: WebSocket management
- **Status**: Nostr package exists but not integrated with SDK

#### AI Providers
- **Claude integration**: For MAX subscribers via CLI
- **Provider abstraction**: Unified interface
- **Status**: AI package implemented, needs SDK integration

### 📋 Placeholder/Stub Features

#### Economic Model
- **Lightning payments**: Invoice generation returns stub data
- **Agent economics**: Lifecycle states defined but not implemented
- **Bitcoin integration**: Vision established, implementation pending

#### Infrastructure
- **Container deployment**: Firecracker VM integration planned
- **Compute management**: Resource allocation stubs
- **Distributed agents**: Multi-node deployment concepts

## Short Term (Q3 2025)

### 1. Complete Core Integration
- [ ] Wire Nostr package into SDK for real key generation
- [ ] Integrate AI package for multiple providers
- [ ] Implement basic agent persistence
- [ ] Add agent communication via Nostr

### 2. Developer Experience
- [ ] Improve error messages and debugging
- [ ] Add more examples and tutorials
- [ ] Create project templates
- [ ] Enhance component explorer

### 3. Testing & Quality
- [ ] Increase test coverage to 80%+
- [ ] Add integration test suite
- [ ] Implement visual regression tests
- [ ] Performance benchmarks

### 4. Documentation
- [ ] API documentation generator
- [ ] Video tutorials
- [ ] Architecture deep dives
- [ ] Migration guides

## Medium Term (Q4 2025 - Q1 2026)

### 1. Lightning Network Integration
- [ ] Real Lightning invoice generation
- [ ] Payment flow implementation
- [ ] Wallet integration guides
- [ ] Micropayment streaming

### 2. Agent Capabilities
- [ ] Tool use (function calling)
- [ ] Memory and context management
- [ ] Multi-agent coordination
- [ ] Task scheduling

### 3. Infrastructure
- [ ] Container isolation (Firecracker)
- [ ] Resource monitoring
- [ ] Auto-scaling agents
- [ ] Distributed deployment

### 4. Ecosystem
- [ ] Plugin system for agent extensions
- [ ] Marketplace for agent templates
- [ ] Community agent registry
- [ ] Integration templates

## Long Term Vision (2026+)

### 1. Autonomous Agent Economy
- **Self-sustaining agents**: Earn Bitcoin to cover operational costs
- **Market dynamics**: Supply and demand for agent services
- **Reputation system**: Trust and quality metrics
- **Agent evolution**: Natural selection based on profitability

### 2. Technical Innovation
- **Zero-knowledge proofs**: Privacy-preserving agent interactions
- **Homomorphic encryption**: Compute on encrypted data
- **Decentralized compute**: P2P resource sharing
- **Cross-chain integration**: Beyond Bitcoin/Lightning

### 3. Use Cases
- **Personal assistants**: AI agents you truly own
- **Business automation**: Self-managing service providers
- **Research agents**: Autonomous scientific investigation
- **Creative collaborators**: AI partners for content creation

### 4. Standards & Protocols
- **Agent communication protocol**: Standardized message formats
- **Capability discovery**: How agents find each other
- **Payment negotiation**: Automated pricing mechanisms
- **Identity verification**: Trust without centralization

## How to Contribute

### Immediate Needs
1. **Testing**: Write tests for existing functionality
2. **Documentation**: Improve guides and examples
3. **Bug fixes**: Check GitHub issues
4. **Examples**: Build demo applications

### Feature Development
1. **Lightning integration**: Help implement real payments
2. **Nostr features**: Enhance protocol support
3. **AI providers**: Add new inference backends
4. **UI components**: Expand WebTUI library

### Research Areas
1. **Agent economics**: Model sustainable pricing
2. **Distributed systems**: Multi-agent coordination
3. **Security**: Adversarial agent scenarios
4. **Performance**: Optimize resource usage

## Design Principles

As we build toward this vision, we maintain these principles:

1. **Local-first**: User control over data and compute
2. **Open protocols**: No vendor lock-in
3. **Economic alignment**: Agents must provide real value
4. **Privacy preserving**: Minimal data exposure
5. **Developer friendly**: Great tooling and documentation

## Get Involved

### Community
- **GitHub**: [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents)
- **Discord**: Coming soon
- **Twitter**: [@OpenAgentsInc](https://twitter.com/OpenAgentsInc)

### Development
- Read the [Development Guide](./development)
- Check [GitHub Issues](https://github.com/OpenAgentsInc/openagents/issues)
- Join our contributor calls (schedule TBD)

### Feedback
- **Feature requests**: Open a GitHub issue
- **Bug reports**: Include reproduction steps
- **Documentation**: PRs always welcome
- **Ideas**: Start a discussion

## Disclaimer

This roadmap represents our current vision and is subject to change. Timelines are estimates, not commitments. We prioritize based on:

1. Community feedback
2. Technical feasibility
3. Resource availability
4. Strategic alignment

The best way to influence priorities is to contribute code, documentation, or constructive feedback.

---

*Building the future of autonomous AI agents, one commit at a time.* 🚀