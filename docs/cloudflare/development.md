# Cloudflare Development Guide

## Prerequisites

1. **Install wrangler**
   ```bash
   npm install -g wrangler
   # or
   bun install -g wrangler
   ```

2. **Install worker-build**
   ```bash
   cargo install worker-build
   ```

3. **Login to Cloudflare** (for deployment)
   ```bash
   wrangler login
   ```

## Local Development

```bash
cd crates/cloudflare

# Start local dev server (uses Miniflare)
wrangler dev
```

The relay will be available at `ws://localhost:8787`.

## Testing

### Using websocat

```bash
# Install websocat
cargo install websocat

# Connect to local relay
websocat ws://localhost:8787

# Send NIP-01 messages:
["REQ", "test", {"kinds": [1], "limit": 10}]

# Expected response:
["EOSE", "test"]
```

### Using TypeScript

```typescript
const ws = new WebSocket('ws://localhost:8787');

ws.onopen = () => {
  // Subscribe to kind 1 events
  ws.send(JSON.stringify(['REQ', 'test-sub', { kinds: [1], limit: 10 }]));
};

ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  console.log('Received:', msg);

  if (msg[0] === 'EOSE') {
    console.log('Subscription ready!');
  }
};
```

## Deployment

```bash
cd crates/cloudflare

# Deploy to Cloudflare
wrangler deploy

# Deploy to dev environment
wrangler deploy --env dev
```

## Configuration

### wrangler.toml

```toml
name = "openagents-relay"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[durable_objects]
bindings = [{ name = "RELAY", class_name = "RelayDurableObject" }]

[[migrations]]
tag = "v1"
new_sqlite_classes = ["RelayDurableObject"]
```

### Environment Variables

Set via wrangler or dashboard:

```bash
# Relay identity (optional)
wrangler secret put RELAY_PRIVATE_KEY

# Admin pubkeys (optional)
wrangler secret put ADMIN_PUBKEYS
```

## Debugging

```bash
# View logs
wrangler tail

# View specific worker
wrangler tail openagents-relay
```

## Troubleshooting

### Build fails with WASM errors

Make sure you're using the `wasm` feature for `nostr-relay`:

```toml
nostr-relay = { path = "../nostr-relay", default-features = false, features = ["wasm"] }
```

### WebSocket not connecting

Check that:
1. wrangler dev is running
2. You're connecting to the right port (default: 8787)
3. The URL is `ws://` not `http://`

### Durable Object errors

Ensure migrations are up to date:

```bash
wrangler migrations apply
```
