+++
v = "openagents.eval-case.v1"
description = "Rename a configuration key everywhere it appears."
kind = "should-fire"
tags = ["edit", "rename"]

[run]
allowed_operations = ["read", "write"]
+++

Rename the configuration key `timeout_secs` to `timeout_seconds`
everywhere it appears in this project, in the config file and in the code
that reads it, so nothing still refers to the old name.
