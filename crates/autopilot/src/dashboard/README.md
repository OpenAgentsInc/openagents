# Dashboard Module Structure

This directory contains the autopilot metrics dashboard implementation.

## Current Status

**Note**: The dashboard is currently implemented as a monolithic file (`../dashboard.rs`, 1037 lines). This directory contains the modular structure that will be migrated to in the future.

## Planned Structure

```
dashboard/
├── mod.rs           # Public API, re-exports
├── server.rs        # Actix web server setup
├── routes.rs        # Route handlers
├── state.rs         # Shared state management (IMPLEMENTED)
└── views/
    ├── mod.rs
    ├── index.rs     # Dashboard home
    ├── sessions.rs  # Session list/detail
    └── metrics.rs   # Metrics views
```

## Migration Plan

### Phase 1: Extract State (DONE)
- ✅ `state.rs` - Dashboard application state

### Phase 2: Extract Route Handlers
Create `routes.rs` with all route handler functions:
- `index()` - Home page with session list
- `sessions_list()` - Sessions list page
- `session_detail()` - Session detail page
- `export_json()` - Export sessions as JSON
- `export_csv()` - Export sessions as CSV
- `websocket()` - WebSocket endpoint
- API routes: `api_sessions()`, `api_session_detail()`, `api_metrics()`, `api_anomalies()`, `api_trends()`

### Phase 3: Extract View Rendering
Create `views/` modules for rendering functions:

**`views/index.rs`:**
- `dashboard_page()` - Main dashboard HTML
- `summary_card()` - Summary statistics card
- `sessions_table()` - Sessions table
- `session_row()` - Individual session row

**`views/sessions.rs`:**
- `sessions_list_page()` - Sessions list page
- `session_detail_page()` - Session detail page

**`views/metrics.rs`:**
- `calculate_error_rate_trend()`
- `calculate_completion_rate_trend()`
- `calculate_duration_trend()`
- `calculate_cost_trend()`
- `calculate_tokens_trend()`

**`views/styles.rs`:**
- `dashboard_styles()` - CSS styles (407 lines!)

### Phase 4: Extract Server Setup
Create `server.rs`:
- `start_dashboard()` - Main server initialization
- Route configuration
- Middleware setup

### Phase 5: Create mod.rs
Tie it all together with public API:
```rust
pub use state::DashboardState;
pub use server::start_dashboard;
```

## Why This Refactoring Matters

The current monolithic structure makes it difficult to:
1. Add WebSocket streaming (#417)
2. Add new visualizations
3. Test components in isolation
4. Understand the codebase
5. Make concurrent changes

The modular structure enables:
1. Clear separation of concerns
2. Easy addition of new routes/views
3. Better testability
4. WebSocket integration in `server.rs`
5. Live streaming in new `views/live.rs`

## Migration Steps

When ready to execute this refactoring:

1. **Create `routes.rs`**
   ```bash
   # Extract all route handlers (lines 59-1005)
   # Update imports to use `super::*`
   ```

2. **Create views modules**
   ```bash
   # Extract rendering functions to views/
   # Keep maud imports local to each view
   ```

3. **Create `server.rs`**
   ```bash
   # Move start_dashboard() and route config
   # Import routes and state
   ```

4. **Create `mod.rs`**
   ```bash
   # Re-export public API
   # Keep internal modules private
   ```

5. **Update `lib.rs`**
   ```bash
   # Change from `pub mod dashboard;`
   # to dashboard being the directory
   ```

6. **Delete old `dashboard.rs`**
   ```bash
   rm crates/autopilot/src/dashboard.rs
   ```

7. **Run tests**
   ```bash
   cargo test -p autopilot
   ```

## Benefits Once Complete

- **Smaller files**: Each module <300 lines
- **Clear structure**: Easy to find code
- **Extensible**: Add features without touching core
- **Testable**: Unit test individual components
- **WebSocket ready**: Clean place to add streaming

## Notes

This refactoring is a prerequisite for #417 (real-time session streaming). Once the modular structure is in place, WebSocket streaming can be added cleanly in a new `ws.rs` module without touching existing code.

The current `dashboard.rs` works fine and passes all tests. This refactoring is about long-term maintainability and enabling future features.
