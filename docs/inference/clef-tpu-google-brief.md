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
state at 4k-token prompts, batch 8. int8 = int8 weights and activations
(speed only; its accuracy on Clef is the next step).

| Chip | Model | Chips | Tokens/s per chip | Spot $/M tokens | On-demand $/M |
| --- | --- | ---: | ---: | ---: | ---: |
| **v6e (Trillium)** | **Clef-Flash 9B, int8** | 1 | **37,600** | **$0.0043** | **$0.020** |
| v6e | Clef-Flash 9B, bf16 | 1 | 25,900 | $0.0063 | $0.029 |
| v6e | Clef 27B, int8 | 4 | 6,400 | $0.025 | $0.12 |
| v6e | Clef 27B, bf16 | 4 | 5,360 | $0.029 | $0.14 |
| v5p | Clef-Flash 9B, int8 | 1 | 29,200 | $0.0082 | $0.040 |
| v5p | Clef 27B, int8 | 1 | 9,800 | $0.024 | $0.12 |
| v5p | Clef 27B, bf16 | 1 | 6,230 | $0.038 | $0.19 |
| v5e | Clef-Flash 9B | 8 | 4,000 | $0.025 | $0.083 |
| v5e | Clef 27B | 8 | 1,370 | $0.073 | $0.24 |

What we pay or charge today, per million tokens: our spot H100 **$0.11**,
spot A100 $0.072; TypeSafe's Jev sells at **$0.042**; Workers AI sells
Clef-Flash at $0.038 and Clef 27B at $0.24.

- **A spot v6e chip serves Clef-Flash at 1/25 of our H100 cost and 1/10 of
  Jev's price** (int8). Even **on-demand v6e ($0.020/M) is half of Jev's
  price**. A 1-year
  commitment ($1.89/chip-h) would be about $0.014/M; 3-year ($1.22) about
  $0.009/M (int8).
- **int8 (W8A8) is 1.45x bf16 end to end** on v6e (37.6k vs 25.9k tokens/s
  at 4k prompts; 41.9k at 1k × 32), and real Qwen3.5-9B weights run at the
  same speed as our dummy-weight runs (25.8k vs 25.9k). The chips run at
  their datasheet: bf16 matmuls at Clef's exact shapes hit 98 % of peak on
  v5e and v5p, 85 % on v6e.
- Google's own kernels cover our architecture: the Gated DeltaNet scan costs
  about 10 µs per token over all layers on v5e, under 10 % of the forward
  pass; splash attention runs at 77 % of peak.

**What more chips unlock.** One billion tokens a day is 11,600 tokens/s.
At a realistic 50 % utilization, from the measured rates (Flash int8
37,600/chip; 27B int8 TP 4 6,400/chip):

| Daily volume | Clef-Flash: v6e chips | Clef 27B: v6e chips |
| --- | ---: | ---: |
| 1 B tokens | 1 | 4 |
| 10 B tokens | 6–7 | ~36 |
| 50 B tokens | ~31 | ~181 |

## 2. What blocked us

Spot and on-demand TPU capacity, not quota and not software, limited this
campaign. Between 23:20 on 2026-10-12 and 01:30 UTC on 2026-10-13 (EIDs in
[clef-tpu-data.md](clef-tpu-data.md), "Capacity log"):

- **v6e, on demand:** at 01:30 UTC we asked for v6e-4 on demand in nine
  zones (us-central1-a/c, us-south1-a/c, us-east5-a/b, us-west1-c,
  asia-northeast1-b, asia-southeast1-b): **all nine failed** ("Insufficient
  capacity" / "no more capacity", e.g. us-central1-a EID 0x4795f544a937c738).
  v6e-1 on demand in ten zones: only **Tokyo** (asia-northeast1-b) worked,
  at $3.24/chip-h. In **us-central1-b** and **us-east1-d** on-demand v6e is
  refused outright: "User does not have permission to submit requests for
  accelerator type v6e-1" (EIDs 0xf986782ffb08c245, 0x454746e6c3bafdd2).
  Yet *spot* v6e did run in both zones for us (a v6e-1 in us-central1-b, a
  v6e-8 in us-east1-d), so the on-demand block is an access setting on our
  project, not missing hardware.
- **v6e, spot:** 56 create attempts (v6e-1, -4, -8) across 17 zones on four
  continents. Two succeeded, both v6e-1 (us-central1-b and europe-west4-a).
  Every v6e-4 and v6e-8 attempt failed with "no more capacity" or
  "insufficient capacity" (a few with internal errors). The one v6e-8 we
  got came through a queued request in us-east1-d after a 29-minute wait.
  Queued requests in us-central1-c, us-south1-a and us-west1-c were refused
  outright ("Insufficient capacity", EID 0xaa5a8f264fa7994f).
- **Spot v6e lives 5–14 minutes.** Every spot v6e we got was preempted:
  v6e-1 after 5 and 11 min, v6e-8 after 14 min, two v6e-4s after 10 and
  8 min (us-east1-d, us-central1-b). Loading and compiling Clef 27B takes
  ~6.5 min, so spot v6e cannot serve. (Spot v5p-8 lasted 43 min; spot
  v5e-8 has run for hours.)
- **Quota:** v6e spot quota is **0 in us-east4** (the cheapest v6e spot
  region, $0.243/chip-h) and in us-east5-c; **Ironwood (TPU7x) has no quota
  at all**. We filed requests at 01:05 UTC: **v6e in us-east4-a (64 spot,
  16 on-demand) was denied within 9 minutes** (01:14 UTC, trace IDs ee8a1273-c8da-4605-baed-aeda04c6c5dd and 3701d69a-a05c-430e-983e-49f37425c90d); Ironwood (8 spot chips in
  us-central1 and us-east5) is still pending.
- v5e and v5p were easier: spot v5e-8 came up at once in us-east1-c,
  us-west1-c, us-west4-a, us-south1-a and europe-west4-b; spot v5p-8 in
  us-east5-a and europe-west4-b. But v5e/v5p cost 3–10x more per token
  than v6e for our model.

## 3. The asks

1. **v6e capacity we can count on in US regions:** a reservation (or a
   spot-capacity commitment) of **32 v6e chips** (four v6e-8 hosts) in
   us-central1 or us-east1 now, with a path to **128 chips**. 32 chips at
   50 % busy serve about 50 B Clef-Flash tokens a day (int8), or about 9 B
   Clef 27B tokens.
2. **On-demand v6e access in us-central1-b and us-east1-d** (today:
   "permission denied for accelerator type v6e-1").
3. **Quota:** v6e spot in **us-east4** (64 chips; $0.243/chip-h makes Flash
   ~$0.003/M) and **Ironwood (TPU7x)**: 8 chips to measure, then more.
4. **Commitment pricing:** a quote for 1-year and 3-year v6e commitments at
   32–128 chips; at the 3-year list price Clef-Flash costs ~$0.009/M (int8).
5. **Credits** to cover the bring-up and port (int8 calibration, our joint
   head on TPU): a few thousand dollars of TPU time.
6. **A named TPU contact** for capacity and for the Pallas Gated DeltaNet
   kernel (tpu-inference `gdn/v3`), which our model depends on.
