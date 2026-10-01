+++
v = "openagents.eval-case.v1"
description = "The commits of a release saved to a file the request names: every fact is only in the file."
kind = "should-fire"
tags = ["saved", "conventional"]

[run]
allowed_operations = ["read", "write"]
+++

I saved `git log v2.0.0..v2.1.0` to release/commits.txt. Write the release notes for v2.1.0 from it.
