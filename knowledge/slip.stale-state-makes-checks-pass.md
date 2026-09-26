---
id: slip.stale-state-makes-checks-pass
version: 1
kind: slip
title: Leftover state can make your checks pass when a clean run fails
summary: >-
  Checks run in the same workspace you built in can pass on leftovers: a
  running process, a cached build, an old output file, a populated database
  volume, an installed package. A grader starts clean. Before finishing, reset
  to a clean state and rerun the deliverable exactly as the grader would.
tags: [acceptance-tests, reproducibility, clean-state, caching, verification]
applies_when: >-
  You have been iterating in one environment for a while and are about to
  declare the work finished based on checks that ran there.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Reproducible Builds project, Definition and documentation: a build is reproducible when independent clean builds give identical artifacts"
    - "Adam Wiggins, The Twelve-Factor App (2011), factors V (build, release, run) and VI (stateless processes)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Iterating leaves state behind that the final deliverable may silently depend
on:

- an output file written by an earlier version of the code, which a later
  broken version no longer produces, while the check only reads the file;
- a server or background worker you started by hand that the grader will not
  start, or an old server still bound to the port serving old code;
- build caches, compiled objects, `__pycache__`, `node_modules`, or a
  previously built container image that masks a broken build step;
- a database, volume, or migration table already in the target state, so
  migrations or seeds never actually run;
- packages installed interactively that are not declared in the manifest or
  install script;
- environment variables exported in your shell.

A grader typically runs in a fresh process, often a fresh container, with only
the files and startup path the task defines. Anything your checks relied on
that is not produced by that path is missing there.

## How to check

Before finishing: stop the processes you started, delete generated outputs
(or write them to a fresh directory), clear build caches where cheap, reset
stateful services, and then run the deliverable through its documented entry
point (the command, script, service start, or build the task names) in a new
shell. Rerun every acceptance check against *those* outputs. If the task
must be idempotent, run the entry point twice and check both runs.
