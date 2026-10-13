# Clef on Cloud TPU: brief for the Google meeting

2026-10-13, early morning UTC. One page. Measured by us on our own project
(`openagentsgemini`) over the night before the meeting. Full data, scripts
and raw JSON: [clef-tpu-data.md](clef-tpu-data.md),
`gs://openagentsgemini-decision-throughput/tpu/2026-10-13/`. Tracking:
[#11459](https://github.com/OpenAgentsInc/openagents/issues/11459).

Clef is our decision model: one forward pass over the prompt (prefill
only, no generation). Two sizes: Clef-Flash (9B) and Clef (27B), both the
Qwen3.5 hybrid architecture. We sell by the input token, so the number that
matters is **prefill tokens per second per chip**, and from it **dollars per
million tokens**.

## 1. What we measured

Off-the-shelf open-source JAX stack (SGL-JAX, Google's Pallas kernels), no
tuning yet. Prompts 1k–32k tokens, batches 1–32; numbers are the steady
state at 4k-token prompts, batch 8 or more.

| Chip | Model | Chips | Tokens/s per chip | Spot $/M tokens | On-demand $/M |
| --- | --- | ---: | ---: | ---: | ---: |
| **v6e (Trillium)** | **Clef-Flash 9B** | 1 | **25,900** | **$0.0063** | **$0.029** |
| v6e | Clef 27B | 4 | ~5,400 (partial; slice was preempted) | ~$0.030 | ~$0.14 |
| v5e | Clef-Flash 9B | 8 | 4,000 | $0.025 | $0.083 |
| v5p | Clef 27B | 4 | 4,190 | $0.062 | $0.28 |

What we pay or charge today, per million tokens: our spot H100 **$0.11**,
spot A100 $0.072; TypeSafe's Jev sells at **$0.042**; Workers AI sells
Clef-Flash at $0.038 and Clef 27B at $0.24.

- **A spot v6e chip serves Clef-Flash at 1/17 of our H100 cost and 1/7 of
  Jev's price.** Even on-demand v6e ($0.029/M) is under Jev. A 1-year
  commitment ($1.89/chip-h) would be about $0.020/M; 3-year ($1.22) about
  $0.013/M.
- The chips run at their datasheet: bf16 matmuls at Clef's exact shapes hit
  **98 % of peak** on v5e and v5p, and int8 (W8A8) doubles matmul
  throughput, so int8 should cut these costs by another ~1.5–1.8x once the
  stack quantizes the model (in progress).
- Google's own kernels cover our architecture: the Gated DeltaNet scan costs
  about 10 µs per token over all layers on v5e, under 10 % of the forward
  pass; splash attention runs at 77 % of peak.

**What more chips unlock.** One billion Clef-Flash tokens a day is 11,600
tokens/s, about half a v6e chip fully busy, or one chip at a realistic 50 %
utilization. So:

| Daily volume | Clef-Flash (v6e chips at 50 % busy) | Clef 27B (v6e chips at 50 % busy) |
| --- | ---: | ---: |
| 1 B tokens | 1 | 4–5 |
| 10 B tokens | 9 | ~43 |
| 50 B tokens | 45 | ~215 |

## 2. What blocked us

Spot and on-demand TPU capacity, not quota and not software, limited this
campaign. Between 23:20 on 2026-10-12 and 01:30 UTC on 2026-10-13 (EIDs in
[clef-tpu-data.md](clef-tpu-data.md), "Capacity log"):

- **v6e, on demand:** "User does not have permission to submit requests for
  accelerator type v6e-1" in **us-central1-b** (EID 0xf986782ffb08c245)
  and **us-east1-d** (0x454746e6c3bafdd2). Yet *spot* v6e did run in both
  zones for us (a v6e-1 in us-central1-b, a v6e-8 in us-east1-d), so the
  on-demand block is an access setting on our project, not missing
  hardware. "No more capacity" in us-east5-a/b
  and europe-west4-a; "Insufficient capacity" in us-south1-a. The only
  on-demand v6e we got was in **asia-northeast1-b** (Tokyo).
- **v6e, spot:** 56 create attempts (v6e-1, -4, -8) across 17 zones on four
  continents. Two succeeded, both v6e-1 (us-central1-b and europe-west4-a).
  Every v6e-4 and v6e-8 attempt failed with "no more capacity" or
  "insufficient capacity" (a few with internal errors). The one v6e-8 we
  got came through a queued request in us-east1-d after a 29-minute wait.
  Queued requests in us-central1-c, us-south1-a and us-west1-c were refused
  outright ("Insufficient capacity", EID 0xaa5a8f264fa7994f).
- **Preemption:** spot v6e-1 in us-east1-d preempted 5 minutes after
  creation (2026-10-12); spot v6e-1 in us-central1-b after 11 minutes;
  spot v6e-8 in us-east1-d after 14 minutes; spot v5p-8 in europe-west4-b
  after 43 minutes. Each one cost us an in-flight benchmark.
- **Quota:** v6e spot quota is **0 in us-east4** (the cheapest v6e spot
  region, $0.243/chip-h) and in us-east5-c; **Ironwood (TPU7x) has no quota
  at all**. We filed requests tonight (Ironwood 8 chips spot in us-central1
  and us-east5; v6e 64 spot / 16 on-demand in us-east4-a); none granted yet.
- v5e and v5p were easier: spot v5e-8 came up at once in us-east1-c,
  us-west1-c, us-west4-a, us-south1-a and europe-west4-b; spot v5p-8 in
  us-east5-a and europe-west4-b. But v5e/v5p cost 3–10x more per token
  than v6e for our model.

## 3. The asks

1. **v6e capacity we can count on in US regions:** a reservation (or a
   spot-capacity commitment) of **32 v6e chips** (four v6e-8 hosts) in
   us-central1 or us-east1 now, with a path to **128 chips**. 32 chips at
   50 % busy serve about 35 B Clef-Flash tokens a day, or about 7–8 B
   Clef 27B tokens.
2. **On-demand v6e access in us-central1-b and us-east1-d** (today:
   "permission denied for accelerator type v6e-1").
3. **Quota:** v6e spot in **us-east4** (64 chips; $0.243/chip-h makes Flash
   ~$0.003/M) and **Ironwood (TPU7x)**: 8 chips to measure, then more.
4. **Commitment pricing:** a quote for 1-year and 3-year v6e commitments at
   32–128 chips; at the 3-year list price Clef-Flash costs ~$0.013/M.
5. **Credits** to cover the bring-up and port (int8 calibration, our joint
   head on TPU): a few thousand dollars of TPU time.
6. **A named TPU contact** for capacity and for the Pallas Gated DeltaNet
   kernel (tpu-inference `gdn/v3`), which our model depends on.
