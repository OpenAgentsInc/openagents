---
name: test-reports
description: Canonical structured-report invocations for suite runners, so failure names are read from a file instead of recovered by re-running a suite. Use whenever a test run might fail, or whenever you need the list of failing tests from a run you already paid for.
---

# Test reports: pay for the suite once

A full-suite run costs minutes. A grep over its report costs milliseconds. The
difference is whether the run left behind something addressable.

## The rule

Whenever you run a suite that can fail — and you did not just run it seconds
ago and watch it pass — write the machine-readable report to disk next to the
raw output. Then every follow-up question ("which tests failed?", "was X among
them?") is answered from the file, never from a second execution.

Long commands in this session already keep their whole output as a `cmd-N.log`
in the session directory; the result names the path. A structured report beats
grepping that log, because it holds failure names as data instead of terminal
rendering.

## Canonical invocations

### vitest / vp (TypeScript)

```sh
vp test --run --reporter=json --outputFile=/tmp/vitest-last.json 2>&1 | tail -20
```

Failing tests:

```sh
jq -r '.testResults[].assertionResults[] | select(.status=="failed") | .fullName' /tmp/vitest-last.json
```

One file's failures only:

```sh
jq -r --arg f thresholds.test.ts '.testResults[] | select(.name|contains($f)) | .assertionResults[] | select(.status=="failed") | .fullName' /tmp/vitest-last.json
```

(`npx vitest …` takes the same flags when a package does not expose `vp`.)

### cargo (Rust)

```sh
cargo test 2>&1 | tee /tmp/cargo-test-last.log | tail -5
```

The JUnit form, where the reporter is available:

```sh
cargo test -- --format json > /tmp/cargo-test-last.json 2>/dev/null || cargo test 2>&1 | tee /tmp/cargo-test-last.log | tail -5
```

Failing tests:

```sh
grep -E '^test .* FAILED' /tmp/cargo-test-last.log
```

Then re-run nothing: each failing name sits on its own line with its module
path, which is what you needed.

### pytest (Python)

```sh
python -m pytest -q --junitxml=/tmp/pytest-last.xml 2>&1 | tail -5
```

Failing tests:

```sh
xpath -q -e '//failure/ancestor::testcase/@classname | //failure/ancestor::testcase/@name' /tmp/pytest-last.xml 2>/dev/null \
  || python3 -c "import xml.etree.ElementTree as ET; [print(t.get('classname'), t.get('name')) for t in ET.parse('/tmp/pytest-last.xml').getroot().iter('testcase') if t.find('failure') is not None]"
```

## What NOT to do

- Do not run the same suite twice inside one command (`suite | grep A; suite |
  grep B`). This session refuses duplicate executions outright.
- Do not pipe a long run through `tail` alone and assume the summary is all
  you will ever need: the tail drops the names, and the names are the point.
- Do not re-run to see "different columns" of output you once held. Read the
  log file or the report.

## Where reports land

`/tmp/<runner>-last.<json|log|xml>` unless the task names another path. One
file per runner means "last night's failed run" always has an address.
