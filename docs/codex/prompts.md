# Prompt System

## Overview

Codex assembles prompts from system instructions, user instructions, project docs, and the environment context. This doc covers the overall assembly. For the precise system‑prompt behavior (which files are used, how they are chosen, and where they are sent), see docs/systems/system-prompts.md.

## Core System Prompts

### Primary System Prompts

1. **Base System Prompt** (`core/prompt.md`)
   - Primary instructions for most models
   - Behavior guidelines, capabilities, tools, and safety

2. **GPT‑5 Codex Prompt** (`core/gpt_5_codex_prompt.md`)
   - Base instructions used for `codex-*` and `gpt-5-codex*` families
   - Tailored to GPT‑5/Codex behavior

3. **Review Prompt** (`core/review_prompt.md`)
   - Specialized code‑review instructions with structured JSON output

4. **Compact Mode Templates** (`core/templates/compact/`)
   - `prompt.md` — summarization instructions
   - `history_bridge.md` — resume‑from‑summary bridge template

Note: `tui/prompt_for_init_command.md` is a built‑in content template used by the `/init` command; it is not part of the system‑prompt used on every turn.

### Model-Specific Prompt Selection

Location: `core/src/model_family.rs`

Codex determines a model family from the configured model slug via prefix matching. That family carries the `base_instructions` string used as the system instructions and several capability flags. See docs/systems/core-model-family.md for details. Highlights:

- `o3`, `o4-mini` — reasoning summaries enabled.
- `codex-mini-latest` — reasoning summaries + local_shell.
- `gpt-4.1`, `gpt-4o`, `gpt-3.5` — add apply_patch usage guidance unless the tool is present.
- `gpt-oss*` — prefer function form of apply_patch.
- `codex-*` and `gpt-5-codex*` — use GPT‑5 Codex prompt.
- `gpt-5*` — reasoning summaries enabled; default base prompt.

## Custom Prompts System

### Custom Prompt Discovery

Location: `core/src/custom_prompts.rs`

**Behavior**:
- **Location**: `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`)
- **Format**: Markdown files with `.md` extension
- **Naming**: Filename becomes slash command (e.g., `my-prompt.md` → `/my-prompt`)
- **Discovery**: Automatic when requested via the protocol; sorted alphabetically. Built‑in command names may be excluded.

**Usage Example**:
```bash
# Create custom prompt
mkdir -p ~/.codex/prompts
echo "Analyze this code for performance issues" > ~/.codex/prompts/perf-review.md

# Use in TUI
/perf-review
```

### Custom Prompt Structure

```rust
pub struct CustomPrompt {
    pub name: String,      // Slash command name
    pub path: PathBuf,     // File path
    pub content: String,   // Prompt content
}
```

### TUI Integration

Location: `tui/src/bottom_pane/custom_prompt_view.rs`

- **Multi-line text input** for custom review instructions
- **Slash command popup** with auto-completion
- **Dynamic loading** of custom prompts in chat composer

## Prompt Assembly Architecture

### Core Prompt Structure

Location: `core/src/client_common.rs`

```rust
pub struct Prompt {
    pub input: Vec<ResponseItem>,           // conversation items for this turn
    pub(crate) tools: Vec<OpenAiTool>,      // available tools
    pub base_instructions_override: Option<String>,
}
```

### Assembly Process

The prompt assembly follows a layered approach:

1. **System Instructions**
   - Model-specific base instructions
   - Core behavior and safety guidelines

2. **User Instructions**
   - Wrapped in `<user_instructions>` XML tags
   - Combined with project documentation

3. **Environment Context**
   - Current working directory
   - Sandbox mode and approval policy
   - Available tools and capabilities

4. **Project Documentation**
   - AGENTS.md files from repository hierarchy
   - Project-specific instructions and context

5. **Conversation History**
   - Previous messages and responses
   - Maintaining context across turns

### Environment Context Injection

Location: `core/src/environment_context.rs`

Automatically injects structured environment information:

```xml
<environment_context>
  <cwd>/path/to/project</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>restricted</network_access>
  <writable_roots>
    <root>/tmp</root>
    <root>/workspace</root>
  </writable_roots>
  <shell>bash</shell>
</environment_context>
```

**Context Elements**:
- Working directory, approval policy, sandbox mode
- Network access and writable roots (workspace‑write)
- Shell name (initial context only)

## Project Documentation Integration

### AGENTS.md Discovery

Location: `core/src/project_doc.rs`

**Discovery Process**:
1. Start from git repository root
2. Walk directory tree to current working directory
3. Collect all `AGENTS.md` files found
4. Concatenate with appropriate separators
5. Apply size limits for safety

