# Vibe UI Implementation Plan

Build a complete UI vision for Vibe following marketplace crate patterns. This represents the fully-realized product 3 years from now.

## Architecture Overview

```
crates/vibe/src/
├── lib.rs              # Module exports (update)
├── screen.rs           # Main VibeScreen component (NEW)
├── types.rs            # All data types + mock constructors (NEW)
├── projects/           # Projects tab components (NEW)
│   ├── mod.rs
│   ├── project_grid.rs
│   ├── project_card.rs
│   └── template_picker.rs
├── editor/             # Editor tab components (NEW)
│   ├── mod.rs
│   ├── file_tree.rs
│   ├── code_editor.rs
│   ├── preview_panel.rs
│   ├── terminal_panel.rs
│   └── agent_panel.rs
├── database/           # Database tab components (NEW)
│   ├── mod.rs
│   ├── table_browser.rs
│   ├── sql_editor.rs
│   └── schema_view.rs
└── deploy/             # Deploy tab components (NEW)
    ├── mod.rs
    ├── deploy_panel.rs
    ├── domain_manager.rs
    └── analytics_view.rs
```

## Tab Structure

| Tab | Purpose | Key Components |
|-----|---------|----------------|
| Projects | Browse/create projects | ProjectGrid, TemplatePickr |
| Editor | Main IDE workspace | FileTree, CodeEditor, Preview, Terminal, AgentPanel |
| Database | Visual DB management | TableBrowser, SQLEditor, SchemaView |
| Deploy | Hosting & analytics | DeployPanel, DomainManager, AnalyticsView |

## Implementation Steps

### Step 1: types.rs - Data Types
Create all data structures with mock constructors:
- `VibeTab` enum
- `Project`, `ProjectTemplate`, `ProjectFile`
- `DatabaseTable`, `DatabaseColumn`, `DatabaseRow`
- `Deployment`, `Domain`, `AnalyticsData`
- Mock functions for each type

### Step 2: screen.rs - Main Component
Following marketplace pattern:
- `VibeScreen` struct with focus_handle, current_tab, tab states
- Tab switching UI
- Resource bar (compute usage, credits)
- Render implementation with tab-based content

### Step 3: projects/ - Project Management
- `ProjectGrid` - Grid of project cards with search
- `ProjectCard` - Individual project display
- `TemplatePicker` - Browse starter templates

### Step 4: editor/ - IDE Workspace
- `FileTree` - Project file browser with OANIX namespace
- `CodeEditor` - Syntax-highlighted code editing
- `PreviewPanel` - Live app preview
- `TerminalPanel` - OANIX terminal integration
- `AgentPanel` - AI agent task feed and controls

### Step 5: database/ - Database Management
- `TableBrowser` - Browse tables and records
- `SQLEditor` - Execute SQL queries
- `SchemaView` - Visual schema editor

### Step 6: deploy/ - Deployment Pipeline
- `DeployPanel` - One-click deploy controls
- `DomainManager` - Custom domain configuration
- `AnalyticsView` - Traffic and usage metrics

## Mock Data Strategy

Each component gets realistic mock data:
- 6-8 sample projects (SaaS dashboard, landing page, e-commerce, etc.)
- 10+ template categories
- Sample database with users, products, orders tables
- Deployment history with versions
- Analytics with traffic graphs

## Design Aesthetic

Following Bloomberg Terminal / marketplace style:
- Dense, information-rich layouts
- Text-first, minimal icons
- Sharp edges, no rounded corners
- Monospace fonts for code
- Subtle borders, muted colors
- No emojis in production UI
