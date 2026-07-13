# Episode 252: Preemptive Quality Assurance

### Draft Video Transcript

Before recording, have the exact revision-6 MVP ProductSpec open in the
OpenAgents Desktop workroom. Keep a clean checkout and a terminal ready to run
the AssuranceSpec proposal utility, then open the generated companion beside
the ProductSpec. Nothing on screen should imply that a proposal has been
admitted, executed, or verified.

**[00:00] Speaker:** All right, we're back. So we've got the first release
candidate for the new OpenAgents Desktop app getting put together right now,
which is great. In the last video, I showed you this ProductSpec that we're
using to build the first basic version of the app. The agents can take the
acceptance criteria, turn them into a plan, and then actually start working
through the plan.

**[On screen: OpenAgents Desktop with the revision-6 MVP ProductSpec open.]**

**[00:28] Speaker:** Cool. But there is an obvious question here, which is:
when one of these agents says it finished one of these things, how do I know?
Not just, did it make a pull request? Not just, did it write some tests? Does
the product actually do the thing that we designed it to do?

**[00:49] Speaker:** Because agents are very good at doing a bunch of work and
then giving you a very convincing little summary at the end. Everything is
green, everything is done, here are eleven files I changed, blah blah blah.
Okay. According to whom? What did we actually check?

**[01:08] Speaker:** And actually, funny timing here: Gokul, who created
ProductSpec, just added this Evidence Loop idea to the standard, and I think
the boundary he draws is exactly right.

