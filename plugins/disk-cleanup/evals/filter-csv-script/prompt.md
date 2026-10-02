+++
v = "openagents.eval-case.v1"
kind = "should-not-fire"

[run]
allowed_operations = ["read", "write"]
+++

Write a standalone Python function that reads a CSV file from disk and yields only rows where the 'status' column equals 'completed'.
