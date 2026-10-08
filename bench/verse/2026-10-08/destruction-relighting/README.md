# Destruction relighting evidence

The matched controls and R captures below remain tied to source
`46cf41188b8067f10a08a2eb2a49466446b1760d`. Supplementary product checks on
`36df9ceb4bbb7fb0c7be30bd1c3538e0be78ab3e` and four native motion/reactive
tests on fixture source `df355b0cd1b45843b54aea40a6e6f3d665dfa237` are recorded
separately in `verification.json`. The temporal evidence retains the first
native shader parse failure and its fixture-only fix. These checks do not
change the source attribution of the retained images.


The [final-source R supplement](supplementary/final-source-fb9a5fd280db/README.md) adds the reviewed High aftermath and saved-view restoration from runtime `fb9a5fd280db673926cfd0d649f924da3091a24b`. It retains the exact release report, 960 primary rows, extra R readback, three original PNGs, command, binary digest, and independent GPU/quiet receipts. R follows frame 959 as artifact frame 960 and stays outside primary phase statistics. Root review observes plausible aftermath receiver light and the roof, walls, windows, and ground look returning. Clock and avatar state continue, so RGB equality is not asserted. This component establishes no new matched off/on control or selective-worker convergence result.

The production captures and focused regressions support the showcase
acceptance for [#10938](https://github.com/OpenAgentsInc/openagents/issues/10938).
Source `46cf41188b8067f10a08a2eb2a49466446b1760d` uses production kit `c5599554…`,
High at 1920×1080, and one release executable on Apple M5 Max / Metal. Both
live runs settle lighting, disable TAA, advance 960 frames, and use explicit
serial completion. Only `--no-destruction-relighting` differs. The full-town
B2 multilayer sidecar is not an input to this showcase capture.

The [contact crops](current-relight-contact-crops.png) retain impact frame 469,
smoke frame 600, and aftermath frame 900. Debris silhouettes and effects
match. Relighting slightly changes exposed interior, rubble, and receiver
light; the standing neighbor remains plausible. No new lighting seam,
detached light patch, or fire regression is visible. Fire and smoke obscure
some interior contacts, so these wide views cannot isolate every stale
shadow. The [ground crops](current-ground-crops.png) show the receiver region
without changing contrast or exposure.

Physical phase maxima and end snapshots match, including 509 chunks and
391 retired chunks with 674 static parts and 53,550 merged vertices at frame
959. Lighting changes renderer grouping: the final static group count is
626 with fallback disabled and 148 enabled. The reports contain counters,
not per-frame geometry hashes; they do not assert exact geometry identity.

The [rebuild crops](current-rebuild-crops.png) show the baked roof, wall,
window, and ground look returning after the R intent. Frame 0 and post-loop
frame 960 use the same saved camera and stage. The runtime clock and character
state continue: 7,157 of 2,073,600 pixels differ between pristine and restored,
so image equality is not asserted. Unit tests check exact baked ambient/lamp
restoration, support neighbors, ground receivers, late bake delivery, bounded
occlusion selection, and LOD reset.

Both raw reports retain all 960 serial timing rows and the extra R readback.
`current-off-frame-ledger.jsonl` and `current-on-frame-ledger.jsonl` export
the exact raw rows. All six selected PNGs from each run remain retained.
These visual controls do not assert GPU duration or the #10937 bounded
throughput budget. The enabled serial swarm p99 is 19.145 ms. Earlier DAE
control reports remain labeled historical. Commands, executable digests,
checks, leases, crop coordinates, and RGB comparison statistics are recorded
in `verification.json`; `SHA256SUMS` covers the retained files.

The broader town repair evidence for [#10907](https://github.com/OpenAgentsInc/openagents/issues/10907) remains [separate](../baked-light-repair/README.md). Its clock, blend, selective completion, and receiver-inspection components retain their own source and scope. This showcase R supplement does not establish their worker convergence.
