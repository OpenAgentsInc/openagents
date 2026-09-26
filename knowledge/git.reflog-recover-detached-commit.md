---
id: git.reflog-recover-detached-commit
version: 1
kind: method
title: Recover and integrate commits made on detached HEAD
summary: >-
  Use reflog to locate commits abandoned by checking out a branch, inspect
  their changes, then merge or cherry-pick them without losing current branch
  work. Applies when a commit is absent from branch history but may still
  exist locally.
tags: [git, reflog, detached-head, recovery]
applies_when: >-
  A repository is clean on a named branch, but a user's recent commit is not
  reachable from that branch's log.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - fix-git
  cites:
    - Git, *git-reflog* documentation, Description
    - Git, *git-merge* documentation, DISCUSSION
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
A commit made while `HEAD` is detached remains a valid Git object, even after checking out another branch; it is simply not named by a branch ref. `git reflog` records recent updates to `HEAD`, including detached commits and checkout transitions. Locate the candidate commit, inspect it with `git show <commit>`, and compare its ancestry to the target branch before integrating. Use a merge when preserving the commit and its ancestry is appropriate; use cherry-pick when only the change should be replayed. Resolve conflicts by considering both sides' intent, stage the resolution, and complete the operation. Verify the resulting history and a clean working tree. Avoid reset or garbage-collection operations before recovery.

Source: Git documentation, *git-reflog* and *git-merge* manual pages, “Description” and “DISCUSSION” sections.

## How to check
```sh
git status --short
git reflog --all --date=iso -20
git show --stat <candidate>
git merge-base --is-ancestor <candidate> HEAD || true
# After integration:
git log --graph --oneline --decorate -10
git status --short
```
Confirm the recovered commit's changes are present and the final status has no unresolved paths or unintended edits.
