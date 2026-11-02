# Tagged values and recoverable errors

This is the R2 native LP64 language contract. The release catalogue records
implementation status. Coax owns type identity, layout, exhaustive selection,
error handling and borrow admission. The bootstrap host transports those checked
decisions and executes them. Async ownership and effect admission have separate
contracts.

```jai
Failure :: struct { domain:u32; code:int; operation:u32; }
Reply :: union { bytes:[]u8; refused:Failure; done; }

read :: (out:[]u8)->Result(int,Failure) { /* ... */ }
```

A nominal union has 1–64 uniquely named alternatives. `name:T;` carries a
payload; `name;` is unit. A value has exactly one active alternative. Construct
it explicitly with `Reply.{bytes=buffer}` or `Reply.{done}`. The payload executes
once and follows ordinary literal coercion. Zero or multiple alternatives,
payload defaults, generic union declarations, by-value cycles, and a payload
on a unit alternative are refused. Bare fields remain invalid in ordinary
record literals.

`Result(T,E)` is a reserved compiler-owned type family, interned by the ordered
pair of exact type identities. Its alternatives are `ok:T` and `err:E`; E is
non-void. Constructors supply both types:

```jai
Result(int,Failure).{ok=9}
Result(int,Failure).{err=failure}
Result(void,Failure).{ok}
```

Result payload types may contain views under the sum-aware borrow rules. This
does not relax ordinary generic-record restrictions. Direct Result parameters,
locals and returns are supported. Result cannot occur transitively inside
another Result, an optional, an array, a view element, a record or a union.
Top-level Result constants are refused in this first profile; handle a
compile-time Result before freezing its ordinary scalar/aggregate payload.
This initial handling profile prevents error obligations from disappearing
inside storage; it does not establish general ownership.

Union and Result have no implicit default. This requirement propagates through
by-value records and arrays. A record field's explicit default may construct a
valid union, but omission cannot invent an active alternative. Explicit
`=---` is refused for all sums, including optionals, and for by-value arrays
and records containing them. Legacy sum-free uninitialized declarations retain
their existing behavior.

`T?` is a tagged optional, with none/some alternatives. T is non-void and may
itself be optional. An optional declaration without an initializer defaults to none without
constructing T. `null` needs an expected optional type. `some(value)` explicitly
wraps a payload, using context when supplied; an ordinary declared procedure
named `some` takes precedence over this unbound builtin. There is no implicit
T-to-T? conversion. `some(null)` requires enough expected optional depth.

Suffix `?` binds to the named or parenthesized element type. `[]T?` is a view
of optionals; `([]T)?` is an optional view. `[N]T?` and `([N]T)?` differ in the
same way. Repeated suffixes nest. Optional `==null` and `!=null` are allowed
on either side, but do not authorize later payload access. Sums have no
truthiness, raw-payload access, integer-tag cast or general equality.

## Exhaustive selection

```jai
switch result {
  case ok(value) { /* ... */ }
  case err(error) { /* ... */ }
}
switch optional { case none {} case some(value) { /* ... */ } }
switch reply { case bytes(data) {} case refused(error) {} case done {} }
switch state { case State.ready {} case State.done {} }
```

Switch is a statement over a union, Result, optional or enum. The scrutinee is
evaluated once and captured by value. Every alternative/member appears exactly
once. Payload cases require one immutable, case-local binding; unit and enum
cases prohibit one. Existing no-shadowing rules apply. There is no default,
wildcard, guard, fallthrough or integer/string switch in this profile.

Case binding copies the payload. Inline arrays gain case-local storage; view
descriptors retain their original pointees. Selection does not mutate the
scrutinee. A switch nested in a loop does not intercept `break` or `continue`:
those still target the enclosing loop. Without a loop they remain errors.
An exhaustive switch returns on all paths when every case does. A malformed
runtime tag is a fatal invariant fault, never an arbitrary case selection.

## Propagation, fallback and cleanup

`try operand` has unary precedence. For Result(T,E), success yields T and
failure returns the identical E payload from the current procedure, which must
return Result(U,E). The exact nominal E must match; names, field shapes and
integer widths do not convert error domains. T and U may differ. A unit
success supports a void expression statement. `try` on T? similarly yields T
or returns none from a procedure returning U?. It never maps between optional
and Result families. Use an explicit switch for that conversion.

Try is invalid at top level, in a field default, or when it would exit a
deferred block. `operand or_else fallback` consumes Result(T,E) or T? and
yields T. It evaluates the operand once, evaluates fallback only on err/none,
and requires the same payload type, with ordinary literal coercion. A void
payload needs a void fallback. This explicitly discards the error; code that
needs its details uses switch. The operator associates right to left below
`||`; `try f()+1` means `(try f())+1`.

