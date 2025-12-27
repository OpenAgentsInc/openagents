# Demo Gallery Deployment Guide

## Quick Start (Local Testing)

```bash
cd demo
python3 -m http.server 8000
# Visit http://localhost:8000
```

## Production Deployment Options

### Option 1: GitHub Pages (Recommended for MVP)

**Pros:** Free, automatic SSL, simple deployment
**Cons:** Public only, no server-side logic

```bash
# 1. Enable GitHub Pages in repo settings
# Settings → Pages → Source: main branch, /demo folder

# 2. Commit demo files
git add demo/
git commit -m "Deploy demo gallery"
git push origin main

# 3. Access at: https://openagentsinc.github.io/openagents/
```

**Custom Domain Setup:**
```bash
# Add CNAME file
echo "demo.openagents.com" > demo/CNAME
git add demo/CNAME
git commit -m "Add custom domain for demo"
git push origin main

# Configure DNS:
# Type: CNAME
# Name: demo
# Value: openagentsinc.github.io
```

### Option 2: Cloudflare Pages

**Pros:** Faster than GitHub Pages, analytics included, automatic deploys
**Cons:** Requires Cloudflare account

```bash
# 1. Connect GitHub repo to Cloudflare Pages
# 2. Build settings:
#    - Build command: (none)
#    - Build output directory: /demo
#    - Root directory: /
# 3. Deploy triggers on git push automatically
```

### Option 3: Vercel

**Pros:** Serverless functions available, preview deployments
**Cons:** Rate limits on free tier

```bash
# Install Vercel CLI
npm install -g vercel

# Deploy from demo/ directory
cd demo
vercel --prod

# Or connect GitHub repo for automatic deploys
```

### Option 4: Self-Hosted (Nginx)

**Pros:** Full control, can add auth, analytics, rate limiting
**Cons:** Server maintenance overhead

```nginx
# /etc/nginx/sites-available/demo.openagents.com
server {
    listen 80;
    listen [::]:80;
    server_name demo.openagents.com;

    root /var/www/openagents/demo;
    index index.html;

    location / {
        try_files $uri $uri/ =404;
    }

    # Cache replay bundles
    location ~* \.json$ {
        expires 1h;
        add_header Cache-Control "public, immutable";
    }

    # CORS for replay bundles (if needed)
    location ~* \.json$ {
        add_header Access-Control-Allow-Origin "*";
    }
}
```

## File Structure

```
demo/
├── index.html                      # Demo gallery (landing page)
├── replay-viewer.html              # Replay viewer component
├── sample-replay.json              # Example demo (tiny)
├── demo-keyboard-shortcuts.json    # Real demo (605 KB)
├── README.md                       # Usage documentation
├── DEPLOYMENT.md                   # This file
└── demos/                          # (future) Additional demos
    ├── feature-implementation.json
    ├── bug-fix.json
    └── test-suite.json
```

## Adding New Demos

### 1. Generate Replay Bundle

Run autopilot on an issue, then convert the session log:

```bash
# Find latest session log
SESSION_LOG=$(ls -t ~/.openagents/sessions/*/*.jsonl | head -1)

# Convert to replay bundle
cargo run -p autopilot --example replay_demo \
  $SESSION_LOG \
  demo/my-new-demo.json
```

### 2. Quality Check

Before publishing, verify:
- ✅ Session completed successfully (no crashes)
- ✅ Tests passed (if applicable)
- ✅ PR was merged (or would be mergeable)
- ✅ No secrets leaked (redaction worked)
- ✅ Duration is reasonable (2-15 min compressed)
- ✅ Story is clear (good issue description)

```bash
# Validate JSON
jq . demo/my-new-demo.json > /dev/null && echo "Valid JSON"

# Check size (should be < 2MB)
ls -lh demo/my-new-demo.json

# Preview in viewer
python3 -m http.server 8000 &
open http://localhost:8000/replay-viewer.html?demo=my-new-demo.json
```

### 3. Add to Gallery

Edit `demo/index.html`, add entry to `demos` array:

