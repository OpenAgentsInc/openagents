# Foundation publication after concurrent main updates

Chapter 109 publication rebased onto 4b50a5c112 and kept both the new chapter 113
causal-decoder inventory/dispatcher and the new owned-physics inventory/dispatcher.
No other chapter plan row was replaced.

The book evidence gate exposed concurrent changes in 49e92d76ed and ed1c24d3f2.
Review of their exact bound-file diffs found built-in generic constraints and
fragment-depth graphics support. The generics chapter’s old claim that all
constraints were planned was no longer accurate: its current scope/narrative
now describes the four implemented built-ins and retains user constraints,
slash syntax, casts/layout queries and operators as separate limitations.
The existing 0.40.0 verification record supplies the new compiler evidence;
its passing compiler checks were not repeated.

WGSL now carries a depth bit in a field’s lowered facts, emits one F32
fragment-depth output without consuming a color location, refuses depth on
vertex interfaces and retains SPIR-V refusal. The native-GPU note reports
the named fixture consumer and keeps realm/waterlab follow-up explicit.
These graphics changes do not establish Psionic browser compute. Only their
six affected claim/evidence bindings were reviewed and refreshed.

## Chapter 111 publication follow-up

Concurrent commit e60ab60b87 changes the native-GPU consumer note: realm and
waterlab now follow the translator with zero changed golden blocks after local
procedure-name collisions were removed. The exact reviewed diff changes no
Psionic compute claim. Only the two native-GPU evidence bindings were refreshed.
The post-rebase book gate detected stale bindings before chapter 111 commit
4b90dcf4e1 was pushed, but a shell sequence failed to stop on its nonzero exit.
This follow-up corrects that metadata; future dependent publication commands
use errexit. Chapter 111’s numerical source identities and passed native,
independent and mutation evidence were unaffected and were not rerun.

## Sensitivity-bridge publication follow-up

Rebase onto 2ce0aab113 brings the realm template and build-time gpu-module
generation into the named graphics consumer note. The exact reviewed diff
adds a mounted-template golden comparison (zero changed blocks), removal of
its hand WGSL and the generation rule. The bounded graphics/Psionic-compute
claim scope stays unchanged; only the two native-GPU evidence bindings change.
The book gate stopped publication until that review completed. Numerical
bridge sources and their passed records are unchanged.

## Quantity integration on current main

Rebase onto 2a6f0ebbef includes the existing graphics extension that emits one
F32 fragment depth as BuiltIn FragDepth with DepthReplacing. Review of the
authored shader policy confirms color locations remain contiguous, up to eight
targets plus one depth are admitted, and duplicate/non-F32/varying depths are
refused. The native-GPU note now records SPIR-V validation of page entries.
This extends the named graphics evidence; it does not qualify Psionic browser
compute or quantity device code. Only the two changed native-GPU evidence
bindings were refreshed. The quantity compiler sources are unchanged by this
review; their immutable merged bootstrap snapshot is retained separately.

## Scalar-unit publication on main 0.43

The final main rebase included 06b6eb364b: Hold the Stair can run natively,
the default-initialization wrapper now follows the wasm path for native
realms, optional exports/sockets match the browser host, and the named
flat-pass graphics check records its browser/Vulkan comparison. Review of
the native-GPU note adds this named consumer only; no Psionic browser
compute or quantity-device execution is qualified. The two affected
capability/evidence hashes are refreshed.

The book structure check passed before that final main rebase, but the push
command omitted the check after rebase; the first check of the resulting main
therefore caught the upstream documentation change after e6d4ab1bc0 was
pushed. This correction records that ordering mistake. Future publication
uses a single dependent sequence with the book check *after* rebase and
*before* push. Quantity and Psionic source/numerical proof is unaffected.