Every operand, argument, constructor payload and index follows source order.
Assignment evaluates its place before its right side, once. Propagation stops
before later operands, fields, indices, arguments or effects. `&&`, `||`, ifx
and or_else remain lazy. Loop conditions are reevaluated on each iteration;
range bounds execute once, left to right, before the loop.

Recoverable propagation is an ordinary return. It runs previously registered
defers once, inner scope before outer, in reverse registration order. Unreached
defers are not registered. The return payload is captured before cleanup;
cleanup cannot change its captured inline value. Shallow view aliases retain
their ordinary meaning. A callee's normal Result return is a value in its
caller; only the caller's explicit try propagates it again.

Fatal bounds, arithmetic, tag and invariant faults retain their fatal contract,
without normal-cleanup or continuation promises. A fallible cleanup must handle
its own Result inside the deferred block. It cannot use try to override an
in-flight return. Explicit code can preserve the primary error and record a
separate secondary cleanup failure. Automatic error aggregation and destructors
are not introduced.

## Handling obligations

Each direct Result parameter, local and produced temporary carries a checked
obligation. Naming or assigning it is insufficient. Result locals are immutable
even with `:=`. Exactly one of these consumes or transfers the obligation:

- exhaustive switch, try or or_else;
- returning the Result;
- passing it as a direct Result parameter;
- transferring it into a fresh Result local.

A transfer creates the receiver's obligation. The old binding cannot be read
again. The callee must handle its Result parameters on every normal exit.
Type queries, print operands, unused names and field lookups do not count as
handling. An explicit empty err case or fallback does.

Every exit checks pending obligations: fallthrough, return, break, continue,
and implicit failure edges introduced by try. `a:=f(); x:=try g();` refuses
while a remains pending, because g's failure could drop it. Continuing branches
must agree on pending/consumed state for each outer binding. A returning branch
checks its own exit and does not join. An ifx producing a fresh Result creates
one obligation; outer bindings in its arms still obey the same join rule.

Iteration-local Results must be handled before every iteration exit. Consuming
an outer Result inside a loop, or capturing it in a deferred block, is refused
in this first structured-flow profile. A defer may produce and immediately
handle its own Result. These restrictions do not claim loop or cleanup fixed
points. Range bounds and foreach collections execute once before iteration,
so they may handle an outer Result there. A while condition executes repeatedly
and keeps the loop restriction.

Arguments are evaluated in order and transfer into the callee only when all
arguments succeed and the call begins. Consequently
`consume(f(),try g())` refuses if f produces a pending Result that g's failure
would abandon. Evaluate the fallible g first. Duplicate argument transfers are
also refused. Concrete generic instances obey the same rules, including a $T
bound to Result; an uninstantiated template is not a handled concrete body.

## Borrowing and representation

Descriptor paths into sums carry a distinct checked variant projection. Known
constructors have only their active payload origins; none and unit have no
descriptor pointees. Unknown parameters conservatively retain all possible
alternatives. Case selection projects the selected alternative without treating
unknown facts as complete-empty facts.

Returning or retaining either success or error payload with known local backing
is E0383 at the original witness, including through try, helper calls, mapping
and case binding. Copied inline storage and shallow descriptor pointees remain
distinct. Unknown/incomplete alternatives cannot erase an earlier known bad
return or store. Existing A1/B1 depth, path, query, origin, effect and work
limits remain; exhaustion is E0381. This does not implement B2 helper-store
composition, async retention, nested-region ownership or recursion completeness.

The initial layout target is native LP64. The tag is u32, with alternatives in
declaration order; Result uses ok=0/err=1 and optional none=0/some=1. With maximum
payload size S and alignment A, the payload offset is align_up(4,A), and total
size is align_up(offset+S,max(4,A)). An all-unit sum is size/alignment4/4 with
no payload. Coax checks layout arithmetic, cycles and defaultability.

C11 uses a tag and union payload. The evaluator stores and validates only the
active payload during construction, copy, clone, freeze and extraction.
Native C uses ordinary aggregate value copies; tag checks precede selection,
extraction, comparison and printing, rather than every value copy.
Inactive bytes and padding are unspecified,
never a wire format, hash, equality input or initialized value. #run keeps its
existing storage/traversal/step bounds and cannot call foreign effects.

Scalar/view foreign restrictions remain; sums are not a new raw foreign ABI.
Exported C sums require the generated sum-capable header/schema and actual
layout observation. Published sum-free schema identities remain unchanged.
Other layout targets, reflection, inspection and device profiles must support
these forms explicitly under their declared schema or refuse them; they cannot
emit a successful partial inventory. Existing effects.v1 remains conservative
inspection, not purity. R2 refuses every #pure/#effects annotation until the
separate R11 admission policy is implemented.
