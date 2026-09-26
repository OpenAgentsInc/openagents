---
id: environment.container-without-git-or-sudo
version: 1
kind: environment
title: Working in a minimal container without git or sudo
summary: >-
  Minimal container images often lack git, sudo, editors, ps, and network
  access, and run as root. Check what exists with command -v before relying
  on it, back up files with cp before editing, and use what's installed.
tags: [environment, container, docker, git, sudo, minimal-image, offline, root, tools]
applies_when: >-
  Commands run inside a container or sandbox, a command fails with "command
  not found", or the task forbids internet access.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Docker documentation, Best practices for writing Dockerfiles"
    - "POSIX.1-2017, the command utility (command -v)"
evidence: []
---

## Details

- **Check first.** `for c in git sudo python3 pip curl ps; do command -v $c
  || echo "no $c"; done` shows what's available in one step.
- **No sudo.** You're usually root already (`id -u` prints 0); drop `sudo`.
- **No git.** You can't `git diff` or `git stash`. Before editing a file,
  copy it (`cp f f.orig`) and compare later with `diff -u f.orig f`. Don't
  leave backup copies where a test or importer collects them (for example a
  `.py` copy inside a package); put them under `/tmp`.
- **No editor.** Write whole files with a quoted heredoc, or edit in place
  with a short Python script that reads, replaces, and writes.
- **No network.** `pip install` and `apt-get` fail when internet is off. Use
  what's installed: `python3 -m pip list`, `python3 -c "import numpy"`.
- **No ps or top.** Read `/proc/*/cmdline`, or use `pgrep` when it exists.
- **Python.** `python` may not exist while `python3` does; `pip` may be
  missing while `python3 -m pip` works.
- **Paths.** Use absolute paths; every command may start in the working
  directory rather than where the last one left off.

## How to check

Run the tool inventory once at the start, and note it in your rationale so
later steps don't retry a missing tool.