**[On screen: Gokul's ProductSpec Evidence Loop post.]**

> ProductSpec defines intent.
>
> Evidence shows what happened.
>
> Decision Trace records what changed.

**[01:28] Speaker:** So an acceptance criterion can point to the pull request,
the test, the release, whatever implemented it. An eval can point to the eval
run. A success metric can point to the actual dashboard where you see whether
the thing worked. ProductSpec doesn't need to become GitHub or Datadog or a
test runner. It gives all of that stuff a stable place to attach back to the
original product intent. Super good.

**[02:02] Speaker:** But a link to a test still doesn't tell me whether it is a
good test. A test can pass because it tested the mock. A UI can say Fable while
the thing underneath actually ran Sonnet—which literally happened to us in
the last video. An agent gave me a launch command that did not exist. All the
surrounding words sounded correct; the one load-bearing piece was wrong.

**[02:31] Speaker:** So I don't want QA to show up at the end and ask the
implementation what it would like to be tested on. I want to decide what proof
should count while we are designing the product—before the feature gets to
pick the easiest possible test for itself.

**[02:51] Speaker:** We went through basically everything we've built around
quality so far. We have the QA swarm, behavior contracts, product promises,
Eval Suites, browser tests, device tests, property tests, little formal models,
receipts, all this stuff scattered around the repo. There is a lot of good
shit here. What we did not have was one rigorous companion to the ProductSpec
that says how we intend to know.

**[On screen: `docs/assurance/README.md`, then briefly
`docs/assurance/ASSURANCE_SPEC.md`.]**

**[03:21] Speaker:** So for now I'm calling that an **AssuranceSpec**.
ProductSpec says what the product should do. It also gives the evidence a place
to attach. AssuranceSpec says: all right, what would make us believe it? What
environment matters? What's the oracle? How do we try to break it on purpose?
What evidence has to come back, and who is allowed to review it?

**[03:50] Speaker:** That's really the whole idea. There is a lot of detailed
spec work in here—admission, adapters, deterministic manifests, receipts,
versioning, blah blah blah. We don't need to read all that right now. The
important thing is that the proof plan is a real reviewed artifact, not a test
the implementation quietly invented after it was already done.

**[04:13] Speaker:** The working internal name for our thing is **Observer**,
because in StarCraft the Observer reveals hidden stuff. That's what I want
this to do: show me the failure surfaces I cannot see from the pull request. I
thought about Arbiter, but that sounds like it gets to decide, and we already
have an Arbiter. Science Vessel is funny but terrible to say. Overseer sounds
like management software. Observer feels right. Name can change. We're not
shipping Blizzard's little eyeball guy, don't sue me.

**[04:33] Speaker:** And obviously we should use our thing on our thing. This
is the actual OpenAgents Desktop MVP ProductSpec: revision six, exact digest,
eighteen acceptance criteria. Not a toy example. This is what the agents are
working on right now.

**[On screen: the ProductSpec identity, revision, digest, and criteria in the
Desktop workroom.]**

**[04:55] Speaker:** So the first thing I wanted was not a dashboard. I wanted
a boring little program I could actually run. Give it a ProductSpec, optionally
give it the repo, and have it produce the first AssuranceSpec proposal in the
real format.

**[On screen: `packages/assurance-spec/README.md`, then the terminal.]**

**[05:16] Speaker:** Let's just do it with the MVP.

**[On screen: run `assurance-spec propose` against the MVP ProductSpec with
`--repo .` and the co-located output path.]**

**[05:31] Speaker:** All right. Eighteen obligations. Eighteen need design.
Zero ready. Structurally valid. Design ready: no. Execution authorized: no.
Good. That is exactly the answer. It did not read eighteen English sentences,
hallucinate a bunch of Playwright tests, and tell me we're covered.

**[05:53] Speaker:** Let's take a look.

**[On screen: open the generated `.assurance-spec.md` beside the ProductSpec.
Show exact path, revision, digest, criterion IDs, then one generated
obligation.]**

**[06:08] Speaker:** This is bound to the exact ProductSpec bytes. Same
revision, same digest, all eighteen acceptance-criterion IDs. Each one gets a
stable proposed obligation and the exact source claim it came from. If this
ProductSpec changes, we do not let the assurance plan kind of vaguely float
over to the new thing.

**[06:34] Speaker:** And because I gave it the repo, it records the committed
Git tree and the test-looking artifacts and package scripts that exist there.
But—and this is the important part—it does not say any of those tests prove
anything. They are candidates. A filename that happens to contain `CW-AC-04`
is not an oracle. A passing `test` script is not evidence for every criterion.

**[07:02] Speaker:** So now run validation.

**[On screen: run `assurance-spec validate`, then `assurance-spec coverage`.]**

**[07:16] Speaker:** The document is valid. The proof plan is not adequate.
Those are different questions. Every obligation is still missing its domain,
technique, environment, oracle, falsifier, evidence policy, independence rule,
and activation gate. This is a very useful failure report.

**[07:43] Speaker:** The first one we should actually design is intentionally
boring. `CW-AC-04` says, among other things, that executable ProductSpec
criteria need unique, author-visible IDs and duplicate or missing IDs stop
work. We already have a good-case test and a bad-case test for that narrow
piece.

**[On screen: show the existing tests for “the MVP spec is executable with
unique author-visible criteria” and “duplicate criterion IDs refuse executable
admission.” Do not edit the generated AssuranceSpec on camera yet.]**

**[08:07] Speaker:** The tempting thing would be for the generator to notice
those names and wire everything together. No. We have to review the claim.
The oracle could be: the exact MVP exposes all eighteen IDs. The falsifier
could be: inject a duplicate ID and require the validator to reject it. Same
local Bun environment, exact source digest. Then a reviewer can commit that
proof design as a new AssuranceSpec revision.

**[08:40] Speaker:** Even after that, it still would not prove all of
`CW-AC-04`. It doesn't prove the guided conversation creates a valid spec. It
doesn't prove the UI points to the exact broken section. It doesn't prove the
packaged app refuses to start work. It would prove one small contract at the
local-test level. So the right display later is partial support, not
“criterion complete.”

**[09:06] Speaker:** This is also why I want the UI to come second. The UI
should parse and visualize this actual document—proposed, needs design, not
run—not have some hardcoded mock version of the plan that quietly becomes a
second source of truth. We can make it editable later, but the artifact comes
first.

**[09:32] Speaker:** Then we grow it one real obligation at a time. Browser
tests where the promise is in the browser. A real signed Mac app journey where
the promise is about installation. Device tests where the seam is the product.
Maybe formal verification for one small nasty state machine where a bad
transition can ruin your week. Human review where the thing is actually
subjective. The environment is part of the claim.

**[10:03] Speaker:** Agents can absolutely propose that richer proof design.
But model output is a proposal. The agent that wrote the feature does not get
to invent a weak test, run the weak test, and grade its own homework. Admission,
evidence production, verification, owner acceptance, and release are different
steps.

**[10:28] Speaker:** So now we have the first piece: ProductSpec in,
AssuranceSpec proposal out, exact repo context if you give it one, and an
honest list of everything we still have to design. Next we review the first
real obligation, compile it into an immutable run manifest, execute the good
and bad cases, produce a receipt, and attach that receipt back through the
ProductSpec Evidence Loop.

**[10:54] Speaker:** All right. This is cool. We can actually start testing
the QA system against the product while the product is still being built,
which is kind of the whole point. See ya soon!

## On-screen artifacts

- `docs/mvp/openagents-codex-workroom-mvp.product-spec.md`
- `docs/assurance/README.md`
- `docs/assurance/ASSURANCE_SPEC.md`
- `docs/assurance/MVP_FIRST_ASSURANCESPEC.md`
- `packages/assurance-spec/README.md`
- `packages/assurance-spec/src/cli.ts`
- `docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md`
- `packages/product-spec/test/product-spec.test.ts`

The architectural specification, evidence-boundary rules, and implementation
plan remain in `docs/assurance/`. These notes only script the recording.
