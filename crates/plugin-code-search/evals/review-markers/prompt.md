+++
v = "openagents.eval-case.v1"
description = "Find the spots a reviewer marked as questionable in a Go service."
kind = "should-fire"
tags = ["search", "xxx"]

[run]
allowed_operations = ["read", "write"]
+++

A reviewer left markers on the parts of this service they weren't sure
about. Which lines did they mark, and in which files?
