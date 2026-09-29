+++
v = "openagents.eval-case.v1"
description = "Read saved pytest output and say which tests broke."
kind = "should-fire"
tags = ["reports", "pytest"]

[run]
allowed_operations = ["read", "write"]
+++

Our Python tests broke after the last merge; the output is saved in this
folder. What broke?
