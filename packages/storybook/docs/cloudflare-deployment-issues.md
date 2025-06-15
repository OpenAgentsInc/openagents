# Cloudflare Workers Storybook Deployment Issues

## Current Status
The Storybook application builds successfully but returns 404 errors when accessed via:
- https://openagents-storybook.openagents.workers.dev/
- https://storybook.openagents.com/ (DNS not propagated yet)

## What Works
1. ✅ Storybook builds successfully with `pnpm build-storybook`
2. ✅ Build output exists in `storybook-static/` directory with all files including `index.html`
3. ✅ Wrangler deploys successfully without errors
4. ✅ Assets are uploaded (66 files total)
5. ✅ Custom domain is configured in wrangler.toml

## What Doesn't Work
1. ❌ Root path `/` returns Cloudflare's default 404 page
2. ❌ Direct path `/index.html` also returns 404
3. ❌ No static assets are being served
4. ❌ Getting Cloudflare's default 404 page instead of worker responses

## Configurations Attempted

### 1. Modern Assets API with SPA Support (Following Guide)
```toml
# wrangler.toml
name = "openagents-storybook"
compatibility_date = "2024-09-25"

[build]
command = "pnpm build-storybook"

[assets]
directory = "storybook-static"
not_found_handling = "single-page-application"

[[routes]]
pattern = "storybook.openagents.com"
custom_domain = true
```

### 2. With Worker and Binding
```toml
name = "openagents-storybook"
main = "src/worker.js"
compatibility_date = "2024-09-25"

[assets]
directory = "./storybook-static/"
binding = "ASSETS"
not_found_handling = "single-page-application"
html_handling = "auto-trailing-slash"
run_worker_first = false
```

### 3. JSONC Configuration (Also Attempted)
- Same configurations in JSONC format
- Both formats produced identical 404 errors

### 4. Various Directory Path Formats
- `./storybook-static/`
- `./storybook-static`
- `storybook-static/`
- `storybook-static`
- All produced the same 404 errors

## Key Observations

1. **Cloudflare's Default 404**: We're getting Cloudflare's own 404 page, NOT our worker's 404
   - This suggests the worker isn't being invoked at all
   - Or the assets binding is completely failing

2. **Successful Upload But No Access**: Wrangler reports uploading 66 files successfully, but none are accessible

3. **SPA Configuration Not Working**: Despite adding `not_found_handling = "single-page-application"`, routes still return 404

4. **No Error Messages**: Deployment succeeds without any warnings or errors

## Hypothesis
The issue appears to be that:
1. Assets are uploading to some location
2. But the worker/routing configuration isn't correctly mapped to serve them
3. The Cloudflare edge is returning its own 404 before our worker or assets are consulted

## Next Steps
1. Create a minimal test project from scratch using Cloudflare's getting started guide
2. Compare the working structure with our current setup
3. Identify what's different about our configuration

## Build Output Structure (Verified)
```
storybook-static/
├── index.html (exists, 4.5KB)
├── iframe.html
├── assets/
│   ├── *.js (multiple bundles)
│   └── *.css
├── sb-addons/
├── sb-common-assets/
├── sb-manager/
└── sb-preview/
```

## Questions Remaining
1. Why does Cloudflare return its own 404 page instead of attempting to serve assets?
2. Is there a required project structure we're missing?
3. Are we using an outdated or incorrect API?
4. Is the issue related to the monorepo structure or package location?