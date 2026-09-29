+++
v = "openagents.eval-case.v1"
description = "Write a unit test file for a small function."
kind = "should-fire"
tags = ["edit", "tests"]

[run]
allowed_operations = ["read", "write"]
+++

Write `tests/test_slug.py` with pytest tests for `slugify()` in
`src/slug.py`: spaces become hyphens, letters are lowercased, and trailing
punctuation is dropped. Create the file; don't change `src/slug.py`.
