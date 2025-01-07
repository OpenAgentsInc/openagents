# Rust Backend Setup for Nostr-HTMX Integration

## Overview
We'll create a Rust/Actix web server that serves our Nostr-HTMX frontend while providing additional backend capabilities. This setup will be based on patterns from the existing "oa" repository but optimized for our Nostr integration needs.

## Architecture Plan

### 1. Project Structure
```
openagents/
├── src/
│   ├── main.rs           # Application entry point
│   ├── server/           # Server configuration and setup
│   │   ├── mod.rs        # Server module exports
│   │   └── routes.rs     # Route handlers
│   └── static/           # Static file serving
│       └── nostr/        # Nostr-HTMX frontend files
├── Cargo.toml
└── .env                  # Environment configuration
```

### 2. Core Components

#### Actix Web Server
- Configure Actix to serve static files from the `static` directory
- Set up CORS and security middleware
- Handle both API routes and static file serving
- Configure WebSocket support for potential future real-time features

#### Static File Serving
- Port the Nostr-HTMX frontend to `static/nostr/`
- Configure proper MIME types for .js, .html, and other static assets
- Set up caching headers for optimal performance

#### Build Process
1. Build the Nostr-HTMX frontend using esbuild
2. Copy built assets to `static/nostr/`
3. Compile Rust application
4. Serve everything through Actix

## Implementation Steps

1. **Initial Setup**
```bash
cargo new openagents
cd openagents
```

2. **Dependencies** (to be added to Cargo.toml)
```toml
[dependencies]
actix-web = "4.4"
actix-files = "0.6"
actix-cors = "0.6"
env_logger = "0.10"
dotenv = "0.15"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

3. **Frontend Integration**
- Copy built Nostr-HTMX assets to `static/nostr/`
- Configure Actix to serve these files
- Set up proper routing to handle both API endpoints and static files

4. **Security Considerations**
- Implement proper CORS configuration
- Set up security headers
- Configure rate limiting if needed
- Handle proper error responses

## Development Workflow

1. **Local Development**
```bash
# Terminal 1 - Frontend
cd static/nostr
just build  # Build Nostr-HTMX frontend

# Terminal 2 - Backend
cargo watch -x run  # Auto-reload Rust server on changes
```

2. **Production Build**
```bash
# Build frontend
cd static/nostr
just build

# Build Rust application
cargo build --release
```

## Future Considerations

1. **WebSocket Integration**
- Potential direct WebSocket connection to Nostr relays through our Rust backend
- Implement connection pooling and relay management

2. **API Extensions**
- Add REST endpoints for additional functionality
- Implement caching layer for Nostr data
- Add authentication/authorization if needed

3. **Monitoring**
- Add logging and metrics collection
- Implement health check endpoints
- Set up error tracking

## Next Steps

1. Set up basic Rust project structure
2. Implement static file serving
3. Port Nostr-HTMX frontend
4. Add basic API endpoints
5. Configure deployment pipeline

## References

- Current "oa" repo structure and patterns
- Actix-web documentation
- Nostr-HTMX integration patterns
- Rust async/await best practices

This setup provides a solid foundation for serving the Nostr-HTMX frontend while allowing for future expansion of backend capabilities. The architecture maintains simplicity while providing clear paths for scaling and adding features as needed.