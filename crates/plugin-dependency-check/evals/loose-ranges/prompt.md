+++
v = "openagents.eval-case.v1"
description = "A project whose manifests leave some dependencies at any version, with no upper bound, or on a Git branch."
kind = "should-fire"
tags = ["ranges"]

[run]
allowed_operations = ["read", "write"]
+++

Which of our dependencies are not pinned tightly enough to give the same build next month?
