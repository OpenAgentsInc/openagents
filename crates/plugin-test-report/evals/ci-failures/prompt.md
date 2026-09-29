+++
v = "openagents.eval-case.v1"
description = "Read a JUnit report from CI and say which tests failed and why."
kind = "should-fire"
tags = ["reports", "junit"]

[run]
allowed_operations = ["read", "write"]
+++

The last CI run of this project failed. Which tests failed, and why?
