# Probe GEPA Terminal-Bench 2 Live Canary

Date: 2026-06-08

This folder retains the first public-safe Probe GEPA Terminal-Bench 2 canary
that completed the live OpenAgents product surface Pylon assignment lifecycle through
`openagents.com`.

The canary is **unpaid smoke evidence**. It proves that a registered Pylon can
receive a Probe GEPA validation assignment, accept it, report progress, submit
artifact/proof refs, and reach operator accepted-work closeout. It does not
claim a public Terminal-Bench score, paid work, settlement, model training, or
runtime promotion.

## Live Refs

- Assignment:
  `assignment.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- Pylon: `pylon.artanis.gepa_stats_canary.20260608150415`
- Probe run:
  `probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- Receipt:
  `receipt.public.probe_gepa.terminal_bench_2.canary.20260608151057`
- Psionic import request:
  `psionic_import.public.probe_gepa.terminal_bench_2.canary.20260608151057`

## Files

- `canary-receipt.json` records the live Pylon lifecycle refs, public Pylon
  stats snapshot, Artanis snapshot, and no-overclaim boundary.
- `psionic-import-request.json` is the public-safe import request Psionic can
  consume for live closeout import review. It is marked pending and does not
  authorize frontier mutation by itself.
- `closeout-bundle/` contains the schema-backed Probe closeout bundle emitted
  by the Bun/Effect runtime. The bundle records retained Terminal-Bench initial
  evidence for the primary `configure_git_webserver` fixture while the live
  Pylon assignment also names the `filter_js_from_html` retained task ref as
  secondary initial evidence.

## Boundary

The Pylon assignment closed as accepted evidence work. The Probe benchmark
closeout remains retained evidence with a `service_readiness` failure family.
That distinction is deliberate: accepting the worker's evidence bundle is not
the same thing as passing a benchmark, publishing a score, paying the worker,
settling bitcoin, or promoting a Probe candidate.
