# Referrals and promo credit

Status: **spec, 2026-11-27. Nothing is built or deployed.**

Owner's ask: invite codes with free credit, a share of invitees' top-ups for
the inviter, a reward for connecting GitHub, maximum virality, minimum sybil
abuse.

## 1. Summary

One system, not two. Every account gets a personal **referral link** that
never runs out. The link *is* the invite code; there is no separate scarce
"invite code" product. Admins can also mint **campaign codes** (events,
creators, partners) from the same table.

| Event | Who is paid | Amount | Gate |
| --- | --- | --- | --- |
| Connect GitHub | The account | $5 promo credit | Eligible GitHub identity (§4.1), once per GitHub identity and once per account |
| Redeem a referral code | Invitee | $10 promo credit | Eligible GitHub identity; redeemed inside the window (§3.3) |
| Invitee activates | Inviter | $5 promo credit | Invitee redeemed and was active on 2 separate days (§3.5); inviter has an eligible GitHub identity; rolling cap (§4.3) |
| Invitee tops up | Inviter | 1% of the top-up, as credit | Settled paid top-ups only; for the life of both accounts; no cap |
| Post the link on X / Nostr | Nobody | $0 | Not paid (§6) |

So an uninvited person who connects GitHub gets **$5**; an invited person
gets **$15**. That 3x gap is the reason to go and find a code, and the reason
a code is worth handing out.

The pitch on every share surface: **"Give $10. Get $5, plus 1% of what they
spend, forever."**

GitHub is the single gate for every reward on both sides. A code can be
shared, clicked and redeemed without GitHub, but nothing is granted until
the account holds an eligible GitHub identity. Ungranted rewards show as
**pending** with a "Connect GitHub to claim" button; pending is itself a
conversion hook.

## 2. Naming

"Invite" is taken. `access.c`, `/invite#code=`, `openagents access redeem`
and `openagents admin invite` are the realm-making gate
([realm invites](../auth/2026-10-31-realm-invites-and-auth.md)). To keep the
two apart:

- New identifiers, tables, routes and wire fields say **referral**
  (`referral.*`, `/api/v1/referral/*`, `/r/<code>`).
- Product copy may say "Invite friends". It never says "invite code" for
  this feature; it says "your link" or "referral code".
- The balance granted here is **promo credit**. Paid balance stays
  **credit** (`inference.credit`).

Add "Referral", "Referral code", "Promo credit" and "Kickback" to
[the glossary](../books/glossary.md) in the first build commit.

## 3. Product

### 3.1 Codes and links

- Every account gets one personal code at creation: 8 characters of
  Crockford base32, case-insensitive, no profanity (reroll on a blocklist
  hit). Link form: `https://openagents.com/r/<code>`.
- Once an eligible GitHub identity is connected, the account may switch its
  link to its GitHub handle: `openagents.com/r/<handle>`. The handle is
  proven by OAuth, so there is no squatting. The random code keeps working.
  If the handle later changes on GitHub, the old vanity link keeps resolving
  to the same account until another verified account holds that handle.
- A personal code has unlimited uses and does not expire.
- Campaign codes (`openagents admin referral new --uses N --days N --note T
  [--bonus-micros N]`) have a use limit, an expiry and an optional different
  invitee amount. They have no inviter, so they pay no inviter bonus and no
  kickback. Only the hash of a campaign code is stored, like realm invites.

### 3.2 The invited landing

`/r/<code>` renders, before any login:

- The inviter's GitHub avatar and handle if they have connected GitHub and
  left "show my name on my link" on (default on); otherwise "A friend".
- "**@handle gave you $10 of OpenAgents credit.**" and one button:
  **Continue with GitHub**. Other login methods sit below it. Choosing
  GitHub login creates the account, satisfies the gate and claims the $15 in
  one step; this is the fast path and it is the default.
- Unknown, revoked or exhausted codes render the normal landing with no
  error wall; the person can still sign up and earn the $5.

### 3.3 Attribution

- Landing on `/r/<code>` stores the code first-party for 30 days. Last code
  seen wins until an account exists.