**Integration Process**:
```rust
// Combines config instructions with project docs
pub fn wrap_user_instructions(
    instructions_from_config: Option<String>,
    project_docs: Option<String>,
) -> Option<String>
```

**Size Limits**:
- Configurable byte limits prevent excessive context usage
- Automatic truncation with warnings
- Priority given to more specific (deeper) AGENTS.md files

## Prompt Caching System

### Caching Strategy

Location: `core/src/client.rs`

The Responses API request includes `prompt_cache_key: conversation_id` so the server can reuse a cached prefix (system instructions, fixed tool schemas, initial context) across turns and only charge/count new input.

**Optimization Features**:
- **Stable Prefixes**: System instructions remain consistent
- **Turn Preservation**: Cache hits maintained across turns
- **Context Changes**: Handles model/setting overrides gracefully
- **Conversation Continuity**: Same cache key throughout session

### Cache Benefits

1. **Performance**: Reduces prompt processing time
2. **Cost Efficiency**: Minimizes token usage for repeated contexts
3. **Consistency**: Ensures stable instruction interpretation
4. **Scalability**: Supports long conversations efficiently

## Specialized Prompt Features

### Review Mode

Location: `core/review_prompt.md`

**Features**:
- **Dedicated Instructions**: Specialized for code review tasks
- **Issue Classification**: Structured severity levels (P0-P3)
- **Output Format**: JSON response format specification
- **Coverage Areas**: Security, performance, bugs, style

**Example Output Format**:
```json
{
  "issues": [
    {
      "severity": "P1",
      "type": "security",
      "description": "SQL injection vulnerability",
      "file": "src/user.rs",
      "line": 42
    }
  ]
}
```

### Compact Mode

Location: `core/src/codex/compact.rs`

**Purpose**: Conversation summarization when context limits are reached

**Features**:
- **Automatic Triggering**: Based on model token limits
- **Summary Generation**: Specialized prompt for context compression
- **History Bridging**: Resume from summaries with context preservation
- **Trigger Detection**: `"Start Summarization"` command

**Template Structure**:
```markdown
# Previous Conversation Summary
[Compressed context from previous conversation]

# Current Context
[Current working state and objectives]
```

### Reasoning Integration

**Reasoning-Enabled Models**:
- Support for reasoning summaries (GPT-5, O3 series)
- Configurable reasoning effort levels
- Encrypted reasoning content handling

**Configuration**:
```toml
model_reasoning_effort = "high"    # minimal|low|medium|high
model_reasoning_summary = "detailed"  # auto|concise|detailed|none
```

## Tool-Specific Prompt Engineering

### Apply Patch Instructions

**Dynamic Injection**:
- Added when apply_patch tool isn't present
- Model-specific adaptation based on capabilities
- Integrated tool documentation and examples

### Shell Tool Integration

**Features**:
- **Local Shell**: Native tool support for certain models
- **Command Safety**: Sandbox-aware command descriptions
- **Approval Context**: Context-aware approval request formatting

### Tool Description Generation

Location: `core/src/openai_tools.rs`

```rust
// Automatic tool description generation
pub fn create_tools_json_for_responses_api(tools: &[OpenAiTool]) -> Result<Vec<serde_json::Value>>
```

## API Format Adaptation

### Chat Completions API

**Format**:
```json
{
  "model": "gpt-5",
  "messages": [
    {"role": "system", "content": "System instructions..."},
    {"role": "user", "content": "User message..."}
  ],
  "tools": [...]
}
```

### Responses API

**Format**:
```json
{
  "model": "gpt-5", 
  "instructions": "System instructions...",
  "input": [
    {"type": "text", "text": "User message..."}
  ],
  "tools": [...]
}
```

### Response Processing

Location: `core/src/chat_completions.rs`

**Features**:
- **Reasoning Handling**: Special processing for reasoning blocks
- **Message Aggregation**: Streaming response combination
- **Error Recovery**: Graceful handling of incomplete responses

## Configuration and Customization

### System Instruction Overrides

- Use `experimental_instructions_file = "/path/to/instructions.md"` in `~/.codex/config.toml` to replace the built‑in base instructions with file contents (see core/src/config.rs for loading).
- Via MCP, the `codex` tool supports a `base_instructions` string parameter per invocation (see mcp-server/src/codex_tool_config.rs).

### Related

- System prompts (selection, wire format): docs/systems/system-prompts.md
- Model families and capabilities: docs/systems/core-model-family.md

## Testing and Validation

Location: `core/tests/suite/prompt_caching.rs`

- Verifies that base instructions remain stable across turns and that the prompt cache key stays constant when appropriate.
- Ensures the initial context prefixing (user instructions + environment context) is consistent across requests.
