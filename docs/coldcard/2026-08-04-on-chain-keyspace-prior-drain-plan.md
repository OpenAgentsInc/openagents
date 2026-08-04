# On-chain keyspace + prior-drain test plan (Fable priority)

Status: **collection / engineering plan.** Does not authorize unknown-key search
against live wallets beyond public chain data and already-published victim
ledgers. Follow
[`2026-08-01-omega-coldcard-forensic-practice-runbook.md`](2026-08-01-omega-coldcard-forensic-practice-runbook.md)
stopping rules.

Related claim ledger:
[`2026-08-04-cto-inside-job-thesis-analysis.md`](2026-08-04-cto-inside-job-thesis-analysis.md)
(claims **G**, **D**, scenarios **S5/S6**).

---

## Why this is the highest flip-value work

Public identity work (A/B) is already strong. **Theft / prior-knowledge claims
stall** without:

1. Independent verification of the **weak keyspace → real addresses** map.
2. Tests of whether **pre-2026 drain victims** sit inside that space (**G**).
3. A durable **drain ledger** for any future attribution to plug into (**D**).

---

## Arm 1 — Reproduce weak keyspace (Mk3 ~40-bit class)

### Inputs (public)

- Mechanism docs: [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md),
  Coinkite
  [technical backgrounder](https://blog.coinkite.com/entropy-technical-backgrounder/)
  (~40-bit Mk3 / ~72-bit Mk4–Q under stated assumptions).
- MicroPython STM32 `rng.c` Yasmarang fallback path (upstream May 2018;
  relevant only once linked into seed gen Mar 2021).
- Kelbie / Galaxy public address sets when available:
  [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md).

### Steps

1. Pin exact firmware versions under test (Mk3 4.0.1–4.1.9 primary).
2. Implement or import a **read-only** candidate seed generator matching the
   documented fallback path (no device bricking; no customer seed handling).
3. Enumerate or sample the reduced space with explicit resource bounds and a
   written stop condition (runbook).
4. Derive P2WPKH/P2PKH addresses for candidates; intersect with:
   - published drain address lists;
   - optional self-reported victim addresses (only with consent / public posts).
5. Record: version pins, code digest, hit rate, false-positive controls.

### Success criterion

At least one **independently derived** address that matches a published drain
or victim address, with reproducible steps. That upgrades vendor blast-radius
prose to **fixture-grade** evidence.

### Non-goals

- Searching for unknown third-party wallets to empty them.
- Claiming total economic loss from partial keyspace coverage.

---

## Arm 2 — Prior-drain claims (flip G)

### Inputs

Social claims pointing at 2022–2024 drains (detail E12/E18). Any public address
from those claims.

### Steps

1. Build a table: claim source → date → address (if any) → model/firmware if
   stated.
2. Run membership test against Arm 1 keyspace (or against a bloom/filter of
   generated addresses).
3. Outcomes:
   - **Hit:** G upgrades toward “on-chain corroborated prior exploitation.”
     Pressures E17 “unaware until today.”
   - **Miss:** claim may still be true (different bug, phishing, wrong address)
     — record as non-confirming.
   - **No address:** claim stays social-only.

### Success criterion

Any single pre-2026 address with public provenance that hits the weak space.

---

## Arm 3 — Drain ledger ingest (dataset for D)

1. Import Galaxy Research threads / Kelbie generated ledgers as versioned JSON
   under `docs/coldcard/receipts/` (public data only).
2. Cluster collectors; label consolidation waves; note exchange deposit
   candidates without doxxing.
3. Re-check periodically for movement (watch lane).

This does **not** identify staff; it builds the substrate for LE/exchange
attribution later.

---

## Implementation ownership

| Lane | Owner surface |
| --- | --- |
| Keyspace tool + tests | Omega forensics / practice runbook code paths |
| Ledger JSON receipts | `docs/coldcard/receipts/` |
| Claim status updates | claim ledger A–I table |

---

## Current status (2026-08-04, continued)

| Arm | Status |
| --- | --- |
| Arm 1 keyspace reproduction | **Planned** — public educational PoC candidate noted: [HenryqueBrito/coldcard-mk3-rng-poc](https://github.com/HenryqueBrito/coldcard-mk3-rng-poc) (Yasmarang Mk3). Review license + reproduce before trust. |
| Arm 2 prior-victim membership | **Blocked on victim addresses** + Arm 1 |
| Arm 3 ledger ingest | **Partial** — public Galaxy collector set recorded in [`receipts/2026-08-04-galaxy-public-attacker-addresses.md`](receipts/2026-08-04-galaxy-public-attacker-addresses.md); Kelbie still separate |
