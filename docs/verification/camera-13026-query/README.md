# Checked camera query boundary — #13026 progress

Source base: `31a30eb2dd`. Final source and artifact identities are in
`identities.txt`; the commit containing this record supplies the exact source.
The checkpoint was rebased onto `8b8b4fe754`, preserving the independently
landed purpose-filter/parity modules and Experimental lock-on/hit-stop changes.
Work used the existing isolated `/home/christopherdavid/oa-open-issues`
worktree after updating it from remote main. The older uncommitted actor
perception draft was preserved in owner-local storage after the independently
completed #12992 landed in `2b23ac884a`; it was not restored over that delivery.

## Behavior and limits

`attn_step_selected` rechecks a semantic selection's epoch and entity handles
before the existing attention policy. The owner supplies category/role/self
selection through the shared query filter. A failed terrain ray or selection
clears held/runner targets, preserves the query status, and publishes no prefix
ranking. Ordinary blocked sight still follows retention/hold/grace. Successful
steps reset the refusal status. The policy examines at most 32 candidates and
can make acquisition and retention rays for a held candidate; it allocates no
per-step heap storage. Ranking/zero-ID/counter/state-boundary checks cover the
complete production entry source. The retained original ranking oracle and
checks now live in a loaded test source part rather than the runtime source.
No checks were removed or mutant equivalence waivers added.

`cam_eye_clip` sweeps a radius in (0, 2] m over a segment of at most 200 m,
adding 1 mm to the swept radius as a clearance skin. It returns the blocking
collider's full ID/generation. A start inside the skin is blocked and unsafe;
a query failure is unsafe with its exact status. Neither permits a caller to
label an embedded head or old shot a safe fallback. No partial output geometry
or fabricated ground is introduced. Arrays remain borrowed and unchanged.

The source-part adapters reuse `cam_centred`'s ground/pitch/press result and
apply the semantic body sweep. Shake tests its actual displaced segment,
including sideways motion beside a rounded corner; the accepted eye delta
moves the aim, preserving the base facing. Listener selection remains the
accepted unshaken shot. Owned ground/mode records and finished poses are inputs,
not a newly decoded external contract. Existing public C facades, protocol and
save versions retain their identities; internal Coax attention records gain
query-status fields and dependent consumers must rebuild normally.

## Evidence

Host: coderos-4080, Linux x86_64, GCC 15.2.0, Coax 0.52.0. The local compiler
SHA-256 is `3917d4c94c03cb731dc6a1742e78981288d63021d0783dfa67048c4a001122b9`.
No matching published build was available. A full bootstrap attempt stopped
at its 180-second bound; a subsequent `COAX_MUTATION=0` local bootstrap completed
within its 600-second bound, reusing completed compiler checks. That is a local
tool build, not a new published compiler qualification. Product policy gates
below ran separately with the resulting compiler and all baseline checks.

| Gate | Result |
| --- | --- |
| `attention.coax --library --root-only` mutation | 189/189 killed, zero survivors, exit 0 |
| `eye.coax --library --root-only` mutation | 32/32 killed, zero survivors, exit 0 |
| `semantic.coax --library --root-only` mutation | 43/43 killed, zero survivors, exit 0 |
| Independent trace oracle | 768 rows, all 28 words per row checked; 28 material-corruption controls rejected |
| Native/Wasm/sanitized agreement | Integer words, including f32 bit patterns, identical across all recorded rows |
| Existing Experimental save-spell consumer | Focused “a blow strikes ahead…” test passes (34,093,707 evaluator steps); one selected test, not all 623 checks |
| Rebased purpose-filter composition | 150 checks pass, including checked body/attention against all six shared purpose/self classes |
| Rebased parity companion | 198 checks pass, including upstream fixed-step attention/actor parity and actor-digest camera independence |
| Rebased Experimental consumers | Both selected lock-on and forward-aim tests pass; two selected checks, not all 633 checks |
| Browser | Actual muted headless Chrome; 1 MiB Wasm stack and default 64 MiB memory |
| Wasm artifact | 57,981 bytes; SHA-256 `1aebef2c0876c03d5cab5f35d73bac691bfd37640ae18dd3f63db896d098549b` |
| Chapter registries | All six registry checks pass; chapter 10 status and CPU target list retained |

