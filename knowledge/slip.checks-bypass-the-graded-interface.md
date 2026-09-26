---
id: slip.checks-bypass-the-graded-interface
version: 1
kind: slip
title: Checks that call your internals miss faults in the interface the grader uses
summary: >-
  Tests that import your function and call it directly skip the command line,
  file paths, service endpoint, output file names, and formats the grader will
  actually use. Drive at least one check per deliverable through the exact
  entry point, location, and format the task specifies, from a fresh process.
tags: [acceptance-tests, black-box-testing, interfaces, verification, cli, http]
applies_when: >-
  The task names a command, script, file path, URL, port, output file, or
  schema that will be used to judge the work, and your checks so far exercise
  functions or classes directly.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Glenford Myers, Corey Sandler, Tom Badgett, The Art of Software Testing, 3rd ed. (Wiley, 2011), chapter 6 (higher-order testing: function, system, and acceptance testing)"
    - "ISO/IEC/IEEE 29119-1:2022, Software testing, concepts: test levels and black-box techniques"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

A grader sees only the outside of the work: it runs the named command with
its arguments, reads the named output path, calls the named endpoint, or
imports the named module and symbol. Unit checks against internal functions
pass while any of these is wrong:

- the script is not executable, has the wrong shebang, or depends on the
  current directory; the CLI parses arguments differently than specified;
- output goes to a different path, file name, extension, or encoding, or is
  printed instead of written (or both, with extra log lines mixed into
  stdout);
- the service listens on another port or host (`127.0.0.1` instead of
  `0.0.0.0` inside a container), or is not started by the documented start
  path;
- the schema differs in a field name, type, key order where order matters,
  number format, date format, or trailing newline;
- the module or function name, signature, or return type differs from the
  one the task names.

## How to check

For each deliverable, write one black-box check that uses only what the task
text gives the grader: run the exact command from a new shell in the stated
working directory, read the stated output path, validate it against the
stated format (parse it with a strict parser; compare field names
literally), and call the endpoint over the network from outside the process.
Keep these checks separate from unit tests so a pass means the interface
itself works.
