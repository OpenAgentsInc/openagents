# Coax and Psionic book audit: stopping assessment

Three of the original eight items are complete. Five remain open. The work is tracked in the Coax project; completing a borrow subtask does not complete its umbrella issue.

| Item | Standing | Delivered or remaining |
| --- | --- | --- |
| Book accuracy and shared recipes (`coax-book-usage-accuracy`, #35c5) | Complete | Capability claims, scope/provenance, cross-domain recipes and shared-library adoption reconciled. Later book additions keep their own recorded coverage. |
| Native check execution (`r0-native-check-exe`, #ccba) | Complete | Authored-body native checks and finite properties, baseline-first verdicts, optimization/sanitizer and real-consumer observations. This records failures as well as passes. |
| Bounded generic values (`r4a-generic-values`, #76ba) | Complete | Generic procedures/records/value parameters and shared bounded-storage use in Psionic, Verse and web consumers. |
| Borrow summaries and consumers (`r3a-borrow-summary-consumer`, #1a6a) | Open | A1 returned-borrow checking and B0 retained-store policy are on main. B1 retained-store integration is unqualified and unpublished; B2 helper-write/returned-field composition and B3 request ownership remain open. |
| Results and optionals (`r2a-results`, #6541) | Open | Tagged results, exhaustive handling, must-handle outcomes and cleanup need implementation and a real consumer. |
| Backend artifact profiles (`r8-backend-profile-manifest`, #af3c) | Open | Actual target, ABI, arithmetic, toolchain, dependency and link identities need explicit artifact records. |
| Typed GPU records (`r8-typed-gpu-records`, #3152) | Open | Checked record/storage layouts, real device reflection and consumer adoption remain. |
| Formatting and evidence tooling (`r7a-format-evidence`, #ca69) | Open | Lossless comments/formatting, evidence completeness and revision/CAS guarantees remain beyond existing partial query fixes. |

B1 is not yet a qualified compiler release. Its implementation checks concrete bodies and substitutes caller obligations, preserving known bad alternatives beside incomplete facts. The first 30-case source smoke passed on its explicitly recorded intermediate compiler. Later integration exposed the compiler's own large test source exhausting the existing shared work budget. The final frozen Coax program passes925 authored checks on the older qualified producer. A rebuilt intermediate still refuses its self-input at checker.coax835:2; the first exhausted tick occurs during pending-effect call application after seeding. This locates the failure, not the dominant cost. The retained records distinguish original failures, static-source lifetime projection, direct local-address lookup and type-presence caching. Full authority mutation, bootstrap/fixed points, matching native artifacts and final consumer gates are required before the implementation can be pushed or B1 closed.

The main remaining project-wide benefit is to finish buffer ownership and handled outcomes together. Continue with B2, then B3: a helper-written local carrier and an asynchronous request need different proofs. Follow with Results, explicit backend artifacts, typed GPU bindings and completed formatter/evidence tooling. These improve the same contracts used by Verse, Psionic and host interfaces.

Known limits remain visible: accepted manual borrow paths are caller duties, not lifetime proofs; the retained HUD baseline had 25 mutation survivors; and evidence's advertised per-check budget does not always report the authored effective cap. None is relabelled as a current pass. Production publication remains separate from source and compiler qualification.

The accompanying progress record and raw inventory bind the exact stopping observations. Unqualified compiler work stays isolated in the Coax audit checkout. No B2 implementation or new workstream was started for this stopping request.

Main advanced again before this documentation push: `629a66626e` fixes returned storage through passed-on view parameters and static-origin overflow, with five new source fixtures. The unpublished B1 checkpoint predates that change and needs a deliberate merge and renewed qualification. Its historical source smoke and program observations are not attributed to this later main.
