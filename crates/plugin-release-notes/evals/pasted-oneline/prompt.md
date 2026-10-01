+++
v = "openagents.eval-case.v1"
description = "A pasted oneline log in Conventional Commits: breaking changes first, merges left out, every line cited."
kind = "should-fire"
tags = ["pasted", "conventional"]

[run]
allowed_operations = ["read", "write"]
+++

Turn these commits into user-facing release notes for v1.5.0, grouped into breaking changes, features, and fixes, citing each commit:

```
4f2a9c1 feat(export): export invoices as CSV (#212)
8b7c6d5 Merge pull request #211 from acme/dark-theme
c3d4e5f feat: add a dark theme to the dashboard
d4e5f6a fix(tax): round tax per line, not per invoice
e5f6a7b refactor: split the invoice module
a7b8c9d feat(api)!: drop the v1 export endpoint
```
