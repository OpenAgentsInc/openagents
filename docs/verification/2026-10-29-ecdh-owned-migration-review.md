# ECDH internal-stage correspondence

The internal `ecdh_stage` source SHA-256 is
`8bd78fccaa462d832e5c89621ac2159c78aa8035d4f13568783f3d195ad07c28`.
Compared with the prior reviewed SHA-256
`2f6509d0bb427fa811c274838ad19aa6d1a22550663b62720d2cd595e4d35c39`,
its sole source change removes the C export annotation from that procedure.
The private adapter SHA-256
`e52c179ee19e4b038fe924e3ea25c0b7c05bb940f7f6555e82bc564dcefc0d55`
forwards all three views to the same stage. Independent source review
confirmed that correspondence and the owner host's changed call name; it
did not rerun tests or approve target behavior. The adapter bridge must be
hidden by the actual public build, rather than treated as intrinsically
private merely because its source comment says so.

The unchanged peer-load, field chain and normalization paths retain the
separate manually reviewed correspondence and the multiplication post-fold
bound documented in the prior public-guard and field-guard reviews. The new
raw classified audit still refuses with the same two public-unproved guards.
Any new source-only decision binds the changed exact graph and caller/tool
premises; it is not a target-timing, hidden-erasure or production approval.
