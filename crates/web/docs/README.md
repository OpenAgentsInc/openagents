# WGPUI Web Documentation

GPU-accelerated UI rendering in the browser via WebAssembly.

## Quick Start

```bash
cd crates/web

# Build WASM
wasm-pack build --target web

# Start dev server
bun serve.ts

# Open http://localhost:3000
```

## Documentation

| Document | Description |
|----------|-------------|
| [deployment.md](./deployment.md) | Build, optimize, compress, deploy |
| [architecture.md](./architecture.md) | Technical architecture, data flow, memory model |

## Key Commands

```bash
# Development build (fast, large)
wasm-pack build --target web --dev

# Production build
wasm-pack build --target web

# Production + aggressive optimization
wasm-pack build --target web && wasm-opt -Oz -o pkg/openagents_web_bg.wasm pkg/openagents_web_bg.wasm

# Check binary size
ls -lh pkg/openagents_web_bg.wasm

# Analyze what's in the binary
cargo install twiggy
twiggy top pkg/openagents_web_bg.wasm
```

## Size Optimization Checklist

- [ ] Use `--release` (wasm-pack default)
- [ ] Run `wasm-opt -Oz`
- [ ] Enable gzip/brotli compression on server
- [ ] Use `<link rel="preload">` for WASM
- [ ] Consider lazy loading if not immediately needed

## Browser Requirements

| Feature | Required | Fallback |
|---------|----------|----------|
| WebAssembly | Yes | None |
| ES Modules | Yes | None |
| WebGPU | No | WebGL2 |
| WebGL2 | No | WebGL1 |

Minimum browsers: Chrome 80+, Firefox 80+, Safari 14+, Edge 80+

## Troubleshooting

**Black canvas?**
Check console for WebGPU/WebGL errors. Ensure canvas has CSS dimensions.

**Slow load?**
Enable compression. Pre-compress with brotli. Use CDN.

**"SharedArrayBuffer not defined"?**
Add COOP/COEP headers. See [deployment.md](./deployment.md#server-configuration).

## File Structure

```
crates/web/
├── Cargo.toml          # Crate config
├── src/
│   └── lib.rs          # WASM entry point + demo
├── index.html          # HTML shell
├── serve.ts            # Bun dev server
├── pkg/                # Build output (git-ignored)
│   ├── openagents_web.js
│   ├── openagents_web.d.ts
│   ├── openagents_web_bg.wasm
│   └── package.json
└── docs/
    ├── README.md       # This file
    ├── deployment.md   # Deployment guide
    └── architecture.md # Technical details
```
