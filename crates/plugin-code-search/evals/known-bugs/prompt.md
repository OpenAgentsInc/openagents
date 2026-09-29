+++
v = "openagents.eval-case.v1"
description = "Find the known bugs someone flagged in an import script."
kind = "should-fire"
tags = ["search", "fixme"]

[run]
allowed_operations = ["read", "write"]
+++

Has anyone flagged known bugs in this code that haven't been fixed yet?
Tell me what they are and where.
