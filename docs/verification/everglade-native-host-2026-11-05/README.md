# Shared Everglade admission correction

This is a completed shared renderer correction found while building the Mac
Metal host. It does not qualify interactive Metal parity or change the desktop
default. Issues #12868 and #12869 retain their remaining exits.

The current producer emits 2432 uniform floats and the shader declares 608
vec4s. The catalog still declared 575. The catalog and its measured span fixture
now retain the complete 9728-byte uniform range; the particle uniform remains
seven vec4s. Historical qualification snapshots retain their original bytes.

An ordinary live frame also contains other Rangers with marker 3. The shader
uses eight-word vertices, a pose base from the selected object's colour.y, and
two appearance rows selected by its shape code. The geometry authority had
refused every marker above 2 and treated every marker above 1.5 as a six-word
person. It now matches the shader's person interval (1.5, 2.5), admits the live
Ranger marker, and validates the complete remote pose and appearance spans.
All four joint bytes remain checked, including zero-weight joints.

The new remote Ranger check failed before the correction. Checks cover the
2.5 boundary, nonzero object row, pose base zero, negative/fractional/first
excess pose bases, both appearance rows, nonfinite metadata, and all existing
geometry domains. No admission bypass or equivalent-mutant annotation was used.

Focused checks and entry-source mutation passed on these exact source bytes:

| Source | SHA-256 | Mutation |
| --- | --- | --- |
| `aethyr/render/catalog.coax` | `3962b6dacfb2ef126cdbc579601a372f6eb29db3e3f990dde77a55ba117c7bb7` | 46/46 killed |
| `aethyr/render/geometry.coax` | `8d8a8db9dfae215f50b38ecd10b6be0213d0631dda67b3c0f42a914271de659a` | 737/737 killed |
| `aethyr/render/host-inputs.coax` | `79cf7c8ec542f1774a55136fbe298e29c285710ee4024bde0d137e2fd71ec55a` | 70/70 killed |

Commands used `COAX_JOBS=2 COAX_CHECK_JOBS=2`, `--library --root-only --jobs 2`;
geometry was split into four mutation shards and merged. Earlier shards were
replaced after adding the final nonzero-row check because the merger correctly
refused records from a different source hash. Passing unchanged final gates
were not repeated. The qualified tool runner was the existing cached Coax
image; no new compiler release or shader arithmetic qualification is claimed.

Regenerate shared renderer package metadata and carriers for the corrected
catalog. Old 575-vec4 package metadata is stale against this current catalog.

`tools-check` ran before push. Its five existing failures are the unchanged
`verse/tools/humans/{face_rig,metahuman,outfits,pack_human,shape}.py` files
without classification headers. This correction adds no non-Coax tool.
