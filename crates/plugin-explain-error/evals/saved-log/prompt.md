+++
v = "openagents.eval-case.v1"
description = "Saved CI output named by path: the failure and the code it points at are only in files."
kind = "should-fire"
tags = ["python", "saved-output"]

[run]
allowed_operations = ["read", "write"]
+++

CI failed on main and I saved the job output to ci/test-output.log. What broke, and how do I fix it?