- At account creation the stored code is bound to the account. An account
  with no stored code may type one in Settings within **7 days** of
  creation. Accounts that exist on launch day get 7 days from launch.
- One inviter per account, written once, never changed.
- Refused, with a plain reason: your own code; a code owned by an account
  that shares your GitHub identity; a reciprocal pair (A redeemed B's code,
  so B cannot redeem A's); a redemption outside the window.
- Binding an inviter does not grant anything. Grants follow §3.5.

### 3.4 Where people see it

1. **Settings → Invite** (web, Desktop, iOS): the link, copy button, share
   buttons (X, Nostr, system share sheet) with prefilled text, and the
   dashboard: clicks, signups, pending, activated, credit earned from
   bonuses, credit earned from kickbacks.
2. **Right after the $5 GitHub grant**: "You got $5. Give friends $10 and
   get $5 each." This is the highest-intent moment.
3. **Low or empty balance**: next to "Top up", "or invite a friend for $5".
4. **After a top-up settles**: "Friends you invite earn you 1% of theirs."
5. **CLI**: `openagents referral` prints the link and the same counts;
   `openagents referral redeem CODE` binds a code.
6. **README badge** (§3.6).

The inviter's dashboard shows each invitee as a status only (pending,
activated) plus the GitHub handle when the invitee left "let my inviter see
I joined" on at redemption. It never shows an invitee's top-up amounts;
kickbacks appear as one running total and a per-day sum.

### 3.5 Grant rules

A reward moves `pending → granted` when its gate holds, and `pending →
void` when it can no longer hold. Every transition is one ledger row.

| Grant | Becomes pending | Becomes granted | Voids |
| --- | --- | --- | --- |
| `github_connect` $5 | Account created | Eligible GitHub identity connected | GitHub identity already used for this grant; account frozen |
| `referral_invitee` $10 | Code bound | Eligible GitHub identity connected | Same, or 30 days pass with no eligible GitHub |
| `referral_inviter` $5 | Invitee's `referral_invitee` granted | Invitee has completed model turns on 2 different UTC days within 30 days of redeeming, and the inviter holds an eligible GitHub identity | 30 days pass; either side frozen; inviter over the cap (§4.3) |
| `kickback` 1% | Invitee's paid top-up settles | Immediately, in the same transaction as the top-up's credit, if the inviter holds an eligible GitHub identity; otherwise held pending for up to 90 days | 90 days pass with no eligible inviter GitHub; either side frozen |

Kickback details:

- Base: the settled amount of a paid top-up (`inference.topups.settled`),
  nothing else. Promo credit, grants and admin adjustments are never a base.
- Amount: `floor(top-up × 100 / 10000)` in the top-up's own unit, converted
  to promo-credit units at the rate recorded for that top-up.
- One level only. An inviter earns from people they invited, never from the
  people those people invite.
- No expiry on the relationship and no cap on the total. It is paid out of
  revenue that already arrived.

### 3.6 GitHub-native loop

The audience lives on GitHub, so the link should too.

- Settings → Invite offers a Markdown badge, "Built with OpenAgents", that
  links to the account's `/r/<handle>`.
- Agent-authored commits and pull requests made through OpenAgents may carry
  the account's link in the existing trailer/footer line, **off by default,
  one toggle to turn on**. Never added silently.
- Public share pages for chats and worlds carry the owner's link on the
  "Try it" button when the owner opted in.

### 3.7 Milestones

Activated-invitee counts of 5, 25 and 100 grant a one-time $10, $50 and
$250 promo credit and a profile mark. These count toward the programme
budget (§4.5) and obey the same GitHub gate. Phase 3; ship the core loop
first.

## 4. Sybil resistance

The design goal: a fake account must cost more to make than it yields, and
what it yields must be hard to turn into money.

### 4.1 Eligible GitHub identity

A GitHub identity is one GitHub numeric user id, proven by OAuth through
either GitHub login or the Settings → Account GitHub connection. It is
**eligible** when all of these hold at check time:

