# Typed runtime clock and fixed-schedule precursor

The [clock profile](../../psionic/docs/physics/step-clock.md) supplies distinct
nominal domains, exact integer 500/20Hz scheduling, request reanchoring and
explicit fail-stop integration receipts. Its owned oscillator consumes the
original unchanged RK4 implementation. The public
[qualification](../../psionic/fixtures/physics/step-clock-qualification.json)
binds 17 source/fixture/doc files, the six-file runtime closure, private qualified
0.43 compiler, generated C/headers, modes and independent event truth.

Thirteen new Coax checks pass. Domain 81, schedule 221 and consumer 27 give
329/329 killed root mutants with no survivors or equivalent waivers. Each
native debug/release/fail-fast ASan+UBSan build passes 1,808,302 checks.
The four 18,000-step scenarios yield 72,000 events/1,368,000 integer words exact
against the independent integer/Fraction oracle. Five domain compile-fails
reject with E0317. Eight corrupted proof-binding cases refuse.

Review caught a wrong-phase/post-commit duplicate receipt that initially
returned order without halting. The added check precedes its branch fix;
changed scheduler mutation and native builds were refreshed, while unchanged
domain/consumer mutation and the independent event truth were reused. Root
review confirmed state must be adopted even on the bad-receipt status, and
that sc_sim_age only calculates age. Shared gate/book registration was then
added; no old model, SDK or native arithmetic corpus reran.

This is not full compiler R13, R14 bounds, a real watchdog/cancellation,
maximum-age/R18 supervision, full snapshots or robot/Verse integration. The
pinned SDK cache convention is separately audited from XML and 3.3.1 source.
