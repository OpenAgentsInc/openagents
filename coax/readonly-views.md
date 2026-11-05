# Read-only views

Issue: #12924. Initial profile: evaluator and C output, including generated
native interfaces. Device outputs refuse read-only syntax with E0951; this
profile does not qualify GPU execution or add exclusive borrowing.

## Access and syntax

`[] const T` is a counted read-only borrow of T elements. `const` is contextual
immediately after `[]`, not a new globally reserved identifier. `[] T` keeps its
existing writable contract. Both descriptors have the same pointer/count size
and alignment, but are distinct types and generated C names. The read-only C
form contains `const T *data`. Published writable descriptor names and layouts
are unchanged.

A mutable view can restrict its access to a read-only view in an explicitly
typed initializer, assignment, argument, return or sum/aggregate payload. An
explicit `cast([] const T)` also restricts access. Conversions preserve exact
element identity, count, backing and lifetime and evaluate the operand once.
They do not allocate or copy the element array. There is no reverse conversion,
including through a cast. Conditional branches can share the read-only form.

Array slicing in a read-only expected context borrows local arrays, constants,
and immutable fixed-array parameters without an authored array copy. In an
untyped context `a[..]` keeps existing writable-slice admission; use an explicit
read-only binding or cast when borrowing immutable storage. Temporary array
values must be put into a local before borrowing. An array parameter is still
passed by value: borrowing it avoids an additional local copy but does not
change that call's value semantics.

Indexing and slicing preserve read-only access. Writes through a read-only
index or inline aggregate field are E0359. Copying an element's inline value
produces a separate ordinary value which can be changed locally. A record can
carry a read-only descriptor; helper returns, optionals and union payloads
keep that descriptor's type and the ordinary borrow provenance rules.

## Nested capabilities and lifetimes

A read-only element may contain scalars, inline arrays/records, strings and
other read-only descriptors. It may not contain a writable view anywhere inside
an array, record or sum: copying that descriptor would expose writable backing.
The complete concrete type graph is validated after record layouts and generic
instances resolve, including forward declarations. Writable or recursively
unresolved descriptor containment refuses with E0950; analysis depth/storage
exhaustion remains a refusal, never an assertion of safety. Generic templates
are validated again in their concrete instances. Generic fixed-storage records
retain their existing prohibition on borrowed type arguments.

Read-only access is not immutable backing, unique ownership, race freedom or
an authorization grant. An existing writable alias may update the backing and
readers observe those updates. The caller still excludes concurrent conflicting
access. No new Aethyr job scheduler or resource-lifecycle policy is introduced.

The existing returned-borrow and retained-store analyses still apply to these
views and their carriers. A read-only view of a local array or a by-value array
parameter cannot escape that procedure. Static constants can be borrowed for
the program's lifetime. Host-provided buffers keep the host's lifetime/extent
obligations; `const` cannot authenticate a pointer or prevent a malicious foreign
callee from violating its contract.

## Verification

`tests/pass/readonly_views.coax` exercises static and parameter borrows, generic
readers, carriers, alias observation, conditional unification and single operand
evaluation. `tests/fail/readonly_*.coax` reject writes, capability upgrades,
unsafe containment and escaping/temporary backing. Checker and parser source
include direct policy/grammar checks. Native interface and sanitizer evidence
is recorded with the implementation verification record; source support alone
is not a new target qualification.
