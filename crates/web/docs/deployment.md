# WGPUI Web Deployment Guide

Comprehensive guide for building, optimizing, and deploying the WGPUI web demo to production.

## Table of Contents

1. [Build Process](#build-process)
2. [WASM Optimization](#wasm-optimization)
3. [Bundle Size Analysis](#bundle-size-analysis)
4. [Compression](#compression)
5. [CDN & Caching](#cdn--caching)
6. [Server Configuration](#server-configuration)
7. [Browser Compatibility](#browser-compatibility)
8. [Performance Tuning](#performance-tuning)
9. [Troubleshooting](#troubleshooting)

---

## Build Process

### Development Build

```bash
cd crates/web
wasm-pack build --target web --dev
bun serve.ts
```

Development builds are faster but larger (~15-20MB) with debug symbols.

### Release Build

```bash
wasm-pack build --target web
```

This runs:
1. `cargo build --release --target wasm32-unknown-unknown`
2. `wasm-bindgen` to generate JS bindings
3. `wasm-opt` for optimization (if installed)

### Build Targets

| Target | Use Case | Output |
|--------|----------|--------|
| `--target web` | ES modules, modern browsers | `pkg/*.js` + `pkg/*.wasm` |
| `--target bundler` | Webpack/Vite/esbuild | Requires bundler |
| `--target nodejs` | Node.js/Bun server-side | CommonJS |
| `--target no-modules` | Legacy `<script>` tags | Global variable |

We use `--target web` for direct ES module loading without a bundler.

---

## WASM Optimization

### What is wasm-opt?

`wasm-opt` is the WebAssembly optimizer from the [Binaryen](https://github.com/WebAssembly/binaryen) project. It performs:

- **Dead code elimination** - Removes unused functions
- **Inlining** - Merges small functions into callers
- **Constant folding** - Pre-computes constant expressions
- **Control flow optimization** - Simplifies branches and loops
- **Memory optimization** - Reduces memory access overhead

### Installation

```bash
# macOS
brew install binaryen

# Linux
apt install binaryen

# From source
git clone https://github.com/WebAssembly/binaryen
cd binaryen && cmake . && make
```

### Optimization Levels

| Flag | Description | Size | Speed | Build Time |
|------|-------------|------|-------|------------|
| `-O0` | No optimization | Largest | Slowest | Fastest |
| `-O1` | Basic optimization | Large | Slow | Fast |
| `-O2` | Standard optimization | Medium | Medium | Medium |
| `-O3` | Aggressive speed optimization | Medium | Fastest | Slow |
| `-O4` | Very aggressive speed | Medium | Fastest | Very slow |
| `-Os` | Optimize for size | Small | Medium | Medium |
| `-Oz` | Aggressive size optimization | Smallest | Slower | Slow |

### Manual Optimization

After `wasm-pack build`, you can re-optimize:

```bash
# Size-optimized (recommended for web)
wasm-opt -Oz -o pkg/openagents_web_bg_opt.wasm pkg/openagents_web_bg.wasm

# Speed-optimized
wasm-opt -O3 -o pkg/openagents_web_bg_opt.wasm pkg/openagents_web_bg.wasm

# Maximum size reduction (slower runtime)
wasm-opt -Oz --zero-filled-memory --strip-debug --strip-producers \
  -o pkg/openagents_web_bg_opt.wasm pkg/openagents_web_bg.wasm
```

### Advanced wasm-opt Flags

```bash
wasm-opt \
  -Oz \                          # Optimize for size
  --zero-filled-memory \         # Assume memory is zero-initialized
  --strip-debug \                # Remove debug info
  --strip-producers \            # Remove producer section
  --strip-target-features \      # Remove target features section
  --enable-simd \                # Enable SIMD optimizations
  --enable-bulk-memory \         # Enable bulk memory ops
  --enable-multivalue \          # Enable multi-value returns
  --converge \                   # Run until no more improvements
  -o output.wasm input.wasm
```

### Expected Size Reductions

| Stage | Size | Notes |
|-------|------|-------|
| Debug build | ~20 MB | Full debug symbols |
| Release build | ~8 MB | No debug, LTO |
| wasm-pack (default) | ~5.7 MB | Basic wasm-opt |
| wasm-opt -Oz | ~4 MB | Aggressive size |
| + gzip | ~1.2 MB | Compressed transfer |
| + brotli | ~900 KB | Best compression |

---

## Bundle Size Analysis

### What's in the WASM?

Our 5.7MB WASM includes:

| Component | Approx Size | Purpose |
|-----------|-------------|---------|
| cosmic-text | ~1.5 MB | Text shaping, font handling |
| syntect | ~1.2 MB | Syntax highlighting (themes + grammars) |
| wgpu | ~1.0 MB | WebGPU/WebGL abstraction |
| embedded fonts | ~800 KB | Vera Mono (4 variants) |
| pulldown-cmark | ~200 KB | Markdown parsing |
| taffy | ~150 KB | Flexbox layout |
| other | ~850 KB | Misc dependencies |

### Reducing Size

**Option 1: Strip syntax highlighting**

If you don't need code highlighting, disable syntect:
```toml
# In wgpui/Cargo.toml, make syntect optional
syntect = { version = "5.2", optional = true }
```

Saves ~1.2 MB.

**Option 2: Reduce font variants**

Edit `wgpui/src/text.rs` to embed fewer fonts:
- Regular only: saves ~600 KB
- No embedded fonts (require user to provide): saves ~800 KB

**Option 3: Use wasm-split for lazy loading**

```bash
# Split WASM into primary + secondary modules
wasm-split input.wasm -o1 primary.wasm -o2 secondary.wasm --profile=profile.txt
```

### Analyzing the Binary

```bash
# List all functions by size
wasm-objdump -d pkg/openagents_web_bg.wasm | \
  grep -E "^[0-9a-f]+ <" | \
  awk '{print $2}' | \
  sort | uniq -c | sort -rn | head -20

# Use twiggy for detailed analysis
cargo install twiggy
twiggy top pkg/openagents_web_bg.wasm
twiggy dominators pkg/openagents_web_bg.wasm
```

---

## Compression

### Server-Side Compression

WASM compresses extremely well. Always serve with compression.

**Compression Ratios (5.7MB WASM):**

| Method | Compressed Size | Ratio |
|--------|-----------------|-------|
| None | 5.7 MB | 1.0x |
| gzip -6 | 1.4 MB | 4.1x |
| gzip -9 | 1.3 MB | 4.4x |
| brotli -6 | 1.1 MB | 5.2x |
| brotli -11 | 900 KB | 6.3x |

### Pre-Compression

Pre-compress at build time for static hosting:

```bash
# Gzip
gzip -k -9 pkg/openagents_web_bg.wasm
# Creates: pkg/openagents_web_bg.wasm.gz

# Brotli (better compression)
brotli -k -q 11 pkg/openagents_web_bg.wasm
# Creates: pkg/openagents_web_bg.wasm.br
```

### Build Script with Compression

```bash
#!/bin/bash
set -e

cd crates/web

# Build
wasm-pack build --target web

# Optimize
wasm-opt -Oz -o pkg/openagents_web_bg.wasm pkg/openagents_web_bg.wasm

# Pre-compress
gzip -k -9 pkg/openagents_web_bg.wasm
brotli -k -q 11 pkg/openagents_web_bg.wasm
gzip -k -9 pkg/openagents_web.js
brotli -k -q 11 pkg/openagents_web.js

echo "Build complete:"
ls -lh pkg/
```

---

## CDN & Caching

### Cache Headers

WASM and JS files are immutable per build. Use aggressive caching:

```
# For versioned/hashed files
Cache-Control: public, max-age=31536000, immutable

# For index.html (must revalidate)
Cache-Control: no-cache, must-revalidate
```

### Content Hashing

Rename files with content hash for cache busting:

```bash
# Generate hash
HASH=$(sha256sum pkg/openagents_web_bg.wasm | cut -c1-8)
mv pkg/openagents_web_bg.wasm "pkg/openagents_web_bg.${HASH}.wasm"

# Update JS to reference new filename
sed -i "s/openagents_web_bg.wasm/openagents_web_bg.${HASH}.wasm/" pkg/openagents_web.js
```

### CDN Configuration

**Cloudflare:**
```
# _headers file
/pkg/*
  Cache-Control: public, max-age=31536000, immutable
  Access-Control-Allow-Origin: *

/*.wasm
  Content-Type: application/wasm
```

**AWS CloudFront:**
```json
{
  "CacheBehaviors": [{
    "PathPattern": "/pkg/*",
    "DefaultTTL": 31536000,
    "Compress": true
  }]
}
```

**Vercel (vercel.json):**
```json
{
  "headers": [
    {
      "source": "/pkg/(.*)",
      "headers": [
        { "key": "Cache-Control", "value": "public, max-age=31536000, immutable" }
      ]
    }
  ]
}
```

---

## Server Configuration

### Required Headers

WASM with SharedArrayBuffer (for threads) requires:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Our `serve.ts` already sets these.

### MIME Types

Ensure your server sends correct MIME types:

| Extension | MIME Type |
|-----------|-----------|
| `.wasm` | `application/wasm` |
| `.js` | `application/javascript` |
| `.html` | `text/html` |

### Nginx Configuration

```nginx
server {
    listen 80;
    root /var/www/wgpui;

    # WASM MIME type
    types {
        application/wasm wasm;
    }

    # Compression
    gzip on;
    gzip_types application/wasm application/javascript;

    # Pre-compressed files
    gzip_static on;
    brotli_static on;

    # COOP/COEP headers
    add_header Cross-Origin-Opener-Policy same-origin;
    add_header Cross-Origin-Embedder-Policy require-corp;

    # Cache immutable assets
    location /pkg/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

### Caddy Configuration

```caddyfile
example.com {
    root * /var/www/wgpui
    file_server

    header /pkg/* Cache-Control "public, max-age=31536000, immutable"
    header * Cross-Origin-Opener-Policy same-origin
    header * Cross-Origin-Embedder-Policy require-corp

    @wasm path *.wasm
    header @wasm Content-Type application/wasm

    encode gzip brotli
}
```

---

## Browser Compatibility

### WebGPU vs WebGL

WGPUI automatically falls back:

1. **WebGPU** (preferred) - Chrome 113+, Edge 113+, Firefox Nightly
2. **WebGL2** (fallback) - All modern browsers
3. **WebGL1** (legacy) - Older browsers

### Browser Support Matrix

| Browser | WebGPU | WebGL2 | Status |
|---------|--------|--------|--------|
| Chrome 113+ | Yes | Yes | Full support |
| Edge 113+ | Yes | Yes | Full support |
| Firefox 120+ | Flag | Yes | WebGL2 fallback |
| Safari 17+ | Yes | Yes | Full support |
| Safari 16 | No | Yes | WebGL2 fallback |
| Mobile Chrome | Partial | Yes | WebGL2 recommended |
| Mobile Safari | iOS 17+ | Yes | Full support |

### Feature Detection

```javascript
async function checkSupport() {
    // Check WebGPU
    if ('gpu' in navigator) {
        const adapter = await navigator.gpu.requestAdapter();
        if (adapter) {
            console.log('WebGPU supported');
            return 'webgpu';
        }
    }

    // Check WebGL2
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl2');
    if (gl) {
        console.log('WebGL2 supported');
        return 'webgl2';
    }

    // Check WebGL1
    const gl1 = canvas.getContext('webgl');
    if (gl1) {
        console.log('WebGL1 supported (limited)');
        return 'webgl1';
    }

    return null;
}
```

### Polyfills

No polyfills needed - wgpu handles fallbacks internally.

---

## Performance Tuning

### Startup Optimization

**1. Streaming Compilation**

Browsers compile WASM while downloading. Ensure:
- Server sends `Content-Type: application/wasm`
- Use `WebAssembly.instantiateStreaming()` (wasm-bindgen does this)

**2. Lazy Loading**

Load WASM only when needed:

```javascript
let wasmModule = null;

async function initWasm() {
    if (!wasmModule) {
        const { default: init, start_demo } = await import('./pkg/openagents_web.js');
        await init();
        wasmModule = { start_demo };
    }
    return wasmModule;
}

// Load on user interaction
button.onclick = async () => {
    const { start_demo } = await initWasm();
    await start_demo('canvas');
};
```

**3. Preloading**

Hint browser to fetch early:

```html
<link rel="modulepreload" href="/pkg/openagents_web.js">
<link rel="preload" href="/pkg/openagents_web_bg.wasm" as="fetch" crossorigin>
```

### Runtime Performance

**1. RequestAnimationFrame**

Already used via `run_animation_loop()`. Ensures 60fps sync.

**2. Avoid Layout Thrashing**

Batch DOM reads/writes. Our canvas-only approach avoids this.

**3. GPU Memory**

Monitor with:
```javascript
if ('memory' in performance) {
    console.log('JS Heap:', performance.memory.usedJSHeapSize / 1024 / 1024, 'MB');
}
```

**4. Profiling**

```javascript
// In browser console
performance.mark('wasm-start');
await start_demo('canvas');
performance.mark('wasm-ready');
performance.measure('WASM Init', 'wasm-start', 'wasm-ready');
```

---

## Troubleshooting

### Common Issues

**1. "WebAssembly.instantiate(): expected magic word"**

Cause: Server sending wrong MIME type or corrupted file.

Fix: Ensure `Content-Type: application/wasm` header.

**2. "SharedArrayBuffer is not defined"**

Cause: Missing COOP/COEP headers.

Fix: Add headers:
```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**3. "Failed to execute 'compile' on 'WebAssembly'"**

Cause: Browser doesn't support WASM or file too large.

Fix: Check browser compatibility, ensure WASM < 4GB.

**4. "RuntimeError: memory access out of bounds"**

Cause: WASM memory exhausted or corruption.

Fix: Increase memory limit in Cargo.toml:
```toml
[profile.release]
lto = true
# Memory grows as needed, but check for leaks
```

**5. Canvas is black/blank**

Cause: WebGPU/WebGL context creation failed.

Fix: Check console for adapter errors. Ensure canvas has dimensions:
```css
#canvas { width: 100%; height: 100%; display: block; }
```

**6. Slow initial load**

Cause: Large uncompressed WASM.

Fix:
- Enable gzip/brotli compression
- Use `wasm-opt -Oz`
- Preload WASM file
- Show loading indicator

### Debug Mode

Build with debug info:

```bash
wasm-pack build --target web --dev
```

Enable console logging in Rust:
```rust
web_sys::console::log_1(&"Debug message".into());
```

### Memory Profiling

```javascript
// Check WASM memory usage
const memory = wasmModule.__wbindgen_export_0;
console.log('WASM Memory:', memory.buffer.byteLength / 1024 / 1024, 'MB');
```

---

## Quick Reference

### Build Commands

```bash
# Development
wasm-pack build --target web --dev

# Production
wasm-pack build --target web

# Production + extra optimization
wasm-pack build --target web && \
  wasm-opt -Oz -o pkg/openagents_web_bg.wasm pkg/openagents_web_bg.wasm

# With compression
wasm-pack build --target web && \
  wasm-opt -Oz -o pkg/openagents_web_bg.wasm pkg/openagents_web_bg.wasm && \
  brotli -k -q 11 pkg/openagents_web_bg.wasm && \
  brotli -k -q 11 pkg/openagents_web.js
```

### Size Targets

| Environment | Target Size (compressed) |
|-------------|-------------------------|
| Fast 3G | < 2 MB |
| Average mobile | < 1.5 MB |
| Desktop | < 3 MB |

### Performance Targets

| Metric | Target |
|--------|--------|
| WASM compile | < 2s |
| First frame | < 3s |
| Frame rate | 60 fps |
| Input latency | < 16ms |
