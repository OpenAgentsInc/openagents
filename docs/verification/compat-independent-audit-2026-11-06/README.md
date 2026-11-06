# Independent compatibility integration audit

On November 6, 2026, approximately 05:08–05:10 Chicago time, an isolated
temporary worktree combined main `d6e35a4430` with the concurrent compiler
delivery at `b969dccdef`. The merge was used only for this audit. Its staged
compiler/application changes were not committed or pushed from the audit
worktree, which was then removed. The concurrent delivery remains responsible
for its final networking integration and issue closure.

The [Coax audit](audit.coax) tests ten compositions through the actual
`coax compat` command. They cover a changed final array-default element,
optional tag and nested payload changes, enum numeric identity, C ABI element
layout through ordinary/nested views, evaluated-default baseline equivalence,
a different incompatible default, a permitted nested defaulted append and a
changed existing default alongside that append. All ten pass with the
candidate Coax 0.52.0 binary, both when the driver is compiled by 0.51.2 and
by 0.52.0. The [candidate receipt](candidate.log) and [binary identity](compiler.sha256)
are retained. The [previous compiler](previous-compiler.log) supplies no
compatibility command and passes none of the ten cases; its usage refusals
are not mistaken for compatibility diagnostics.

The frozen driver contains the candidate's required effect declaration.
After Coax 0.52 is available, reproduce it from a normal isolated worktree:

```sh
coax/web/tool.sh docs/verification/compat-independent-audit-2026-11-06/audit.coax \
  "$PWD/coax/bin/coax"
```

## Physics Lab integration

Main added the Physics Lab and `cxproc_now_ns` after the concurrent compiler
branch's qualification point. Its new authority procedures, exported tool
entries and foreign clock need effect declarations under 0.52. The original
[refusal](lab-red.log) is retained. The [prepared lab patch](lab-annotations.patch)
adds declarations only. The merge also retains the nanosecond foreign
boundary with `#effects(foreign, clock)`, alongside the concurrent branch's
other process declarations. Its public wrapper and native implementation
remain unchanged.

With those declarations, the [full lab suite](lab-checks.log) passes 63/63,
the [scene authority](lab-mutation.log) kills all 116 root mutants with zero
survivors, and the [native system test](clock.log) passes. No lab arithmetic,
profile, clock effect or captured state was changed. The measured 0.51.2
physics results remain the results recorded in the Physics Lab delivery;
this audit does not replace them with new 0.52 timing measurements.

Coax 0.51 deliberately refuses effect declarations with E0940. Apply this
transition only with the compiler delivery, then rebase it onto the final
main sources. This audit does not close #12952–#12955 or claim the concurrent
branch's final networking/runtime gates. It did not deploy or access production.
