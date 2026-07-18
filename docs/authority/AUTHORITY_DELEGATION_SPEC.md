# AuthorityDelegationSpec 0.1

- Class: normative design proposal
- Status: proposed format; root profile admitted separately by current owner direction
- Date: 2026-07-18
- Owning product intent:
  [`../../specs/openagents/authority-delegation.product-spec.md`](../../specs/openagents/authority-delegation.product-spec.md)
- First profile: [`../../AUTHORITY.md`](../../AUTHORITY.md)

## Purpose

ProductSpec says what outcome OpenAgents intends. AssuranceSpec says what proof
would justify confidence. FastFollowSpec says what outside systems and lessons
may generate candidate work. None says who may take an action, against which
resource, under what conditions, or for how long.

AuthorityDelegationSpec fills that gap. It is a typed, revocable grant from an
authority holder to operating roles. It exists to let agents finish work
without turning ordinary access, evidence, a plan, a stale blocker, or a model
recommendation into implied permission.

The format is designed for autonomous operation, not owner-attention theater.
An authorized agent uses the least-powerful available route, records receipts,
and continues through a deterministic blocker-resolution ladder. It asks the
owner only when the remaining action is reserved, inherently human, outside a
budget, or impossible with every admitted route.

## The four companion contracts

| Contract                | Question it answers                               | It cannot provide                                           |
| ----------------------- | ------------------------------------------------- | ----------------------------------------------------------- |
| ProductSpec             | What and why should be built?                     | execution, proof, release, or authority                     |
| AssuranceSpec           | What evidence would justify confidence?           | product intent, self-admission, or action authority         |
| FastFollowSpec          | What external lessons may produce candidate work? | target mutation, adoption, or promotion authority           |
| AuthorityDelegationSpec | Who may do what, where, under which conditions?   | new intent, sufficient evidence, or authority amplification |

The contracts compose by reference and intersection. A grant can authorize an
action required by a ProductSpec, but it cannot make the ProductSpec true. A
grant can designate an independent AssuranceSpec reviewer, but it cannot make
missing evidence pass. A grant can admit a Fast Follow implementation program,
but it cannot turn an upstream source into an instruction.

## Design laws

1. **No self-amplification.** A delegate cannot add actions, resources, roles,
   budgets, credentials, environments, or exceptions to its own grant.
2. **Higher authority wins.** System and current owner instructions, law,
   repository invariants, resource policy, and exact runtime gates bound every
   profile. Composition is intersection, never union. An explicit deny wins.
3. **Exact resources and actions.** Access to a repository, cloud project,
   device, provider session, or secret does not imply every action on it.
4. **Conditions travel with the grant.** Budget, environment, evidence,
   rollback, redaction, lease, and time bounds are part of authority, not
   optional implementation advice.
5. **Least power first.** Prefer read-only inspection, existing authenticated
   application state, documented automation identities, staging, reversible
   mutations, and narrow target substitution before broader power.
6. **Evidence is not authority.** A receipt may satisfy an assurance
   obligation; it never independently grants deploy, release, spend, public
   claim, custody, or settlement authority.
7. **Authority is not evidence.** Permission to run a check or promote a
   release does not prove it passed or make an artifact releasable.
8. **Separation of duties is explicit.** Producer, evidence producer,
   verifier, admitter, releaser, and public-claim roles are distinct whenever
   the bound AssuranceSpec or profile requires independence.
9. **No waiting as a state.** A blocker invokes the resolution ladder. Only a
   terminal reserved-action receipt may enter `needs_owner`; all other
   blockers become workaround, substitution, implementation, narrowed claim,
   or another admitted packet.
10. **Revocation is monotonic.** A current owner instruction, profile
    replacement, budget exhaustion, invariant failure, security incident, or
    explicit revocation stops new actions immediately. Already-started work
    must reach the safest bounded checkpoint and emit a receipt.
11. **Receipts are bounded and redacted.** Record the decision, grant, target,
    condition evaluation, outcome, and evidence refs; never record raw secrets
    or unbounded private content.
12. **Public claims stay evidence-bound.** Delegation may authorize a typed
    promise transition only when its existing verification gates are green. It
    never authorizes unsupported marketing copy.

## Authored document

An authored profile is Markdown with YAML frontmatter and strict-JSON fenced
blocks. Format 0.1 requires these frontmatter fields:

- `authority_delegation_format_version`
- `authority_profile_id`
- `authority_revision`
- `title`
- `lifecycle_state`: `proposed`, `admitted`, `suspended`, `revoked`, or
  `superseded`
- `admitted_by`
- `effective_at`
- `expires_when`

It requires these blocks:

- `authority-delegation-order`
- `authority-delegation-programs`
- `authority-delegation-grants`
- `authority-delegation-conditions`
- `authority-delegation-independence`
- `authority-delegation-escalation`
- `authority-delegation-reserved`
- `authority-delegation-receipts`

