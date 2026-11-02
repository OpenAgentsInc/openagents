# Checked-source effect inventory

`coax deps FILE --effects [--proc NAME]` emits `coax.effects.v1`: a conservative
inspection of concrete, resolved checked procedures and their syntactically
reachable callees. It is an R11 prerequisite, not complete transitive-effect
enforcement, an allocation/blocking proof, or `#bounded` admission. The
[qualification record](../docs/verification/coax-effects-inventory-2026-10-31.md)
retains the actual compiler, consumer reports and verification scope.

```shell
coax/bin/coax deps psionic/lib/robotics/panda-control.coax --effects --proc pc_step
coax/bin/coax deps aethyr/camera.coax --effects --proc verse_orbit_follow
```

Coax owns effect classification, reachable closure, cycle detection and witness
selection in `lib/compiler/effects.coax`. Native `src/effects.cpp` observes
resolved call targets, arithmetic trap facts, operation kinds, borrowed access
bases and physical source locations, and formats JSON. It does not search source
text for calls or substitute a second effect-policy implementation.

Each procedure row names its concrete instance key and declaration, a may-effect
mask, reachable procedure count, reachable recursion, and one witness for each
reported effect. The witness gives a shortest breadth-first call path, breaking
ties by observed source-edge order, and its terminal operation's source location.
Generic instances participate with their actual bindings. Uninstantiated template
bodies are counted separately and contribute no runtime call edges. Loaded units
keep their physical paths. Every source's SHA-256 binds the inspection to its
actual Coax bytes. Descriptor/schema inputs remain a separate provenance
obligation; this is not a hermetic build receipt. Local numeric procedure indices are not stable cross-build IDs.

The inventory includes both sides of branches, all `defer` bodies and reachable
error paths, without proving their feasibility. Frozen compile-time values do
not retain their former computation as a runtime call. Recursive graphs are
reported, not executed. This pass does not infer iteration bounds.

| Bit | Meaning |
| --- | --- |
| 1 | Host I/O may occur: source printing or current native fatal-trap logging |
| 2 | A foreign procedure is reachable |
| 4 | Effects/resources remain unknown or unqualified |
| 8 | An always-on language trap, or checked integer overflow, may occur |
| 16 | Referenced storage may be read through a borrowed base or iteration |
| 32 | Referenced storage may be written through a borrowed base |
| 64 | Math-intrinsic implementation resources are unqualified |
| 128 | Uninitialized storage is declared; definite assignment is not established |
| 256 | An address-overlap observation is reachable |

Unknown obligations remain explicit. Foreign calls can allocate, block, read
clocks/random sources or access arbitrary regions; none is presumed absent.
Math and byte intrinsics currently retain resource obligations. Returning a
borrowed value retains a lifetime obligation. Borrowed reads/writes do not
establish region identity, disjointness, an exact byte range or nonescape.
A zero mask for a scalar-only procedure is **not** a general resource proof.

`--arithmetic wrapping` suppresses conditional integer-overflow sites only.
Division, indexing, narrowing and explicit assertions retain their always-on
trap obligations. The current native trap helper flushes/prints before aborting;
those paths cannot be silently called nonblocking. No foreign-effect declarations
or trusted resource exemptions are introduced by this pass.

The report modes `--effects`, `--lowering`, `--declarations` and `--modules`
are mutually exclusive. `--proc` selects all concrete instances with that authored
name; their instance keys and physical declarations distinguish them. Missing
names and malformed/oversized graphs fail without publishing a partial JSON
report. Default inspection includes checks as separate concrete roots, as well
as product procedures; the ordinary frontend still validates the input program.

The Coax graph policy admits at most 4,096 procedures, 65,536 edges and 65,536 sites,
with `procedures * (edges + sites) <= 16,777,216` per root. Caller-owned parent,
edge, queue, indegree and nine-entry witness spans must be valid and disjoint.
Invalid facts or short/overlapping spans leave outputs unchanged. These are
inspection work limits, not target runtime stack or real-time guarantees.

Focused verification is in `tests/effects.py` and `tests/effects-native.c`.
The independent native oracle uses Floyd-Warshall reachability, enumerates
simple paths for shortest witnesses, and identifies cycles by transitive closure,
where the Coax implementation uses BFS and topological removal. It compares
247 graphs/refusals across debug, release and ASan+UBSan and checks atomic refusal
and spare tails. CLI checks cover foreign/defer closure, concrete generic calls,
loaded-source provenance, profile-dependent traps, borrowed access/returns,
frozen compile-time calls, recursion and report-option refusal. No unrelated
compiler, original-model or earlier robotics mutation gate is required.

```shell
COAX_TEST_COMPILER=/path/to/current/compiler python3 coax/tests/effects.py EffectsCLI
COAX_EFFECTS_RECORD_DIR=/tmp/new-effects-native \
  python3 coax/tests/effects.py PolicyNative
```

R11 still needs reviewed foreign summaries, allocation/blocking/clock/random and
atomic/region semantics, alias and escape reasoning, broader supported procedure
forms and declared-versus-observed resource evidence. R14 additionally needs
visible loop bounds, caller-owned scratch lifetimes, target-lowered stack bounds
and error-path treatment before a bounded contract may be admitted. Units,
frames, typed clocks and multirate schedules remain separate compiler work.
