---
id: security.repository-secret-remediation
version: 1
kind: method
title: Remediate exposed credentials in repository files safely
summary: >-
  Find and replace committed credentials across text and embedded metadata,
  preserve placeholders and unrelated content, verify the working tree, and
  separately assess Git history and credential rotation.
tags: [security, secrets, git, repository-hygiene]
applies_when: >-
  A repository audit or task identifies credentials in tracked or untracked
  text files, configuration, scripts, or serialized metadata.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - sanitize-git-repo
  cites:
    - Git project, git-filter-repo Documentation, “Sensitive Data Removal” (https://github.com/newren/git-filter-repo/blob/main/Documentation/git-filter-repo.txt)
    - OWASP Foundation, Secrets Management Cheat Sheet, “Revoking or Rotating Secrets” and “Handling Exposure” (https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Treat secret remediation as two related but distinct operations: remove credentials from the current working tree and address exposure. Revoke or rotate credentials; replacing a file does not invalidate a credential. Existing Git commits and other clones may retain the original value, so history cleanup requires a separately coordinated history rewrite and remote update. Do not rewrite history or push without authorization.

Inspect repository guidance and status first. Search using provider-specific token patterns and credential-context patterns; avoid printing candidate values in logs, reports, or tool output. Generic searches for words such as `token`, `password`, or `key` generate false positives in documentation, model names, and ordinary code, so classify candidates using surrounding context and provider formats. Include embedded code or diffs inside JSON and other serialized metadata: parse supported formats when practical, but also inspect string values containing embedded text.

Replace each confirmed credential with a clearly named, consistent placeholder that preserves the surrounding file format and intent. Apply exact-value replacements, not broad edits to every matching word or configuration field. Preserve pre-existing placeholders and unrelated content. Keep a baseline of original file contents or a scoped diff so verification can establish that only intended files changed.

## How to check

Use a redacted scanner: report file and line or JSON field, match category, and count, never the matched secret. After replacement, rerun the same provider-specific scan over the working tree and assert there are no confirmed credential matches. Verify serialized formats parse, inspect `git diff --check` and `git diff --stat`, and compare the changed-file list against the intended scope. Check historical exposure separately without printing matching values; communicate that file cleanup does not erase prior commits. Cite: Git project, *git filter-repo* documentation, section “Sensitive Data Removal,” for limitations and consequences of purging sensitive data from history; OWASP Foundation, *Secrets Management Cheat Sheet*, sections “Revoking or Rotating Secrets” and “Handling Exposure.”
