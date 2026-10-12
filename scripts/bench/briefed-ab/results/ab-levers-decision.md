## Decision (frozen keep rule)

The rule (`plans/ablate-levers.json` "decision"): keep a lever only when its success CI's lower bound is above -0.05 AND its cost per accepted is below B0's, with the lower bound of the reduction CI above 0.

**No arm passes, so the production levers stay at B0** (`common.LEVERS` as it stands: 8 files, 140-line excerpts, 2 past changes, template v3, default effort, Opus 5.5).

- Judge 1 (frozen, Claude Opus 5.5) has a verdict on all 378 trials (49 that first hit the login's usage limit were rerun on 2026-10-12).
- Bsmall comes closest: accepted +0.048 [+0.000, +0.127] clears the success bound, and it is 15.4% cheaper per accepted change, but the reduction CI is [-4.1%, +44.8%]. Its lower bound is below 0, so it is not shown.
- Blarge: accepted +0.032 [-0.032, +0.095], cost +3.9% [-24.7%, +27.6%]. Not shown.
- Bterse: accepted +0.000 [-0.079, +0.095] fails the success bound; judge acceptance falls 9.5 points [-0.175, -0.016].
- Blow and Bsonnet are cheaper per run but lose judge acceptance by 30 and 38 points (CIs entirely below 0), and accepted by 8 and 10 points. Both fail the success bound.
- Judge 2 (Gemini 3.1 Pro on Vertex) scored all 378 trials. With it as the judge no arm saves money either: Bsmall and Blarge match B0 on accepted at +0.000 [-0.048, +0.048]; Blow and Bsonnet lose 14 points with CIs below 0. The two judges agree on 82-86% of B0 and Bsmall verdicts (kappa 0.55-0.64).
- One harness duplicate: 10201-{Blow,Bsmall,Bterse}-1 ran twice. A shard machine was restored from a partial archive that lacked them. `merge_shards.py` keeps the first run of each.
