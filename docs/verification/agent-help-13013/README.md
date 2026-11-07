# Generated agent help — #13013

Baseline: `1b44242125`, macOS arm64, 2026-11-07. All work, builds and the
headless muted browser trial ran in the existing issue-fixes worktree.

## Sources and drift gate

The four local commands are `openagents help anim`, `help film`,
`help realm-edit` and `help appearance`. Each accepts `--all`;
`help field PATH` returns exactly one canonical field or refuses it.
These run before account/profile validation and require no credentials.

`aethyr/tools/agent-help-build.coax` generates the committed
`apps/openagents-cli/help-generated.coax` from:

| Surface | Reader-owned source | Entries |
| --- | --- | ---: |
| anim | 36 object-key lists in `aethyr/anim/field-catalog.coax`, used by 11 component/import/directing readers | 196 |
| film | reflected `CaptureProfile` / `CaptureSource` and their CLI metadata | 11 |
| realm-edit | reflected `EgThing` and the 34-item `EG_EDIT_NAME` catalogue | 42 |
| appearance | reflected saved `CharacterAppearance` fields and 40 frozen CC2 wardrobe slots from `charactercreator-parts.coax` | 53 |

All 302 paths are distinct. Expanding each new animation key lookup back to
its original literal reproduces the 11 baseline reader files byte for byte
(except the new catalogue loads). Their accepted schemas are unchanged.
The generator admits the complete scene example through `anim_scene_load`,
round-trips the edit text through `eg_edits_read`, and round-trips the
56-byte CC2 example through `ca_restore` before publishing output.

The CLI build runs this gate:

```sh
coax/web/tool.sh aethyr/tools/agent-help-build.coax --check
```

It regenerates the complete output in memory and compares every byte. A
catalogue addition/removal, missing help entry, or invented field makes it
fail. The positive control matched: 302 fields, 64,904 bytes. Two retained
negative controls were made outside the checkout: one removed the first
entry, the other changed `anim.buffer.pack` to `anim.buffer.rejected_field`.
`--check PATH` returned 1 with `generated agent help is stale` for each.
The tracked file was never replaced by either negative fixture.

## First-page budget

```sh
coax/web/tool.sh aethyr/tools/agent-help-count.coax
```

Tokenizer: `gpt-oss-harmony-text-v1` ordinary text, the repository's exact
`o200k_base` ranks in `psionic/fixtures/harmony/gptoss.hst`; no chat framing.
HST SHA-256: `713aefbe74715ac78e32ad789ff6e6ad2d6d65372515462b76ab5ecaef8a0e7a`.
The Coax check encodes all four actual generated strings and refuses any
first page over 2,500 tokens.

| Page | Tokens | UTF-8 bytes |
| --- | ---: | ---: |
| anim | 463 | 1,797 |
| film | 482 | 1,969 |
| realm-edit | 308 | 1,085 |
| appearance | 183 | 795 |

## Recorded agent film trial

This is a same-agent help-driven trial, not a blinded independent-agent
study. With the normal source-checkout prerequisite satisfied (Everglade II
Wasm and shared host components built, existing tracked asset pack), the
agent read `openagents help film`, then ran its complete example unchanged:

```sh
# Server kept running separately, exactly as the help page specifies:
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:9377

openagents labs clip http://127.0.0.1:9377/worlds/everglade2 /tmp/help-film.mp4 --size 640x360 --seconds 1 --fps 24 --frames /tmp/help-film-frames --fresh
```

Exit 0. The CLI reported `frames 24`, `seconds 1`, `cached 0` and
`label /tmp/help-film.mp4.json`. `ffprobe` independently reported H.264,
640×360, 24/1 fps, 24 frames, duration 1.000000 seconds. All 24 PNGs were
retained; [frame 12](frame-12.png) was visually inspected and shows the
world, foliage and character. The existing [capture sidecar](capture.json)
records source/input/output identities.

MP4 SHA-256: `263fa32156c9e70c1980be46ef396492c1dd1439aa84af543f5e51fd51184db6`.
Frame 12 SHA-256: `a88c1b43b66876203ceb1e0feca04aae0e0ba4f98080db1b4c270684609665c2`.
The video and remaining frames are temporary local artifacts; the review
frame and sidecar are retained here.

Film help describes the existing live-quality WebGPU renderer captured
with a fixed step. It does not claim F1 accumulation/physical optics, traced
film, or the remaining native F0 parity/Metal qualification. It points to
the existing native-frame encoder without advertising a renderer that is
not implemented. The animation component example needs no licensed assets;
its enclosing ALP1 project/library lifecycle is stated separately.

## Validation

- Animation component reader suite: 240/240 checks pass.
- Help parsing/field lookup: 8/8 checks pass, including its Args dependency.
- New catalogue and capture-profile checks pass; mutation gates kill 1/1
  and 19/19 mutants respectively, with no survivors or equivalents.
- Help dispatch root gate: 61/61 mutants killed after the readonly view
  correction, with dependency checks retained and no equivalents.
- Native CLI build, all four first pages and all four `--all` pages pass.
  All 302 exact field lookups pass ([native receipt](native-checks.txt)). Unknown topics/fields and extra
  arguments return usage exit 2. Invalid fps/odd width return field-specific
  `film.capture.FIELD` diagnostics for fps, duration, skip and both odd dimensions before capture.
- `coax/web/tool.sh tools-check`: 0 new tools without a reason.

All 11 changed animation reader root gates pass. Each ran with
`coax/bin/coax mutate aethyr/anim/FILE.coax --library --root-only --jobs 2`;
dependency checks remain enabled. No equivalents were declared.

| Reader | Killed / total |
| --- | ---: |
| document | 71 / 71 |
| rig-document | 434 / 434 |
| clip-document | 246 / 246 |
| performance-document | 154 / 154 |
| scene-document | 397 / 397 |
| cut-document | 244 / 244 |
| pose-document | 90 / 90 |
| direction-wire | 101 / 101 |
| direction-select | 234 / 234 |
| direction | 724 / 724 |
| unreal-source | 169 / 169 |

Together with catalogue (1), capture profile (19) and help rules (61), these
kill all 2,945 mutants, with zero survivors. Every command finished within
15 minutes; the longest was the directing-engine root gate at about eight
minutes. The final CLI build includes the complete 302-field generated data.
Other agents' subsequent realm/asset/browser-pack commits do not change these
catalogues, readers, CLI files or tokenizer. The capture receipt above stays
bound to its stated baseline rather than claiming a later renderer run.
