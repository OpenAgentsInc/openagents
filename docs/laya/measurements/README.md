# Laya measurement records

The [September 22 port baseline](2026-09-22-port-baseline.md) records exact
fixture conformance, load time, request latency, and resident memory for the
three pinned Laya checkpoints. The small conformance corpus measures the Rust
port against its Python reference; it does not establish task quality or
calibration on Coder work.

Read the [Laya overview](../README.md) for serving and the
[conformance contract](../conformance.md) for version pins and the RoPE behavior
that the port reproduces. Preserve existing records when checkpoint bytes or
execution settings change, and measure the new identity separately.
