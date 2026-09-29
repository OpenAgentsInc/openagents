+++
v = "openagents.eval-case.v1"
description = "Find the biggest file in a small web service and say how big it is."
kind = "should-fire"
tags = ["layout", "size"]

[run]
allowed_operations = ["read", "write"]
+++

Which file in this project takes up the most space, and how many bytes is it?
