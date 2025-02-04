# OpenAgents Solver Documentation

The OpenAgents Solver is a system that automatically solves GitHub issues by analyzing the problem, generating an implementation plan, and creating a pull request with the solution. This document explains how the solver works and the flow of execution.

## Architecture Overview

The solver is split into two main parts:

1. CLI Interface (`src/bin/solver.rs` and `src/bin/solver_impl/*`)
2. Core Library (`src/solver/*`)

## Execution Flow

### 1. Entry Point (`src/bin/solver.rs`)

The process begins when a user runs:

```bash
cargo run --bin solver -- --issue <number> [--repo owner/name] [--live]
```

The main function:

1. Initializes logging
2. Parses command line arguments
3. Loads environment variables (GITHUB_TOKEN, OLLAMA_URL)
4. Initializes the GitHub service
5. Fetches issue details
6. Generates repository map
7. Orchestrates the planning and solution phases

### 2. Issue Analysis (`src/bin/solver_impl/issue.rs`)

The `handle_issue` function:

1. Initializes GitHub context
2. Fetches issue details
3. Fetches issue comments
4. Returns issue and comments for analysis

### 3. File Identification (`src/bin/solver_impl/files.rs` and `pre_analysis.rs`)

The file identification process now uses a two-step approach:

1. DeepSeek Pre-Analysis:

   - Streams real-time reasoning about which files need modification
   - Shows step-by-step thinking process
   - Provides detailed analysis with reasoning
   - Supports up to 10 files with relevance scores (1-10)

2. Mistral Structured Output:
   - Takes DeepSeek's analysis and reasoning
   - Formats the output as structured JSON
   - Validates file paths against repository
   - Normalizes relevance scores for internal use

### 4. Planning Phase (`src/bin/solver_impl/planning.rs`)

The `handle_planning` function:

1. Creates a PlanningContext with Ollama URL
2. Generates implementation plan using AI model
3. Streams the response for real-time updates
4. Extracts and validates JSON plan from markdown response
5. Returns the validated plan

### 5. Solution Generation (`src/bin/solver_impl/solution.rs`)

The `handle_solution` function:

1. Generates list of files to modify
2. For each file:
   - Reads current content
   - Generates changes using AI model
   - Applies changes to file
3. Returns success/failure status

### 6. Core Library Components

#### Changes Management (`src/solver/changes/`)

- `generation.rs`: Generates code changes using AI
- `parsing.rs`: Parses search/replace blocks
- `types.rs`: Defines change types and structures

#### File Management

- `file_list.rs`: Generates list of files to modify
- `context.rs`: Manages repository context and operations

#### GitHub Integration

- `github.rs`: Handles GitHub API interactions
- `types.rs`: Defines GitHub-related types

#### Planning & Execution

- `planning.rs`: Implementation plan generation
- `solution.rs`: Solution application logic
- `streaming.rs`: Handles streaming responses

## Data Flow

1. User Input → CLI
2. CLI → GitHub API (fetch issue)
3. Issue → DeepSeek Pre-Analysis
4. DeepSeek → Reasoning Stream + Analysis
5. Analysis → Mistral (structured output)
6. Mistral → Implementation Plan
7. Plan → Solution Generator
8. Changes → GitHub PR

## Key Components

### 1. DeepSeek Pre-Analysis

- Provides real-time reasoning about file selection
- Shows step-by-step thinking process
- Handles up to 10 files with 1-10 relevance scores
- Streams tokens in real-time to console

### 2. Planning Context

- Manages AI model interaction
- Generates structured implementation plans
- Handles streaming responses

### 3. Solution Context

- Manages file modifications
- Applies changes safely
- Validates changes

### 4. GitHub Context

- Manages repository operations
- Creates branches and PRs
- Posts comments and updates

## Configuration

The solver requires:

1. `GITHUB_TOKEN`: For GitHub API access
2. `OLLAMA_URL`: For model access (default: http://localhost:11434)

## Modes

1. **Dry Run Mode** (default)

   - Shows what would be done
   - Doesn't create branches or PRs
   - Prints plan locally

2. **Live Mode** (`--live`)
   - Creates branch
   - Applies changes
   - Creates PR
   - Posts comments

## Error Handling

The solver implements comprehensive error handling:

1. Environment validation
2. GitHub API error handling
3. File operation safety checks
4. AI model response validation
5. JSON plan validation

## Future Extensions

1. WebSocket Integration

   - Real-time status updates
   - Progress tracking
   - Error notifications

2. Conversation History

   - Complex problem tracking
   - Multi-step solutions
   - User feedback integration

3. Error Recovery
   - Automatic retries
   - Fallback strategies
   - State recovery

## Testing

The solver includes extensive tests:

1. Unit tests for each component
2. Integration tests for full flow
3. Mock AI responses
4. GitHub API mocks
5. File operation tests

## Usage Examples

Basic usage:

```bash
cargo run --bin solver -- --issue 123
```

With custom repository:

```bash
cargo run --bin solver -- --issue 123 --repo owner/name
```

Live mode:

```bash
cargo run --bin solver -- --issue 123 --live
```
