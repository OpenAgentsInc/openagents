# Hooks

Hooks intercept agent lifecycle events, enabling policy enforcement, context injection, and behavior modification.

## Hook Trait

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    /// Unique identifier for this hook
    fn name(&self) -> &str;

    /// Priority (higher = runs first)
    fn priority(&self) -> i32 { 0 }

    /// Called on session lifecycle events
    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        HookResult::Continue
    }

    /// Called before tool execution
    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        HookResult::Continue
    }

    /// Called after tool execution
    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        HookResult::Continue
    }

    /// Called to inject context
    async fn inject_context(&self, ctx: &mut ContextBuilder) -> HookResult {
        HookResult::Continue
    }
}
```

## Hook Results

```rust
pub enum HookResult {
    /// Continue execution
    Continue,
    
    /// Block execution with message
    Block { message: String },
    
    /// Signal that data was modified
    Modify,
}
```

## Builtin Hooks

### Session Hooks

#### SessionRecoveryHook

Automatically recovers from transient errors.

```rust
let hook = SessionRecoveryHook::new(3); // max 3 retries
```

Behavior:
- Tracks consecutive errors
- Exponential backoff between retries
- Resets on success
- Blocks after max retries

#### SessionNotificationHook

Sends notifications on session events.

```rust
let hook = SessionNotificationHook::new()
    .on_idle(|session| { /* notify */ })
    .on_error(|session, error| { /* notify */ });
```

### Tool Hooks

#### ToolBlockerHook

Blocks dangerous commands and tools.

```rust
let hook = ToolBlockerHook::new()
    .block_tool("rm")
    .block_command("git push --force")
    .block_pattern(r"rm\s+-rf\s+/");
```

Default blocked:
- `git push --force`
- `git reset --hard`
- `rm -rf /`
- Interactive flags (`-i`)

#### ToolLoggerHook

Logs tool executions with callbacks.

```rust
let hook = ToolLoggerHook::new()
    .on_before(|call| println!("Calling: {}", call.tool_name))
    .on_after(|call, output| println!("Result: {}", output.content));
```

#### ToolOutputTruncatorHook

Prevents context overflow from large outputs.

```rust
let hook = ToolOutputTruncatorHook::new(10_000); // max 10k chars
```

Truncation message:
```
[Output truncated: 50000 chars exceeded limit of 10000]
```

### Context Hooks

#### ContextInjectionHook

Injects standard files into agent context.

```rust
let hook = ContextInjectionHook::new("/path/to/workspace")
    .with_agents_md(true)    // Inject AGENTS.md
    .with_readme(true)       // Inject README.md
    .add_file("CONTRIBUTING.md");
```

#### DirectiveInjectionHook

Loads active directives into context.

```rust
let hook = DirectiveInjectionHook::new("/path/to/workspace")
    .with_config(DirectiveInjectionConfig {
        include_active: true,
        include_related: true,
        max_directives: 5,
        priority_filter: None,
    });
```

When DSPy is configured, directive status/priority parsing and semantic matching
use learned classifiers with frontmatter-based fallbacks.

#### CompactionContextHook

Preserves critical state during context compaction.

```rust
let hook = CompactionContextHook::new()
    .preserve_todos(true)
    .preserve_session_state(true);
```

### Todo Hooks

#### TodoHook

Enforces task completion before proceeding.

```rust
let hook = TodoHook::new(true); // enforce = true
```

Behavior:
- Tracks todo items with status
- Blocks final response if pending items exist
- Injects todo context into prompts

```rust
// Add todo
hook.add_todo("Implement feature X", TodoStatus::Pending);

// Update status
hook.set_status("Implement feature X", TodoStatus::Completed);

// Check pending
let count = hook.pending_count();
```

#### ContextWindowMonitorHook

Monitors context window usage.

```rust
let hook = ContextWindowMonitorHook::new(100_000, 0.8); // 100k tokens, warn at 80%
```

Logs warning when approaching limit:
```
Context window at 85% capacity (85000/100000 tokens)
```

## HookManager

### Registration

```rust
let mut hooks = HookManager::new();

// Register with default priority (0)
hooks.register(MyHook::new());

// Register with custom priority
hooks.register_with_priority(CriticalHook::new(), 100);
```

### Dispatch

```rust
// Dispatch session event
hooks.dispatch_session(&SessionEvent::Created { session_id }).await;

// Dispatch before tool (can block)
let result = hooks.dispatch_before_tool(&mut call).await;
if let HookResult::Block { message } = result {
    return Err(message);
}

// Dispatch after tool
hooks.dispatch_after_tool(&call, &mut output).await;

// Dispatch context injection
hooks.dispatch_inject_context(&mut context).await;
```

### Disabling

```rust
// Disable by name
hooks.disable("tool-blocker");

// Re-enable
hooks.enable("tool-blocker");

// Check status
if hooks.is_enabled("tool-blocker") { ... }
```

## Priority Ordering

Hooks execute in priority order (higher first):

| Priority | Use Case |
|----------|----------|
| 100+ | Critical security hooks |
| 50-99 | Policy enforcement |
| 1-49 | Logging, metrics |
| 0 | Default |
| -1 to -49 | Post-processing |
| -50 to -99 | Cleanup |
| -100 | Last resort |

Example:
```rust
hooks.register_with_priority(SecurityHook::new(), 100);  // First
hooks.register_with_priority(LoggingHook::new(), 10);    // Middle
hooks.register_with_priority(CleanupHook::new(), -50);   // Last
```

## Custom Hooks

### Example: Rate Limiter

```rust
struct RateLimiterHook {
    calls: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    max_per_minute: usize,
}

#[async_trait]
impl Hook for RateLimiterHook {
    fn name(&self) -> &str { "rate-limiter" }
    fn priority(&self) -> i32 { 90 }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        let mut calls = self.calls.write().await;
        let tool_calls = calls.entry(call.tool_name.clone()).or_default();
        
        // Remove old entries
        let cutoff = Instant::now() - Duration::from_secs(60);
        tool_calls.retain(|t| *t > cutoff);
        
        if tool_calls.len() >= self.max_per_minute {
            return HookResult::Block {
                message: format!("Rate limit exceeded for {}", call.tool_name),
            };
        }
        
        tool_calls.push(Instant::now());
        HookResult::Continue
    }
}
```

### Example: Audit Logger

```rust
struct AuditLoggerHook {
    writer: Arc<Mutex<BufWriter<File>>>,
}

#[async_trait]
impl Hook for AuditLoggerHook {
    fn name(&self) -> &str { "audit-logger" }
    fn priority(&self) -> i32 { -100 }  // Run last

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        let log_entry = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "tool": call.tool_name,
            "parameters": call.parameters,
            "success": output.success,
        });
        
        let mut writer = self.writer.lock().await;
        writeln!(writer, "{}", log_entry).ok();
        
        HookResult::Continue
    }
}
```

## Best Practices

1. **Keep hooks fast** — Hooks run synchronously; slow hooks block everything
2. **Use priority wisely** — Security hooks should run first
3. **Handle errors gracefully** — Don't panic in hooks
4. **Log hook actions** — For debugging and auditing
5. **Test hooks in isolation** — Each hook should be independently testable
