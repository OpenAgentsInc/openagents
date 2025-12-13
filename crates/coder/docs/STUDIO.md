# Coder Studio

Coder Studio is the familiar vibe-coding surface - the editor + file tree + terminal + preview that developers expect. But Studio is explicitly a *client* to the MechaCoder platform, not the platform itself.

---

## Core IDE Features

### Code Editor

**Technology:** Monaco Editor (VS Code core) or custom Dioxus editor

| Feature | Status | Description |
|---------|--------|-------------|
| Syntax Highlighting | MVP | 50+ languages |
| IntelliSense | MVP | AI-powered completions |
| Multi-cursor | MVP | Multiple selection editing |
| Find & Replace | MVP | Regex support |
| Minimap | MVP | Code overview sidebar |
| Code Folding | MVP | Collapse regions |
| Git Integration | Phase 2 | Inline diff, blame |
| Vim Mode | Phase 2 | Optional keybindings |
| Split View | Phase 2 | Side-by-side editing |
| Collaborative Editing | Phase 3 | Real-time multiplayer |

**AI Enhancements:**
- Inline completions (Copilot-style)
- Code explanations on hover
- Refactoring suggestions
- Bug detection
- Documentation generation

### File Explorer

| Feature | Status | Description |
|---------|--------|-------------|
| Tree View | MVP | Hierarchical file display |
| File Search | MVP | Fuzzy filename matching |
| New File/Folder | MVP | Quick creation |
| Rename/Delete | MVP | Context menu actions |
| Drag & Drop | MVP | File reorganization |
| File Icons | MVP | Language-specific icons |
| Git Status | Phase 2 | Modified/staged indicators |
| OANIX Mounts | Phase 2 | Show /cap/*, /logs |
| Search in Files | MVP | Content search |
| File Upload | MVP | Drag files from desktop |

### Terminal

| Feature | Status | Description |
|---------|--------|-------------|
| PTY Support | MVP | Full terminal emulation |
| Multiple Terminals | MVP | Tabbed terminals |
| Command History | MVP | Up/down arrow navigation |
| Copy/Paste | MVP | Clipboard support |
| Theming | MVP | Match IDE theme |
| Split Terminal | Phase 2 | Side-by-side terminals |
| OANIX Integration | Phase 2 | Connect to /logs stream |
| Link Detection | MVP | Clickable URLs |

### Preview Panel

| Feature | Status | Description |
|---------|--------|-------------|
| Live Reload | MVP | Auto-refresh on save |
| Hot Module Replacement | MVP | Preserve state on change |
| Device Frames | MVP | Mobile/tablet preview |
| Responsive Mode | MVP | Drag to resize |
| Console Output | MVP | Browser console in IDE |
| Isolated Frame | MVP | Sandboxed iframe |
| External Preview | MVP | Open in new tab |

---

## AI Code Completion

| Feature | Status | Description |
|---------|--------|-------------|
| Inline Suggestions | MVP | Ghost text completions |
| Multi-line | MVP | Complete entire blocks |
| Accept/Reject | MVP | Tab to accept |
| Partial Accept | Phase 2 | Accept word-by-word |
| Context Aware | MVP | Uses open files |
| Language Specific | MVP | Framework-aware |

---

## Templates

**Built-in Templates:**

| Template | Stack | Description |
|----------|-------|-------------|
| Landing Page | React + Tailwind | Marketing site |
| SaaS Dashboard | React + shadcn | Admin dashboard |
| API Server | Rust WASM | Backend API |
| Full Stack | React + Rust | Complete app |
| Blog | MDX + React | Content site |
| E-commerce | React + Stripe | Store |

| Feature | Status | Description |
|---------|--------|-------------|
| One-click Setup | MVP | Instant project creation |
| Customization | MVP | Modify before create |
| Preview | MVP | See template in action |
| Community Templates | Phase 2 | User-submitted |
| Private Templates | Phase 2 | Team templates |

---

## Environment Management

| Feature | Status | Description |
|---------|--------|-------------|
| .env Support | MVP | Environment variables |
| Secrets Management | MVP | Encrypted storage |
| Multiple Environments | MVP | Dev/staging/prod |
| Environment Cloning | Phase 2 | Duplicate configs |
| Shared Secrets | Phase 2 | Team secret sharing |

---

*Last Updated: December 2025*
