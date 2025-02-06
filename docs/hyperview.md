# Hyperview Integration

OpenAgents uses [Hyperview](https://hyperview.org) to serve server-driven UI to the Onyx mobile app. This allows us to update the mobile app's UI and behavior by deploying backend changes, without requiring app store updates.

## Overview

Hyperview consists of two main parts:

1. Server-side HXML generation (this repo)
2. Client-side HXML rendering (Onyx repo)

The server exposes HXML endpoints that the mobile app consumes to render its interface.

## HXML Endpoints

Base URL: `https://openagents.com/hyperview/`

### Main Endpoints

- `/hyperview` - Entry point, serves the initial screen
- More endpoints coming soon...

### MIME Type

All HXML responses use the content type:

```
application/vnd.hyperview+xml
```

## Example Response

A basic HXML response looks like:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="text" alignItems="center" justifyContent="center" />
    </styles>
    <body>
      <view style="text">
        <text>Hello from OpenAgents!</text>
      </view>
    </body>
  </screen>
</doc>
```

## Server Implementation

The Hyperview server code lives in `src/server/hyperview/` with the following structure:

```
src/server/hyperview/
├── mod.rs       - Module exports
├── handlers.rs  - Request handlers
├── routes.rs    - Route definitions
├── templates.rs - HXML templates
└── types.rs     - Type definitions
```

### Adding New Endpoints

1. Create handler function in `handlers.rs`:

```rust
pub async fn my_screen(State(state): State<AppState>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(/* HXML content */.to_string().into())
        .unwrap()
}
```

2. Add route in `routes.rs`:

```rust
pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview/my-screen", get(handlers::my_screen))
}
```

## Client Integration

The Onyx mobile app integrates the Hyperview client to render HXML content. See the [Onyx Hyperview docs](https://github.com/OpenAgentsInc/onyx/blob/main/docs/hyperview.md) for client-side details.

## Resources

- [Hyperview Documentation](https://hyperview.org/docs/guide_introduction)
- [HXML Reference](https://hyperview.org/docs/reference_index)
- [Example Apps](https://hyperview.org/docs/example_index)

## Future Improvements

1. Add templating system for HXML
2. Add authentication/authorization
3. Create reusable HXML components
4. Add proper error handling
5. Add response caching
6. Add logging and monitoring
7. Add testing infrastructure
