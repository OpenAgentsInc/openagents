# Historical briefing replay

This experimental harness compares prepared context with a native Claude Code
control. It uses fresh historical exports, identical mandatory instructions,
and one persistent CLI process per run. Each run receives the same external
verification and at most one repair turn. Rust checks run on an isolated Boat
sandbox; the model has file tools only on the local export.

The protocol, frozen inputs, all outcomes, and limitations are recorded under
`docs/audits/2026-10-03-independent-efficiency/`. Raw model streams and account
metadata stay outside the repository. Published costs are CLI list-price
estimates, not verified subscription charges.
