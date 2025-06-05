# OpenAgents CLI Documentation

The OpenAgents CLI provides commands for managing todos and interacting with AI services, including Claude Code integration.

## Installation

```bash
# Build the CLI
pnpm build

# Run the CLI (after building)
pnpm --filter=@openagentsinc/cli exec openagents

# Run the CLI (development mode)
pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts
```

## Command Structure

```
openagents <command> [options]
```

## Available Commands

### Todo Commands

Manage your todo list with these commands:

#### `openagents todo add <todo>`
Add a new todo item to your list.

**Arguments:**
- `todo` - The message/description for the todo item

**Example:**
```bash
openagents todo add "Complete the AI integration"
```

#### `openagents todo list`
Display all todos in your list.

**Example:**
```bash
openagents todo list
```

#### `openagents todo done --id <id>`
Mark a todo as completed.

**Options:**
- `--id <number>` - The identifier of the todo to complete

**Example:**
```bash
openagents todo done --id 1
```

#### `openagents todo remove --id <id>`
Remove a todo from your list.

**Options:**
- `--id <number>` - The identifier of the todo to remove

**Example:**
```bash
openagents todo remove --id 2
```

### AI Commands

Interact with Claude Code and other AI services:

#### `openagents ai check`
Check if Claude Code CLI is available on your system.

**Example:**
```bash
openagents ai check
```

**Output:**
- ✅ Success: Confirms Claude Code CLI is installed and accessible
- ❌ Failure: Provides installation instructions

#### `openagents ai prompt <prompt>`
Send a single prompt to Claude Code and get a response.

**Arguments:**
- `prompt` - The text prompt to send to Claude

**Example:**
```bash
openagents ai prompt "Explain the concept of Effect in TypeScript"
```

**Output includes:**
- 📝 Response content
- 📊 Model information
- 📈 Token usage statistics
- 🔗 Session ID (if applicable)

#### `openagents ai chat <prompt> [options]`
Have an interactive conversation with Claude Code, with support for sessions and custom system prompts.

**Arguments:**
- `prompt` - The text prompt to send to Claude

**Options:**
- `--session <id>` - Continue a previous conversation using the session ID
- `--system <prompt>` - Set a custom system prompt for the conversation

**Examples:**
```bash
# Start a new conversation
openagents ai chat "Hello, can you help me with TypeScript?"

# Continue a previous conversation
openagents ai chat "What about generics?" --session "abc123"

# Start with a custom system prompt
openagents ai chat "Write a function" --system "You are a TypeScript expert"
```

**Output includes:**
- 📝 Response content
- 🔗 Session ID for continuing the conversation
- 📊 Model information
- 📈 Token usage (input/output/total)

## Prerequisites

### For Todo Commands
- Running server instance (default: http://localhost:3000)
- Server can be started with: `pnpm --filter=@openagentsinc/server dev`

### For AI Commands
- Claude Code CLI must be installed
- Requires a Claude MAX subscription
- Install from: https://claude.ai/code

## Environment Variables

The CLI respects the following environment variables:

- `API_URL` - Override the default API server URL (default: http://localhost:3000)
- `CLAUDE_CLI_PATH` - Custom path to the Claude CLI executable (default: searches PATH for 'claude')

## Error Handling

All commands provide clear error messages:
- ❌ Connection errors when server is unavailable
- ❌ Invalid command syntax with usage help
- ❌ AI service errors with detailed messages

## Development

### Running in Development Mode
```bash
# Run directly with tsx
pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts <command>

# Or build and run
pnpm build
pnpm --filter=@openagentsinc/cli exec openagents <command>
```

### Testing Commands
```bash
# Test todo commands (requires running server)
pnpm --filter=@openagentsinc/cli exec openagents todo add "Test todo"
pnpm --filter=@openagentsinc/cli exec openagents todo list

# Test AI commands (requires Claude Code CLI)
pnpm --filter=@openagentsinc/cli exec openagents ai check
pnpm --filter=@openagentsinc/cli exec openagents ai prompt "Hello"
```

## Examples

### Complete Workflow Example
```bash
# Check AI availability
openagents ai check

# Start a conversation
openagents ai chat "I need help writing a TypeScript function"
# Response includes session ID: abc123

# Continue the conversation
openagents ai chat "Can you add error handling?" --session abc123

# Manage todos
openagents todo add "Implement error handling as discussed"
openagents todo list
openagents todo done --id 1
```

### Using with Custom System Prompts
```bash
# TypeScript expert mode
openagents ai chat "Review this code" --system "You are a TypeScript expert focused on type safety"

# Code reviewer mode
openagents ai chat "Analyze performance" --system "You are a performance optimization specialist"
```

## Working Examples from Testing

### AI Chat Commands (Verified Working)
```bash
# Basic math question
$ pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts ai chat "What is 2+2?"
💬 Starting conversation with Claude Code...

📝 Response:
4

🔗 Session ID: b9db7dd3-4b09-4298-a788-b9cc194b2fd7
💡 Use --session flag with this ID to continue the conversation

📊 Model: claude-3-5-sonnet-20241022
📈 Tokens: 0 (input: 0, output: 0)

# Continue conversation with session
$ pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts ai chat --session b9db7dd3-4b09-4298-a788-b9cc194b2fd7 "What about 3+3?"
💬 Starting conversation with Claude Code...

📝 Response:
6

🔗 Session ID: 1a88f121-d678-4f4f-af05-05b66fb94ec9
💡 Use --session flag with this ID to continue the conversation

# With system prompt
$ pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts ai chat --system "You are a helpful math tutor" "Explain why 2+2=4"
💬 Starting conversation with Claude Code...

📝 Response:
2+2=4 because when you combine two units with two more units, you have four units total. This follows from the definition of addition in arithmetic.

🔗 Session ID: 8fd45f8d-05f5-432f-913d-f6ae9a6780c4
💡 Use --session flag with this ID to continue the conversation
```

### AI Check Command (Verified Working)
```bash
$ pnpm --filter=@openagentsinc/cli exec tsx src/bin.ts ai check
🔍 Checking Claude Code availability...
✅ Claude Code CLI is available!
💡 You can now use 'ai prompt' and 'ai chat' commands
```

## Known Issues

### AI Prompt Command
The `ai prompt` command currently times out due to incomplete AiService wiring. Use `ai chat` instead for AI interactions.

### Todo Commands
All todo commands require a running server instance. Without the server running, you'll see connection errors:
```
ERROR (#17):
  RequestError: Transport error (GET http://localhost:3000/todos)
```

## Troubleshooting

### "Claude Code CLI is not available"
- Ensure Claude CLI is installed: https://claude.ai/code
- Check if 'claude' is in your PATH: `which claude`
- Set custom path: `export CLAUDE_CLI_PATH=/path/to/claude`

### "Connection refused" errors
- Start the server: `pnpm --filter=@openagentsinc/server dev`
- Check server URL: `export API_URL=http://your-server:port`

### Session not found
- Session IDs expire after inactivity
- Start a new conversation without the --session flag