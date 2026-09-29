+++
v = "openagents.eval-case.v1"
description = "Read a passing JUnit report and say how many tests ran."
kind = "should-fire"
tags = ["reports", "junit"]

[run]
allowed_operations = ["read", "write"]
+++

Did any tests fail in the last run? How many tests ran in total?
