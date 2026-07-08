# The Two Numbers

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


## On Autopilot, Accepted Code, and the Path to Compounding Weekly Growth

Date: 2026-06-14

Author: Claude (Opus 4.8)

---

## I. There are only two numbers

Paul Graham has a habit, when he meets a founder, of asking one question
first: what is your growth rate? He asks it because the entire fate of a
company is encoded in two numbers — the rate at which it grows each week or
month, and how long that rate can continue. Everything else is commentary.
A company growing 5% a week for three years ends up somewhere most people
cannot picture, because human intuition is linear and the math is not. You
get the first number by making something people love enough to tell their
friends about. You get the second by being in a market large enough to keep
absorbing that love for years.

That is the whole game, and it is worth stating plainly before we talk about
ourselves, because it is easy to confuse activity with growth. We can ship
features, file issues, run smokes, close milestones, and feel productive
without moving either number. The only work that matters is the work that
either raises the rate at which people adopt and pay for Autopilot, or
lengthens the runway over which that rate can hold. This essay is about how
to do both, specifically, for the product we are actually building, with the
rollout we are actually capable of executing.

The good news, the part that should make us a little reckless with ambition,
is that we are unusually well-positioned on both numbers. We have a credible
path to a high growth rate because we are building something we and our
closest friends desperately want and cannot currently buy. And we have a
credible path to a long duration because the market — reliable agentic work,
priced and settled as accepted outcomes — is not a niche. It is most of the
economy, eventually. The job is to not screw up the connection between the
two.

## II. The growth rate comes from our own kitchen

The most counterintuitive thing Graham says is also the most freeing: the
best startups don't come from looking for startup ideas. They come from
people building something for themselves and their friends, scratching an
itch so specific and so real that it never occurred to them it was a
business. The reason this works is that your own needs, and the needs of the
small circle around you, are a far better signal of future demand than any
amount of market analysis. You are the leading edge. What you reach for and
can't find, everyone will reach for in a couple of years.

We did not have to invent our pain. We live it. There is a short, honest list
of frustrations that the people around this project feel every single day,
and it is the entire reason Autopilot exists:

1. **The limit wall and the account shuffle.** You hit a rate limit
   mid-task, switch to a second account, then a third, keep a note of when
   each one resets, and lose your place every time. The tool fights you for
   the privilege of doing more work.

2. **The tethered laptop.** The agent only runs while you babysit it. You
   carry the machine to dinner so a refactor keeps going. You cannot queue
   work the night before and wake up to progress, because nothing runs
   unattended.

3. **No control from your phone.** You are away from the desk and there is no
   way to approve a step, check status, or get told that a mission finished.
   The most natural device in your hand is useless for the thing you most
   want to check.

4. **Context evaporates at every boundary.** Switch accounts, switch agents,
   switch machines, and the thread of what you were doing snaps. You spend
   the first ten minutes of every session rebuilding situational awareness
   the tool should have kept for you.

5. **No team budget or visibility.** You cannot pool capacity across the
   people you work with, cannot see what a given piece of work cost, and
   cannot set a ceiling on a mission before it runs.

6. **Permission fatigue and no real isolation.** The agent interrogates you
   for access over and over, and there is no clean way to scope it to one
   repo or one dataset — which is exactly the thing a careful person, or a
   regulated team, requires before they will trust it with anything real.

These are not personas invented for a deck. They are the specific complaints
of a specific small circle — the team and the handful of people closest to
it — who already spend their days inside coding agents and already pay for
the privilege. That is the Graham signal in its purest form. We are not
guessing what power users will want. We *are* the power users, a year or two
ahead of the curve, and we are annoyed.

The strategic instruction that follows is almost embarrassingly simple:
**build the thing that makes those six annoyances disappear for us first, and
do not declare any of them solved until we feel the relief in our own daily
work.** Not until a smoke passes. Not until a milestone closes. Until the
person who complained about the account shuffle stops shuffling accounts.
That felt relief is the seed of the growth rate, because it is the only thing
strong enough to make someone interrupt their day to tell a friend.

## III. The thing you tell your friends about is a merged diff

A high growth rate is word of mouth compounding. So the central design
question for the rollout is not "what features ship" but "what is the
shareable moment?" What, exactly, does a delighted user say to the friend
sitting next to them, and what do they show on the screen when they say it?

For a lot of AI products the answer is uncomfortably thin. The shareable
moment is a clever chat response, which is impressive for an afternoon and
forgettable by the weekend. The reason coding is the right wedge — the reason
to start here and not with the grand general-purpose vision — is that code
produces the most concrete shareable moment in all of software: **a change
that got accepted.** A diff that passed the tests, opened a preview, survived
review, and merged. You can point at it. It is in the repository. It is real
in a way a chat transcript never is.

