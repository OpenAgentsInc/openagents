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
