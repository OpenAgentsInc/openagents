# Agents

The agent-orchestrator provides 7 specialized agents, each optimized for specific tasks. Agents are configured with model, prompt, temperature, and permission settings.

## Agent Overview

| Agent | Model | Role | Mode |
|-------|-------|------|------|
| **Sisyphus** | codex-sonnet-4-5 | Primary orchestrator | Primary |
| **Oracle** | gpt-5.2 | Architecture, debugging | Subagent |
| **Librarian** | codex-sonnet-4-5 | External docs, OSS | Subagent |
| **Explore** | grok-code | Codebase navigation | Subagent |
| **Frontend** | gemini-3-pro | UI/UX development | Subagent |
| **DocWriter** | gemini-3-pro | Documentation | Subagent |
| **Multimodal** | gemini-3-flash | PDF/image analysis | Subagent |

## Sisyphus (Primary)

The primary orchestrator agent. Named after the myth — humans roll their boulder every day, so do agents.

**Responsibilities:**
- Parse implicit requirements from explicit requests
- Delegate specialized work to subagents
- Coordinate parallel execution
- Track task completion

**Behavior:**
- Never works alone when specialists are available
- Fires explore/librarian agents in background for research
- Delegates frontend visual changes to frontend agent
- Consults Oracle for architecture decisions

