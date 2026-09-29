+++
v = "openagents.eval-case.v1"
description = "Fix a pager that drops the last item of every page."
kind = "should-fire"
tags = ["edit", "bug"]

[run]
allowed_operations = ["read", "write"]
+++

The `paginate()` function in `src/pager.py` drops the last item of every
page: a page of size 3 comes back with 2 items. Fix it in place and keep
the function's signature.
