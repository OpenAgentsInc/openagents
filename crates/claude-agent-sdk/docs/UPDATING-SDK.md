# Keeping the Rust Claude Agent SDK Updated

This guide explains how to maintain parity between our Rust SDK and the official TypeScript SDK as new versions are released.

## Overview

The Claude Agent SDK consists of two main components:
1. **TypeScript SDK** (`@anthropic-ai/claude-agent-sdk`) - The official SDK
2. **Claude Code CLI** (`cli.js`) - The bundled CLI that handles actual execution

The TypeScript SDK is a thin wrapper that spawns the CLI and communicates via JSONL over stdin/stdout. Our Rust SDK does the same thing.

## Finding the TypeScript SDK

The TypeScript SDK is **not open source**. It's bundled with the Claude Code CLI and installed via npm/npx:

```bash
# The SDK is cached in the npm cache after running claude
ls ~/.npm/_npx/*/node_modules/@anthropic-ai/claude-agent-sdk/
```

### Key Files to Review

| File | Purpose | Update Priority |
|------|---------|-----------------|
| `sdk.d.ts` | Main type definitions (~1000 lines) | HIGH |
| `sandboxTypes.d.ts` | Sandbox configuration types | MEDIUM |
| `cli.js` | Minified CLI (search for patterns) | HIGH |

## Comparing Versions

### Step 1: Check Current Versions

```bash
# Our SDK version (in Cargo.toml)
cat crates/claude-agent-sdk/Cargo.toml | grep "^version"

# Installed TypeScript SDK version
cat ~/.npm/_npx/*/node_modules/@anthropic-ai/claude-agent-sdk/package.json | grep version

# CLI version
claude --version
```

### Step 2: Get Latest SDK

```bash
# Clear cache and get fresh SDK
npx --yes @anthropic-ai/claude-agent-sdk@latest --version

# Find the new cache location
find ~/.npm/_npx -name "sdk.d.ts" 2>/dev/null
```

### Step 3: Diff the Type Definitions

Compare `sdk.d.ts` with our Rust types:

```bash
# Open in diff tool
diff ~/.npm/_npx/*/node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts \
     <previous-version>/sdk.d.ts
```

## What to Look For

### 1. New Message Types

Search `sdk.d.ts` for message type definitions:

```typescript
// Look for patterns like:
type SDKMessage = ...
type SDKSystemMessage = ...
type SDKResultMessage = ...
```

Map to Rust in `src/protocol/messages.rs`.

### 2. New System Message Subtypes

Search `cli.js` (minified) for new subtypes:

```bash
# Find all subtype strings in CLI
grep -oE 'subtype:"[^"]+"' ~/.npm/_npx/*/node_modules/@anthropic-ai/claude-agent-sdk/cli.js | sort -u
```

Expected results:
```
subtype:"init"
subtype:"compact_boundary"
subtype:"status"
subtype:"hook_response"
subtype:"api_error"
subtype:"stop_hook_summary"
subtype:"informational"
subtype:"local_command"
```

### 3. New Query Options

Search `sdk.d.ts` for `QueryOptions`:

```typescript
interface QueryOptions {
  model?: string;
  maxTurns?: number;
  // ... look for new fields
}
```

Map to Rust in `src/options.rs`.

### 4. New Control Requests/Responses

Search for control protocol types:

```typescript
type ControlRequestData = ...
type ControlResponseData = ...
```

Map to Rust in `src/protocol/control.rs`.

### 5. New Hook Events

Search for hook event types:

```typescript
type HookEvent = 'PreToolUse' | 'PostToolUse' | ...
```

Map to Rust in `src/hooks.rs`.

### 6. New Permission Types

Search for permission-related types:

```typescript
type PermissionMode = ...
type PermissionResult = ...
```

Map to Rust in `src/permissions.rs` and `src/protocol/permissions.rs`.

## Mapping TypeScript to Rust

### Type Mappings

| TypeScript | Rust |
|------------|------|
| `string` | `String` |
| `number` | `u32`, `u64`, `f64` |
| `boolean` | `bool` |
| `T \| null` | `Option<T>` |
| `T \| undefined` | `Option<T>` with `#[serde(skip_serializing_if = "Option::is_none")]` |
| `Record<K, V>` | `HashMap<K, V>` |
| Union types | Rust `enum` with `#[serde(tag = "type")]` or `#[serde(untagged)]` |
| Interface | Rust `struct` |

