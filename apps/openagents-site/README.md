# OpenAgents Website (`openagents.com`)

Zero-based minimalist homepage for **OpenAgents** — pure black background (`#000000`), white OpenAgents logo, app showcase, **Download for Mac**, **Build from source for Windows and Linux**, and **Join the TestFlight**.

> **Note:** The previous Coax Native Web site is preserved completely intact in [`apps/openagents-web/`](../openagents-web/).

## Local Dev Server

```bash
apps/openagents-site/dev.sh
# Serves http://127.0.0.1:8800/
```

## Deploy to Staging (`https://staging.openagents.com`)

```bash
apps/openagents-site/deploy-staging.sh
```

Optionally pass a packaged `OpenAgents.dmg` to publish `/downloads/OpenAgents.dmg` alongside the site:

```bash
apps/openagents-site/deploy-staging.sh /path/to/OpenAgents.dmg
```
