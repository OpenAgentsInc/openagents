# OpenAgents Development Guide

This document contains common patterns, gotchas, and best practices for developing with OpenAgents. It's maintained by the OpenAgents team and our automated coding agents.

## Table of Contents

- [Hyperview](#hyperview)
- [GitHub Integration](#github-integration)
- [Error Handling](#error-handling)

## Hyperview

### XML String Escaping

When working with Hyperview HXML templates, proper string escaping is crucial. Here are some key points:

1. Use triple-quoted raw strings (`r###"..."###`) for HXML templates to avoid escaping issues:
```rust
// Good
let xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>...</screen>
</doc>"###;

// Bad - requires escaping quotes
let xml = "<doc xmlns=\"https://hyperview.org/hyperview\">";
```

2. Use underscores instead of hyphens in style IDs to avoid Rust's prefix syntax warnings:
```rust
// Good
<style id="repo_item" backgroundColor="#111111" />

// Bad - triggers Rust prefix syntax warning
<style id="repo-item" backgroundColor="#111111" />
```

3. Use full 6-digit hex codes for colors to avoid parsing issues:
```rust
// Good
backgroundColor="#111111"
color="#999999"

// Bad - can cause formatting issues
backgroundColor="#111"
color="#999"
```

4. When using format strings in HXML, separate the template from the closing tag:
```rust
// Good
format!(
    r###"<list>{}</list>"###,
    items.join("\n")
)

// Bad - can cause unused argument warnings
format!(
    r###"<list>{}
</list>"###,
    items.join("\n")
)
```

### Common Patterns

1. Always include proper content type header:
```rust
.header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
```

2. Structure HXML templates with consistent indentation:
```rust
<doc>
    <screen>
        <styles>
            <style ... />
        </styles>
        <body>
            <content />
        </body>
    </screen>
</doc>
```

3. Use semantic style IDs that reflect component purpose:
```rust
<style id="header" ... />
<style id="content" ... />
<style id="footer" ... />
```

## GitHub Integration

### Metadata Handling

When working with GitHub metadata in the user object:

1. Use proper pattern matching for nested JSON structures:
```rust
match user.metadata {
    Some(Value::Object(m)) => m,
    _ => handle_error()
}
```

2. Access tokens should be handled securely and validated:
```rust
match github.get("access_token") {
    Some(Value::String(token)) => token,
    _ => handle_error()
}
```

## Error Handling

### Response Wrapping

When returning errors in Hyperview responses:

1. Always return proper HXML even for errors:
```rust
Response::builder()
    .status(StatusCode::OK)  // Use OK even for errors
    .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
    .body(render_error_screen("Error message").into())
```

2. Include retry mechanisms where appropriate:
```rust
<view style="retry_button" href="/retry/endpoint">
    <text>Retry</text>
</view>
```

3. Log errors with appropriate context:
```rust
error!("Failed to fetch data for user {}: {}", user_id, error);
```