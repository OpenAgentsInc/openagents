# Durable scratch

Agents on this machine keep scratch files, such as captures, scripts, and
notes, in a per-session directory under `~/.openagents/scratch/` instead of
`/tmp` or `/private/tmp`, which a reboot clears. Evidence that a check needs
still belongs in the repository or the task store, not in scratch.

The design is in
[Many agents on one machine](../design/many-agents-one-machine.md), and the
code is `coder_lease::scratch`
([`crates/coder-lease/src/scratch.rs`](../../../crates/coder-lease/src/scratch.rs)).

## Get your scratch directory

```sh
openagents scratch [--session SESSION] [--json]
```

The command creates the session's directory, private to you (mode `0700`),
and prints its path:

```sh
dir=$(openagents scratch)
cp capture.png "$dir/"
```

Under `--json`, it prints `path`, `session`, `root`, and `from`, which says
where the session came from.

## Which session

The command chooses the directory in this order:

1. `--session SESSION`.
1. The directory that `OPENAGENTS_SCRATCH` names. A lease and a Coder
   delegation set it.
1. The session that the lease broker detects (see
   [Leases](../runtime/leases.md#holders-and-sessions)): `OPENAGENTS_SESSION`,
   else the agent's own session variable, such as `claude-code:ID`, else the
   nearest agent process above this one, as `NAME:PID`.

The session becomes a directory name. Letters, digits, `.`, `_`, and `-`
stay, `:` becomes `-`, any other character becomes `_`, a leading `.` is
dropped, and the name is at most 96 characters. For example,
`claude-code:abc` becomes `claude-code-abc`. Each directory holds a
`.session` file that names its session.

## Where it lives

The root is `$OPENAGENTS_SCRATCH_ROOT`, else `scratch` beside the lease root,
which is `~/.openagents/scratch` unless `OPENAGENTS_LEASE_ROOT` moves the
lease root. Tests set a temporary `HOME` or root.

## Who gets it

- **Leases.** A command run under `openagents lease` gets
  `OPENAGENTS_SCRATCH`, the holder session's directory.
- **Coder's delegates.** Programs that run Coder turns (`coder`,
  `microcoder`, the desktop app, and `openagents` for `coder`, `host`,
  `task`, `chat`, `terminal`, and `studio`) give every agent they delegate
  to `OPENAGENTS_SCRATCH`: Claude Code, Codex, and OpenCode delegations, the
  ACP agents, and Microcoder's commands. The delegates share the directory of
  `$OPENAGENTS_SESSION`, else `coder:<pid>` for the Coder process, and the
  delegation brief tells them to keep scratch there.

## Cleanup

The disk cleanup rule (class 10 in
[Background processes](../../background/2026-10-02-background-processes.md))
removes a session's scratch when the session has ended and nothing in the
directory changed for seven days (`classes.scratch_days`). A session has
ended when no lease names it and, for a session whose identity names a
process, such as `codex:4242`, that process is gone. A directory a process
has open, or a lease table that can't be read, keeps the scratch.
