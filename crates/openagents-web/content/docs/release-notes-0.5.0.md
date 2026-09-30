# Coder 0.5.0 release notes

0.5.0 is one version covering all surfaces: the service, the web bundle,
the desktop app, the phone app, and the terminal. These notes describe what
changed since 0.4.1.

[`CHANGELOG.md`](changelog.md) lists every change with its issue or commit.
The terminal's own notes record what each channel points at.

## Install or update

One command installs or updates the terminal:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh
```

The bare command follows `stable`. `CODER_TERMINAL_CHANNEL=rc` follows the
newest release candidate. The installer verifies the download against the
release's checksum file and installs the command `coder` into
`~/.openagents/bin`.

## Highlights

**Thought drawer folds model reasoning.** When interacting with reasoning
models, deliberative tokens and reasoning steps now fold neatly into an
expandable thought drawer with live line counters and preview snippets,
keeping your main transcript focused and readable (#483).

**In-tree trigram code search.** The terminal now provides high-performance
`code_search` and symbol resolution (`where_is`) backed by a per-checkout
trigram index, giving models instant precise lookups without polluting the
transcript with multi-step grep commands (#397).

**Autopilot multi-agent orchestration.** Autopilot takes feature delivery
to the next level by autonomously breaking goals into scoped tasks, deploying
isolated child workers in dedicated worktrees, managing parent-child fact
inheritance, and coordinating verification gates (#522, #558, #561).

**Local inference daemon with GPU acceleration.** CoderOS hosts and local
machines can run the in-tree `coder-inference-daemon`, serving open weights
such as GPT-OSS with MXFP4 loading, Tile IR GPU decode kernels, and sliding-window
KV cache rewinds (#478, #479, #480, #518).

**Device-to-device terminal follow.** Follow peer terminal sessions over
Tailnet connections directly without relying on central relay servers (#429).

## QA evidence

The [QA release ladder for the running service](https://openagents.com/qa/tree/current)
shows its source commit, qualifying receipts, and missing release evidence.
The deploy log links the exact commit it rolled. A lower rung does not establish
the evidence required by a higher rung.
