# MCP servers

Upstream: https://docs.x.ai/build/features/mcp-servers

MCP ([Model Context Protocol](https://modelcontextprotocol.io)) servers
expose external tools to Grok. Once configured, tools are available
alongside built-ins, namespaced as `<server>__<tool>`.

## Adding a server

```bash
# Local stdio server; everything after -- is the server command
grok mcp add filesystem -- npx -y @modelcontextprotocol/server-filesystem /path/to/dir

# Remote server over HTTP (OAuth handled automatically)
grok mcp add --transport http linear https://mcp.linear.app/mcp

# Remote server with a static auth header (--header is repeatable)
grok mcp add --transport http api https://mcp.example.com/mcp \
  --header "Authorization: Bearer ${API_TOKEN}"
```

| Command | What it does |
| --- | --- |
| `grok mcp list` | Show configured servers (`--json` for machine-readable) |
| `grok mcp remove <name>` | Delete a server |
| `grok mcp doctor [name]` | Diagnose config/connectivity (`--json` ok) |

## config.toml

User-level (`~/.grok/config.toml`):

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
env = { API_KEY = "${MY_API_KEY}" }   # ${VAR} expands at load time
startup_timeout_sec = 30              # default 30
tool_timeout_sec = 6000               # default 6000

[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"
headers = { "x-mcp-session-id" = "{{session_id}}" }
```

Grok expands `${VAR}` and `${VAR:-default}` in `url`, `command`, `args`,
`env`, and `headers`. OAuth servers trigger a browser flow on first use;
tokens land in `~/.grok/mcp_credentials.json`.

## Project scope

```bash
grok mcp add --scope project ...
```

Writes `.grok/config.toml` in the current directory. On load, Grok walks
from the current directory up to the git root reading each
`.grok/config.toml`. A project server with the same name as a user one
**replaces it entirely**.

## In the TUI

`/mcps` opens the MCP tab of the extensions modal:

| Key | Action |
| --- | --- |
| `Space` | Toggle server |
| `r` | Refresh after config edits |
| `i` | Authenticate OAuth servers |
| `a` / `x` | Add / remove |

## Compatibility

Grok also loads MCP configs from:

- `~/.claude.json`
- `.cursor/mcp.json`
- project `.mcp.json`

Merged **below** `config.toml` in priority. Disable a vendor:

```toml
[compat.claude]
mcps = false

[compat.cursor]
mcps = false
```

`grok inspect` shows every loaded server and its origin.

## Troubleshooting

1. Run `grok mcp doctor`.
2. Stdio stderr: `~/.grok/logs/mcp/<server>.stderr.log`.
3. Cold-start `npx` downloads may need a higher `startup_timeout_sec`.