This is why the unit of the business and the unit of virality are the same
thing, and that is not a coincidence — it is the whole bet. We are not
selling tokens, seats, or GPU-hours. We are selling **accepted code**:
work that was defined in advance, executed unattended, graded against a
rubric, and accepted with a receipt that proves it. That unit is verifiable,
which is why we can charge for it honestly — cost plus a thin margin, paid
on outcomes that landed, not on attempts that failed. And that same unit is
demonstrable, which is why it travels. "My agent merged eleven PRs overnight
while I slept, here are the diffs" is a sentence that recruits the next user.
"I talked to a chatbot about my code" is not.

So the rollout has to be engineered, deliberately, to manufacture that
sentence. Every mission should close with an artifact a human is proud to
show: the diff, the tests that ran, the preview that opened, the briefing
that explains what happened and what's next, the cost it took. We should
treat the mission briefing as a marketing object, not just a status report,
because in the early rings of growth it *is* the marketing. The
accepted-code receipt is both the invoice and the testimonial. Design it so
that the moment of acceptance is the moment someone wants to share, and the
growth rate takes care of a surprising amount of itself.

## IV. The rollout is concentric rings, and you do not skip one

Here is where founders most often damage their own growth rate: they launch
to strangers before the product is good enough that the people who love them
would stake their reputation on it. They trade a small number of people who
would have become evangelists for a large number who try it once, find the
rough edge, and quietly never return. You only get to make a first impression
on each person once, and burning the leading edge of your market to hit a
launch date is the most expensive mistake available.

The discipline is to grow outward in rings, and to refuse to widen the ring
until the current one is genuinely delighted and the receipts prove it.

**Ring 0 — Ourselves.** The team is the first workload. Every one of the six
pains has to die here first. The bar is not "it works in a demo," it is "we
have stopped using the old painful way because the new way is better." We
should be running our own real coding missions, overnight, across our own
accounts, controlled from our own phones, on our own repos. The receipts from
this use are the readiness evidence; the public opening waits on them. If we
will not run our own production work through Autopilot, we have no business
asking anyone else to.

**Ring 1 — Our friends.** The handful of developers who already complained to
us. Gabriel, Ben, the people in the group chat. These are not a market test;
they are co-conspirators. They will forgive the rough edges because they want
the thing to exist, and they will tell us the truth because they like us.
Their first accepted PR through Autopilot is the first external proof that
the loop closes for someone who is not on the team. Their second one is the
first proof of repeat use, which is the actual signal of product-market fit —
not the first task someone submits, but the second.

**Ring 2 — People like us.** Codex power users we don't personally know, who
feel the same six pains, found through the same channels we live in. This is
the first ring where word of mouth has to carry weight beyond personal
loyalty. If the shareable moment is real, Ring 1 recruits Ring 2 for us. If
it isn't, no amount of launch noise will substitute, and that is useful
information to get early and cheaply.

**Ring 3 — Design-partner teams.** Small teams who will pay for bounded
workrooms, pooled capacity, and accepted-work quotas, with explicit approval
before anything writes to their repos. This is where the team-budget,
visibility, and isolation pains (5 and 6 on the list) stop being our pains
and start being the buyer's procurement checklist. We charge real money here —
modest monthly plans tied to accepted outcomes, not seats — and the goal of
the ring is a positive gross margin excluding our own labor and a short
human-review time per accepted patch. If the unit economics work at ten
design partners, they work at a thousand.

The reason to be religious about the rings is that each one validates a
different link in the growth chain. Ring 0 proves the product solves a real
pain. Ring 1 proves the pain is not unique to us. Ring 2 proves word of mouth
functions. Ring 3 proves people will pay and the margin survives contact with
reality. Skip a ring and you are guessing about the link you skipped, usually
the expensive way, in public.

## V. Designing the growth rate on purpose

Word of mouth is not luck. It is a rate, and rates can be engineered. A few
principles specific to this product:

**Compress time-to-first-accepted-outcome.** The single most important number
in the early funnel is how long it takes a new user, from sign-up, to see one
piece of their own code accepted by the loop. Every hour of setup friction,
every confusing permission prompt, every "now configure your account" step is
a place where the leading edge of your market leaks out before they ever feel
the magic. Pain point 6 — permission fatigue — is not just a usability
complaint; it is a direct tax on the growth rate, because it sits between the
new user and their shareable moment.

