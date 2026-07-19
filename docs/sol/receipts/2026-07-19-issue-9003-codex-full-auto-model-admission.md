# Issue #9003 — Codex Full Auto model-admission closure

- Date: 2026-07-19
- Issue: #9003
- Status: implementation present on `main`; explicit regression coverage added
- Runtime authority: installed Codex app-server model catalog

## Resolution

The two-literal Codex model gate described in #9003 is no longer present on
`main`. Commit `7a01228b7d49c3a2832a44ef8ab6ae97dcd15941` made Codex model identifiers
bounded wire data and moved exact admission to the live visible catalog
reported by the installed Codex app server. The same catalog now owns:

- the composer model options;
- provider-lane capabilities and `allowedModels`;
- first-turn provider admission; and
- durable Full Auto continuation-profile revalidation.

The continuation regression suite now names `gpt-5.6-terra` explicitly. It
proves that a non-default installed model survives durable-profile decoding,
while a structurally valid Codex identifier absent from the installed catalog
still fails closed. Existing runtime coverage separately proves the installed
app server projects Terra, Luna, 5.5, 5.4, 5.4 Mini, and 5.3 Codex Spark.

## Boundary

This closes the stale static-model admission defect. It does not claim that an
arbitrary `gpt-*` identifier is runnable: exact membership in the currently
observed installed catalog remains mandatory. It also does not promote rc.21
or alter the signed Desktop feed; those release gates remain separately owned.
