---
id: openagents.plugin-list
version: 1
kind: product
title: "Which plugins there are"
summary: >-
  The plugins in the Gym, which you can test on Coder: Project map, Code finder, Test reader, Explain this error, Release notes, and Dependency check.
tags: [gym, plugins, catalog, extension]
applies_when: >-
  The user asks which plugins there are, which plugins are in the Gym, or which plugins they can test or use with Coder.
answer: >-
  The Gym has six plugins you can test on Coder: Project map, Code finder, Test reader, Explain this error, Release notes, and Dependency check. Each has its own test set, run with the plugin and without it, so you can see whether it makes Coder better. Ask us what one does, or ask to test one.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - deploy/eval-runner/catalog
    - crates/plugin-repo-map/package.json
    - crates/plugin-code-search/package.json
    - crates/plugin-test-report/package.json
    - crates/plugin-explain-error/package.json
    - crates/plugin-release-notes/package.json
    - crates/plugin-dependency-check/package.json
evidence:
  - "Generated from the hosted runner's catalog and each plugin's package.json by crates/coder/tests/plugin_catalog.rs (#10090); PLUGIN_LIST_WRITE=1 rewrites it, and its version moves when its words do."
---

## Answer

The Gym has six plugins you can test on Coder: Project map, Code finder, Test reader, Explain this error, Release notes, and Dependency check. Each has its own test set, run with the plugin and without it, so you can see whether it makes Coder better. Ask us what one does, or ask to test one.

## Details

- **Project map** (`crates/plugin-repo-map`): Shows Coder how the project is laid out before it starts: its files, languages, largest files, build files, and tests.
- **Code finder** (`crates/plugin-code-search`): Finds the lines of code people marked for follow-up: TODO, FIXME, XXX, and HACK notes, grouped by file.
- **Test reader** (`crates/plugin-test-report`): Reads the test reports in a project for Coder: which tests failed, where, and why.
- **Explain this error** (`crates/plugin-explain-error`): Reads a failing command's output, finds the file and line in your project it points at, and explains the likely cause and a likely fix.
- **Release notes** (`crates/plugin-release-notes`): Turns the commits between two releases into grouped, user-facing release notes (breaking changes, features, fixes), each line citing its commit.
- **Dependency check** (`crates/plugin-dependency-check`): Reads your manifests and lockfiles offline and flags duplicate versions, loose or unpinned version ranges, and licenses your declared policy doesn't allow.

## Sources

- `deploy/eval-runner/catalog`
- `crates/plugin-repo-map/package.json`
- `crates/plugin-code-search/package.json`
- `crates/plugin-test-report/package.json`
- `crates/plugin-explain-error/package.json`
- `crates/plugin-release-notes/package.json`
- `crates/plugin-dependency-check/package.json`