The full book command stops at inherited reviewed-capability pins for
`psionic.language.current` and `coax.wgsl.current` (compiler WGSL, native shader
and Everglade GPU bindings). All three named files are unchanged from the source
base in this work. The exact failure is retained in `receipts/book.txt`; no pin
was waived or rewritten. The independent registry command passes.

The retained Wasm build report measures 0.605 s for the final face rebuild
(cached Coax/clang unit, uncached face/link); the earlier uncached source build
took 9.6 s. These are build times, not frame-performance or phone receipts.

Rebased integration changed the Experimental consumer and added the purpose
and parity modules, so those checks were run against the merged tree. The
264 passing policy mutants and numerical fixture were reused: their entire
production dependency closure was unchanged by the upstream commits. The
purpose/parity companion checks do not qualify those modules' mutation scope
or native/Wasm behavior beyond the separate query trace recorded here.

Native and sanitized native each record 64 paid steps at 30/60/144 FPS and
uneven 5/27/14/31 ms frames (256 rows apiece). Wasm records the same four
cadences (256 more). The 28 columns retain complete attention state, query
status, accepted eye/aim positions, clearance, collider identity and unchanged
geometry. The native trace SHA-256 is
`2b52d61b819fb4b0f7ce6733d458846cfe1d928304582845909df8de2495db37`.
The Python oracle independently applies absolute hold/visibility stamps and
analytic wall clearance. It allows 1e-5 m for analytic-to-f32 comparison;
cross-platform/cadence trace comparison requires exact integer words.

The fixture exercises camera-only, sight-only, water, decorative sensor and
ignored-self classes; target-generation replacement; sight loss/recovery;
translated finished poses; and selection-epoch invalidation. Direct checks add
actual entity despawn invalidation, terrain work exhaustion, rounded corners,
an embedded start, zero displacement, exact distance/radius endpoints and
malformed bounded inputs. The translated pose does not qualify a dynamic
platform renderer. The main replay's orbit yaw/pitch are zero; a separate VM
check covers a nonzero yaw/pitch. No broad trig or image equivalence is inferred.

Large generated C, Wasm, binaries and raw logs remain below
`~/.openagents/aethyrforge/camera-13026/query-v1/`. Complete numerical traces
and compact gate/oracle receipts are retained here. Layout counts in
`layout.txt` describe fixed record storage, not measured GPU allocations or
phone memory peaks. Each eye call makes one bounded shared sweep. The adapters
and attention never step actors; geometry immutability in this fixture is not
the requested full playable actor-simulation digest proof.

## Reproduction and remaining issue acceptance

```sh
coax/bin/coax mutate aethyr/cam/attention.coax --library --root-only --jobs 4
coax/bin/coax mutate aethyr/cam/eye.coax --library --root-only --jobs 4
coax/bin/coax mutate aethyr/cam/semantic.coax --library --root-only --jobs 4
coax/web/tool.sh aethyr/cam/tests/query-native.coax
COAXIAL_ADMITTED_DIR=DIR/admitted coax/web/wasm.sh aethyr/cam/tests/query-trial.coax aethyr/cam/tests/query-face.c DIR/camera.wasm
coax/web/tool.sh aethyr/cam/tests/query-browser.coax DIR/camera.wasm DIR/browser
python3 aethyr/cam/tests/query_oracle.py DIR/native.txt DIR/sanitized.txt DIR/browser-*.txt
```

This does **not fulfill all of #13026's original acceptance**. Upstream closed
the issue while this checkpoint was being prepared, explicitly listing unmet
items in its closing comment. Experimental lock-on/HP targeting landed in
`c1fa4864f2` and is preserved. A playable NPC talk prompt still needs persistent
attention with hold/grace; two-subject framing needs its consumer. Real drawing must use checked shake
and sight fade, with indoor/moving-platform journeys, actor digest invariance,
desktop/phone captures and timing receipts. Existing mode/frame/hand-back
modules still need their own remaining mutation qualification. This checkpoint
does not close an issue or claim those residuals complete; chapter status and
qualified-target claims were not expanded.
