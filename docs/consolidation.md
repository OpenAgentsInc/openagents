# Code Consolidation Analysis

## Overview

This document analyzes areas of the codebase where we have redundant, outdated, or competing implementations that should be consolidated or removed. The goal is to simplify the codebase and reduce maintenance burden.

## 1. OAuth Implementation

### 1.1 Competing State Management

#### Current Issues:

- Multiple `OAuthState` implementations:
  - One in `services/oauth/mod.rs`
  - Another in `handlers/oauth/mod.rs`
  - Referenced in `config.rs` and `services/mod.rs`
- Inconsistent state handling across different parts of the auth flow
- Mixed old and new Axum state patterns

#### Code to Remove:

```rust
// In src/server/services/mod.rs
pub use oauth::{OAuthConfig, OAuthService, OAuthState};  // Remove OAuthState

// In src/server/handlers/oauth/mod.rs
pub struct OAuthState {
    // Remove entire struct and implementation
}

// In src/server/config.rs - Update to use new state pattern
pub oauth_state: Arc<OAuthState>,  // Update type
```

### 1.2 Token Handling

#### Current Issues:

- Custom `TokenInfo` struct predates OAuth2 v5
- Multiple token exchange implementations
- Inconsistent error handling patterns
- `TokenInfo` imported in multiple files:
  - `services/oauth/github.rs`
  - `services/oauth/scramble.rs`
  - `services/oauth/mod.rs`

#### Code to Remove:

```rust
// In src/server/services/oauth/mod.rs
pub struct TokenInfo {
    pub access_token: String,
    pub token_type: BasicTokenType,
    pub scope: Option<String>,
    pub id_token: Option<String>,
}

// Remove TokenInfo imports from:
use super::{OAuthConfig, OAuthError, OAuthService, TokenInfo};  // In provider files
```

### 1.3 OAuth Client Implementation

#### Current Issues:

- Inconsistent `BasicClient` initialization across files
- Old OAuth2 v4 patterns mixed with v5
- Multiple client wrapper implementations
- Duplicate client configuration code

#### Code to Remove/Update:

```rust
// In src/server/services/oauth/mod.rs
let client = BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url))
    // Update to use OAuth2 v5 builder pattern

// Remove duplicate BasicClient imports from provider files
use oauth2::basic::{BasicClient, BasicTokenType};  // Centralize in mod.rs
```

### 1.4 Redundant OAuth Handlers

#### Current Issues:

- Duplicate callback handling logic
- Mixed response type patterns
- Inconsistent error handling
- Old module-level handlers competing with provider-specific ones

#### Code to Remove:

- All handlers in `src/server/handlers/oauth/mod.rs`
- Keep only provider-specific handlers in:
  - `src/server/handlers/oauth/github.rs`
  - `src/server/handlers/oauth/scramble.rs`

## 2. Response Type Handling

### 2.1 Mixed Axum Versions

#### Current Issues:

- Inconsistent response type usage:
  ```rust
  error[E0308]: mismatched types
  expected `axum::body::Body`, found `axum_core::body::Body`
  ```
- Multiple response conversion patterns
- Mixed old and new Axum response handling
- Competing versions of axum and axum-core

#### Code to Consolidate:

- Standardize on latest Axum response types
- Remove custom response conversion code
- Update all handlers to use consistent response pattern
- Pin axum and axum-core versions

## 3. Configuration Management

### 3.1 Redundant Config Structures

#### Current Issues:

- Multiple config handling patterns
- Inconsistent environment variable loading
- Duplicate OAuth configuration code
- Config structs spread across multiple files

#### Code to Remove:

```rust
// Remove old config patterns in favor of centralized AppConfig
pub struct OAuthConfig {
    // Remove in favor of provider-specific configs
}

// Centralize all config in src/server/config.rs
```

## 4. Session Management

### 4.1 Competing Cookie Handlers

#### Current Issues:

- Multiple cookie creation patterns
- Inconsistent session duration handling
- Mixed cookie security settings
- Duplicate cookie utility functions

#### Code to Consolidate:

- Standardize on `axum_extra::extract::cookie`
- Remove custom cookie builders
- Centralize session management
- Move all cookie handling to dedicated module

## 5. Template Handling

### 5.1 Mixed Template Patterns

#### Current Issues:

- Inconsistent template response handling
- Multiple template inheritance patterns
- Mixed old and new Askama patterns
- Duplicate template utility functions

#### Code to Consolidate:

- Standardize on `askama_axum::IntoResponse`
- Remove custom template response conversions
- Unify template inheritance structure
- Centralize template utilities

## 6. Error Handling

### 6.1 Inconsistent Error Types

#### Current Issues:

- Multiple error conversion patterns
- Inconsistent error response formats
- Redundant error handling code
- Error types spread across modules

#### Code to Consolidate:

- Centralize error types in `src/server/error.rs`
- Remove custom error conversions
- Standardize error response format
- Implement consistent error handling pattern

## Recommended Action Plan

1. **Phase 1: OAuth Cleanup**

   - Remove old OAuth state management
   - Delete outdated token handling
   - Consolidate OAuth handlers
   - Update to OAuth2 v5 patterns

2. **Phase 2: Response Standardization**

   - Update to consistent Axum patterns
   - Remove custom response conversions
   - Standardize error handling
   - Pin dependency versions

3. **Phase 3: Configuration Consolidation**

   - Centralize configuration management
   - Remove redundant config structures
   - Standardize environment handling
   - Create unified config module

4. **Phase 4: Session Management**

   - Unify cookie handling
   - Centralize session management
   - Remove custom implementations
   - Create dedicated session module

5. **Phase 5: Template Cleanup**
   - Standardize template responses
   - Remove custom conversions
   - Unify inheritance patterns
   - Centralize template utilities

## Impact Analysis

### Benefits:

- Reduced code complexity
- Easier maintenance
- Fewer potential bug sources
- Clearer implementation patterns
- Better type safety
- More consistent error handling
- Simplified dependency management
- Clearer code organization

### Risks:

- Potential breaking changes
- Migration effort required
- Need for comprehensive testing
- Possible deployment coordination
- Temporary increase in complexity during migration

## Next Steps

1. Create detailed removal plan for each section
2. Implement changes in isolated branches
3. Add comprehensive tests for new patterns
4. Document new standardized approaches
5. Plan careful deployment strategy
6. Update dependency versions
7. Create migration guides
8. Set up automated testing
