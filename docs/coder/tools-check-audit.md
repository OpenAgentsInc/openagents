# Tools-check audit, 2026-11-03

The audit began at `2d02e38d98`. It found eight files without firm reasons and
one stale exception-list entry. No exception-list expansion is proposed.

| Files | Resolution |
| --- | --- |
| `coax/tests/vectors/check_pins.py`, `test_pins.py` | Production pin-policy checking ported to `check-pins.coax` / `pin-policy.coax`. Python remains an independent reference and vector regeneration oracle (reason 2). Native and reference successful stdout match byte for byte. |
| `verse/tools/humans/pack_checks.py` | Reason 2: independent pixel oracle, recomputes source UV/albedo errors, material coverage and LOD comparisons outside the Coax renderer. Synthetic matching and mismatching images tested; no private assets read. |
| `psionic/tests/decoder_oracle.py` | Removed stale port-list entry: it already declares its legitimate independent-oracle reason. |

Five asset-generation tools needed real ports. The four MetaHuman packers
(`face_rig.py`, `metahuman.py`, `outfits.py`, `pack_human.py`, with `shape.py`) were
ported to `verse/tools/humans/pack_human.coax` and deleted in #12859 (2026-11-05).
The fifth, `verse/tools/fivemmo_hotbar.py`, and `everglade_hotbar.py` were
ported to Coax the same day (#12859, `coax/tools-in-coax.md`). None remain.

They were production generators with substantial logic, so each was ported
rather than given a reason; no waiver was applied.

A port should first freeze public synthetic fixtures for mesh/skin/material,
rig and image outputs, then port shared readers/container arithmetic, then
individual generators and visual effects. Compare binary sections and decoded
pixels rather than blindly requiring encoder-dependent compressed bytes.
Keep any unavoidable native image/font/SVG calls as platform effects with all
format and product decisions in Coax. Finish with tools-check and public fixture
parity, then ask the owner to run private asset/render parity. Agents must not
read the private MetaHuman assets or renders. This dependency chain is larger
than a checker-header cleanup; full private parity cannot be manufactured here.
