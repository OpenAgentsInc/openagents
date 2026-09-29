+++
v = "openagents.eval-case.v1"
description = "Add the one function a library's index forgot to re-export."
kind = "should-fire"
tags = ["edit", "exports"]

[run]
allowed_operations = ["read", "write"]
+++

`lib/index.js` is meant to re-export every function that `lib/format.js`
exports, and it is missing one. Add the missing re-export and change
nothing else.