Every program and grant has a stable ID. Grants name exact action classes,
resource selectors, applicable programs, and condition refs. Conditions name
numeric budgets and fail-closed predicates. Reserved actions use stable
categories so tooling can prove that a grant does not overlap an explicit
deny.

## Authority resolution

Before mutation, an executor resolves the effective decision in this order:

1. establish the actor and exact requested action;
2. resolve the target resource and environment without secret disclosure;
3. reject a suspended, revoked, superseded, expired, or unknown profile;
4. find a grant that matches actor role, action, resource, and active program;
5. intersect that grant with higher instructions, invariants, resource policy,
   issue/claim ownership, runtime gates, and referenced Product/Assurance/Fast
   Follow contracts;
6. reject any reserved-action match or unmet condition;
7. acquire required claim, lease, approval digest, or generation fence;
8. execute the narrow action;
9. emit its bounded authority receipt and any separately required evidence;
10. re-resolve before every promotion, deploy, spend, destructive action, or
    public-claim transition.

An unresolved selector fails closed. A broader grant never substitutes for a
narrower missing condition. Owner access and operating-agent access are not
interchangeable identities.

## Blocker-resolution ladder

An executor must exhaust this ordered ladder before creating an owner action:

1. **Verify the blocker.** Inspect live state, exact logs, current issue/claim,
   current credentials-as-state, and the named target. Stale prose is not a
   blocker.
2. **Use existing authority.** Use a documented automation service account,
   current app session, existing provider login, repository tool, API, or
   signed local identity without extracting its secret material.
3. **Use the product surface.** Drive a typed API or visible UI, including safe
   browser/computer control, when the action is already granted and the UI is
   the only available transport.
4. **Route around the dependency.** Substitute an admitted GCP worker, owned
   device, provider lane, build host, staging environment, or equivalent proof
   rung that preserves the target contract.
5. **Build the missing adapter.** When access exists but the automation seam is
   absent, implementing that seam is work—not an owner gate.
6. **Repair or reprovision.** Restart, replace, or reprovision an admitted
   ephemeral resource within budget and rollback conditions.
7. **Narrow honestly.** Complete all unaffected packets and narrow the current
   claim to the strongest actually proven tier. Never upgrade a substitute
   into stronger evidence.
8. **Escalate only the irreducible action.** Create one exact UI-first owner
   action only for a reserved/inherently human act, inaccessible external
   identity, or budget expansion. Continue all independent admitted work.

`waiting`, `owner unavailable`, `device unavailable`, and `credential maybe
missing` are not terminal dispositions.

## Independence

An admitted profile may designate operating roles as independent reviewers or
AssuranceSpec admitters when the source authority explicitly grants it. That
designation does not waive independence:

- the same execution identity that authored an obligation cannot verify or
  admit that obligation;
- the same evidence-producing run cannot impersonate a verifier;
- deterministic validators may supply objective observations but cannot make
  judgment calls outside their encoded contract;
- a separate clean session, separately claimed reviewer packet, or named
  deterministic verifier must reproduce the relevant evidence;
- release remains separately gated even after assurance admission.

The root profile is an owner designation. It permits distinct operating-agent
reviewers to perform independent review only where the bound AssuranceSpec
already accepts an `owner_designated_independent_reviewer`. It does not let a
producer relabel itself as independent.

## Reserved categories

Every profile must reserve at least:

- raw secret, credential, signing-key, mnemonic, or token extraction or
  disclosure;
- treasury, wallet, custody, payout, payment, settlement, or irreversible
  financial movement;
- legal contracts, employment decisions, tax/regulatory attestations, or
  representations requiring a natural person;
- destructive production customer-data deletion or irreversible migration;
- identity, biometric, platform-terms, or account-recovery ceremonies that
  require the human account holder;
- spend above the exact profile cap;
- weakening a security, privacy, custody, evidence, or repository invariant;
- unsupported public claims or fabricated evidence; and
- modifying the profile to increase its own authority.

Profiles may reserve more. They may not reserve less by omission; format 0.1
validation treats missing mandatory categories as invalid.

## Relationship to owner-action tracking

`NEEDS_OWNER.md` is an exception queue, not the operating queue. A valid entry
must name the exact reserved category or failed external identity, the ladder
steps attempted, the smallest UI-first owner action, what remains in motion,
and the receipt that will close it. Broad entries such as “authorize device,”
“check release,” or “review when available” are invalid.

## Bootstrap and implementation path

Format 0.1 deliberately starts as an authored profile plus deterministic
repository validation. The implementation sequence is:

1. validate the root profile and its references;
2. use it as the standing operating contract for the admitted program;
3. project effective grants into Full Auto work/turn records and issue claims;
4. add a typed Effect authority service and receipt schema;
5. enforce grants at GCP, release, device, provider, SCM, and public-promise
   adapters; and
6. add revocation, budget, liveness, and independence model checking.

Until the Effect service lands, current instructions, repository policy,
claims, runtime gates, and this validated root profile are jointly enforced.
The absence of a compiler is never permission to exceed the authored profile.