1. The GitHub account was created at least **90 days** ago.
2. It shows real use: at least one of — 3 or more public repositories that
   are not forks; 5 or more followers; a verified organisation membership
   that is public.
3. Its id has never been used for a grant by another OpenAgents account.
4. It is not on the deny list.

The thresholds are config (§5.3), not code, so they can be tightened in
minutes during an attack.

Uniqueness is permanent. `referral.github_claims` keeps the GitHub id for
every grant it backed, and that row survives a disconnect, an account
deletion and a reconnect elsewhere. Disconnecting GitHub and connecting it
to a second account earns nothing the second time. (The row holds a keyed
hash of the id when the owning account is deleted, so deletion still removes
the readable identifier.)

An identity that fails rule 1 or 2 is not refused; the reward stays pending
and the page says what is missing ("Your GitHub account is 41 days old;
rewards unlock at 90"). It unlocks by itself on a later check.

### 4.2 What promo credit can and cannot do

- **Not money.** Promo credit is a discount on our own service. It cannot
  be withdrawn, transferred, gifted, refunded or used to pay a maker, an
  upgrade or any other account. This keeps the Bitcoin-only rule intact:
  the only money in the system is still the sats a person pays in.
- **Held in its own balance**, in USD micros (the unit model prices already
  use), separate from paid `inference.credit` in msat. A BTC price move
  never changes what a grant is worth.
- **Spends first.** A call draws promo credit before paid credit, oldest
  expiry first.
- **Promo-eligible models only.** A config list names the models promo
  credit may pay for. At launch: the default OpenAgents Cloud chat model
  and the router decision model. Everything else needs paid credit.
- **Expires.** Granted promo credit expires **90 days** after its grant.
  Kickback credit expires only after 12 months with no account activity.
- **Rate-limited.** Promo-funded calls run on the standard free-tier rate
  limits, and an account may spend at most **$5 of promo credit per UTC
  day**. A farm cannot turn a pile of grants into a resold high-volume
  endpoint.
- **Counts toward the existing daily spend limit** like any service credit
  ([wallets and custody](../auth/wallets-and-custody.md)).

### 4.3 Caps and velocity

| Limit | Default | Applies to |
| --- | --- | --- |
| Inviter flat bonuses | 20 per rolling 30 days | `referral_inviter`. Over the cap the invitee still gets $10; the inviter's bonus voids with reason `cap` |
| Trusted inviter | cap raised by an admin, per account | Creators and partners we know |
| Redemptions per code per hour | 50 | Anything above queues as pending for review, not refused |
| Grants per day, whole programme | config | §4.5 |
| Kickback | none | Funded by settled payments |

### 4.4 Signals and response

Recorded per redemption: account age at redemption, GitHub age and counts
at check time, a coarse network prefix, a device hint, and time from click
to redemption. These feed review; none of them alone blocks a person.

- **Automatic hold** (rewards stay pending, the account works normally):
  more than 5 redemptions of one code from one network prefix in a day;
  GitHub identities created within the same hour as each other; invitees
  who spend the full grant in one burst and never return.
- **Bot protection**: turn on the auth provider's bot and abuse screening
  for signup when the owner enables it (already listed as deferred in
  `NEEDS_OWNER.md`). The referral system does not depend on it.
- **Clawback**: an admin may void pending rewards and remove *unspent*
  promo credit from accounts in a confirmed ring. Spent credit is not
  charged back; paid credit is never touched.
- **Freeze**: a frozen account earns and spends no promo credit.
- Every admin action is one ledger row with the admin's key and a reason.

### 4.5 Budget and kill switches

- A config row holds the programme's total budget and a per-day grant
  ceiling, both in USD micros. Flat grants (`github_connect`,
  `referral_invitee`, `referral_inviter`, milestones) draw from it;
  kickbacks do not.
- When the day's ceiling is reached, new grants stay **pending** and are
  granted, oldest first, when the next day opens. Nobody sees an error;
  they see "Your $10 arrives within 24 hours".
- When the total budget is reached, the same happens and admins are
  alerted. The amounts or the budget are then changed by a person.
- Flags, each independent: `referral_redeem`, `referral_grants`,
  `referral_kickback`, `github_connect_grant`. Off means pending, never
  lost.
- Amounts are versioned (`terms_version`). A grant records the version it
  was made under; lowering an amount never changes a reward that is already
  pending for a bound invitee.

## 5. Design

### 5.1 Data

A new `referral` schema, next free migration number at build time. Sketch:

```sql
referral.codes        (code_hash PK, code_hint, kind 'personal'|'vanity'|'campaign',
                       owner_account NULL, uses_left NULL, expires NULL,
                       invitee_micros NULL, revoked, created)
referral.attributions (invitee_account PK, inviter_account NULL, code_hash,
                       bound_at, source 'link'|'typed'|'cli', signals jsonb)
referral.github_claims(github_id_or_hash PK, account, first_granted)
referral.grants       (id PK, account, kind, state 'pending'|'granted'|'void',
                       micros, terms_version, source_ref, reason,
                       created, decided, expires,
                       UNIQUE (kind, account, source_ref))
referral.config       (one row: amounts, thresholds, caps, budget, ceilings,
                       promo-eligible model list, terms_version)
inference.promo_credit(account, grant_id, micros_left, expires)
```

- `UNIQUE (kind, account, source_ref)` is the idempotency key. `source_ref`
  is empty for once-per-account grants, the invitee account for
  `referral_inviter`, and the top-up's payment hash for `kickback`. A
  retried request or a second API instance cannot grant twice.
- Every function that checks a condition and then grants takes one advisory
  lock covering both, per account, as
  [plan 002](../plans/002-serialize-credit-purchases-and-the-first-tip-bonus.md)
  requires for money paths. The budget draw locks the config row.
- The kickback row is written in the same transaction that settles the
  top-up and adds its credit.

### 5.2 Rules in Coax

The decisions live in one `#authority` suite, `pay/referral-policy.coax`,
written check-first and mutated to zero survivors:

- `rp_eligible(github, config, now)` → eligible, or the first missing rule.
- `rp_bind(invitee, code, now)` → bind, or the refusal reason.
- `rp_grant(kind, account, state, config, now)` → the next state.
- `rp_kickback(topup_msat, rate)` → micros, with `#forall` checks: never
  negative, never more than 1%, zero for a zero top-up, monotone in the
  top-up.
- `rp_spend_order(promo_rows, paid_msat, cost, model, now)` → what each
  balance pays, with `#forall` checks: promo is never spent on a model off
  the list, never spent past expiry, never past the daily promo limit, and
  the two parts always sum to the cost.

C in `coax/web/api/` only moves bytes and runs SQL.

### 5.3 API

All under `/api/v1/referral`, account-authenticated except the first.
Shapes are registered in `proto/registry.json` before the routes land.

| Route | Does |
| --- | --- |
| `GET /r/<code>` (page) | Stores the code, renders the invited landing |
| `GET /api/v1/referral` | The account's link, counts, pending and granted rewards, totals |
| `POST /api/v1/referral/redeem {code}` | Binds an inviter (§3.3) |
| `POST /api/v1/referral/vanity {on}` | Switches the link to the GitHub handle |
| `POST /api/v1/referral/recheck` | Re-runs GitHub eligibility now (also runs daily for pending rewards) |
| `GET /api/v1/account` (existing) | Adds `promo_micros` and the nearest expiry beside `credit_msat` |
| Admin | `referral new|list|revoke`, `referral hold|void|clawback ACCOUNT`, `referral cap ACCOUNT N`, `flag ...`, `referral config` |

Balance display: Settings shows paid credit and promo credit as two lines,
promo with its nearest expiry date. One blended number would hide the
expiry and the model restriction.

### 5.4 Privacy

- The GitHub fields read for eligibility are the public profile only (id,
  login, created date, public repository and follower counts, public
  organisations). No repository contents, no private data, no extra scopes
  beyond what the connection already asks for.
- Network prefix and device hint are kept 90 days, then dropped.
- An inviter never learns an invitee's email, spend or top-up amounts.

## 6. Paying for posts: no

"$5 to tweet your code" is left out on purpose:

1. It is the cheapest reward to farm. A throwaway social account costs
   nothing, and verifying a post means paying for and depending on a
   third-party API.
2. A paid post is an endorsement that must be disclosed as paid in most
   places. Undisclosed ones are our liability; disclosed ones read as ads
   and convert badly.
3. It pays for the act of posting. The $5 inviter bonus and the 1% pay for
   the result, which is the same money aimed at the thing we want.

What we do instead: make sharing one tap with good prefilled text and a
link preview card that shows "@handle gave you $10", and let the inviter
bonus do the paying. If we later want a posting reward, the shape that
resists farming is a campaign code given to specific people, not an open
bounty.

## 7. Terms the product must state

- Promo credit has no cash value, cannot be transferred or withdrawn, and
  expires as shown.
- One GitHub identity earns each reward once. Accounts made to collect
  rewards may have unspent promo credit removed.
- Amounts can change for future rewards; rewards already pending keep their
  amount.
- The 1% is paid as credit on settled top-ups by people you invited, for as
  long as both accounts stay in good standing.

A short public page (`/referrals`) carries these in plain words, linked
from every share surface.

## 8. Metrics

Tracked from day one, per day and per cohort:

- Funnel: link clicks → accounts → GitHub connected → first turn →
  activated (2 days) → first paid top-up.
- K-factor: activated invitees per activated account, per 30 days.
- Cost per activated account and per paying account (flat grants only).
- Promo credit granted, spent, expired; share of promo spend that is
  followed by a paid top-up within 30 days.
- Held, voided and clawed-back rewards as a share of all rewards.
- Kickback paid as a share of top-up revenue (must read 1% or less).

Stop conditions, checked weekly: voided-plus-clawed-back above 15% of
grants, or cost per paying account above the agreed ceiling. Either one
means tighten §4.1 thresholds or lower amounts before spending more.

## 9. Build order

| Phase | Ships | Done when |
| --- | --- | --- |
| 0 | Promo-credit balance, spend order, expiry, daily promo limit, model list; `rp_spend_order`; Settings shows both balances | A promo-funded turn on a listed model debits promo; an unlisted model debits paid credit; expiry and the daily limit hold on staging with a real account |
| 1 | GitHub eligibility, `github_claims`, the $5 `github_connect` grant, budget and flags | Connecting an eligible GitHub on staging grants $5 once; the same GitHub on a second account grants nothing; a too-new GitHub shows pending with the reason |
| 2 | Codes, `/r/<code>`, attribution, `referral_invitee` and `referral_inviter`, Settings → Invite, CLI, caps, holds, admin commands | Two real staging accounts complete the loop: click, GitHub login, $15 to the invitee, $5 to the inviter after the second active day; self and reciprocal redemptions are refused |
| 3 | Kickback in the top-up settlement transaction | A real settled Lightning top-up on staging credits 1% to the inviter exactly once, including under a replayed settlement |
| 4 | Vanity links, README badge, opt-in commit footer, milestones, share cards | Each surface exercised with a real account |

Each phase is behind its own flag and is useful alone. Phase 1 can launch
before Phase 2: the GitHub grant needs no referral code.

## 10. Open decisions for the owner

1. **Amounts.** $5 / $10 / $5 / 1% as above, or fold the GitHub $5 into the
   invitee's $10 (invited people get $10 total, not $15). This spec assumes
   stacking because the bigger invited number is the stronger hook.
2. **Programme budget and daily ceiling** for flat grants (§4.5).
3. **Kickback lifetime.** Forever (assumed) or the invitee's first 12
   months.
4. **Promo-eligible model list** at launch (§4.2).
5. **GitHub thresholds.** 90 days plus one activity signal (assumed).
   Stricter costs real signups; looser costs budget.
6. **Bot screening at signup** on or off for launch (§4.4).
