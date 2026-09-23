# Installing Coder

`scripts/install-coder.sh` makes `coder` on your `PATH` run this
repository's Coder Terminal (`crates/coder`), and `--rollback` switches back
to the build it replaced.

Status: implemented in `scripts/install-coder.sh`, with `coder --version`
and `coder doctor` in `crates/coder` (`identity.rs`, `build.rs`, and
`checkup.rs`). Issue
[#9579](https://github.com/OpenAgentsInc/openagents/issues/9579) holds the
plan.

## Why this exists

Two different programs have answered to `coder` on the same machine: this
repository's Coder Terminal and a build from a separate repository. When
`~/.openagents/bin/coder` points at the other one, none of this
repository's work, the [delegate door](../runtime/delegate-door.md)
included, is what runs when you type `coder`. The install script, the
version line, and the doctor make it obvious which one runs.

## Install

From a checkout of this repository:

```sh
./scripts/install-coder.sh
```

The script does four things:

1. Builds `crates/coder` in release mode with the toolchain
   `rust-toolchain.toml` pins, in its own target directory,
   `~/.cache/openagents/target-install-coder`.
2. Copies the binary to
   `~/.openagents/versions/coder-openagents-<short-sha>`, with `-dirty`
   appended when the tree has uncommitted changes to tracked files.
3. Points `~/.openagents/bin/coder` at that copy by renaming a new
   symbolic link over the old one, so `coder` is always one build or the
   other and never missing.
4. Prints the build it replaced, records it in
   `~/.openagents/versions/coder.previous`, and runs `coder --version`.

A failed build leaves the link unchanged. The script expects
`~/.openagents/bin` on your `PATH`, or `~/.local/bin/coder` linked to
`~/.openagents/bin/coder`, and says so when neither resolves.

| Variable | Effect |
| --- | --- |
| `OPENAGENTS_HOME` | Where `bin/` and `versions/` live. Default `~/.openagents`. |
| `CODER_INSTALL_TARGET_DIR` | Cargo's target directory for the release build. |
| `CODER_INSTALL_CARGO` | The Cargo command. `scripts/test-install-coder.sh` points it at a stub. |

## Roll back

```sh
./scripts/install-coder.sh --rollback
```

Rollback points the link back at the recorded build and records the one it
replaced, so a second rollback undoes the first. The install output also
prints the one `ln -sfn` command that does the same thing by hand.

## Which Coder is running

```console
$ coder --version
coder 0.1.0 (OpenAgentsInc/openagents a55f667b04 clean)
```

The line names the repository, the commit, and whether the tree the build
came from was `clean` or `dirty`. The install script stamps both from the
checkout it builds. A plain `cargo build` asks Git itself and can miss an
unstaged edit outside `crates/coder`, so the install script is the exact
path. The trace's session header records the same build as its version,
such as `0.1.0+a55f667b04`.

## Which door a turn uses

`coder doctor` reports which door a turn would use and why, without running
one:

```console
$ coder doctor
coder 0.1.0 (OpenAgentsInc/openagents a55f667b04 clean)
binary     ~/.openagents/versions/coder-openagents-a55f667b04
door       delegate (claude-code) because claude-code is installed at ~/.local/bin/claude and authenticated (cli_login)
settings   CODER_DELEGATE=auto · CODER_DELEGATE_AGENT=unset · CODER_DELEGATE_MODEL=unset
targets
  claude-code  ~/.local/bin/claude, credential found (cli_login)  (chosen)
  codex        ~/.local/bin/codex, credential found (codex_auth_json)
executor   claude-code on claude-opus-5-5 · effort low · tools Bash,Read,Edit,Write,Glob,Grep · prompt cache 5m · deadline 600s · policy coder-one-jevprobe2-opus-lean-low-5m
jev        jev-1.13.0 from ~/.openagents/jev.json
boundary   available (/run/current-system/sw/bin/bwrap): read-only turns cannot write the workspace
fallback   stub: no Open Responses key is set (CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY), so a fallback turn answers with a canned line
trace      ~/.openagents/traces
```

A credential is reported as found, not verified: the doctor spawns
nothing and costs nothing, and the first turn is the call that proves the
credential works. The doctor exits `1` when the environment would refuse
to start a turn, such as `CODER_DELEGATE=always` with no target.

## Environment

| Variable | Effect |
| --- | --- |
| `CODER_DELEGATE` | `auto` (default), `always`, or `off`. See [the delegate door](../runtime/delegate-door.md). |
| `CODER_DELEGATE_AGENT` | `claude-code` or `codex`. |
| `CODER_DELEGATE_MODEL` | The delegation target's model. |
| `TYPESAFE_API_KEY` or `~/.openagents/jev.json` | The Jev key the delegate door's probes and survey ask through. Without one, the briefing carries the request alone. |
| `CLAUDE_CODE_OAUTH_TOKEN` | A Claude Code subscription token. The CLI's stored login also counts. |
| `CODER_DOOR_KEY`, `CODER_DOOR_URL`, `CODER_MODEL` | The Open Responses fallback. `CODER_DOOR_URL=https://openagents.com` with an OpenAgents bearer as the key serves `google/gemini-3.8-flash`. |
| `CODER_SHELL=off` | Turns run no commands, and a delegated turn runs read-only. |

## Related

- [The delegate door](../runtime/delegate-door.md)
- [Headless mode](headless.md)
- [Traces](../runtime/traces.md)
