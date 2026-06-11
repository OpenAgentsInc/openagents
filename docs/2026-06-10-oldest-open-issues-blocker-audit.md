# Oldest Open Issues: Blocker Audit

Date: 2026-06-10 (late). Scope: the eleven oldest open issues
(#4641–#4658) and their comment threads (~100 comments read). Written
because multiple agents working oldest-first are spending their cycles
re-verifying these issues without being able to advance them.

## The one-paragraph finding

None of the eleven is blocked on missing code. Every thread shows the
same shape: implementation landed days ago, local tests green on every
recheck (17, 24, 71, 20, 11 tests passing again and again), live
endpoints probed repeatedly — and the acceptance evidence cannot exist
yet because it requires something outside the repo: an online Pylon
fleet, a live spend authorization, an external counterparty, or an
owner-side account action. The issues are **evidence-starved, not
work-starved**, and the oldest-first agents are burning their passes on
no-op rechecks (9–11 comments per issue, mostly identical "rechecked
from current main; no change; no close" entries within the same day).

## Issue-by-issue

| Issue | What it needs to close | Actual blocker class |
| --- | --- | --- |
| #4641 compute (kind 5050 paid smoke) | one settled live compute job | **fleet dark**: every recheck shows `pylonsOnlineNow=0`, `sellablePylonsOnlineNow=0`, `pylonsWalletReadyNow=0` |
| #4642 GEPA endpoint + paid settlement | live GEPA smoke w/ dispatch + settlement | **fleet dark** + admin dispatch token absent in the smoke env (`status=partial` by design) |
| #4645 data (first dataset sale) | one settled NIP-DS sale | **counterparty**: listing/offer/redaction all green; needs a real buyer (deps #4639/#4643/#4644 all closed) |
| #4648 labor (first paid overnight job) | one settled labor receipt | **counterparty**: now explicitly hitched to epic #4726's demonstration issue #4732 (independent contributor required) |
| #4651 referrals (first settled payout) | one real referral conversion | **counterparty**: 24 ledger/policy/gate tests green; nobody has converted a referral |
| #4652 five-streams stacking | all five stream counters > 0 on one install | **aggregate of all the above**: every counter is `jobsSettledTotal=0` |
| #4653 tips webhook + refund/reversal | live MDK webhook callback + refund case | **provider-side config/event**: 71 tests green; needs a real MDK webhook delivery and a real refund occurrence |
| #4654 pylon CI release gate | — | **possibly done**: workflow exists, active (ID 293341329), correctly scoped/triggered. The remaining named gap is the npm publish story credential (owner) — recommend close-or-name-the-residual explicitly |
| #4655 Windows/WSL smokes | install smoke on Windows + WSL | **hardware**: no Windows machine reachable on the Tailnet during any recheck |
| #4656 packaged network smoke | — | **possibly done**: `status=passed`, `blockerRefs=[]` on three consecutive runs, now including the executor-replay leg. If acceptance is the passing smoke, this is closeable today |
| #4658 install-to-bitcoin (live_small_sats) | live small-sat spend on a real machine | **spend authorization**: every recheck states "no live spend"; promise `pylon.install_without_wallet_knowledge.v1` red with restore-readiness blockers |

## The four real blockers behind eleven issues

1. **The fleet is dark.** 19 registered Pylons, zero online, zero
   wallet-ready at every probe. This single fact blocks #4641, #4642,
   #4652 and degrades everything else. It is also now the only thing
   between the live Artanis administrator tick (#4701/#4697, deployed
   tonight, recording `skipped: no_eligible_online_pylons` every
   minute) and a fully autonomous dispatch→verify→accept span. **One
   operator machine left online overnight with `pylon go-online` and a
   ready wallet unblocks more than half this list.** rc2's auto-declared
   executor capability and auto-claimed tip readiness (#4711/#4712)
   make that machine dispatch-eligible and tippable by construction.
2. **External counterparties.** A dataset buyer (#4645), a labor
   contributor (#4648 → #4732), a referral conversion (#4651). These
   are recruitment problems, not engineering problems. The Forum now
   has the recruitment surface: the responder answers contributor
   questions in ~71 seconds and Artanis tips from a budget; the labor
   epic (#4726) was created today to manufacture exactly this evidence.
3. **Owner-held authorizations.** The npm publish credential (#4654
   residual), the live_small_sats spend approval (#4658), and the MDK
   dashboard/webhook provider configuration (#4653). Each is minutes of
   owner action; no agent can substitute.
4. **Hardware.** A Windows/WSL machine on the Tailnet (#4655). Offline
   at every check today.

## The process finding

The recheck loop is itself a cost. The oldest-first agents posted
roughly thirty near-identical recheck comments today across these
eleven issues — each one re-running the same green tests and re-probing
the same zero counters. Two cheap conventions would stop the burn:

- **A `blocked-on:` line at the top of each issue** (fleet /
  counterparty / owner-action / hardware), updated only when the
  blocker class changes. An agent seeing an unchanged blocker line
  skips the recheck.
- **Close what is done.** #4656 (and arguably #4654 minus its named
  owner residual) appear complete by their own acceptance text; keeping
  them open invites infinite re-verification. If a residual is real,
  rename the issue to be only the residual.

## Compounding observation

Eight of the eleven converge on the same single event: **the first
machine that stays online**. When it appears, the now-deployed
machinery fires in sequence with no further engineering: the admin tick
dispatches an executor trace (#4701), the worker verifies and accepts
the closeout on the digest predicate (#4697), settlement can ride the
treasury envelope (#4703) or the reliable-tips ladder
(`payments.reliable_tips_sweepable_balances.v1`, green), the stream
counters move (#4641/#4652), and the evolution-loop promise starts its
unattended streak. The oldest issues and the newest systems are blocked
on the same doorbell.
