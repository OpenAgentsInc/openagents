# Portable host bundle infrastructure

`scripts/coder-host.py` stages verified Coder binaries, checks their task
interface, records an active bundle, retains rollback, and generates one-shot
service definitions for explicit task grants. It is Python installation
infrastructure around the public Rust task owner, not a second agent runtime.
It was implemented here without copying private CoderOS code.

[Issue #9690](https://github.com/OpenAgentsInc/openagents/issues/9690) delivers
this bounded M11 packaging slice. The
[retained platform evidence](../verification/2026-09-26-portable-host/README.md)
includes real macOS launchd tasks and a separately labeled Linux systemd
service fixture. It does not complete the M11 clean-host matrix or the M12
bootable CoderOS release in the [migration tracker](../migration-status.md).

## Install and inspect

Build Coder with the repository's pinned toolchain, then supply the exact
binary digest and source declaration. The helper does not download, build, or
trust a remote binary on your behalf.

```sh
python3 scripts/coder-host.py \
  --root "$HOME/.openagents/host-bundle" \
  --tasks "$HOME/.openagents/tasks" \
  install --binary /absolute/path/to/coder \
  --sha256 <64-character-sha256> \
  --source-revision <40-character-git-commit>
```

Use `--uncommitted-source` when the binary includes uncommitted changes.
`--source-revision` is an operator declaration, not a build attestation. The
manifest separately retains the binary's actual `--version` response. A
SHA-256 pin establishes byte identity; it does not establish the publisher's
trustworthiness or prove reproducible compilation.

The root must be a private ordinary directory. The helper holds a stable OS
lock, stages a private version directory, copies and syncs the binary, checks
its digest again, and runs bounded `--version` and `task --help` probes with a
cleared environment. Only then does it atomically select the bundle. An
insufficient disk reserve, failed health check, or changed digest leaves the
active bundle untouched. Interrupted staging is retained as an unselected
directory; a reader never promotes it implicitly.

Each version is named by its binary digest and has immutable declared
provenance. Installing the same bytes with different provenance refuses.
Reinstalling an identical version is inert. The readable task schemas default
to `openagents.coder.task-store.v2`; repeat `--read-state-schema` only for
schemas that the supplied binary actually supports. Existing task state must
match this declared compatibility before activation or rollback.

```sh
python3 scripts/coder-host.py --root "$HOME/.openagents/host-bundle" \
  --tasks "$HOME/.openagents/tasks" doctor
python3 scripts/coder-host.py --root "$HOME/.openagents/host-bundle" \
  --tasks "$HOME/.openagents/tasks" rollback
```

Doctor verifies the retained binary digest, task-state compatibility, version,
and durable task interface. This is host availability evidence. It performs no
model request, task execution, device access, or wallet operation. An install
root missing its journal or stable lock refuses instead of recreating history.

## Explicit one-shot services

Submit a task and prepare its separate operator grant using the
[task-owner guide](task-owner.md). Generate a service only after reviewing that
grant:

```sh
python3 scripts/coder-host.py --root "$HOME/.openagents/host-bundle" \
  --tasks "$HOME/.openagents/tasks" service --platform macos \
  --grant /absolute/path/to/GRANT.json --label org.openagents.my-task \
  > /absolute/path/to/my-task.plist
```

Use `--platform linux` for a systemd user service. Generation retains a private,
digest-named canonical JSON copy of the grant and pins the selected binary's
absolute version path. The copy preserves parsed fields, not original whitespace
or the input file's byte digest. It does not install or start a service. A service invokes only
`coder task execute --grant ... --store ...`; it cannot invent a task or infer
execution permission from the inbox. The Rust owner validates the complete
grant and source before any effect.

The launchd definition sets `KeepAlive=false`; the systemd definition sets
`Restart=no`, `Type=oneshot`, and `KillMode=control-group`. Explicitly replaying
the service after a completed or uncertain attempt does not authorize another
attempt: the durable owner rejects the same task/grant. Service restart is not
task recovery.

A rendered service remains pinned to its original binary. Installing or
rolling back the selected bundle does not replace a running owner or silently
change that service's executable. Stop and unload the old service, inspect its
retained result, then generate a new definition under the intended bundle.
Do not create an automatic restart loop for unresolved effects.

After unloading services, remove the bundle's activation selection with:

```sh
python3 scripts/coder-host.py --root "$HOME/.openagents/host-bundle" \
  --tasks "$HOME/.openagents/tasks" uninstall
```

Uninstall preserves task data, traces, retained binaries, grants, and installation
history. It does not reach into launchd or systemd to stop services you installed
separately. The result explicitly reports that boundary. This is a reversible
selection removal, not an erasure command.

## Acceptance and platform limits

Run infrastructure checks without a workspace-wide release gate:

```sh
python3 scripts/test-coder-host.py
python3 -m py_compile scripts/coder-host.py scripts/test-coder-host.py \
  scripts/test-coder-host-runtime.py scripts/test-coder-host-systemd.py
```

The nine tests cover upgrade, duplicate install, rollback, state-preserving
uninstall, incompatible schema, bad digest, failed health checks, disk-pressure
refusal, interrupted staging, changed retained binaries/provenance, symlinks,
missing state, stable locks, nonblocking FIFO refusal, cleanup of health-check
descendants after their parent exits, and service restart policy.

The opt-in macOS acceptance creates only temporary files and uniquely named
launchd services, then boots the services out in a cleanup block:

```sh
python3 scripts/test-coder-host-runtime.py \
  --binary /absolute/path/to/coder \
  --source-revision <40-character-git-commit> \
  --output /tmp/coder-host-acceptance-unique
```

It installs a real public Coder build and a stripped, newly ad-hoc-signed
variant as distinct binary bundles. This tests byte-version activation, not a
claim of two production releases. Real task-owner commands append one line in
a synthetic linked Git worktree. Both the upgraded and rolled-back bundles
complete through launchd; an explicit service restart preserves the original
run and does not append another line. Uninstall leaves exact task-store bytes
unchanged.

On coderos, the installed Coder lacks `task execute`, so the bundle health check
refuses it. The machine also has no `/usr/bin/git`, which the current task
owner expects. The retained Linux acceptance runs a clearly labeled shell
service fixture through systemd, verifies the unit, starts it, confirms one
invocation across duplicate starts while active, and stops/unlinks it. It
proves service-manager wiring only, not a Rust task-owner result on Linux.
No benchmark jobs or existing services were modified.

A complete portable-host release still needs an accepted Linux owner binary,
admitted system-tool resolution on NixOS, clean Linux/macOS installation tests,
resource/capability admission beyond the selected task's grant, update and
state-migration policies across actual releases, and service recovery on host
reboot. A complete CoderOS profile additionally needs pinned OS packages,
installation and boot proof, hardware absence behavior, and staged system
update/rollback acceptance. Those remain explicit M11/M12 work.
