# Plan Mode Agent Launching

Plan mode now supports parallel agent launching during Explore and Design phases for comprehensive codebase analysis.

## Overview

During plan mode, you can launch specialized agents in parallel to:
- **Explore Phase**: Investigate different aspects of the codebase simultaneously
- **Design Phase**: Analyze implementation approaches from multiple perspectives

## Explore Phase: Parallel Exploration

### Automatic Topic Generation

The `generate_exploration_topics(goal)` function generates 3 investigation areas:

1. **Existing Patterns**: Find similar implementations and architectural choices
2. **Dependencies & Constraints**: Identify libraries and technical constraints
3. **Files & Modules**: Locate components that need creation/modification

### Usage Example

```rust
use autopilot::planmode::{generate_exploration_topics, explore_agent_prompt};

let goal = "add user authentication";
let topics = generate_exploration_topics(goal);

// Launch 3 agents in parallel (using Task tool in Claude)
for (topic, focus) in topics {
    let prompt = explore_agent_prompt(&topic, &focus);
    // Use Task tool with this prompt
}
```

### Suggested Helper

```rust
use autopilot::planmode::suggest_explore_agents;

let goal = "implement caching layer";
let prompts = suggest_explore_agents(goal);
// prompts is Vec<String> of 3 ready-to-use Task prompts
```

### Aggregating Results

```rust
use autopilot::planmode::format_exploration_results;

let results = vec![
    ("Topic A".to_string(), "Findings for A...".to_string()),
    ("Topic B".to_string(), "Findings for B...".to_string()),
    ("Topic C".to_string(), "Findings for C...".to_string()),
];

let formatted = format_exploration_results(&results);
// Returns markdown-formatted results ready for plan file
```

## Design Phase: Multi-Perspective Analysis

### Automatic Perspective Generation

The `generate_design_perspectives()` function provides 3 analytical perspectives:

1. **Simplicity and Maintainability**: Focus on code clarity and long-term maintenance
2. **Performance and Scalability**: Focus on efficiency and growth potential
3. **Type Safety and Error Handling**: Focus on robustness and safety

### Usage Example

```rust
use autopilot::planmode::{generate_design_perspectives, plan_agent_prompt};

let feature = "JWT authentication";
let context = "Found 3 existing auth modules, Redis available";

let perspectives = generate_design_perspectives();

// Launch 3 agents in parallel (using Task tool)
for perspective in perspectives {
    let prompt = plan_agent_prompt(feature, context, &perspective);
    // Use Task tool with this prompt
}
```

### Suggested Helper

```rust
use autopilot::planmode::suggest_plan_agents;

let feature = "add caching";
let context = "Redis and in-memory options available";
let prompts = suggest_plan_agents(feature, context);
// prompts is Vec<String> of 3 ready-to-use Task prompts
```

### Aggregating Results

```rust
use autopilot::planmode::format_design_results;

let results = vec![
    ("Simplicity".to_string(), "Use in-memory cache...".to_string()),
    ("Performance".to_string(), "Use Redis with...".to_string()),
    ("Type safety".to_string(), "Define traits for...".to_string()),
];

let formatted = format_design_results(&results);
// Returns markdown-formatted analysis ready for plan file
```

## Launching Agents in Claude

When in plan mode, use the Task tool to launch agents in parallel:

```markdown
I'm going to launch 3 exploration agents in parallel to investigate:
1. Existing authentication patterns
2. Security dependencies and constraints
3. Auth-related files and modules
```

Then use multiple Task tool calls in a single response to run them in parallel.

## Phase Guidance Updates

The Explore and Design phase prompts now include:

### Explore Phase
- **PARALLEL EXPLORATION STRATEGY** section
- Instructions for launching up to 3 agents
- Topics: patterns, dependencies, files
- Guidance on aggregating findings

### Design Phase
- **PARALLEL DESIGN ANALYSIS** section
- Instructions for multi-perspective analysis
- Perspectives: simplicity, performance, type safety
- Guidance on comparing approaches

## API Reference

### Exploration Functions

- `generate_exploration_topics(goal: &str) -> Vec<(String, String)>`
  - Returns (topic, focus) pairs for investigation

- `explore_agent_prompt(topic: &str, focus: &str) -> String`
  - Generates a complete prompt for an exploration agent

- `suggest_explore_agents(goal: &str) -> Vec<String>`
  - Returns ready-to-use Task prompts for 3 exploration agents

- `format_exploration_results(results: &[(String, String)]) -> String`
  - Formats investigation results as markdown

### Design Functions

- `generate_design_perspectives() -> Vec<String>`
  - Returns 3 analytical perspectives

- `plan_agent_prompt(feature: &str, context: &str, perspective: &str) -> String`
  - Generates a complete prompt for a planning agent

- `suggest_plan_agents(feature: &str, context: &str) -> Vec<String>`
  - Returns ready-to-use Task prompts for 3 planning agents

- `format_design_results(results: &[(String, String)]) -> String`
  - Formats design analyses as markdown

## Benefits

1. **Comprehensive Coverage**: Multiple agents explore different aspects simultaneously
2. **Parallel Efficiency**: All investigations run at the same time
3. **Diverse Perspectives**: Design analysis from multiple viewpoints
4. **Structured Aggregation**: Helper functions format results consistently
5. **Guided Workflow**: Phase prompts suggest when and how to use agents

## Testing

All helper functions have comprehensive unit tests in `crates/autopilot-core/src/planmode.rs`:

```bash
cargo test -p autopilot planmode
```

## Example Workflow

### 1. Enter Plan Mode
```rust
enter_plan_mode(PlanModeConfig::new("add-auth", "Add user authentication"))
```

### 2. Explore Phase
- Launch 3 exploration agents using `suggest_explore_agents()`
- Aggregate findings with `format_exploration_results()`
- Update plan file with findings

### 3. Design Phase
- Launch 3 design agents using `suggest_plan_agents()`
- Aggregate analyses with `format_design_results()`
- Document recommended approach in plan

### 4. Review & Exit
- Validate plan completeness
- Exit plan mode and implement

## Future Enhancements

Potential improvements:
- Custom agent counts (not limited to 3)
- Configurable investigation topics
- Additional perspectives (security, testing, documentation)
- Automatic agent result extraction from Task tool outputs
- Agent swarm coordination for complex planning