**Configuration:**
```rust
AgentConfig {
    name: "sisyphus".to_string(),
    model: "codex-sonnet-4-5".to_string(),
    temperature: 0.1,
    mode: AgentMode::Primary,
    permission: AgentPermission {
        edit: PermissionLevel::Allow,
        bash: BashPermission::AllowWithDenyList(vec![
            "rm -rf".to_string(),
            "git push --force".to_string(),
        ]),
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## Oracle

Senior engineering advisor for architecture decisions and hard debugging.

**When to consult:**
- Complex architecture design
- After 2+ failed fix attempts
- Multi-system tradeoffs
- Security/performance concerns

**When NOT to consult:**
- Simple file operations
- First attempt at any fix
- Questions answerable from code
- Trivial decisions

**Configuration:**
```rust
AgentConfig {
    name: "oracle".to_string(),
    model: "gpt-5.2".to_string(),
    temperature: 0.3,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Deny,  // Read-only
        bash: BashPermission::Deny,
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## Librarian

External reference specialist — searches docs, GitHub, and OSS implementations.

**Trigger phrases:**
- "How do I use [library]?"
- "What's the best practice for [feature]?"
- "Find examples of [pattern]"

**Capabilities:**
- Context7 library documentation
- Exa web search
- Grep.app GitHub code search
- Official API references

**Configuration:**
```rust
AgentConfig {
    name: "librarian".to_string(),
    model: "codex-sonnet-4-5".to_string(),
    temperature: 0.2,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Deny,
        bash: BashPermission::Deny,
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## Explore

Fast codebase navigation and pattern discovery.

**Use cases:**
- Multiple search angles needed
- Unfamiliar module structure
- Cross-layer pattern discovery
- "Where is X?" questions

**Characteristics:**
- Low temperature for precise results
- Fast execution
- Read-only access

**Configuration:**
```rust
AgentConfig {
    name: "explore".to_string(),
    model: "grok-code".to_string(),
    temperature: 0.0,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Deny,
        bash: BashPermission::AllowWithAllowList(vec![
            "find".to_string(),
            "grep".to_string(),
            "rg".to_string(),
        ]),
        webfetch: PermissionLevel::Deny,
    },
    // ...
}
```

## Frontend

UI/UX specialist for visual changes.

**Handles:**
- Styling (colors, spacing, typography)
- Layout (flexbox, grid, responsive)
- Animations and transitions
- Visual component design

**Does NOT handle:**
- Business logic
- API integration
- State management
- Data fetching

**Configuration:**
```rust
AgentConfig {
    name: "frontend".to_string(),
    model: "gemini-3-pro".to_string(),
    temperature: 0.4,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Allow,
        bash: BashPermission::AllowWithAllowList(vec![
            "npm".to_string(),
            "yarn".to_string(),
            "pnpm".to_string(),
        ]),
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## DocWriter

Technical documentation specialist.

**Creates:**
- README files
- API documentation
- Architecture docs
- User guides

**Configuration:**
```rust
AgentConfig {
    name: "docwriter".to_string(),
    model: "gemini-3-pro".to_string(),
    temperature: 0.3,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Allow,
        bash: BashPermission::Deny,
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## Multimodal

PDF and image analysis specialist.

**Analyzes:**
- PDF documents
- Diagrams and flowcharts
- Screenshots
- Design mockups

**Configuration:**
```rust
AgentConfig {
    name: "multimodal".to_string(),
    model: "gemini-3-flash".to_string(),
    temperature: 0.2,
    mode: AgentMode::Subagent,
    permission: AgentPermission {
        edit: PermissionLevel::Deny,
        bash: BashPermission::Deny,
        webfetch: PermissionLevel::Allow,
    },
    // ...
}
```

## Agent Modes

```rust
pub enum AgentMode {
    Primary,    // Main orchestrator (only one)
    Subagent,   // Specialized worker
    All,        // Can operate in either mode
}
```

## Permission Levels

```rust
pub enum PermissionLevel {
    Allow,  // Always permitted
    Ask,    // Requires confirmation
    Deny,   // Never permitted
}

pub enum BashPermission {
    Allow,                          // All commands
    Deny,                           // No commands
    AllowWithAllowList(Vec<String>), // Only listed
    AllowWithDenyList(Vec<String>),  // All except listed
}
```

## Custom Agents

Register custom agents at runtime:

```rust
let mut registry = AgentRegistry::new();

registry.register(AgentConfig {
    name: "my-agent".to_string(),
    model: "my-model".to_string(),
    prompt: include_str!("my_prompt.md").to_string(),
    temperature: 0.5,
    description: "My custom agent".to_string(),
    mode: AgentMode::Subagent,
    tools: HashMap::new(),
    permission: AgentPermission::default(),
});
```

## Agent Selection

Sisyphus (the primary) decides which subagent to use:

| Task Type | Agent |
|-----------|-------|
| Architecture decisions | Oracle |
| External documentation | Librarian |
| Codebase navigation | Explore |
| UI/UX changes | Frontend |
| Writing docs | DocWriter |
| Analyzing images | Multimodal |

## Parallel Execution

Multiple subagents can run in parallel:

```rust
let bg = BackgroundTaskManager::new();

// Fire multiple agents concurrently
let explore_task = bg.spawn(session, "explore", "Find auth code", "Auth search");
let librarian_task = bg.spawn(session, "librarian", "Find JWT docs", "JWT docs");

// Collect results
let explore_result = bg.get_output(&explore_task, true).await?;
let librarian_result = bg.get_output(&librarian_task, true).await?;
```

## DSPy Signatures (Wave 9)

Each agent has a corresponding DSPy signature that can replace static prompt-based delegation with learned, optimizable behavior.

### DelegationSignature (Sisyphus)

Decides which subagent should handle a task.

```rust
use agent_orchestrator::{DelegationSignature, TargetAgent};
use dsrs::predictors::Predict;

let sig = DelegationSignature::new();
let predictor = Predict::new(sig);

// Inputs: task_description, available_agents, current_workload
// Outputs: assigned_agent, task_refinement, expected_deliverables, fallback_agent
```

### ArchitectureSignature (Oracle) - CoT

Chain-of-thought reasoning for architecture decisions.

```rust
use agent_orchestrator::{ArchitectureSignature, ArchitectureComplexity};

let sig = ArchitectureSignature::new();

// Inputs: requirements, existing_architecture, constraints
// Outputs: reasoning (CoT), proposed_changes, tradeoffs, risks, complexity
```

### LibraryLookupSignature (Librarian)

Finds documentation and usage examples.

```rust
use agent_orchestrator::LibraryLookupSignature;

let sig = LibraryLookupSignature::new();

// Inputs: query, library_name, context
// Outputs: findings, sources, code_examples, confidence
```

### CodeExplorationSignature (Explore)

Fast codebase navigation and pattern search.

```rust
use agent_orchestrator::{CodeExplorationSignature, SearchType};

let sig = CodeExplorationSignature::new();

// SearchType: Definition, References, Pattern, Usage, CallGraph
// Inputs: query, search_type, scope
// Outputs: locations, code_snippets, related_files, confidence
```

### UIDesignSignature (Frontend)

UI/UX design with accessibility focus.

```rust
use agent_orchestrator::UIDesignSignature;

let sig = UIDesignSignature::new();

// Inputs: design_request, existing_styles, constraints
// Outputs: css_changes, component_structure, design_rationale, accessibility_notes
```

### DocumentationSignature (DocWriter)

Technical documentation generation.

```rust
use agent_orchestrator::{DocumentationSignature, DocType};

let sig = DocumentationSignature::new();

// DocType: Readme, ApiRef, Guide, Comment, Changelog
// Inputs: doc_type, subject, audience, existing_docs
// Outputs: content, structure, examples, cross_references
```

### MediaAnalysisSignature (Multimodal)

Visual content analysis.

```rust
use agent_orchestrator::{MediaAnalysisSignature, MediaType};

let sig = MediaAnalysisSignature::new();

// MediaType: Image, Pdf, Diagram, Screenshot, Video
// Inputs: media_type, content_description, analysis_focus
// Outputs: description, extracted_data, structured_output, uncertainties
```

### Signature Integration

All signatures implement `MetaSignature` and can be used with dsrs predictors:

```rust
use dsrs::predictors::Predict;
use dsrs::data::example::Example;
use agent_orchestrator::DelegationSignature;

let sig = DelegationSignature::new();
let predictor = Predict::new(sig);

let example = Example::from([
    ("task_description", "Add a dark mode toggle to settings"),
    ("available_agents", r#"{"frontend": "available", "oracle": "available"}"#),
    ("current_workload", "{}"),
]);

// Run with LM
let result = predictor.forward(&example, &lm).await?;
let agent = result.get("assigned_agent", None); // "frontend"
```

## See Also

- [hooks.md](./hooks.md) - Hook system
- [integrations.md](./integrations.md) - External integrations
- [advanced.md](./advanced.md) - Advanced patterns
