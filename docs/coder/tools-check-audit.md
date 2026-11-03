# Tools-check audit, 2026-11-03

The audit began at `2d02e38d98`. It found eight files without firm reasons and
one stale exception-list entry. No exception-list expansion is proposed.

| Files | Resolution |
| --- | --- |
| `coax/tests/vectors/check_pins.py`, `test_pins.py` | Production pin-policy checking ported to `check-pins.coax` / `pin-policy.coax`. Python remains an independent reference and vector regeneration oracle (reason 2). Native and reference successful stdout match byte for byte. |
| `verse/tools/humans/pack_checks.py` | Reason 2: independent pixel oracle, recomputes source UV/albedo errors, material coverage and LOD comparisons outside the Coax renderer. Synthetic matching and mismatching images tested; no private assets read. |
| `psionic/tests/decoder_oracle.py` | Removed stale port-list entry: it already declares its legitimate independent-oracle reason. |

Five asset-generation tools still need real ports or an explicit owner exception:

| Tool | Approximate lines at audit | Required work |
| --- | ---: | --- |
| `verse/tools/fivemmo_hotbar.py` | 209 | Port spell extraction and card/icon composition; also drawing in `everglade_hotbar.py`, fonts, SVG rasterization and JPEG encoding. |
| `verse/tools/humans/face_rig.py` | 234 | Separate independent rig evaluation from production tables/packing; port the production side. |
| `verse/tools/humans/metahuman.py` | 1279 | OBJ/skin ingestion, joint folding, posing and texture layers; imported glTF/skeleton support in `people.py` and container support in `pack_realm.py`. |
| `verse/tools/humans/outfits.py` | 911 | Port outfit generation, rig/texture transformations and shared packer dependencies. |
| `verse/tools/humans/pack_human.py` | 564 | Port human assembly and binary packing plus shared image/container effects. |

These are production generators with substantial logic. Calling them thin
wrappers or independent oracles would misclassify them. They are newer than
the frozen port-list base and cannot be added there. Existing owner work in
[NEEDS_OWNER.md](../../NEEDS_OWNER.md) already records this decision. The owner's
choice of full ports or a temporary exception is pending; no waiver is applied.

A port should first freeze public synthetic fixtures for mesh/skin/material,
rig and image outputs, then port shared readers/container arithmetic, then
individual generators and visual effects. Compare binary sections and decoded
pixels rather than blindly requiring encoder-dependent compressed bytes.
Keep any unavoidable native image/font/SVG calls as platform effects with all
format and product decisions in Coax. Finish with tools-check and public fixture
parity, then ask the owner to run private asset/render parity. Agents must not
read the private MetaHuman assets or renders. This dependency chain is larger
than a checker-header cleanup; full private parity cannot be manufactured here.
