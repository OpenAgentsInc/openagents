---
id: environment.cgroup-memory-build-limits
version: 1
kind: environment
title: Detect cgroup OOM kills during parallel builds
summary: >-
  Host-level free memory can be misleading in containers; inspect cgroup
  limits and use single-job compilation when builds are killed despite
  apparently abundant RAM.
tags: [linux, cgroup, build, memory]
applies_when: >-
  A package or compiler build exits by SIGKILL or 137 while host memory
  reports ample capacity.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - compile-compcert
  cites:
    - Linux kernel documentation, *Control Group v2*, “Memory Interface Files”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Containers may have a strict memory cgroup limit far below the host's available memory. Check the effective cgroup memory maximum/current/peak and the process exit status before diagnosing a compiler or package defect. A peak at the configured limit together with exit 137 strongly indicates an OOM kill. Reduce build parallelism (including package-manager-specific job settings); parallel Coq compilation can exceed small limits even when the machine-level `free` output looks generous. Retry only after changing concurrency, and inspect completion logs for the real final status.

Cite: Linux kernel documentation, *Control Group v2*, “Memory Interface Files” (`memory.max`, `memory.current`, `memory.peak`, and `memory.events`).

## How to check

```sh
cat /sys/fs/cgroup/memory.max /sys/fs/cgroup/memory.current /sys/fs/cgroup/memory.peak 2>/dev/null
# If the limit is tight, rerun the package build with one job, e.g.:
OPAMJOBS=1 opam install -j1 <package>
```

Confirm the retried command's exit code and successful installed-package status; do not infer success merely because some dependencies were installed before the kill.
