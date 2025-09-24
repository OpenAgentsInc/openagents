**Figma Tools Setup**

Use these steps to enable the built‑in Figma tools with zero CLI flags.

**Recommended Location**
- Put your Personal Access Token here: `.secrets/FIGMA_API_TOKEN` (repo root)
  - File contents: the token string only (no label)
  - Permissions: `chmod 600 .secrets/FIGMA_API_TOKEN`
  - This path is gitignored by default.

**How Codex discovers the token**
- First, it checks `FIGMA_API_TOKEN` from the environment.
- Then tries project‑local files (in order):
  - `.secrets/FIGMA_API_TOKEN`
  - `.secrets/figma_api_token`
  - `.figma_token`
  - `.env.local` (line: `FIGMA_API_TOKEN=...`)
- Finally, it tries `~/.codex/figma_api_token`.

If any of the above is present, tools auto‑enable — you do not need to set env vars.

**Quick Start**
- Create the secrets file:
  - `mkdir -p .secrets`
  - `printf "%s" "your-figma-pat" > .secrets/FIGMA_API_TOKEN`
  - `chmod 600 .secrets/FIGMA_API_TOKEN`
- Run the app normally; the Figma tools (`figma_find_nodes`, `figma_get_nodes`, `figma_export_images`, `figma_extract_tokens`) will be advertised to the model.

**Optional (alternate locations)**
- `.env.local` with `FIGMA_API_TOKEN=your-figma-pat`
- `~/.codex/figma_api_token` for user‑wide setup
- You can still force enable with `CODEX_ENABLE_FIGMA_TOOLS=1`, but it’s not required.

**Notes**
- No token is written to logs; requests go to `https://api.figma.com` with a 10–15s timeout.
- Exported images are returned as Figma CDN URLs (no local writes in this first version).
