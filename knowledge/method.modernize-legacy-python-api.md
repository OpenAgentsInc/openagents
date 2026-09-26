---
id: method.modernize-legacy-python-api
version: 1
kind: method
title: Modernize legacy Python APIs through behavioral slices
summary: >-
  When porting old Python code, preserve required behavior with small modern
  implementations rather than mechanically translating obsolete imports and
  idioms; verify on the current interpreter and treat warnings as failures.
tags: [python, migration, compatibility, testing]
applies_when: >-
  Rewriting Python 2 or older Python 3 scripts that use removed
  standard-library modules, deprecated numerical aliases, legacy pandas APIs,
  or platform-sensitive paths.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - modernize-scientific-stack
  cites:
    - Python Software Foundation, What's New in Python 3.0, sections Text and Print
    - Python documentation, configparser — Configuration file parser, ConfigParser Objects
    - pandas documentation, IO tools (text, CSV, HDF5, …), Date columns
    - NumPy documentation, NumPy 1.20.0 release notes, Deprecations
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Start by identifying the externally required behavior and the smallest data flow that produces it. Read the legacy implementation, representative input, and configuration before editing; distinguish required outputs from incidental features such as plotting or database imports. Replace obsolete APIs with their current equivalents: `ConfigParser`/`SafeConfigParser` with `configparser.ConfigParser`, Python 2 print and iteration syntax with Python 3 syntax, removed NumPy aliases with explicit dtypes or native vectorized operations, and deprecated pandas date parsing with `parse_dates` where suitable. Python 3 `str` is Unicode, so do not decode text that is already decoded.

Use `pathlib.Path` for paths rooted relative to the script, and specify UTF-8 explicitly at text-file boundaries (CSV and configuration). Keep computations vectorized where the library already supports the operation. Add dependency constraints appropriate to the APIs used, then execute the script in the target environment and verify both required output behavior and clean execution under deprecation/future warnings.

Sources: Python Software Foundation, *What's New in Python 3.0*, sections “Text” and “Print”; Python documentation, *configparser — Configuration file parser*, “ConfigParser Objects”; pandas documentation, *IO tools (text, CSV, HDF5, …)*, “Date columns”; NumPy documentation, *NumPy 1.20.0 release notes*, “Deprecations.”

## How to check

Run the program normally and with warnings promoted to errors, then inspect output and dependency metadata:

```sh
python3 -W error::DeprecationWarning -W error::FutureWarning path/to/script.py
cat requirements.txt
```

Also verify the output includes one correctly formatted result per required group and that paths work when launching from a different current working directory; script-relative paths should be based on `Path(__file__).resolve().parent`, not the shell's working directory.
