# Quantities: the first R12 compiler milestone

This milestone adds nominal scalar quantities to the authored checker and C
backend. The operators are compiler-owned; an ordinary struct with a field
named `value` does not gain them. It is the scalar foundation of
[R12](ROADMAP.md#15-r12--units-and-coordinate-frames), with points, vectors,
coordinate frames and rigid transforms still pending.

The public declarations live in [lib/quantity.coax](lib/quantity.coax):

```jai
#module "coax.quantity";
#export Kilogram, Metre, HalfMetre, Second, Radian, Quantity;

Kilogram :: struct @unit(mass=1,length=0,time=0,angle=0,numerator=1,denominator=1) {}
Metre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=1) {}
HalfMetre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=2) {}
Second :: struct @unit(mass=0,length=0,time=1,angle=0,numerator=1,denominator=1) {}
Radian :: struct @unit(mass=0,length=0,time=0,angle=1,numerator=1,denominator=1) {}
Quantity :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;}
```

`@unit` requires an empty nongeneric tag and all six named integer literals,
in any order. Missing, repeated or unknown names are refused. Each exponent
is between −8 and 8; `numerator` and `denominator` are positive integers from
1 through 1,048,576. The scale is their ratio relative to the dimension's
base unit. Expressions and constant names are not metadata literals. The
provided tags use kilograms, metres, seconds and radians as their bases;
angle has its own exponent rather than silently becoming dimensionless.

The unit is nominal. Two separately declared tags remain distinct even when
their dimensions and scale literals are identical. A conversion between them
must be written explicitly. The representation of a concrete quantity must
be `f32` or `f64`; the template's `numeric` parameter constraint does not admit
integer quantity instances.

For a source file at the repository root:

```jai
#load "coax/lib/quantity.coax" as units;

example :: () -> f64 {
    a: units.Quantity(units.Metre,f64) = units.Quantity.{value=4};
    b: units.Quantity(units.Metre,f64) = units.Quantity.{value=1};
    total := a+b;
    doubled := 2.0f64*total;
    half_metres := convert_unit(units.HalfMetre,doubled);
    return half_metres.value; // 20 half-metres
}
```

## Operators and constraints

| Expression | Admission | Result |
| --- | --- | --- |
| `a+b`, `a-b` | Quantities with the same nominal unit and representation | Quantity |
| `a==b`, `a!=b`, `a<b`, `a<=b`, `a>b`, `a>=b` | Quantities with the same nominal unit and representation | `bool` |
| `q*s`, `s*q`, `q/s` | One quantity and one raw scalar of its representation | Quantity |

The `+` and `-` above are binary operators. Quantity products and quotients,
raw-scalar divided by quantity, quantity-plus-scalar, dimensional result
algebra and affine point operators are outside this milestone. Different
units produce E0822 and request `convert_unit`; different representations
produce E0823 and require an explicit raw representation conversion followed
by quantity construction. Unsupported operator families produce E0824.

Quantities satisfy `copyable`, `equatable` and `ordered`, but not `numeric`.
A generic maximum constrained `ordered` can therefore accept them. A generic
routine requiring arbitrary closed numeric multiplication cannot. As with
raw IEEE floats, `ordered` means the comparison operators exist; it does not
make NaNs totally ordered.

## Explicit scale conversion

`convert_unit(TargetUnit, value)` preserves the quantity family and its
representation, and changes its unit tag. The target and source units must
have identical exponents. The ratio of their rational scales must reduce to
an integral power of two; the bounded metadata yields exponents from −40
through 40. Common odd factors may cancel: the individual scale literals do
not themselves have to be powers of two. A ratio of one is allowed, including
between distinct nominal tags. Units must be concrete in this initial
conversion profile.

A dimension mismatch produces E0825. A nonbinary ratio produces E0826: for
example, this profile refuses centimetres-to-metres conversion rather than
silently choosing a decimal rounding policy. General rounded conversions and
other representation profiles remain pending.

A permitted binary ratio does not guarantee that every floating value
converts exactly. Native scaling follows the carrier's ordinary IEEE
arithmetic: it can overflow to infinity, underflow, or round a subnormal.
NaNs and infinities are allowed values; quantities add no finite-value,
range, saturation or actuator-admission checks. Normalization, runtime
bounds and physical-unit claims at an external interface remain the caller's
responsibility.

## Raw boundaries, layout and supported output

`.value` explicitly projects the raw representation. Constructing
`Quantity.{value=raw}` asserts the source-level unit of that number; it does
not inspect a sensor, SDK or file to establish that claim. This boundary is
also how the current Verse precursor forms a dimensionless ratio while
quantity/quantity division remains unavailable.

C exports and foreign scalar calls use raw `float` or `double`. Quantity
arrays and views use the corresponding raw carrier array/view ABI. C callers
do not acquire Coax's nominal unit checks. Nested records preserve the
ordinary carrier size, alignment and offsets; tags occupy no additional
storage inside quantity values. An explicitly constructed unit-marker object
is still an ordinary empty struct, rather than a stored quantity tag.

`size_of` and `type_name` remain available. Schema1 `type_info` and
`field_info` queries on a transparent quantity type are refused with E0827:
the schema cannot truthfully describe both its nominal struct semantics and
its scalar carrier. Versioned physical metadata remains part of the full
R12 work.

This is a native C profile. Concrete quantity use is refused with E0828 for
Metal, Metal compute, CUDA, SPIR-V and WGSL output. No browser, GPU, hardware
or real-time qualification follows from the C evidence.

## Consumer and evidence

[Verse's Grid](../verse/grid.coax) now types the three metre-valued inputs of
`smooth`, used by the fog calculation. Its subtraction uses the built-in
quantity operators; `.value` makes the dimensionless ratio boundary explicit.
This is a scalar precursor. The Grid's `Vec3` values and camera/world frames
are still untagged, and the Panda joints, velocities and poses have not been
migrated. It does not satisfy R12's full Verse transform and simulated-robot
consumer gate.

The focused evidence is:

- The [Coax quantity policy](lib/compiler/quantity.coax) passed six checks
  comprising 672 cases, with 96/96 root mutants killed and zero survivors.
- The [parser record](../docs/verification/coax-quantity-parser-2026-10-31.json)
  retains all 194 parser checks while mutating only exact `PS_struct` and
  `PS_quantity` bodies: 71/71 mutants killed, with zero survivors. New checks
  cover all signed metadata fields, literal boundaries, order, malformed
  names/tokens and first diagnostics.
- The [native record](../docs/verification/coax-quantity-native-2026-10-31.json)
  compares 832 input triples per carrier against independent raw C, giving
  24,139 comparisons in each of debug, release and ASan/UBSan. Scalar,
  nested-record, fixed-array and view ABIs pass, as do foreign-call order and
  single evaluation. Non-NaN bits, including signed zero, subnormals and
  infinities, match; NaNs are compared by classification. NaN payloads and
  floating exception flags are outside that profile.
- The [assembly record](../docs/verification/coax-quantity-assembly-2026-10-31.json)
  shows identical typed/raw instructions for the two no-conversion scalar
  expressions tested: 13 instructions/52 bytes each in debug, four
  instructions/16 bytes each in release. Whole generated files and objects
  differ. Generic procedure instance names still carry semantic bindings,
  and equivalent instantiations can retain separate symbols; this is not a
  whole-module code-size or universal zero-cost proof.

The [checker integration record](../docs/verification/coax-quantity-checker-2026-10-31.json)
retains all 647 checks and mutates 16 exact changed procedures: 1,090/1,090
mutants killed, with zero survivors and no equivalent waivers. Its resolved
AST, failure-sentinel, resource-arena, coefficient and module-shadow checks
cover the authored lowering, including nine focused CLI cases. A
refreshed compiler emits the exact previously qualified native C/header;
those passing arithmetic, ABI and assembly runs are reused rather than repeated.

The [normal-path reconstruction record](../docs/verification/coax-quantity-bootstrap-2026-10-31.json)
uses immutable merged commit `d4db651f8b`: all eight checked-in seeds,
including the new check-domains seed, build stage0 and the authored compiler
graph. The resulting 0.42.0 compiler reproduces the compiler C and header
exactly, and reproduces the checked-in parser/checker C and headers. The
98.4-second reconstruction retains source fingerprint `bc42e221…d88381`.
One new family-interoperation smoke passes on that exact compiler, including
typed return families, explicit conversion, scalar-left multiplication and
refused cross-family assignment. Its emitted native fixture C/header match
the earlier qualified artifacts exactly; updated-main Grid `smooth` and
`fog` C functions also match their raw versions exactly.

The later [0.43 merged-main check](../docs/verification/coax-quantity-main-0.43-2026-10-31.json)
reconstructs the new generic-literal compiler together with quantities in
73.3 seconds. The same C/header fixed points pass. A new check binds a scalar
literal to a quantity's numeric representation while refusing raw quantity
bounds. All 16 changed quantity-checker bodies retain their frozen hashes;
no unchanged mutation scope reruns. The quantity native fixture and Psionic
owned-plant bridge reproduce their prior qualified C and headers exactly,
so their earlier numerical/native proofs carry. The current Grid compiles,
and the private Basic consumer prints the recorded result. No SDK/model
trial reruns.

These private, focused results do not by themselves qualify or publish a
normal release cache. The full normal bootstrap, release/provenance checks
and cache/installed-compiler status must be recorded separately.

For a private compiler without the broad release gates, use the checked-in
[focused reconstruction tool](tests/quantity_bootstrap.py). It follows the
same 24-command seed/compiler/fixed-point plan as the recorded reconstruction,
uses a new output directory, and has one 840-second budget for the whole run.
It preserves failed partial output and never installs into `coax/bin` or a
release cache. `--plan` only reads files and prints the plan; `--verify-receipt`
compares its command/redirection plan and recorded source/seed identities.
That comparison does not independently recompute the full source/toolchain
fingerprint or qualify a release.

Run from the repository root, choosing an output directory that does not
already exist:

```shell
python3 coax/tests/quantity_bootstrap.py /tmp/coax-quantity-private --plan > /tmp/coax-quantity-plan.json
python3 coax/tests/quantity_bootstrap.py /tmp/coax-quantity-private > /tmp/coax-quantity-build.json
/tmp/coax-quantity-private/coax run coax/tests/quantity/private-bootstrap.coax
/tmp/coax-quantity-private/coax c verse/grid.coax --library > /tmp/coax-quantity-grid.c
COAX_TEST_COMPILER=/tmp/coax-quantity-private/coax python3 coax/tests/quantity.py
```

The compiler is `/tmp/coax-quantity-private/coax`; its build logs, generated
C/headers and `receipt.json` stay beside it. It needs no adjacent `lib`
directory for Basic printing: Basic support is emitted into self-contained C,
and `#load` resolves relative to the source file. The new private-consumer
example was run with the qualified output and prints
`private quantity: 12.0 half-metres`. The Grid invocation likewise loads its
unit source from the repository. The focused test drivers accept
`COAX_TEST_COMPILER`, leaving the installed compiler untouched.

An older installed binary requires rebuilding before it accepts the new
syntax. The complete normal release build remains `coax/build.sh`; its broad
standalone gates, release/provenance qualification and cache publication are
separate from this private reconstruction and were not rerun here.

## Family identity at the operator boundary

Two annotated quantity families can interoperate when their nominal unit and
carrier agree. Arithmetic keeps the left quantity family; scalar-left
multiplication keeps its right quantity operand’s family. Comparisons return
bool. Assignment/type identity still distinguish the families, and explicit
unit conversion preserves the input family. This is the written M1 rule,
not a claim that nominal declarations or unit tags have been merged.

A [Psionic compatibility check](../docs/verification/coax-quantity-psionic-compatibility-2026-10-31.json)
uses the reconstructed 0.42 compiler to emit the owned-plant sensitivity library.
Its C and header match the qualified 0.39 emission byte for byte. The earlier
physical/formal/native proofs are therefore reused; no model or SDK trial reran.