**Make repeat use the default, not a decision.** The same power user always
has another overnight mission, another bounded issue queue, another flaky
test to burn down. The product should make starting the next mission the path
of least resistance — a weekly backlog lane that fills itself, a briefing that
ends with the obvious next action one tap away. Retention is just growth rate
measured from the inside; a user who comes back every week is, mathematically,
the same asset as a new user acquired every week, and far cheaper.

**Instrument the chain, not the vanity.** The numbers that predict growth are
specific: first task submitted, second task submitted, weekly lane activated,
tasks per week, briefings opened within a day, acceptance rate, human-review
minutes per accepted outcome, renewal and expansion. These are the leading
indicators of the two numbers. Total registered users is a lagging vanity
metric that can hide a dying growth rate for months. Watch the chain.

**Protect the magic; charge for the work.** Free should be generous enough
that the shareable moment happens before any wallet opens — the first
accepted diff should land before we ask for a card. Payment should attach to
the accepted work itself, cost-plus and honest, so the pricing reinforces
trust rather than competing with it. People tell their friends about products
that feel like a gift and bill like a utility, not the reverse.

If we hold a real growth rate — and for a product that genuinely kills six
daily frustrations for a circle that is itself the leading edge of a huge
market, a strong weekly rate is not fantasy — then the only remaining
question is the second number.

## VI. How long it can continue

The second number is duration, and duration is set by market size. The thing
to internalize is that we do not have to expand the market by force; we have
to be positioned so the market expands *under* us and we are already standing
on the spot it grows toward.

Coding is the beachhead, not the territory. We start with code because it is
the easiest valuable work to verify — a diff, a test, a preview, a review, an
accept-or-reject. But the unit we are building the business on, the accepted
outcome with a receipt, is not specific to code. It is the general shape of
how machine work becomes contractible, gradable, and settleable. The same
machinery — define the work, scope the capabilities, execute wherever it's
cheapest, grade against a rubric, settle on what was accepted — extends from
merged PRs to every kind of latency-tolerant knowledge work that can be
checked. Each vertical we earn the right to enter is another multiple of
demand for the same core loop.

That is what makes the runway long. We are not betting that coding agents are
a big enough market to make us large, though they may well be. We are betting
that "reliable agentic work, paid for by the accepted outcome" is the unit
the whole economy migrates to, the way it once migrated from the hash to the
token. If that is right, the duration number is measured in years of held
growth, not quarters, because there is always an adjacent territory to expand
into from the beachhead. The constraint on us will be execution and trust,
not addressable demand.

And critically — to borrow Graham's point about not having to cheat — none of
this requires extracting anything from anyone. The growth rate comes from
making developers' lives dramatically better at work they were already trying
to do. The duration comes from a real and expanding need. Both numbers go up
precisely to the degree that we serve people well. There is no version of
this where we win by squeezing; the entire mechanism is that users love the
thing enough to tell their friends, and there are an enormous number of
friends to tell.

## VII. What this means on Monday

Strategy that does not change next week's actions is decoration. So, concretely:

- **Finish killing our own six pains before widening the ring.** Ring 0 is not
  done until the team has genuinely stopped reaching for the old painful
  workflow. Receipts from our own production use are the gate to opening
  Ring 1.

- **Treat the first accepted PR for each new person as the only launch metric
  that matters.** Not sign-ups. Not waitlist size. Time-to-first-accepted-
  outcome, and then time-to-second.

- **Make every mission end in a shareable artifact.** The briefing and the
  accepted-code receipt are marketing objects. Design the moment of
  acceptance to be the moment someone wants to show a friend.

- **Pick one growth rate and watch it weekly.** Probably accepted outcomes per
  active user per week, or week-over-week repeat missions. Put it on a wall.
  Let it judge the work. Everything that doesn't move it is, for now,
  decoration.

- **Charge on accepted work, generously free until the magic lands.** Let the
  shareable moment happen before the wallet opens; let payment attach to
  outcomes that landed.

- **Refuse to skip a ring to hit a date.** The leading edge of this market is
  small and irreplaceable. We get one first impression each. Spend them when
  the product earns the evangelism, not before.

The two numbers are the whole story. We have an unusually honest shot at a
high growth rate, because we are building, for ourselves and our friends, the
escape from six frustrations we feel every day — and the relief, expressed as
an accepted diff, is the most shareable object in software. We have an
unusually long runway, because accepted agentic work is where the economy is
going, and code is just the door we walk in through. The job is not to be
clever. The job is to make the six pains disappear, make the moment of
acceptance worth showing a friend, and grow outward one delighted ring at a
time — and then let exponential growth do the thing that, to people who don't
do the math, always looks impossible.
