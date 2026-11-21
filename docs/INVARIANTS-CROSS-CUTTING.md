# Invariants — docs/INVARIANTS-CROSS-CUTTING.md

> Scope: Cross-cutting & general repository invariants (8 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Planning derivatives keep physical execution and fixed context explicit

The oscillator-function bridge augments q/v with held force and builds a
permanent F32 RK4 graph. Its published partial selectors vary q/v/u only;
packed matrix/time/target/weight inputs remain fixed context. Formal F64
partials at stored F32 primals are separately compared with the actual F64
physical transition. They do not differentiate float rounding, bounds refusal,
clipping or contact. `of_pack` admits the starting inputs; actual state commits
retain `os_step`’s next-state admission. Qualified scalar planning derivatives
are not matching pretrained-robot embodiment or whole-backend qualification.

## The owned grip Jacobian is a derivative of one fresh kinematic query

The exported Jacobian path evaluates its own fixed-model FK before projection.
Its linear block means metres per joint radian; its angular block means world
rotation radians per joint radian. UnitOne preserves that ratio explicitly,
and multiplication by a radian-per-second rate restores angular speed.
A coordinate-only arithmetic helper is not a caller-POD admission certificate.

Refusals preserve caller output. Singular poses remain valid without inversion
or command admission. Independent matrix position/SO3 differences use a frozen
primary step and budget; the full sweep is diagnostic. Reference loaders reject
nonfinite values before maxima. The proof does not differentiate rounding,
refusal, saturation or soft joint-limit dynamics, and supplies no sensor stamp
or generic typed transform chain.

## Actuation keeps signed position targets and joint passive force distinct

The fixed-source actuation query admits all shapes and finite numerical boxes
before arithmetic. Arm controls are generalized forces; finger controls are
already signed metre-valued targets. Clamp controls first, preserve the separate
fixed-gain/affine-bias operation order, then clamp finger force. No second right-
finger sign flip or normalized command transformation occurs. Strict clipping
counts and every force publish together; refusals preserve complete output.

Actuator and joint-passive outputs remain separate. Joint stiffness is exactly
zero in the admitted profile, so joint passive specializes to typed damping.
The source fluid is nonzero, and frictionloss belongs to constraints: neither
may be silently inserted into or omitted from a claimed full passive result.
SDK support admission precedes all eighteen new case forwards; all parameter,
flag, named-DOF/gear and actual sparse-row facts stay bound to their receipts.
Named math, initial-wrapper and raw-only mutation results retain distinct scope
and the first missing-observation failure; quantity-token coverage remains
incomplete. Numeric equality with SDK/scalar forces is separate from byte
equality among owned modes. No integration, full qacc, contact, bounded-control
or hardware qualification follows from this force query.

## Animation imports admit complete private output before publication

A4 source readers convert supported glTF/GLB, BVH and wide ARKit CSV into
the shared rig/clip records. FBX uses headless Blender; bounded Unreal body
imports use a trusted compiled read-only SDK and a data-only project. Source
modules are not loaded. Each generated rig, clip and ALB1 is re-admitted
before an owned staging directory is renamed into a new canonical path
below `~/.openagents/`. Existing destinations are refused. Bad input or
failed admission publishes no pack. Receipts retain source/converted/output
hashes, counts and settings; Unreal sequences retain per-asset export hashes.

The source profile is explicitly bounded, including 64 MiB source/output
windows, 512 nodes, 64 clips and 2,000 frames. It does not claim general
nonuniform scale, required-extension or multi-mesh Unreal compatibility.
The frozen A0 Echo/Slay conversions remain qualified: regenerated Unreal
body samples fail exact equality, and A4 does not substitute them in Valley.
The failed qualification, profile limits and verification are recorded in
`docs/verification/animation-lab-a4-2026-11-06/README.md`.


## Lesser Restoration ends one named condition

The spell uses the shared Bonus Action, Touch and slot rules. The Life
Cleric gains it at level 3 without spending a preparation choice. A cast
can remove only Blinded, Deafened, Paralyzed or Poisoned. Removing a
condition clears it from every imposing record and preserves the other
riders, source and deadline of compound effects. An explicit unsupported
choice refuses before payment.

## Aid tracks its Hit Point increase in an owned effect

The caster chooses up to three targets within 30 feet. The shared cast
preflights their effect slots and HP ceilings before spending. Each Aid
record keeps its own deadline. Only the strongest active record raises
current and maximum Hit Points, and its expiry reveals any weaker record
still active. Applying or ending the increase preserves Temporary Hit
Points. The self hotbar control submits a single explicit target.

## Protection from Evil and Good tracks the effect's source

The Devotion Paladin prepares the spell from level 3. The cast requires
a willing Touch target and consumes one qualified holy-water flask.
The ward gives attacks by the six named creature types Disadvantage.
New Charmed and Frightened conditions from those sources are removed
from the attempted effect mask. Existing conditions remain and grant
Advantage on subsequent saves against that effect. A preset change
preserves the used flask in both world adapters.

## Revivify needs a recorded combat death within the last minute

Lethal damage and a third failed death save record the battle time of
death. Revivify admits only a dead creature with that record from the last
60 seconds, within Touch and not behind Total Cover. It consumes the
diamond and slot only on admission, then restores 1 Hit Point. Each world
supplies one diamond a visit, and a preset change does not refill it.

