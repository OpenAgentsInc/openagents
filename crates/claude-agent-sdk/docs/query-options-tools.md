# QueryOptions tool controls and partial messages

## Disallowed tools
QueryOptions::disallowed_tools lets callers block specific tools by name.

Example:
```rust
use claude_agent_sdk::QueryOptions;

let options = QueryOptions::new().disallowed_tools(vec![
    "Edit".to_string(),
    "Write".to_string(),
    "NotebookEdit".to_string(),
]);
```

This maps to CLI flags:
- --disallowed-tools Edit
- --disallowed-tools Write
- --disallowed-tools NotebookEdit

## Partial message streaming
QueryOptions::include_partial_messages(true) enables additional SDK message types:
- SdkMessage::StreamEvent for partial content
- SdkMessage::ToolProgress for elapsed time updates

Callers should enable this when they want real-time tool progress and intermediate streaming updates.
