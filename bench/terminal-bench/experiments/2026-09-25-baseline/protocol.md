# Baseline behavior offline: protocol

Issue [#9633](https://github.com/OpenAgentsInc/openagents/issues/9633).
Written on 2026-09-25, after the measurements ran. It records the rules
they used; nothing here was frozen in advance, because the component has no
parameter a measurement could tune.

## What is measured

`evidence.baseline` finds a task's entry points by code and runs each once
in a scratch copy of the untouched workspace, bounded to 60 seconds and
16 KiB per stream, inside a `coder-boundary` writing boundary with the
network denied. The component has no threshold and asks Jev nothing, so
nothing is fit and no split is needed. The bound and the stream cap are the
issue's.

Two measurements, both offline:

1. **Anatomy table.** On the untouched workspaces of the 18 tasks in
   `docs/terminal-bench/2026-09-24-task-anatomy.md`, rebuilt by the
   departures experiment's `build_workspaces.py`: the entry points found,
   run, and refused, and, for each task with a run, whether its output names
   a decisive fact from the anatomy that the v13 suspects list doesn't. A
   run names a fact when its output points at the code the fact says must
   change, or shows the wrong value the fact describes. The suspects list
   is the v13 `rationale` list as the departures measurement reports it
   (`docs/terminal-bench/2026-09-25-departures-offline.md`).
2. **Duplicated session commands.** On every retained Microluna session log
   under `bench/terminal-bench/traces/`, the `run_command` calls before the
   session's first edit, and how many duplicate a baseline command of the
   same task. The first edit is the first completed `apply_patch` or
   `write_file` outside `/tmp` that isn't the evaluation script. Four rules,
   from strict to loose, stated in `measure.py`:
   - `exact`: the command contains the baseline command, spaces collapsed,
     which is the finish rule's match (#9638);
   - `same_run`: it runs the same entry point on the same set of data
     files, in any order and by any path;
   - `entry`: it runs the same entry point, with any arguments;
   - `purpose`: it runs the entry point or imports its package, and names
     one of the baseline's data files.

   A here-document written to a file doesn't count as running what it
   holds.

## Host

The runs were made on a macOS host, not in the task images. `python3` was a
Python 3.12 virtual environment with NumPy and SciPy, the packages
`embedding-drift-monitor`'s image installs, put first on `PATH`. A run that
fails here for a program or package the image has is reported as not
observable, never as a result.

## In-sample note

The entry-point rules were written knowing the `embedding-drift-monitor`
CLI, which the issue names, so that task is in-sample for the rules. Two
rules were added after the first fixture runs: reading a usage line from a
`CLI:` docstring line and from inside a string literal, and refusing a
compiler call that names no input file (`ks-solver-cpp`'s stated flags).
