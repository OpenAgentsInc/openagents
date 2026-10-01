+++
v = "openagents.eval-case.v1"
description = "A whole decorated history saved to a file: the notes cover only the commits since the previous tag."
kind = "should-fire"
tags = ["saved", "tags"]

[run]
allowed_operations = ["read", "write"]
+++

Our full history is in history.txt (from `git log --decorate`). Write release notes for the newest release only.
