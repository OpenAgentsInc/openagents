+++
v = "openagents.eval-case.v1"
description = "Read saved cargo test output and find the failing test and its file."
kind = "should-fire"
tags = ["reports", "cargo"]

[run]
allowed_operations = ["read", "write"]
+++

I saved the output of our last test run in this folder. Which test failed,
and in which file and line?