```javascript
{
    id: 'my-feature',
    title: 'Implement Feature X',
    description: 'Shows autopilot adding feature X with tests and documentation.',
    duration: '5m 30s',           // Demo duration at 2x speed
    model: 'sonnet-4.5',
    files_changed: 7,
    tests_passed: '89/89',        // Or 'N/A' if no tests
    tags: ['rust', 'feature', 'testing'],
    status: 'live',
    bundle: 'my-new-demo.json'
}
```

### 4. Deploy

```bash
git add demo/my-new-demo.json demo/index.html
git commit -m "Add demo: Implement Feature X"
git push origin main
```

## Performance Optimization

### Compress Large Replays

```bash
# Gzip compression (served by nginx/CDN)
gzip -k demo/large-demo.json  # Creates large-demo.json.gz

# Or use brotli for better compression
brotli -k demo/large-demo.json
```

### CDN Hosting for Bundles

For production, host replay bundles on CDN:

```bash
# Upload to Cloudflare R2
aws s3 cp demo/my-demo.json s3://openagents-demos/my-demo.json \
  --endpoint-url https://[account-id].r2.cloudflarestorage.com

# Update bundle path in index.html
bundle: 'https://demos.openagents.com/my-demo.json'
```

## Analytics Setup

### Option 1: Plausible (Privacy-Friendly)

Add to `<head>` in index.html:
```html
<script defer data-domain="demo.openagents.com" src="https://plausible.io/js/script.js"></script>
```

Track demo views:
```javascript
// In demo card click handler
plausible('Demo View', {props: {demo: demo.id}});
```

### Option 2: Google Analytics

```html
<!-- Google tag (gtag.js) -->
<script async src="https://www.googletagmanager.com/gtag/js?id=G-XXXXXXXXXX"></script>
<script>
  window.dataLayer = window.dataLayer || [];
  function gtag(){dataLayer.push(arguments);}
  gtag('js', new Date());
  gtag('config', 'G-XXXXXXXXXX');
</script>
```

## Security Considerations

### Before Publishing Demos

1. **Manual Review:** Always review replay bundles for leaked secrets
2. **Redaction Check:** Verify API keys, tokens, paths are redacted
3. **Content Review:** Ensure no sensitive business logic exposed
4. **Size Limits:** Keep bundles < 2MB (prevents DOS via large files)

### Production Hardening

```nginx
# Rate limiting
limit_req_zone $binary_remote_addr zone=demo:10m rate=10r/s;

server {
    location / {
        limit_req zone=demo burst=20;
    }
}
```

## Monitoring

### Health Checks

```bash
# Verify demo gallery loads
curl -sI https://demo.openagents.com | grep "200 OK"

# Verify specific demo loads
curl -s https://demo.openagents.com/demo-keyboard-shortcuts.json | jq .id
```

### Metrics to Track

- **Page Views:** Demo gallery visits
- **Demo Plays:** Replay viewer loads
- **Completion Rate:** % of viewers who watch >50% of demo
- **CTA Clicks:** "Try It On Your Repo" button clicks
- **Bounce Rate:** % who leave without engaging

## Troubleshooting

### Demo won't load
```bash
# Check JSON is valid
jq . demo/my-demo.json

# Check file size
ls -lh demo/my-demo.json

# Check browser console for errors
# Common: CORS issues, invalid JSON, missing fields
```

### Slow loading
```bash
# Enable gzip compression on server
# Check bundle size (should be < 500KB ideally)
# Consider removing verbose tool results
```

### Replay timeline broken
```bash
# Verify timeline events have timestamps
jq '.timeline[] | select(.t == null)' demo/my-demo.json

# Check playback speed is set
jq '.metadata.playback_speed' demo/my-demo.json
```

## Next Steps

After deploying MVP:
1. Add 5-10 more demo sessions
2. Implement A/B testing for different demos
3. Add demo search/filter
4. Mobile-responsive design
5. Video export option

## Related Documentation

- [Demo README](README.md) - Usage and features
- [Replay Format](../crates/autopilot/src/replay.rs) - Bundle specification
- [d-027 Progress](.openagents/d-027-progress.md) - Implementation status