### Serde Patterns

```rust
// Tagged union (discriminated by "type" field)
#[serde(tag = "type")]
pub enum MyMessage {
    #[serde(rename = "variant_a")]
    VariantA { ... },
}

// Subtypes (discriminated by "subtype" field)
#[serde(tag = "subtype")]
pub enum MySubtype {
    #[serde(rename = "sub_a")]
    SubA { ... },
}

// Untagged union (try each variant in order)
#[serde(untagged)]
pub enum MyUnion {
    Number(u64),
    String(String),
}

// camelCase to snake_case
#[serde(rename_all = "camelCase")]
pub struct MyStruct {
    some_field: String,  // serializes as "someField"
}

// Specific field rename
#[serde(rename = "specificName")]
pub field_name: String,
```

## Update Checklist

When updating to a new SDK version:

- [ ] Check TypeScript SDK version in `sdk.d.ts`
- [ ] Compare `QueryOptions` interface for new options
- [ ] Search `cli.js` for new `subtype:` values
- [ ] Check `SDKMessage` union for new message types
- [ ] Check `ControlRequestData` for new control requests
- [ ] Check `ControlResponseData` for new control responses
- [ ] Check `HookEvent` for new hook event types
- [ ] Check `HookInput` types for new fields
- [ ] Check `HookOutput` types for new fields
- [ ] Check `PermissionMode` for new modes
- [ ] Check sandbox configuration types
- [ ] Check beta feature types
- [ ] Run `cargo check` to verify compilation
- [ ] Update `GAP-REPORT.md` with new findings
- [ ] Update version number in our `Cargo.toml` if significant changes

## Testing Updates

After making changes:

```bash
# Compile check
cargo check -p claude-agent-sdk

# Run tests
cargo test -p claude-agent-sdk

# Run doc tests
cargo test -p claude-agent-sdk --doc
```

## Common Patterns

### Adding a New System Message Subtype

1. Add variant to `SdkSystemMessage` enum in `messages.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum SdkSystemMessage {
    // ... existing variants ...

    #[serde(rename = "new_subtype")]
    NewSubtype(NewSubtypeMessage),
}
```

2. Define the message struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSubtypeMessage {
    pub field: String,
    pub uuid: String,
    pub session_id: String,
}
```

### Adding a New Query Option

1. Add field to `QueryOptions` in `options.rs`:
```rust
pub struct QueryOptions {
    // ... existing fields ...
    pub new_option: Option<String>,
}
```

2. Add builder method:
```rust
impl QueryOptions {
    pub fn new_option(mut self, value: impl Into<String>) -> Self {
        self.new_option = Some(value.into());
        self
    }
}
```

3. Update `build_args()` if it maps to a CLI flag:
```rust
if let Some(ref opt) = self.new_option {
    args.push("--new-option".to_string());
    args.push(opt.clone());
}
```

### Adding a New Hook Event

1. Add variant to `HookEvent` in `hooks.rs`:
```rust
pub enum HookEvent {
    // ... existing variants ...
    NewEvent,
}
```

2. Update `as_str()` method:
```rust
impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            // ... existing matches ...
            HookEvent::NewEvent => "NewEvent",
        }
    }
}
```

3. Add input type if needed:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEventHookInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub hook_event_name: String,
    // Event-specific fields
}
```

4. Update `HookInput` enum:
```rust
#[serde(untagged)]
pub enum HookInput {
    // ... existing variants ...
    NewEvent(NewEventHookInput),
}
```

5. Update hook event matching in `query.rs`:
```rust
let hook_event = match hook_event_name {
    // ... existing matches ...
    "NewEvent" => HookEvent::NewEvent,
    // ...
};
```

## Resources

- **TypeScript SDK location:** `~/.npm/_npx/<hash>/node_modules/@anthropic-ai/claude-agent-sdk/`
- **CLI source (minified):** `cli.js` in the SDK directory
- **Our gap report:** `docs/GAP-REPORT.md`
- **serde documentation:** https://serde.rs/

## Version History

| Date | TS SDK Version | CLI Version | Changes |
|------|---------------|-------------|---------|
| 2025-12-11 | 0.1.65 | 2.0.65 | Initial parity (~98%) |
| 2025-12-19 | 0.1.61 | 2.0.73 | Added hooks, new message types (~99%) |
