# NIP adoption candidates for Omega and Sarah

- Class: survey and recommendation
- Date: 2026-07-24
- Status: proposed, not admitted
- Source: `projects/repos/nips` at `db5fe3d`
- Library: `nostr-effect` at `078df46`
- Program: `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. The headline finding

I read the full upstream NIP index and compared it against what
`nostr-effect` implements and what the Sarah workroom program actually uses.

**Every candidate in this document is already implemented in `nostr-effect`.**
Not one requires new protocol work in the library.

So this is not a build list. It is an **adoption gap**. The library is far ahead of the product. The useful
question is which implemented NIPs the product should start using, in what
order, and why.

That also changes the cost of each recommendation. The cost is wiring and a
decision, not a protocol implementation.

## 2. Already committed

These are in the program and need no argument here. The standard set is
NIP-01, 09, 11, 17, 29, 32, 34, 40, 42, 44, 46, 58, 59, 65, 70, 77, 85, and 86.
The custom set is the Buzz family: OA, AA, AE, AM, AO, AP, CW, DV, ER, GS, IA,
PL, RS, WP, and AB. `NIP-LBR` is the labor microstandard.

The list below is what is *not* yet in the plan.

## 3. Tier 1: adopt next

Each of these directly serves work that is already committed.

### NIP-89, recommended application handlers

**The most valuable item on this list.**

The program mints custom kinds: a durable turn record, an authority receipt,
community work units. An unknown client that meets one of those events today
can do nothing useful with it.

NIP-89 is how a kind declares its handler. It lets any client route a user to
Omega when it encounters an event Omega owns. It is the missing half of every
custom kind the program has frozen.

Pairs with NIP-21 `nostr:` URIs for deep links into Omega.

### NIP-B7 Blossom, with NIP-94 and NIP-92

NIP-44 caps a payload at 65535 bytes. Full Auto artifacts do not fit: diffs,
logs, test output, evidence bundles. The parity plan already says to put large
objects in Blossom and the signed manifest in the graph.

NIP-94 supplies the file metadata and NIP-92 the attachment metadata. Together
they are the artifact layer that `OMEGA-FA-10` needs to show a change, a test,
and a verification without inlining them into events.

### NIP-98, HTTP authentication

Sign an HTTP request with a Nostr key.

Three immediate uses: the relay management API, Blossom uploads, and the seam
where an agent authenticates to an OpenAgents HTTP surface.

It is the natural replacement for a bearer token in a system that already has a
signing boundary. It removes a class of credential handling rather than adding
one.

### NIP-45 and NIP-67, counting and completeness

These solve a specific problem the memory audit already flagged as a
correctness issue.

NIP-AE listing is best-effort. Relays may cap results, and a capped result set
silently under-reports. For memory, a recall that quietly sees half the record
is worse than one that fails.

NIP-45 gives a `COUNT` verb, and NIP-67 gives an end-of-stored-events
completeness hint. Together they let a client detect truncation rather than
guess. `nostr-effect` implements both, including relay-side support.

**Recommend making these mandatory for any paged read of memory or work
history**, not optional.

### NIP-66, relay discovery and liveness

We already run bespoke relay monitoring in `relay-health.ts`. NIP-66 is the
standard form of the same thing.

Adopting it means our relay publishes its own liveness as signed events, and
our monitoring consumes a standard others can also read. It replaces custom
code with a protocol, and it matters more once the multi-relay policy in the
parity plan is real.

### NIP-05, DNS identifiers

`sarah@openagents.com` as a verifiable identifier that maps to her public key.

Cheap, and it does real work in the community room. A member can verify that an
agent claiming to be ours actually is, without trusting a display name. Display
names are the weakest identity surface we ship, and this is the standard fix.

### NIP-51, lists

A generic list primitive we will otherwise reinvent several times: admitted
relay sets, member rosters, pinned work, bookmarks, muted actors.

Adopt it before inventing the second bespoke list.

## 4. Tier 2: strong candidates

### NIP-37, draft events

Encrypted drafts.

Sarah's authority already separates drafting from publication: she may draft
blog, document, and Forum content now, while outward publication refuses until
its broker is admitted. NIP-37 is the wire format for exactly that state — a
draft that exists, is owner-readable, and is not published.

This is a closer fit to an existing gate than anything else in Tier 2.

### NIP-23, long-form content

Kind 30023. The standard carrier if Sarah's blog and documents are ever
published to Nostr rather than only through repository delivery.

Not needed today. Needed the moment the web-communications broker lands.

### NIP-22, comment

Generic comments on any event. The natural carrier for review comments on a
work unit or a code proposal, and it composes with NIP-34 patches.

### NIP-56, reporting

The community room needs a report format. The program already states that
reports never enforce an action by themselves and that enforcement happens at
identity and command seams. NIP-56 is the wire format for the report, and it
does not change that rule.

### NIP-62, request to vanish

A user's request that a relay erase their data.

For a semi-public room with outside developers, this is not a nicety. It is the
mechanism a person uses to leave. `nostr-effect` implements it relay-side
already, and we should decide our policy before the first member asks rather
than after.

### NIP-38, user statuses

Agent presence — running, idle, stalled. Feeds the roster in `OMEGA-FA-09`
without inventing a presence protocol.

### NIP-78, application-specific data

Already used underneath NIP-RS read state. Worth naming as the sanctioned
escape hatch for Omega settings that should follow a user across devices,
rather than a new kind each time.

### NIP-7D, forum threads

Kind 11 threads with NIP-22 replies. The standard shape if OpenAgents Forum
topics are ever projected onto Nostr.

The parity plan already says to project into the existing Forum rather than
build a second one. This is that projection format, not a second forum.

## 5. Tier 3: watch, do not adopt yet

- **NIP-47, Nostr Wallet Connect.** The standard agent-payment rail on Nostr.
  v1 pays nothing and Spark is the primary rail, so this is a comparison to
  make when the paid version reopens, not now.
- **NIP-60 and NIP-61, Cashu wallet and nutzaps.** Already noted in the
  program as public recognition only, never the settlement record.
- **NIP-99, classified listings.** Upstream's recommended replacement for the
  retired NIP-15 marketplace. Relevant only if the compute market ever lists
  standing offers.
- **NIP-C0, code snippets.** Kind 1337 with language and filename metadata.
  Plausible for a coding product, unnecessary for the current slice.
- **NIP-13, proof of work.** The standard anti-sybil defense. v2 membership is
  invitation only, so this is the tool for the day we open the room.
- **NIP-53, live streaming and spaces.** Possible fit for a live Sarah episode
  or a public Full Auto run. Speculative.
- **NIP-A0, voice messages.** Sarah has voice history. Not on the current path.

## 6. Deprecations that already touch us

This is the part of the survey with the most immediate value. Upstream now
marks thirteen NIPs unrecommended, and four of them intersect our stack.

| NIP | Upstream status | Our exposure |
| --- | --- | --- |
| NIP-31, unknown events | unrecommended, "unnecessarily bloated" | **The parity plan and the memory audit both instruct implementers to use a NIP-31 `alt` tag for unknown-client fallback.** That guidance is now against upstream advice |
| NIP-96, HTTP file storage | unrecommended, replaced by Blossom | `nostr-effect` implements it, and it was touched during the Cloudflare removal on 2026-07-24. Do not build artifact storage on it. Blossom is the path |
| NIP-EE, MLS messaging | unrecommended, superseded by the Marmot Protocol | `nostr-effect` implements `NipEEService`. Not on our path, but the support list should record that it is superseded |
| NIP-90, data vending machines | unrecommended, "got totally out of control" | Already handled correctly. `NIP-LBR` is a use-case-specific microstandard on kinds 5930 to 5939, which is exactly the upstream remedy |

The NIP-31 item needs a decision rather than a note. The requirement it serves
is real: an unknown client should get a safe, non-secret summary instead of an
opaque blob. Options are to keep using `alt` and accept the divergence, or to
find the current idiom. Either way, both documents should stop citing NIP-31 as
though it were recommended.

NIP-06 is also now unrecommended in favor of a single secret key. Our identity
material should not depend on mnemonic derivation being the blessed path.

## 7. Not adopting

- **NIP-26, delegated event signing.** Unrecommended upstream, and we already
  use narrow capability grants plus NIP-46 remote signing, which is the
  stronger model.
- **NIP-28 and NIP-72.** Both superseded by NIP-29, which the program already
  chose.
- **NIP-15.** Superseded by NIP-99.
- **NIP-03, 04, 08, and BE.** Deprecated or unproven upstream.

## 8. The rule for deciding

Three tests, in order.

1. **Does a standard NIP already express the behavior?** If yes, use it. The
   parity direction says to prefer a standard NIP and not to repeat opaque
   extension growth.
2. **Is it already implemented in `nostr-effect`?** For everything in this
   document the answer is yes, which makes adoption a wiring decision.
3. **Does it earn its wire surface?** A NIP we adopt is a compatibility
   commitment. Tier 3 exists so that we can want something without shipping it.

## 9. Recommended first three

If only three land, take these:

1. **NIP-89**, because our custom kinds are currently unreadable to every other
   client, and that is a self-inflicted island.
2. **NIP-45 with NIP-67**, because a silently truncated memory listing is a
   correctness bug we have already written down and not yet closed.
3. **NIP-B7 Blossom with NIP-94**, because the evidence chain in
   `OMEGA-FA-10` needs somewhere to put artifacts larger than an event.
