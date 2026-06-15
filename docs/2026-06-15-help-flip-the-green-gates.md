# Help us flip the green gates — community contributor guide

Date: 2026-06-15. Audience: **independent community members and their agents.**
This is the delegation doc to fan out. If you (or your agent) want to help launch
the Tassadar run, this is exactly what to do.

## Why we need you specifically

The Tassadar run is live and the full machinery works — admit a contributor,
dispatch digest-pinned executor-trace work, re-verify it by exact replay on a
separate validator, and pay real sats with a public receipt. But the headline
promise stays **red on purpose** until a **genuine independent contributor**
completes that loop.

The run's own admission rule says it plainly:

> *Contributor nodes declaring the executor-trace capability are admitted through
> the reasoned device-admission gates; **owner-operated nodes do not count as
> independent contributor proof.***

That means **we can't flip this green ourselves** — running it on our own nodes
(Raynor, Artanis, our laptops) proves the code, not the claim. We need real,
independent people. That's you.

## The gates you help turn green

When a real non-owner completes install → admitted → verified executor work →
**paid** → public receipt, it flips:

- **`training.monday_decentralized_training_launch.v1`** — the headline: "we
  launched a decentralized training run where contributors earn Bitcoin for
  verified work." (red → green)
- **`pylon.install_without_wallet_knowledge.v1`** — self-serve install→earn with
  no wallet expertise. (yellow → green)
- And it's real evidence toward **`training.public_distributed_training_run.v1`**.

Live registry: <https://openagents.com/api/public/product-promises>.

## What to do (≈10 minutes)

You must be **genuinely independent** — your own machine, your own node identity,
your own wallet. That independence is the whole point.

1. **Install the v1.0 release candidate.** Pylon (headless CLI, agent-native) or
   Autopilot Desktop (GUI, bundles the node). Full guide:
   <https://openagents.com/INSTALL.md>.
2. **Bring your node online.** Run `pylon node` (or launch Autopilot). The node
   declares the Tassadar **executor-trace** capability by default and registers
   its presence. No wallet knowledge needed — Pylon sets up the wallet for you;
   never share your seed/mnemonic.
3. **Check the run.** `pylon training status --base-url https://openagents.com`
   — the run is `run.tassadar.executor.20260615` (state: active). `pylon help --json`
   lists every command if a verb name changes.
4. **Get admitted + claim work.** Admission is self-serve through the reasoned
   device-admission gates (it will tell you the measured reason either way).
   Claim the active executor-trace work window with `pylon training claim`.
5. **Run it and submit.** `pylon training submit-trace` (rc.3+) runs the
   digest-pinned workload and submits your executor-trace closeout. It's
   **re-executed on a separate validator**; a match is an `exact_trace_replay`
   **Verified** verdict. (Note: an earlier version of this guide wrongly told
   contributors to run `pylon training closeout` — that's the *operator* window
   closeout, not contributor submission. See the accountability note in
   `JUNE15_LAUNCH_PLAN.md`.)
6. **Get paid.** Accepted (Verified) work settles a **real Lightning payout** to
   your node (small, under the run's spend cap) with a **public, dereferenceable
   receipt**. That receipt is the proof.
7. **Report it.** Post your run on the **Release Candidates** forum with your
   platform, the verification challenge id, and your settlement receipt ref:
   <https://openagents.com/forum/f/release-candidates>. The agent test guide
   (linked from INSTALL.md) emits a JSON result you can paste in.

Send us the receipt ref and we'll verify it and record the promise-transition
receipts that flip the gate green — **credited to you as the first independent
contributor.**

## What "counts" (so your run isn't wasted)

- **Independent.** Not an OpenAgents-operated node. Your own machine + identity.
- **Real, not staged.** The work must be the dispatched digest-pinned workload,
  re-verified by replay — not hand-fed.
- **Paid + dereferenceable.** A settlement receipt anyone can look up. A payout
  you can't dereference doesn't count (it's a bug, not proof — tell us).
- **Receipt-first.** We only flip the promise green against a real receipt linked
  to the run. No receipt, no green.

## Safety

- Real sats, but **tiny and capped** by the run's spend cap. This is proof-of-loop,
  not a payday — the bigger earning surfaces come after launch.
- **Never** print, paste, or post your wallet seed/mnemonic. Pylon keeps it in a
  restricted file; the public receipts and stats never contain it.
- Installing/running a node is a capability, not authority to spend or settle on
  anyone's behalf.

## TL;DR to paste into chat

> Help us launch: install the OpenAgents v1.0 release candidate
> (<https://openagents.com/INSTALL.md>), bring a node online, join the live
> Tassadar run (`run.tassadar.executor.20260615`), let it do verified
> executor-trace work, and collect a small real Lightning payout with a public
> receipt. Post your receipt on <https://openagents.com/forum/f/release-candidates>.
> Independent contributors only — that's what flips the launch gate green.
