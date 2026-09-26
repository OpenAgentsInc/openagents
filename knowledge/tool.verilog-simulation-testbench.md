---
id: tool.verilog-simulation-testbench
version: 1
kind: tool
title: Simulate Verilog with a self-checking testbench and race-free timing
summary: >-
  Compile with Icarus Verilog or Verilator with warnings on, drive the design
  from a testbench that compares every output against a reference model and
  prints an explicit pass or fail summary, sample outputs away from the active
  clock edge, and use nonblocking assignments for sequential logic.
tags: [verilog, systemverilog, rtl, simulation, iverilog, verilator, testbench]
applies_when: >-
  Writing, fixing, or verifying RTL (Verilog or SystemVerilog) and its
  simulation testbenches, including CPU, bus, or peripheral models.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "IEEE Std 1800-2017, SystemVerilog Language Reference Manual, clause 4 (scheduling semantics) and clause 10 (assignment statements)"
    - "Clifford E. Cummings, Nonblocking Assignments in Verilog Synthesis, Coding Styles That Kill! (SNUG 2000)"
    - "Icarus Verilog documentation: iverilog and vvp command-line usage"
    - "Verilator User's Guide: --lint-only, -Wall, --binary, --timing"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Tools.** `iverilog -g2012 -Wall -o sim.vvp tb.v dut.v && vvp sim.vvp` compiles
and runs with Icarus; add `-I` for include paths and `-D` for defines.
`verilator --lint-only -Wall top.v` catches width mismatches, latches, and
undriven signals that simulators accept silently; `verilator --binary`
(Verilator 5) builds a runnable simulation, with `--timing` for delays in the
testbench. Dump waveforms with `$dumpfile("w.vcd"); $dumpvars(0, tb);` and
inspect them with GTKWave or a small VCD parser when a mismatch appears.

**Coding rules that avoid simulation-only bugs.**

- Sequential logic: `always @(posedge clk)` (or `always_ff`) with nonblocking
  `<=`. Combinational logic: `always @*` (or `always_comb`) with blocking `=`
  and a default assignment to every output first, so no latch is inferred.
- Reset every state register explicitly; uninitialized regs are `x` in
  simulation and may be anything in hardware.
- Keep widths explicit (`8'd0`, `{N{1'b0}}`); unsized constants and
  truncation are silent.
- In the testbench, change inputs and sample outputs away from the active
  edge (for example drive on `negedge`, or `#1` after `posedge`) so testbench
  and design do not race in the same time step.
- Compare with `!==`/`===` in the testbench so `x` and `z` count as mismatches.

**Self-checking testbench.** Generate stimulus (directed cases plus seeded
random), compute expected values from an independent reference (a behavioral
model in the testbench, or golden vectors produced by a separate program),
compare every cycle or transaction, count errors, and end with one line such
as `PASS` or `FAIL n errors` followed by `$finish`. Do not rely on the
simulator's exit status alone; print and check the summary, and add a
timeout (`initial #TIMEOUT $fatal;` or a cycle limit) so a hang fails.

## How to check

Confirm the testbench can fail: flip one bit of the design's output (or of the
expected value) and see the summary change to `FAIL`. Run the lint step with
no new warnings, and run the simulation under both Icarus and Verilator when
both are available; disagreement usually means a race or an `x`.
