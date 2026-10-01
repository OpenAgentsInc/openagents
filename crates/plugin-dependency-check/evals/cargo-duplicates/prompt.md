+++
v = "openagents.eval-case.v1"
description = "A Rust project whose lockfile holds two versions of the same crates."
kind = "should-fire"
tags = ["cargo", "duplicates"]

[run]
allowed_operations = ["read", "write"]
+++

Which crates does this project pull in at more than one version?
