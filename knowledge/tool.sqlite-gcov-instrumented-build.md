---
id: tool.sqlite-gcov-instrumented-build
version: 1
kind: tool
title: Build SQLite with gcov instrumentation and verify runtime coverage
summary: >-
  For a SQLite source tree, use its configure-provided gcov mode, retain the
  generated notes beside the executable, exercise the binary, then run gcov to
  confirm runtime counters were written and read.
tags: [sqlite, gcov, build, coverage]
applies_when: >-
  Building SQLite from a vendored or local source archive where coverage data
  must be produced by executing the resulting CLI.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - sqlite-with-gcov
  cites:
    - SQLite, How To Compile SQLite, Compiling SQLite
    - "GCC, Using the GNU Compiler Collection, Program Instrumentation Options: Instrumentation Options"
    - GCC, gcov—A Test Coverage Program, Introduction and Options
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

SQLite's own configure/build scripts provide a `--gcov` mode. A typical debug-oriented build is:

```sh
./configure --gcov --disable-tcl CFLAGS='-O0 -g'
make -j2
```

The configuration adds compiler coverage instrumentation and links the coverage runtime. Keep the `.gcno` files emitted at compilation: they map instrumented code to source. Run the built executable after compilation to produce matching `.gcda` runtime counters. Then, from the build directory, invoke `gcov -b -c` on the relevant `.gcno` files; branch and call summaries help establish that the report is based on executed code, not merely successful compilation. For a CLI built by SQLite's makefiles, instrumented shell and amalgamation objects can have separate note/data files.

If the CLI should be callable by name, expose the built executable via a PATH directory (for example, a symlink); this is independent of whether the source build itself succeeded. Use a small in-memory SQL workload and an integrity check as a smoke test, avoiding persistent test data.

Sources: SQLite, *How To Compile SQLite*, section “Compiling SQLite”; GCC, *Program Instrumentation Options*, section “Instrumentation Options” (`-fprofile-arcs`, `-ftest-coverage`); GCC, *Invoking Gcov*, sections “Introduction” and “Options.”
