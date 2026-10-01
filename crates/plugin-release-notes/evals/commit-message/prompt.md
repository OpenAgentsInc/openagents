+++
v = "openagents.eval-case.v1"
description = "Writing one commit message, where grouping a log has no place."
kind = "should-not-fire"
tags = ["quiet"]

[run]
allowed_operations = ["read", "write"]
+++

Write a one-line commit message for a change that renames the `user_id` column to `account_id`.
