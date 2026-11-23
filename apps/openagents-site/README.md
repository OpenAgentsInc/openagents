# OpenAgents Website (`openagents.com`)

Zero-based minimalist homepage for **OpenAgents** — pure black background (`#000000`), white OpenAgents logo, app showcase, **Download for Mac**, **Build from source for Windows and Linux** (`--depth 1`), **Join the TestFlight**, and `© 2026 OpenAgents, Inc.` footer.

> **Note:** The previous Coax Native Web site is preserved completely intact in [`apps/openagents-web/`](../openagents-web/).

## Local Dev Server

```bash
apps/openagents-site/dev.sh
# Serves http://127.0.0.1:8800/
```

## Deploy

```bash
# Staging (https://staging.openagents.com/)
apps/openagents-site/deploy-staging.sh [/path/to/OpenAgents.dmg]

# Production (https://openagents.com/)
apps/openagents-site/deploy-prod.sh [/path/to/OpenAgents.dmg]
```
