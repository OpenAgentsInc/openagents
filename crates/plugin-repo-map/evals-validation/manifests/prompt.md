+++
v = "openagents.eval-case.v1"
description = "Name the build and dependency manifests of a two-part project and where they are."
kind = "should-fire"
tags = ["layout", "build"]

[run]
allowed_operations = ["read", "write"]
+++

What does this project use to build and run itself? Name each build or
dependency manifest it has, with its path.
