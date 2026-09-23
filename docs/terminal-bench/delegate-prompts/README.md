# What each executor sends, by system prompt variant

This directory holds the first request each delegate CLI sends for each
system prompt variant of `exec.system`, measured through a local capture
server. No inference ran, and no real credential was used.
[`measurements.json`](measurements.json) holds every capture's parts, sizes,
cache markers, and checks. [`codex-request.json`](codex-request.json) is
Codex's default first request, the counterpart of
[Claude Code's](../claude-code-delegate-prompt/README.md).

The section library, the headless core, and the Jev selection are
described in [Run a Coder One component alone](../../coder/guides/coder-one-components.md#tune-the-executors-system-prompt).

## Measurements

Each executor ran with its reference manifest's settings: Claude Code
2.1.280 on Opus 5.5 at low effort with six tools and the five-minute cache
(`jevprobe2-opus-lean-low-5m`), and Codex 0.155.1 on GPT-6 Luna
(`jevprobe3-luna`). Every capture sent the same one-line placeholder
briefing, so the sizes compare. Token counts are estimates: characters
divided by four.

| Executor | Variant | Main prompt | First request | About tokens | Security policy |
| --- | --- | ---: | ---: | ---: | --- |
| Claude Code | Default | 5,453 | 16,832 | 4,208 | Present |
| Claude Code | Core | 1,542 | 12,921 | 3,231 | Present |
| Claude Code | Core and all six optional sections | 3,058 | 14,437 | 3,610 | Present |
| Codex | Default | 18,037 | 46,292 | 11,573 | Absent |
| Codex | Core | 1,542 | 29,796 | 7,449 | Present |
| Codex | Core and all six optional sections | 3,058 | 31,312 | 7,828 | Present |

The first-request column counts every text part and every tool definition.
The core cuts Claude Code's first request by 23% and Codex's by 36%. Per
task, Jev selected zero to two optional sections on the eight development
tasks, so a selected variant sits between the two core rows.

Cache markers don't change with the variant. Claude Code marks the identity
line, the main prompt, and the trailing environment block with
`cache_control: ephemeral`, without a `ttl` under the five-minute setting.
Codex sends no marker; it sends a `prompt_cache_key` per session, and the
service caches the prefix on its own.

These are request sizes, not results. Whether the core changes turns, cost,
or pass rate is untested: no mini-task with a real executor and no
Terminal-Bench screen has run with a variant.

## What replaces what

**Codex 0.155.1.** With GPT-6 Luna's catalog metadata, Codex sends its base
instructions, 18,037 characters, as the first developer message, and leaves
`instructions` empty. The captures confirm both settings:

- `model_instructions_file` replaces the base instructions entirely.
- `developer_instructions` adds a developer text block ahead of the skills
  instructions and keeps the base instructions.

Neither setting removes what Codex adds on its own: the skills instructions
(2,475 characters), sandbox permissions (362), collaboration mode (920),
the multi-agent role (2,429) and mode (271), the environment context, and
21,384 characters of tool definitions. Codex's default carries no security
policy, and its rules include "Do not add or run tests unless the user asks
you to test or verify implementation", which works against a run graded by
a checker. The headless core's `verify` section replaces it.

**Claude Code 2.1.280.** `--system-prompt-file` replaces the 5,453-character
main prompt, its security policy included, which is why the core carries
that policy verbatim. The identity line, the environment block, and the
commit-attribution reminder stay. `--append-system-prompt-file` keeps the
main prompt, adds the text after it, and changes the identity line to "You
are Claude Code, Anthropic's official CLI for Claude, running within the
Claude Agent SDK."

## How it was captured

```sh
coder-one prompt capture --out /tmp/prompt-captures
```

The command writes `measurements.json` and one sanitized request per
capture; this directory keeps the measurements and Codex's default request.
`coder-one prompt capture` starts a loopback server that records each
request body, never its headers, and answers every call with an error. It
runs each CLI with the exact command a dispatch uses, with a cleared
environment, a scratch home, and a dummy credential:

- Claude Code reads `ANTHROPIC_BASE_URL` and a dummy
  `CLAUDE_CODE_OAUTH_TOKEN`.
- Codex gets a `capture` model provider whose base URL is the server and
  whose key is a dummy variable. Codex applies a subcommand's `-c` settings
  instead of the root's, so the provider settings follow `exec`. A custom
  provider can't fetch Codex's model catalog, so the capture passes the
  `models` list from `~/.codex/models_cache.json` as `model_catalog_json`;
  nothing else under `~/.codex` is read. Without it, Codex falls back to
  generic metadata and sends a different, 16,979-character prompt in
  `instructions`.

A capture whose CLI stream names the real service is refused. The saved
requests have host paths rewritten to `/app` and `/root`, and per-session
identifiers removed.
