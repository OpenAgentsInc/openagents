+++
v = "openagents.eval-case.v1"
description = "Find which directory of a service repository holds the most files."
kind = "should-fire"
tags = ["layout", "directories"]

[run]
allowed_operations = ["read", "write"]
+++

Which directory in this project holds the most files, and how many are in
it? Name the directory by its path.
