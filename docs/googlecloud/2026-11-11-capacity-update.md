# Google Cloud capacity: update of 2026-11-11

From our Google Cloud account team, 2026-11-11. Project `openagentsgemini`
(project number 157437760789).

## TPU quota granted

**64 TPU7x (Ironwood) chips in `us-central1-c`, through DWS Flex Start.**

This answers the Ironwood request we filed in October. The 8 spot chips in
us-central1 had been pending since 2026-10-13, and the same request for
us-east5 was denied. The capacity history is in two docs removed in the
2026-10 cleanup and still in git history:

- [`docs/deployment/gcp-capacity-wishlist.md`](https://github.com/OpenAgentsInc/openagents/blob/0b401561a0%5E/docs/deployment/gcp-capacity-wishlist.md):
  the quota requests of 2026-10-21, their denials, and the stockouts for
  RTX PRO 6000 and B200.
- [`docs/inference/clef-tpu-google-brief.md`](https://github.com/OpenAgentsInc/openagents/blob/0b401561a0%5E/docs/inference/clef-tpu-google-brief.md):
  the Clef-on-TPU measurements and the TPU capacity log from the meeting
  of 2026-10-13.

## GPU availability they reported

| GPU | Machine family | How | Where |
| --- | --- | --- | --- |
| H100 | A3-High | DWS Flex Start | us-central1 |
| RTX PRO 6000 | G4 | On demand | Several US zones, including us-west3-a |
| RTX PRO 6000 | G4 | DWS Flex Start | us-central1-b, us-east1-d |
| L4 | G2 | On demand and Flex Start | us-central1-a |

- **Flex Start:** you ask for GPUs for up to 7 days, and they start as soon
  as capacity is free. See Google's Flex-start VMs documentation.
- **Calendar mode reservations:** for planned runs you can book H100, H200
  and B200 ahead of time.
- **More quota:** reply to the account team with the project number, the
  region, the GPU type and the number of GPUs, and they file the increase.

## What this changes for us

- **Availability is not quota.** On-demand RTX PRO 6000 being "open" in
  us-west3-a doesn't lift our own on-demand limit for G4, which was 0 on
  2026-10-21, when nine on-demand requests were denied automatically. Check
  the limit before planning on-demand G4. If it's still 0, ask the account
  team with the details above.
- **Flex Start fits our work.** Most of our GPU work is bounded runs:
  measurement, DeepSeek-V4.1 stages, decision-model throughput. A
  request that waits for capacity and then holds it for up to 7 days avoids
  the zone-by-zone stockout hunts recorded in the wishlist.
- **Budget comes first.** The standing limit is under $1,500 a month across
  the project. Psionic and Clef GPU work is paused, and the pool's guard
  (`openagents cloud budget`) still applies. A full 64-chip Ironwood slice
  for a week would cost far more than that limit. The quota is headroom
  for when that work resumes and is funded, not a plan to use it now.
- **Run anything through the automation service account and the spend
  guard,** as for every other machine.

## Open questions for the account team

- Our current on-demand limits for G4 (RTX PRO 6000) and A4 (B200), per
  region.
- Whether the on-demand v6e block in us-central1-b and us-east1-d ("User
  does not have permission to submit requests for accelerator type v6e-1")
  has been lifted.
- The DWS Flex price for TPU7x per chip-hour, and the smallest slice we can
  request against the 64-chip quota.
