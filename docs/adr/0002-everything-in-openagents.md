# 0002: Everything in the openagents repository

Status: Accepted (owner, 2026-10-24: "we are open-sourcing everything except
for the private cakbags repo"; "all of the new stuff should be in the Open
Agents repo").

## Decision

- New engine, framework, and product work lands in this repository.
- The Aethyrforge engine moves here and becomes open source under this
  repository's license. Until the move is done, Aethyrforge `main` is
  read-mostly: finish in-flight work there, start new work here.
- `cakbags` stays private. Nothing from it is copied here: no review packets,
  briefs, or screen packets. Records here may state product intent in our own
  words.
- Licensed vendor files, anything converted from them, compiled private asset
  sets, and captures of licensed scenes never enter Git
  ([private assets](../verse/private-assets.md)). Builds without private
  assets run on original or CC0 fixtures.

## Not yet decided

How the history moves, the license field of the imported crates, and the
language exceptions are owner decisions listed in
[the merge plan](../aethyr/merge.md#decisions-for-the-owner).
