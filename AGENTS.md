# OpenAgents Development Guide

This document contains common patterns, gotchas, and best practices for developing with OpenAgents. It's maintained by the OpenAgents team and our automated coding agents.

## Table of Contents

- [Code Organization](#code-organization)
- [Framework & Dependencies](#framework--dependencies)
- [Hyperview](#hyperview)
- [GitHub Integration](#github-integration)
- [Error Handling](#error-handling)

## Code Organization

### Module System

When working with Rust's module system, always:

1. Check mod.rs files first:
```rust
// Before adding new files, check mod.rs to see how modules are organized
pub mod handlers;
pub mod routes;
pub mod services;
```

2. Respect existing module structure:
```rust
// Good - Use the established pattern
use crate::routes;  // For root modules
use super::services::github;  // For sibling modules

// Bad - Bypass module structure
use crate::server::services::github::GithubService;  // Too specific
```

3. Export types properly:
```rust
// In mod.rs
pub mod user;
pub use user::User;  // Re-export important types

// In consuming code
use super::User;  // Clean import
```

### Common Mistakes

1. Partial Implementation:
```rust
// Bad - Only implementing what you need
pub struct AppState {
    pub pool: PgPool,  // Breaks existing code that needs other fields
}

// Good - Preserve existing structure
pub struct AppState {
    pub ws_state: Arc<WebSocketState>,
    pub repomap_service: Arc<RepomapService>,
    pub auth_state: Arc<AuthState>,
    pub github_auth: Arc<GitHubAuthService>,
    pub pool: PgPool,
}
```

2. Import Confusion:
```rust
// Bad - Using wrong import path
use super::routes;  // Fails if routes is at crate root

// Good - Check module structure first
use crate::routes;  // For root modules
use super::routes;  // For sibling modules
```

3. Breaking Changes:
```rust
// Bad - Changing type without checking usage
type DateTime = chrono::DateTime<Utc>;  // Breaks code using time::OffsetDateTime

// Good - Respect existing types
type DateTime = time::OffsetDateTime;  // Maintains compatibility
```

## Framework & Dependencies

### Axum Web Framework

OpenAgents uses the Axum web framework exclusively. Never mix in other frameworks or their dependencies.

Common gotchas:
1. Don't add Tower middleware directly - use Axum's built-in middleware support
2. Don't use Actix extractors or types
3. Always use Axum's extractors and routing

Example of proper Axum handler:
```rust
use axum::{
    extract::State,
    response::Response,
};

#[axum::debug_handler]  // Helps with error messages
pub async fn my_handler(
    State(state): State<AppState>,
    // Use Axum extractors
) -> Response {
    // Handler implementation
}
```

### Dependencies to Avoid

- `tower` (use Axum's re-exports instead)
- `actix-web`
- `warp`
- `rocket`

## Hyperview

### XML String Escaping

When working with Hyperview HXML templates, proper string escaping is crucial. Here are key points:

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
let items = repos.iter().map(format_repo).collect::<Vec<_>>().join("\n");
format!(
    r###"<list>{items}</list>"###
)

// Bad - can cause unused argument warnings
format!(
    r###"<list>{}
</list>"###,
    items
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