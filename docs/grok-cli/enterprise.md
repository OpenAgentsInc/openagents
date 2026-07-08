# Enterprise / CI controls

Upstream: https://docs.x.ai/build/enterprise

This is the subset OpenAgents agents care about for **automation, sandboxes,
permissions, and network allowlists**. Full enterprise deployment detail
remains upstream.

## Network requirements

All connections use HTTPS (port 443), TLS 1.2 or 1.3 (`rustls`).

### Required

| Host | Purpose |
| --- | --- |
| `cli-chat-proxy.grok.com` | Inference proxy, settings |
| `auth.x.ai` | OAuth2/OIDC authentication |

Enterprise OIDC also needs your IdP domain (e.g. `login.microsoftonline.com`).

### Additional (optional)

| Host | Purpose | If blocked |
| --- | --- | --- |
| `api.x.ai` | Direct API-key path | Only needed for `api_key` auth vs proxy |
| `code.grok.com` | Remote session sync, sharing, WebSocket relay | Sessions stay local; share links unavailable |
| `assets.grok.com` | UI assets | Avatars only |
| `x.ai` | CLI binary downloads (`curl \| bash`, in-app update) | Use `npm install -g @xai-official/grok` |
| `storage.googleapis.com` | Fallback CDN for CLI binaries | Only if `x.ai` unreachable during install |

Proxy env vars: `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY`. Set proxy idle
timeouts to **≥ 10 minutes** for long SSE responses.

## Configuration layers (low → high)

| Priority | Source |
| --- | --- |
| 1 (lowest) | `/etc/grok/managed_config.toml` |
| 2 | `~/.grok/managed_config.toml` |
| 3 | `~/.grok/config.toml` |
| 4 | `~/.grok/requirements.toml` |
| 5 (highest) | `/etc/grok/requirements.toml` |

`requirements.toml` cannot be overridden by lower layers — use it for
compliance pins. Supports `$VAR` expansion and `[[version_overrides]]`.

## Authentication methods

| Method | Trigger | Best for |
| --- | --- | --- |
| Browser OIDC | `grok login` | Interactive terminals |
| Device code | `grok login --device-auth` | SSH, containers, headless hosts |
| External auth provider | `auth_provider_command` in config | Corporate IdPs / token brokers |
| API key | `XAI_API_KEY` or `model.api_key` | Scripts, CI/CD |

Resolution per model: `model.api_key` > `model.env_key` > session token >
`XAI_API_KEY`.

### API key (CI)

```bash
export XAI_API_KEY="xai-..."
grok --no-auto-update -p "Review this diff" \
  --output-format json --always-approve
```

### Device code

```bash
grok login --device-auth
```

## Sandbox profiles

Applied once at process startup; irreversible. Landlock (Linux 5.13+) /
Seatbelt (macOS).

| Profile | Write | Child network | Use case |
| --- | --- | --- | --- |
| `off` | Unrestricted | Allowed | Default / no sandbox |
| `workspace` | CWD, `/tmp`, `~/.grok/` | Allowed | Normal development |
| `devbox` | Everything except `/data` | Allowed | Cloud devboxes |
| `read-only` | `~/.grok/` + tmp only | Blocked | Review / audit |
| `strict` | CWD, `/tmp`, `~/.grok/` (read limited) | Blocked | Untrusted repos |

```bash
grok -p "..." --sandbox workspace
# or env: GROK_SANDBOX=workspace
```

Always write-protected regardless of profile: `~/.ssh`, `~/.gnupg`,
`~/.grok/auth`, `~/.aws`, `~/.config/gcloud`, `~/.azure`.

Child-process network blocking via seccomp is **Linux-only** in `read-only`
and `strict`.

## Permissions

Independent of sandbox: what the model may **request**.

### Modes (headless `--permission-mode`)

| Mode | Behavior | Typical use |
| --- | --- | --- |
| `default` | Normal prompts | Interactive |
| `acceptEdits` | Auto-approve file edits; prompt for shell | Semi-automated |
| `dontAsk` | Silently deny anything without explicit allow | CI / high-security |
| `bypassPermissions` | Always-approve (same family as `--always-approve`) | Trusted automation |
| `plan` | Plan mode | Planning-only |

### Always-safe ops

Some read-only tools and curated safe shell commands auto-approve even in
`dontAsk` (e.g. `read_file`, `list_dir`, `grep`, `web_search`, `ls`, `cat`,
`git status` / `diff` / `log`, etc.). Shell is parsed **per segment** —
`ls && rm -rf /` may auto-approve `ls` but block `rm`.

### Policy rules

```bash
grok -p "Review the API changes" \
  --permission-mode dontAsk \
  --allow 'Bash(git *)' \
  --allow 'Bash(gh *)' \
  --allow 'Read' \
  --allow 'Grep' \
  --deny 'Bash(rm -rf *)'
```

Supported filters include: `Bash`, `Edit`, `Read`, `Grep`, `MCPTool`,
`WebFetch`. Deny beats allow.

Config form:

```toml
[permission]
rules = [
  { action = "allow", tool = "bash", pattern = "git *" },
  { action = "allow", tool = "read" },
  { action = "deny",  tool = "bash", pattern = "*" },
]
```

### Combining for untrusted work

```text
dontAsk + narrow --allow rules + --sandbox strict
```

Permissions limit what can be requested; sandbox limits what the process can
do even if approved.

## Privacy sketch

Tool execution is **local**. Inference goes over TLS to the proxy. Local
session history lives in `~/.grok/`. Team-level Zero Data Retention (ZDR)
is enforced server-side when enabled for the team.

## OpenAgents CI recipe

```bash
export XAI_API_KEY="..."   # or pre-warmed login on the runner
export GROK_SANDBOX=workspace

grok --no-auto-update \
  -p "Produce a short summary of git status and open TODOs" \
  --cwd "$REPO" \
  --output-format json \
  --permission-mode dontAsk \
  --allow 'Read' \
  --allow 'Grep' \
  --allow 'Bash(git *)' \
  --deny 'Bash(git push*)' \
  --deny 'Bash(rm *)' \
  --sandbox workspace \
  --max-turns 20
```
