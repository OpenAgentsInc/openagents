# Selection-record inconsistency

The frozen `selection.json` calls ID 3 `best_weight_only`, and the frozen
development review's explanation calls the lower-strength choice a tie.
That is inconsistent with its own summed ratings: ID 3 scores 6/16,
ID 4 scores 6/16, and ID 5 scores **7/16**. The label does not make ID 3
a rating argmax.

This was caught during the final audit, after held generation. Neither
file, its bound review hash, the critic's compared ID 3, nor any generation
is changed to manufacture a correct selection history. Interpret ID 3 as
the **frozen α.25 diagnostic**, not the best edited version. ID 5's complete
results remain in the matrix. We cannot claim an unambiguous score ranking
chose the best edited arm. The stated preference for lower strength does
not override the non-tied numbers or independently qualify that choice.

The primary writer 20 (15/16) and native prompt preset 11 (8/16) remain
their groups' highest development totals. No held result reselects them.
The critic's 3 wins / 4 losses / 1 tie concern ID 3 only; they do not rank
all edited arms. The oracle reports this known record defect explicitly
while checking numerical observations.
