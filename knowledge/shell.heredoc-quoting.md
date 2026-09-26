---
id: shell.heredoc-quoting
version: 1
kind: tool
title: Heredoc quoting when writing files from the shell
summary: >-
  A heredoc with a quoted delimiter (<<'EOF') writes its text literally; an
  unquoted one expands $variables, backticks, and backslashes, which corrupts
  code. The closing delimiter must be alone on its line, and a line inside
  that matches it ends the heredoc early.
tags: [shell, bash, heredoc, quoting, files, cat, escaping]
applies_when: >-
  Commands write files with cat and a heredoc, especially source code,
  scripts, or text containing $, backticks, or backslashes.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "GNU Bash Reference Manual, section 3.6.6, Here Documents"
    - "POSIX.1-2017, Shell Command Language, section 2.7.4, Here-Document"
evidence: []
---

## Details

- `cat > f.py <<'EOF'`: any quoting of the delimiter (`'EOF'`, `"EOF"`, or
  `\EOF`) turns off expansion. Use it for code.
- `cat > f <<EOF`: `$name`, `${x}`, `$(cmd)`, backticks, and `\` are
  processed. A shell script's `$1` or a regular expression's `\d` is changed
  or removed without an error.
- The closing delimiter must be the only text on its line: no leading spaces
  (unless `<<-`, which strips leading tabs only, never spaces) and no
  trailing spaces.
- If the content has a line equal to the delimiter, the heredoc ends there.
  Pick a delimiter that can't appear, such as `PYEOF`.
- Nesting: a script that itself writes a heredoc needs a different inner
  delimiter.
- `cat > dir/f` fails when `dir` doesn't exist; run `mkdir -p dir` first.
- `>` replaces the file and `>>` appends; appending twice duplicates code.

## How to check

After writing, print a line you expect to contain `$` or `\` (for example
`grep -n '\$' f.sh`) to confirm it survived, and run the file's own syntax
check (`python3 -m py_compile f.py`, `bash -n f.sh`).
