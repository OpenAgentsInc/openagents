# Coder terminal: Codex model and reasoning controls

Base: `bfe8dfefb1`. Verification on macOS, 2026-11-03, with Coax compiler
SHA-256 `cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.

## Targeted checks

`coax/bin/coax test coder/tui/codex-options-checks.coax --only Codex`:
6 checks passed. Covers catalog ownership across reused JSON buffers and
pages, hidden models, model-specific effort support/defaults, rejected
selections, the resolved configuration, command dispatch, per-engine
selections, startup readiness and catalog retry. The selected model and
effort are checked in the next `turn/start` JSON with the same thread ID.
The selection command starts no turn. No authority file changed; this
filtered run does not claim full authority coverage.

`coax/web/tool.sh coder/tui/coder.coax --version`: native build passed.

## Terminal verification

In an 80-column pseudo-terminal, start on Claude and press Shift-Tab:
the Codex row resolves to `gpt-6.1-sol · reasoning xhigh`, followed by
`/model · /effort`. `/model` lists the installed backend's catalog and
defaults. Verified `/effort max`, `/effort ultra`, a rejected
`/model gpt-6-luna ultra`, `/model gpt-6-luna` selecting its `medium`
default, and `/effort xhigh`. Switching away and back preserves the
selection. Ctrl-C exits with status 0 and restores the terminal. These
live checks use configuration/catalog requests, without sending a model
prompt or changing Codex's config.toml.

## Broader checks

`python3 coder/tui/smoke.py` passed all 26 terminal checks, including
resize and both Shift-Tab encodings. A finished-source run and an isolated
unchanged `bfe8dfefb1` worktree both fail
`watchdog: a worker whose process died ends as lost`. Memory and consent
checks pass on both. An earlier attempt during ongoing builds failed in
memory/consent and stopped at a missing session file; those failures did
not reproduce with the source held fixed.

`coax/web/tool.sh tools-check` fails on existing files outside this change:
missing reasons in `coax/tests/vectors/{check_pins,test_pins}.py`,
`verse/tools/fivemmo_hotbar.py` and
`verse/tools/humans/{face_rig,metahuman,outfits,pack_checks,pack_human}.py`;
`psionic/tests/decoder_oracle.py` has both a reason and a port-list entry.
These files and the port list are unchanged from the base. New source and
checks in this change are Coax.
